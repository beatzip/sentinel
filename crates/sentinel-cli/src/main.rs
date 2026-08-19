use std::collections::BTreeMap;
use std::path::PathBuf;

use sentinel_analysis::{AnomalyModel, Scorer, TemporalTransformer, XgBoostModel};
use sentinel_core::source::{DemoSource, EventData, EventKind, RoundInfo};
use sentinel_core::{FeatureVector, MatchContext, Team, Tick, TickState};
use sentinel_features::FeatureEngine;
use sentinel_map::loader;
use sentinel_memory::{MatchObservation, Memory};
use sentinel_report::{
    AnalysisProvenance, ConfidenceAssessment, Encounter, MatchMetadata, MatchReport,
    ObservedDamage, ObservedShot, PlayerReport, RosterKill, RoundContext, RoundStory,
    SupportingMatch,
};
use sentinel_validation::{DemoValidation, PlayerEvaluation, PlayerLabel, ValidationHarness};
use sentinel_world::WorldRebuilder;

mod dataset;
mod replay;

const DEFAULT_FEATURE_SAMPLE_STRIDE: usize = 16;

fn main() {
    println!("Sentinel AI - CS2 Behavior Analysis Platform");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!();

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "analyze" => {
            if args.len() < 3 {
                eprintln!("Error: Missing demo file path");
                eprintln!("Usage: sentinel analyze <match.dem> [--learn]");
                return;
            }
            let path = PathBuf::from(&args[2]);
            // --learn records the analysis into persistent memory.
            let learn = args.iter().any(|a| a == "--learn");
            // Without --learn, existing memory is still loaded (read-only) so
            // the scorer benefits from learned baselines, but nothing is
            // written back.
            run_analysis(&path, learn);
        }
        "learn" => {
            if args.len() < 3 {
                eprintln!("Error: Missing demo file path");
                eprintln!("Usage: sentinel learn <match.dem>");
                return;
            }
            // `learn` is shorthand for `analyze --learn`.
            let path = PathBuf::from(&args[2]);
            run_analysis(&path, true);
        }
        "memory" => {
            run_memory_command(&args[2..]);
        }
        "calibrate" => {
            let output_path = if args.len() > 2 {
                PathBuf::from(&args[2])
            } else {
                PathBuf::from("calibration.json")
            };
            println!("Generating calibration dataset...");
            let dataset = sentinel_datasets::CalibrationDataset::default_cs2();
            if let Err(e) = dataset.save(&output_path) {
                eprintln!("Error saving calibration data: {e}");
                return;
            }
            println!("Calibration dataset saved to: {output_path:?}");
        }
        "stats" => {
            if args.len() < 3 {
                eprintln!("Usage: sentinel stats <vectors.json>");
                return;
            }
            let path = &args[2];
            match std::fs::read_to_string(path) {
                Ok(json) => {
                    if let Ok(vectors) = serde_json::from_str::<Vec<FeatureVector>>(&json) {
                        let stats = sentinel_datasets::DatasetStats::compute(&vectors);
                        println!("Dataset Statistics:");
                        println!("  Total vectors: {}", stats.total_vectors);
                        println!("  Unique players: {}", stats.unique_players);
                        println!("  Feature coverage: {:.1}%", stats.feature_coverage * 100.0);
                    }
                }
                Err(e) => eprintln!("Error: {e}"),
            }
        }
        "dataset" => dataset::run(&args[2..]),
        "replay" => {
            if args.len() < 3 {
                eprintln!("Usage: sentinel replay <match.dem> [output.replay.json]");
                return;
            }
            let input = PathBuf::from(&args[2]);
            let output = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| input.with_extension("replay.json"));
            match replay::export(&input, &output) {
                Ok(()) => println!("Replay export: {}", output.display()),
                Err(error) => eprintln!("Replay export failed: {error}"),
            }
        }
        "verify" => {
            println!("Running verification...");
            let output = std::process::Command::new("cargo")
                .args(["test", "--workspace", "--", "--quiet"])
                .output();
            match output {
                Ok(out) if out.status.success() => println!("All tests passed!"),
                _ => eprintln!("Some tests failed"),
            }
        }
        "validate" => {
            if args.len() < 3 {
                eprintln!("Usage: sentinel validate <directory_with_demos>");
                return;
            }
            let dir = PathBuf::from(&args[2]);
            run_validation(&dir);
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
        }
    }
}

/// Run the full analysis pipeline using DemoSource adapter.
///
/// When `learn` is true, the analysis is recorded into persistent memory
/// (`sentinel_memory.json`) so future runs learn from it. When false, memory
/// is still loaded read-only to improve scoring with learned baselines.
fn run_analysis(path: &PathBuf, learn: bool) {
    println!("=== Sentinel AI Analysis Pipeline ===\n");

    // Step 1: Parse demo file using Source2Adapter
    println!("[1/7] Parsing demo file: {path:?}");
    let adapter = match sentinel_source2::Source2Adapter::from_file(path) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error parsing demo: {e}");
            return;
        }
    };

    let meta = adapter.metadata();
    println!("  Map: {}", meta.map_name);
    println!("  Server: {}", meta.server_name);
    println!(
        "  Ticks: {} ({}s @ {} tick/s)",
        meta.total_ticks, meta.duration_seconds, meta.tick_rate
    );

    // Step 2: Collect game events from adapter
    println!("[2/7] Collecting events...");
    let events: Vec<_> = adapter.events().collect();
    println!("  Events: {}", events.len());

    // Step 3: Convert DemoSource events to sentinel GameEvents
    println!("[3/7] Transforming events...");
    let mut game_events = Vec::new();
    for event in &events {
        if let Some(ge) = convert_demo_event(event) {
            game_events.push(ge);
        }
    }
    println!("  Game events: {}", game_events.len());
    let (_, observed_damage) = observed_combat_events(&game_events);

    // Step 4: Reconstruct world state with real telemetry data
    println!("[4/7] Reconstructing world state...");
    let mut rebuilder = WorldRebuilder::new();
    let snapshots: Vec<_> = adapter.player_snapshots();
    let tick_states = rebuilder.process_events_with_snapshots(&game_events, &snapshots);
    let kills = rebuilder.take_kills();
    println!("  Tick states: {}", tick_states.len());
    println!("  Kills recorded: {}", kills.len());

    let mut ctx = MatchContext::new(tick_states);
    ctx.set_kills(kills);
    let player_count = adapter.player_ids().len();
    println!("  Players found: {player_count}");

    // Load map data for visibility calculations and capture its geometry version.
    let map_asset_version = match loader::load_map_by_name(&meta.map_name) {
        Some(map) => {
            println!(
                "  Map loaded: {} ({} walls, {} nav nodes)",
                map.name,
                map.walls.len(),
                map.nav_nodes.len()
            );
            let version = map_asset_version(&map);
            ctx.set_map(map);
            version
        }
        None => {
            println!(
                "  Warning: Map '{}' not found, using default dust2",
                meta.map_name
            );
            "unavailable".to_string()
        }
    };

    // Step 5: Compute features
    println!("[5/7] Computing features...");
    let feature_engine = FeatureEngine::new();
    let players = adapter.player_ids();
    let mut all_feature_vectors = Vec::new();
    let sample_stride = std::env::var("SENTINEL_FEATURE_SAMPLE_STRIDE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|stride| *stride > 0)
        .unwrap_or(DEFAULT_FEATURE_SAMPLE_STRIDE);

    for &player in &players {
        let vectors = feature_engine.compute_match(&ctx, player);
        // ponytail: bounds vector retention for long demos; set stride=1 when a host has capacity.
        all_feature_vectors.extend(vectors.into_iter().step_by(sample_stride));
    }
    println!(
        "  Feature vectors: {} (sample stride: {sample_stride})",
        all_feature_vectors.len()
    );
    let vectors_path = path.with_extension("vectors.json");
    match serde_json::to_string_pretty(&all_feature_vectors)
        .and_then(|json| std::fs::write(&vectors_path, json).map_err(serde_json::Error::io))
    {
        Ok(()) => println!("  Feature vectors: {}", vectors_path.display()),
        Err(error) => eprintln!("  Warning: could not save feature vectors ({error})"),
    }

    // Step 6: Run analysis
    println!("[6/7] Running analysis...");
    let mem_path = Memory::default_path();
    let memory = match Memory::load(&mem_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Warning: could not load memory ({e}); using defaults.");
            Memory::new()
        }
    };
    if memory.has_learned() {
        println!(
            "  Memory: learned baselines active ({} demos seen)",
            memory.demos_analyzed
        );
    } else {
        println!("  Memory: using default baselines (run `sentinel learn <demo>` to train)");
    }
    let config = sentinel_analysis::ScorerConfig {
        baselines: memory.learned_baselines(),
        evidence_threshold: 0.6,
        min_evidence_per_category: 1,
    };
    let scorer = Scorer::new(config).with_memory(Box::new(memory.clone()));
    let learned_models = load_production_models();
    let mut player_results = Vec::new();

    for &player in &players {
        let fvs: Vec<&FeatureVector> = all_feature_vectors
            .iter()
            .filter(|fv| fv.player == player)
            .collect();
        if !fvs.is_empty() {
            let mut result = scorer.score_player(player, &fvs);
            if let Some((xgboost, transformer)) = &learned_models {
                let xgboost_score = fvs
                    .iter()
                    .filter_map(|vector| xgboost.predict(vector).ok())
                    .sum::<f64>()
                    / fvs.len() as f64;
                let sequence = fvs
                    .iter()
                    .map(|vector| (*vector).clone())
                    .collect::<Vec<_>>();
                if let Ok(transformer_score) = transformer.predict_sequence(&sequence) {
                    scorer.apply_learned_scores(&mut result, xgboost_score, transformer_score);
                }
            }
            player_results.push(result);
        }
    }
    println!("  Players scored: {}", player_results.len());

    // Optionally record this match into persistent memory (self-learning).
    if learn {
        let mut mem = memory.clone();
        let report_id = api_report_id(path);
        let results_for_memory: Vec<_> = player_results
            .iter()
            .map(|r| MatchObservation {
                report_id: report_id.clone(),
                map_name: meta.map_name.clone(),
                player: r.player,
                overall_score: r.overall_score.overall,
                evidence_count: r.evidence.len(),
                feature_averages: r
                    .feature_scores
                    .iter()
                    .map(|(k, v)| (k.clone(), v.value))
                    .collect(),
                flagged: r.overall_score.overall >= 0.5,
            })
            .collect();
        mem.observe_match(&all_feature_vectors, &results_for_memory);
        match mem.save(&mem_path) {
            Ok(()) => println!(
                "  Memory: recorded match ({} demos total)",
                mem.demos_analyzed
            ),
            Err(e) => eprintln!("  Memory: failed to save: {e}"),
        }
    }

    // Step 7: Generate report
    println!("[7/7] Generating report...");

    let report_meta = MatchMetadata {
        demo_path: path.to_string_lossy().to_string(),
        map_name: meta.map_name.clone(),
        server_name: meta.server_name.clone(),
        total_rounds: adapter.rounds().len() as u32,
        duration_seconds: meta.duration_seconds,
        tick_rate: meta.tick_rate,
    };

    let mut report = MatchReport::new(report_meta);
    report.rounds = build_round_contexts(
        &adapter,
        ctx.states(),
        ctx.kills(),
        &game_events,
        &observed_damage,
    );
    report.provenance = AnalysisProvenance {
        engine_version: format!("sentinel-cli@{}", env!("CARGO_PKG_VERSION")),
        demo_parser_version: sentinel_source2::DEMO_PARSER_VERSION.to_string(),
        demo_fingerprint: file_fingerprint(path),
        map_asset_version,
        feature_schema_version: feature_schema_fingerprint(&all_feature_vectors),
        xgboost_artifact_version: file_fingerprint(&models_dir().join("sentinel-xgboost.sqb")),
        transformer_artifact_version: file_fingerprint(
            &models_dir().join("sentinel-transformer.json"),
        ),
    };

    for result in &player_results {
        let name = adapter
            .player_name(result.player)
            .unwrap_or_else(|| format!("Player_{}", result.player.as_u64()));
        let team = adapter
            .player_team(result.player)
            .map(|t| format!("{t:?}"))
            .unwrap_or_else(|| "Unknown".to_string());
        let history = memory.account_history(result.player);
        let supporting_matches = history
            .supporting_matches
            .into_iter()
            .map(|item| SupportingMatch {
                report_id: item.report_id,
                map_name: item.map_name,
                overall_score: item.overall_score,
                evidence_count: item.evidence_count,
                flagged: item.flagged,
            })
            .collect();
        let confidence = ConfidenceAssessment::assess(
            &result.overall_score,
            history.matches_observed,
            history.flagged_matches,
            supporting_matches,
        );

        report.add_player(PlayerReport {
            steam_id: result.player.as_u64(),
            name: name.clone(),
            team,
            scores: result.overall_score.clone(),
            evidence: result.evidence.clone(),
            summary: format!(
                "Overall: {:.2}, Evidence: {}, Confidence: {:?}/{:?}",
                result.overall_score.overall,
                result.evidence.len(),
                confidence.level,
                confidence.status,
            ),
            confidence,
        });
    }

    // Print summary
    println!("\n=== Analysis Complete ===\n");
    println!("Overall anomaly score: {:.2}", report.overall_anomaly);

    for player in &report.players {
        println!("\n  {} ({}):", player.name, player.team);
        println!("    Overall: {:.2}", player.scores.overall);
        for (cat, score) in &player.scores.categories {
            println!("    {cat}: {score:.2}");
        }
        if !player.evidence.is_empty() {
            println!("    Evidence: {} items", player.evidence.len());
            for ev in player.evidence.iter().take(3) {
                println!("      - {}: {:.2} - {}", ev.feature, ev.score, ev.reason);
            }
        }
    }

    // Publish report and replay sidecar where sentinel-api reads them.
    let reports_dir = api_reports_dir();
    if let Err(error) = std::fs::create_dir_all(&reports_dir) {
        eprintln!("Error creating API reports directory: {error}");
        return;
    }
    let report_id = api_report_id(path);
    let json_path = reports_dir.join(format!("{report_id}.json"));
    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
    if let Err(e) = std::fs::write(&json_path, &json) {
        eprintln!("Error saving report: {e}");
    } else {
        println!("\nJSON report: {json_path:?}");
    }

    let replay_path = reports_dir.join(format!("{report_id}.replay.json"));
    match replay::export_adapter(&adapter, &replay_path) {
        Ok(()) => println!("Replay export: {replay_path:?}"),
        Err(error) => eprintln!("Replay export failed: {error}"),
    }

    let html_path = reports_dir.join(format!("{report_id}.html"));
    let html = sentinel_report::html::HtmlReport::generate(&report);
    if let Err(e) = std::fs::write(&html_path, &html) {
        eprintln!("Error saving HTML: {e}");
    } else {
        println!("HTML report: {html_path:?}");
    }
}

/// Convert a DemoSource event to a sentinel GameEvent
pub(crate) fn convert_demo_event(
    event: &impl sentinel_core::source::DemoEvent,
) -> Option<sentinel_events::kinds::GameEvent> {
    use sentinel_events::kinds::{EventKind as SentinelKind, EventValue, GameEvent};

    let tick = Tick(event.tick().0);
    let mut data = std::collections::BTreeMap::new();

    let kind = match event.kind() {
        EventKind::PlayerDeath => SentinelKind::PlayerDeath,
        EventKind::PlayerSpawn => SentinelKind::PlayerSpawn,
        EventKind::PlayerHurt => SentinelKind::PlayerHurt,
        EventKind::PlayerSound => SentinelKind::PlayerSound,
        EventKind::WeaponFire => SentinelKind::WeaponFire,
        EventKind::RoundStart => SentinelKind::RoundStart,
        EventKind::RoundFreezeEnd => SentinelKind::RoundFreezeEnd,
        EventKind::RoundEnd => SentinelKind::RoundEnd,
        EventKind::WarmupStart => SentinelKind::WarmupStart,
        EventKind::WarmupEnd => SentinelKind::WarmupEnd,
        EventKind::BombPlant => SentinelKind::BombPlant,
        EventKind::BombDefuse => SentinelKind::BombDefuse,
        EventKind::SmokeDetonate => SentinelKind::SmokeGrenadeDetonate,
        EventKind::SmokeExpired => SentinelKind::SmokeGrenadeExpired,
        EventKind::FlashDetonate => SentinelKind::FlashGrenadeDetonate,
        EventKind::HEDetonate => SentinelKind::HEGrenadeDetonate,
        EventKind::MolotovDetonate => SentinelKind::MolotovDetonate,
        EventKind::InfernoStart => SentinelKind::InfernoStart,
        EventKind::InfernoExpire => SentinelKind::InfernoExpire,
    };

    // Convert event data
    for (key, value) in event.data() {
        let sentinel_value = match value {
            EventData::Int(v) => EventValue::Integer(*v),
            EventData::Float(v) => EventValue::Float(*v),
            EventData::String(v) => EventValue::String(v.clone()),
            EventData::Bool(v) => EventValue::Boolean(*v),
            EventData::PlayerId(v) => EventValue::PlayerId(v.as_u64()),
        };
        data.insert(key.clone(), sentinel_value);
    }

    // Source2's public player_death event uses `userid` for the victim;
    // WorldRebuilder exposes the normalized field as `victim`.
    if matches!(event.kind(), EventKind::PlayerDeath)
        && let Some(victim) = data.get("userid").cloned()
    {
        data.insert("victim".to_string(), victim);
    }

    Some(GameEvent { kind, tick, data })
}

pub(crate) fn build_round_contexts(
    adapter: &sentinel_source2::Source2Adapter,
    states: &[TickState],
    kills: &[sentinel_core::KillEvent],
    events: &[sentinel_events::kinds::GameEvent],
    damage: &[ObservedDamage],
) -> Vec<RoundContext> {
    adapter
        .rounds()
        .iter()
        .map(|round| {
            let start_tick = round.start_tick().0;
            let end_tick = round.end_tick().0;
            let end_state = states
                .iter()
                .rfind(|state| state.tick.0 >= start_tick && state.tick.0 <= end_tick);
            let (t_score, ct_score, t_survivors, ct_survivors) = end_state
                .map(|state| {
                    (
                        state.round.t_score,
                        state.round.ct_score,
                        state
                            .players
                            .iter()
                            .filter(|player| player.alive && player.team == Team::Terrorist)
                            .count(),
                        state
                            .players
                            .iter()
                            .filter(|player| player.alive && player.team == Team::CounterTerrorist)
                            .count(),
                    )
                })
                .unwrap_or_default();
            let round_events = events
                .iter()
                .filter(|event| event.tick.0 >= start_tick && event.tick.0 <= end_tick)
                .collect::<Vec<_>>();
            let end_reason = round_events
                .iter()
                .find(|event| event.kind == sentinel_events::kinds::EventKind::RoundEnd)
                .and_then(|event| event.data.get("reason"))
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let bomb_result = if round_events
                .iter()
                .any(|event| event.kind == sentinel_events::kinds::EventKind::BombDefuse)
            {
                Some("defused".to_string())
            } else if round_events
                .iter()
                .any(|event| event.kind == sentinel_events::kinds::EventKind::BombPlant)
            {
                Some("planted".to_string())
            } else {
                None
            };
            let kills: Vec<RosterKill> = kills
                .iter()
                .filter(|kill| kill.tick.0 >= start_tick && kill.tick.0 <= end_tick)
                .map(|kill| RosterKill {
                    tick: kill.tick.0,
                    attacker_id: kill.attacker.as_u64(),
                    attacker_name: adapter
                        .player_name(kill.attacker)
                        .unwrap_or_else(|| format!("Player_{}", kill.attacker.as_u64())),
                    victim_id: kill.victim.as_u64(),
                    victim_name: adapter
                        .player_name(kill.victim)
                        .unwrap_or_else(|| format!("Player_{}", kill.victim.as_u64())),
                    assist_id: kill.assist_player.map(|player| player.as_u64()),
                    assist_name: kill
                        .assist_player
                        .and_then(|player| adapter.player_name(player)),
                    weapon: kill.weapon.clone(),
                    headshot: kill.headshot,
                    wallbang: kill.wallbang,
                    through_smoke: kill.through_smoke,
                })
                .collect();
            let winner = round.winner().map(|team| format!("{team:?}"));
            let story = RoundStory::from_facts(
                round.number(),
                t_score,
                ct_score,
                winner.as_deref(),
                end_reason.as_deref(),
                bomb_result.as_deref(),
                &kills,
            );
            let encounters = kills
                .iter()
                .map(|kill| {
                    let direct_damage = damage
                        .iter()
                        .filter(|entry| {
                            entry.tick >= start_tick
                                && entry.tick <= kill.tick
                                && entry.attacker_id == Some(kill.attacker_id)
                                && entry.victim_id == kill.victim_id
                        })
                        .cloned()
                        .collect();
                    Encounter::from_kill_with_damage(round.number(), kill, direct_damage)
                })
                .collect();
            RoundContext {
                round_number: round.number(),
                start_tick,
                end_tick,
                t_score,
                ct_score,
                winner,
                end_reason,
                bomb_result,
                buy_matchup: None,
                t_survivors,
                ct_survivors,
                kills,
                encounters,
                story,
            }
        })
        .collect()
}

/// Collect facts exposed by Source2's weapon_fire and player_hurt events. Shot target identity is
/// deliberately not inferred because the source event does not provide one.
pub(crate) fn observed_combat_events(
    events: &[sentinel_events::kinds::GameEvent],
) -> (Vec<ObservedShot>, Vec<ObservedDamage>) {
    use sentinel_events::{EventKind, damage_from_game_event, shot_from_game_event};

    let shots = events
        .iter()
        .filter(|event| event.kind == EventKind::WeaponFire)
        .map(shot_from_game_event)
        .map(|shot| ObservedShot {
            tick: shot.tick.0,
            shooter_id: shot.shooter_id,
            weapon: shot.weapon,
            penetrated: shot.penetrated,
            is_alt_fire: shot.is_alt_fire,
        })
        .collect();
    let damage = events
        .iter()
        .filter(|event| event.kind == EventKind::PlayerHurt)
        .map(damage_from_game_event)
        .map(|entry| ObservedDamage {
            tick: entry.tick.0,
            victim_id: entry.victim_id,
            attacker_id: entry.attacker_id,
            weapon: entry.weapon,
            dmg_health: entry.dmg_health,
            dmg_armor: entry.dmg_armor,
            hitgroup: format!("{:?}", entry.hitgroup).to_lowercase(),
            dmg_health_real: entry.dmg_health_real,
        })
        .collect();
    (shots, damage)
}

/// Run validation on a directory of demo files
fn run_validation(dir: &PathBuf) {
    println!("=== Sentinel AI Validation Harness ===\n");

    // Find all .dem files in directory
    let demos: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("dem"))
        .collect();

    if demos.is_empty() {
        eprintln!("No .dem files found in {dir:?}");
        return;
    }

    println!("Found {} demo files", demos.len());
    println!();

    let mut harness = ValidationHarness::new(0.5);

    for (i, entry) in demos.iter().enumerate() {
        let path = entry.path();
        println!(
            "[{}/{}] Processing: {:?}",
            i + 1,
            demos.len(),
            path.file_name().unwrap()
        );

        // Run analysis on this demo
        match run_analysis_silent(&path) {
            Ok(result) => {
                let map_name = result.0;
                let player_count = result.1.len();
                // For now, all players are labeled as Unknown
                // In a real validation, we'd load labels from a file
                let demo_validation = DemoValidation {
                    demo_path: path.to_string_lossy().to_string(),
                    map: map_name,
                    players: result
                        .1
                        .into_iter()
                        .map(|(name, score, evidence)| PlayerEvaluation {
                            steam_id: 0,
                            name,
                            team: "Unknown".to_string(),
                            label: PlayerLabel::Unknown,
                            overall_score: score,
                            category_scores: BTreeMap::new(),
                            evidence_count: evidence,
                            is_true_positive: false,
                            is_false_positive: false,
                            is_true_negative: false,
                            is_false_negative: false,
                        })
                        .collect(),
                    true_positives: 0,
                    false_positives: 0,
                    true_negatives: 0,
                    false_negatives: 0,
                };
                harness.add_demo(demo_validation);
                println!("  Analyzed: {player_count} players");
            }
            Err(e) => {
                eprintln!("  Error: {e}");
            }
        }
    }

    // Print summary
    println!("\n{}", harness.summary());
}

/// Silent analysis that returns results without printing
type PlayerScore = (String, f64, usize);
type AnalysisResult = Result<(String, Vec<PlayerScore>), String>;
use std::path::Path;
fn run_analysis_silent(path: &Path) -> AnalysisResult {
    let adapter = sentinel_source2::Source2Adapter::from_file(path)
        .map_err(|e| format!("Parse error: {e}"))?;

    let meta = adapter.metadata();

    // Collect events
    let events: Vec<_> = adapter.events().collect();

    // Convert events
    let mut game_events = Vec::new();
    for event in &events {
        if let Some(ge) = convert_demo_event(event) {
            game_events.push(ge);
        }
    }

    // Reconstruct world state with real telemetry data
    let mut rebuilder = WorldRebuilder::new();
    let snapshots: Vec<_> = adapter.player_snapshots();
    let tick_states = rebuilder.process_events_with_snapshots(&game_events, &snapshots);
    let kills = rebuilder.take_kills();
    let mut ctx = MatchContext::new(tick_states);
    ctx.set_kills(kills);

    // Load map data for visibility calculations
    if let Some(map) = loader::load_map_by_name(&meta.map_name) {
        ctx.set_map(map);
    }

    // Compute features
    let feature_engine = FeatureEngine::new();
    let players = adapter.player_ids();
    let mut all_feature_vectors = Vec::new();

    for &player in &players {
        let vectors = feature_engine.compute_match(&ctx, player);
        all_feature_vectors.extend(vectors);
    }

    // Score players using learned baselines when memory exists.
    let memory = Memory::load(&Memory::default_path()).unwrap_or_else(|e| {
        eprintln!("Warning: could not load memory ({e}); using defaults.");
        Memory::new()
    });
    let scorer = Scorer::new(sentinel_analysis::ScorerConfig {
        baselines: memory.learned_baselines(),
        evidence_threshold: 0.6,
        min_evidence_per_category: 1,
    });
    let mut results = Vec::new();

    for &player in &players {
        let fvs: Vec<&FeatureVector> = all_feature_vectors
            .iter()
            .filter(|fv| fv.player == player)
            .collect();
        if !fvs.is_empty() {
            let result = scorer.score_player(player, &fvs);
            let name = adapter
                .player_name(player)
                .unwrap_or_else(|| format!("Player_{}", player.as_u64()));
            results.push((name, result.overall_score.overall, result.evidence.len()));
        }
    }

    Ok((meta.map_name, results))
}

/// Handle the `memory` command: show memory state or reset it.
fn run_memory_command(args: &[String]) {
    let sub = args.first().map(String::as_str).unwrap_or("show");
    let path = Memory::default_path();
    match sub {
        "reset" => {
            if path.exists() {
                match std::fs::remove_file(&path) {
                    Ok(()) => println!("Memory reset: removed {}", path.display()),
                    Err(e) => eprintln!("Error removing memory file: {e}"),
                }
            } else {
                println!("Memory already empty (no file at {})", path.display());
            }
        }
        "show" => match Memory::load(&path) {
            Ok(mem) => {
                if mem.demos_analyzed == 0 {
                    println!(
                        "Memory is empty. Run `sentinel learn <match.dem>` to start training."
                    );
                    println!("Memory file: {}", path.display());
                } else {
                    println!("Memory file: {}", path.display());
                    println!();
                    print!("{}", mem.summary());
                }
            }
            Err(e) => {
                eprintln!("Error loading memory: {e}");
            }
        },
        // Unknown subcommand: treat like "show".
        _ => {
            if sub != "show" {
                eprintln!("Unknown memory subcommand: {sub} (use 'show' or 'reset')");
            }
            match Memory::load(&path) {
                Ok(mem) => {
                    if mem.demos_analyzed == 0 {
                        println!(
                            "Memory is empty. Run `sentinel learn <match.dem>` to start training."
                        );
                        println!("Memory file: {}", path.display());
                    } else {
                        println!("Memory file: {}", path.display());
                        println!();
                        print!("{}", mem.summary());
                    }
                }
                Err(e) => {
                    eprintln!("Error loading memory: {e}");
                }
            }
        }
    }
}

fn load_production_models() -> Option<(XgBoostModel, TemporalTransformer)> {
    let root = models_dir();
    let metadata = std::fs::read_to_string(root.join("training-metadata.json")).ok()?;
    let feature_names = serde_json::from_str::<serde_json::Value>(&metadata)
        .ok()?
        .get("xgboost_features")?
        .as_array()?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let xgboost = XgBoostModel::load(&root.join("sentinel-xgboost.sqb"), feature_names).ok()?;
    let transformer = TemporalTransformer::load(&root.join("sentinel-transformer.json")).ok()?;
    println!(
        "  Production models: XGBoost + Transformer active ({})",
        root.display()
    );
    Some((xgboost, transformer))
}

fn models_dir() -> PathBuf {
    PathBuf::from(std::env::var("SENTINEL_MODELS_DIR").unwrap_or_else(|_| "models".to_string()))
}

fn fingerprint_bytes(bytes: &[u8]) -> String {
    format!(
        "fnv1a64:{:016x}",
        bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ *byte as u64).wrapping_mul(0x100000001b3)
        })
    )
}

fn file_fingerprint(path: &std::path::Path) -> String {
    std::fs::read(path)
        .map(|bytes| fingerprint_bytes(&bytes))
        .unwrap_or_else(|_| "unavailable".to_string())
}

fn feature_schema_fingerprint(vectors: &[FeatureVector]) -> String {
    let schema = vectors
        .iter()
        .flat_map(|vector| vector.features.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("|");
    fingerprint_bytes(schema.as_bytes())
}

fn map_asset_version(map: &sentinel_map::MapData) -> String {
    let signature = format!(
        "{}:{}:{}:{}",
        map.name,
        map.walls.len(),
        map.nav_nodes.len(),
        map.spawns.len()
    );
    format!("geometry:{}", fingerprint_bytes(signature.as_bytes()))
}

fn api_reports_dir() -> PathBuf {
    PathBuf::from(std::env::var("SENTINEL_REPORTS_DIR").unwrap_or_else(|_| "reports".to_string()))
}

fn api_report_id(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("match")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn print_usage() {
    println!("Usage: sentinel <command> [options]");
    println!();
    println!("Commands:");
    println!("  analyze <match.dem> [--learn] Analyze a CS2 demo file");
    println!("  learn   <match.dem>           Analyze and train memory from a demo");
    println!("  memory [reset]                Show or reset persistent memory");
    println!("  validate <directory>          Validate on multiple demos");
    println!("  calibrate [output.json]       Generate calibration dataset");
    println!("  stats <vectors.json>          Show dataset statistics");
    println!(
        "  dataset <init|audit|train>    Create, audit, or train from a labeled dataset manifest"
    );
    println!("  replay <match.dem> [output]   Export sampled replay frames with visibility pairs");
    println!("  verify                        Run verification checks");
}
