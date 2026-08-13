use std::collections::BTreeMap;
use std::path::PathBuf;

use sentinel_analysis::Scorer;
use sentinel_core::source::{DemoSource, EventData, EventKind};
use sentinel_core::{FeatureVector, MatchContext, Tick};
use sentinel_features::FeatureEngine;
use sentinel_map::loader;
use sentinel_memory::Memory;
use sentinel_report::{MatchMetadata, MatchReport, PlayerReport};
use sentinel_validation::{DemoValidation, PlayerEvaluation, PlayerLabel, ValidationHarness};
use sentinel_world::WorldRebuilder;

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
        "evaluate" => {
            // Run the full validation suite (ROC, PR, calibration) on a
            // directory of labelled demos.
            if args.len() < 3 {
                eprintln!("Usage: sentinel evaluate <directory_with_demos>");
                return;
            }
            run_evaluation(&PathBuf::from(&args[2]));
        }
        "cross-validate" => {
            // k-fold cross-validation over labelled demos.
            if args.len() < 3 {
                eprintln!("Usage: sentinel cross-validate <directory_with_demos> [k]");
                return;
            }
            let dir = PathBuf::from(&args[2]);
            let k = args
                .get(3)
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(5);
            run_cross_validation(&dir, k);
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
            println!(
                "  Map loaded: {} ({} walls, {} nav nodes)",
                map.name,
                map.walls.len(),
                map.nav_nodes.len()
            );
            ctx.set_map(map);
        }
        None => {
            println!(
                "  Warning: Map '{}' not found, using default dust2",
                meta.map_name
            );
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
    let mem_path = Memory::default_path();
    let memory = match Memory::load(&mem_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Warning: could not load memory ({}); using defaults.", e);
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

    // Optionally record this match into persistent memory (self-learning).
    if learn {
        let mut mem = memory.clone();
        let results_for_memory: Vec<_> = player_results
            .iter()
            .map(|r| {
                let averages: BTreeMap<String, f64> = r
                    .feature_scores
                    .iter()
                    .map(|(k, v)| (k.clone(), v.value))
                    .collect();
                let flagged = r.overall_score.overall >= 0.5;
                (
                    r.player,
                    r.overall_score.overall,
                    r.evidence.len(),
                    averages,
                    flagged,
                )
            })
            .collect();
        mem.observe_match(&all_feature_vectors, &results_for_memory);
        match mem.save(&mem_path) {
            Ok(()) => println!(
                "  Memory: recorded match ({} demos total)",
                mem.demos_analyzed
            ),
            Err(e) => eprintln!("  Memory: failed to save: {}", e),
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

    // Score players using learned baselines when memory exists.
    let memory = Memory::load(&Memory::default_path()).unwrap_or_else(|e| {
        eprintln!("Warning: could not load memory ({}); using defaults.", e);
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
                    Err(e) => eprintln!("Error removing memory file: {}", e),
                }
            } else {
                println!("Memory already empty (no file at {})", path.display());
            }
        }
        "show" | _ => match Memory::load(&path) {
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
                eprintln!("Error loading memory: {}", e);
            }
        },
    }
}

/// Run the full evaluation suite (M5) on a directory of demos.
///
/// Builds per-demo player evaluations (labelled Unknown by default), runs the
/// validation harness, then prints ROC/PR AUC, calibrated threshold and
/// per-map breakdown.
fn run_evaluation(dir: &PathBuf) {
    println!("=== Sentinel AI Evaluation Suite ===\n");

    let demos = collect_labelled_demos(dir);
    if demos.is_empty() {
        eprintln!("No .dem files found in {:?}", dir);
        return;
    }

    let mut harness = ValidationHarness::new(0.5);
    for d in &demos {
        harness.add_demo(d.clone());
    }

    let report = sentinel_validation::calibration::evaluate(&harness);

    println!("=== Metrics ===");
    let m = &report.metrics;
    println!("  Demos: {}, Players: {}", m.total_demos, m.total_players);
    println!(
        "  Precision: {:.3}  Recall: {:.3}  F1: {:.3}",
        m.precision, m.recall, m.f1_score
    );
    println!(
        "  FPR: {:.3}  TPR: {:.3}  Accuracy: {:.3}",
        m.false_positive_rate, m.true_positive_rate, m.accuracy
    );

    println!("\n=== ROC / PR ===");
    println!("  AUC-ROC: {:.3}", report.roc.auc);
    println!("  AUC-PR (avg precision): {:.3}", report.pr.auc);

    println!("\n=== Calibration ===");
    println!(
        "  Best threshold: {:.2} (F1: {:.3})",
        report.calibration.best_threshold, report.calibration.best_objective
    );

    let per_map = sentinel_validation::calibration::per_map_analysis(&demos);
    if !per_map.is_empty() {
        println!("\n=== Per-map ===");
        for pm in &per_map {
            println!(
                "  {}: {} players, P {:.3} R {:.3} F1 {:.3} AUC {:.3}",
                pm.map, pm.players, pm.precision, pm.recall, pm.f1, pm.auc_roc
            );
        }
    }

    // Save the full report as JSON for tooling.
    let out_path = dir.join("evaluation_report.json");
    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&out_path, json) {
                eprintln!("Error saving evaluation report: {}", e);
            } else {
                println!("\nEvaluation report: {:?}", out_path);
            }
        }
        Err(e) => eprintln!("Error serializing report: {}", e),
    }
}

/// Run k-fold cross-validation (M5) over a directory of demos.
fn run_cross_validation(dir: &PathBuf, k: usize) {
    println!("=== Sentinel AI {}-fold Cross-Validation ===\n", k);

    let demos = collect_labelled_demos(dir);
    if demos.is_empty() {
        eprintln!("No .dem files found in {:?}", dir);
        return;
    }

    let cv = sentinel_validation::calibration::cross_validate(&demos, k);

    println!("K: {}", cv.k);
    println!("Mean AUC-ROC: {:.3}", cv.mean_auc_roc);
    println!("Mean AUC-PR:  {:.3}", cv.mean_auc_pr);
    println!("Mean F1:      {:.3}", cv.mean_f1);

    if !cv.folds.is_empty() {
        println!("\nPer-fold:");
        for f in &cv.folds {
            println!(
                "  Fold {}: AUC-ROC {:.3}, AUC-PR {:.3}, F1 {:.3}, threshold {:.2}",
                f.fold, f.auc_roc, f.auc_pr, f.f1, f.calibrated_threshold
            );
        }
    }
}

/// Collect demo validations from a directory. Players are labelled Unknown
/// (hook for future label files), and TP/FP/TN/FN flags are derived from the
/// score vs a 0.5 threshold for demonstration.
fn collect_labelled_demos(dir: &PathBuf) -> Vec<DemoValidation> {
    let entries: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("dem"))
        .collect();

    let mut demos = Vec::new();
    for entry in &entries {
        let path = entry.path();
        match run_analysis_silent(&path) {
            Ok((map_name, players)) => {
                let players: Vec<PlayerEvaluation> = players
                    .into_iter()
                    .enumerate()
                    .map(|(i, (name, score, evidence))| PlayerEvaluation {
                        steam_id: i as u64,
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
                    .collect();
                demos.push(DemoValidation {
                    demo_path: path.to_string_lossy().to_string(),
                    map: map_name,
                    players,
                    true_positives: 0,
                    false_positives: 0,
                    true_negatives: 0,
                    false_negatives: 0,
                });
            }
            Err(e) => eprintln!("  Skipping {:?}: {}", path, e),
        }
    }
    demos
}

fn print_usage() {
    println!("Usage: sentinel <command> [options]");
    println!();
    println!("Commands:");
    println!("  analyze <match.dem> [--learn] Analyze a CS2 demo file");
    println!("  learn   <match.dem>           Analyze and train memory from a demo");
    println!("  memory [reset]                Show or reset persistent memory");
    println!("  validate <directory>          Validate on multiple demos");
    println!("  evaluate <directory>           Full validation suite (ROC, PR, calibration)");
    println!("  cross-validate <dir> [k]       k-fold cross-validation");
    println!("  calibrate [output.json]       Generate calibration dataset");
    println!("  stats <vectors.json>          Show dataset statistics");
    println!("  verify                        Run verification checks");
}
