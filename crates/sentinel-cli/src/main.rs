use std::collections::BTreeMap;
use std::path::PathBuf;

use sentinel_analysis::Scorer;
use sentinel_core::source::{DemoSource, EventData, EventKind};
use sentinel_core::{FeatureVector, MatchContext, Tick};
use sentinel_features::FeatureEngine;
use sentinel_report::{MatchMetadata, MatchReport, PlayerReport};
use sentinel_validation::{DemoValidation, PlayerEvaluation, PlayerLabel, ValidationHarness};
use sentinel_world::WorldRebuilder;
use sentinel_map::loader;

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
                eprintln!("Usage: sentinel analyze <match.dem>");
                return;
            }
            let path = PathBuf::from(&args[2]);
            run_analysis(&path);
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
                eprintln!("Error saving calibration data: {}", e);
                return;
            }
            println!("Calibration dataset saved to: {:?}", output_path);
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
                Err(e) => eprintln!("Error: {}", e),
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

/// Run the full analysis pipeline using DemoSource adapter
fn run_analysis(path: &PathBuf) {
    println!("=== Sentinel AI Analysis Pipeline ===\n");

    // Step 1: Parse demo file using Source2Adapter
    println!("[1/7] Parsing demo file: {:?}", path);
    let adapter = match sentinel_source2::Source2Adapter::from_file(path) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error parsing demo: {}", e);
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
    println!("  Players found: {}", player_count);

    // Load map data for visibility calculations
    match loader::load_map_by_name(&meta.map_name) {
        Some(map) => {
            println!("  Map loaded: {} ({} walls, {} nav nodes)", map.name, map.walls.len(), map.nav_nodes.len());
            ctx.set_map(map);
        }
        None => {
            println!("  Warning: Map '{}' not found, using default dust2", meta.map_name);
        }
    }

    // Step 5: Compute features
    println!("[5/7] Computing features...");
    let feature_engine = FeatureEngine::new();
    let players = adapter.player_ids();
    let mut all_feature_vectors = Vec::new();

    for &player in &players {
        let vectors = feature_engine.compute_match(&ctx, player);
        all_feature_vectors.extend(vectors);
    }
    println!("  Feature vectors: {}", all_feature_vectors.len());

    // Step 6: Run analysis
    println!("[6/7] Running analysis...");
    let scorer = Scorer::default_cs2();
    let mut player_results = Vec::new();

    for &player in &players {
        let fvs: Vec<&FeatureVector> = all_feature_vectors
            .iter()
            .filter(|fv| fv.player == player)
            .collect();
        if !fvs.is_empty() {
            let result = scorer.score_player(player, &fvs);
            player_results.push(result);
        }
    }
    println!("  Players scored: {}", player_results.len());

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

    for result in &player_results {
        let name = adapter
            .player_name(result.player)
            .unwrap_or_else(|| format!("Player_{}", result.player.as_u64()));
        let team = adapter
            .player_team(result.player)
            .map(|t| format!("{:?}", t))
            .unwrap_or_else(|| "Unknown".to_string());

        report.add_player(PlayerReport {
            steam_id: result.player.as_u64(),
            name: name.clone(),
            team,
            scores: result.overall_score.clone(),
            evidence: result.evidence.clone(),
            summary: format!(
                "Overall: {:.2}, Evidence: {}",
                result.overall_score.overall,
                result.evidence.len()
            ),
        });
    }

    // Print summary
    println!("\n=== Analysis Complete ===\n");
    println!("Overall anomaly score: {:.2}", report.overall_anomaly);

    for player in &report.players {
        println!("\n  {} ({}):", player.name, player.team);
        println!("    Overall: {:.2}", player.scores.overall);
        for (cat, score) in &player.scores.categories {
            println!("    {}: {:.2}", cat, score);
        }
        if !player.evidence.is_empty() {
            println!("    Evidence: {} items", player.evidence.len());
            for ev in player.evidence.iter().take(3) {
                println!("      - {}: {:.2} - {}", ev.feature, ev.score, ev.reason);
            }
        }
    }

    // Save reports
    let json_path = path.with_extension("json");
    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
    if let Err(e) = std::fs::write(&json_path, &json) {
        eprintln!("Error saving report: {}", e);
    } else {
        println!("\nJSON report: {:?}", json_path);
    }

    let html_path = path.with_extension("html");
    let html = sentinel_report::html::HtmlReport::generate(&report);
    if let Err(e) = std::fs::write(&html_path, &html) {
        eprintln!("Error saving HTML: {}", e);
    } else {
        println!("HTML report: {:?}", html_path);
    }
}

/// Convert a DemoSource event to a sentinel GameEvent
fn convert_demo_event(
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
        EventKind::RoundEnd => SentinelKind::RoundEnd,
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

    Some(GameEvent { kind, tick, data })
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
        eprintln!("No .dem files found in {:?}", dir);
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
                println!("  Analyzed: {} players", player_count);
            }
            Err(e) => {
                eprintln!("  Error: {}", e);
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
        .map_err(|e| format!("Parse error: {}", e))?;

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

    // Score players
    let scorer = Scorer::default_cs2();
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

fn print_usage() {
    println!("Usage: sentinel <command> [options]");
    println!();
    println!("Commands:");
    println!("  analyze <match.dem>           Analyze a CS2 demo file");
    println!("  validate <directory>          Validate on multiple demos");
    println!("  calibrate [output.json]       Generate calibration dataset");
    println!("  stats <vectors.json>          Show dataset statistics");
    println!("  verify                        Run verification checks");
}

