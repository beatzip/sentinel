//! Sentinel AI - Basic Usage Example
//!
//! This example demonstrates how to use the Sentinel AI library
//! to analyze a CS2 demo file.

use std::path::PathBuf;
use sentinel_core::{MatchContext, Tick, PlayerId};
use sentinel_core::source::DemoSource;
use sentinel_world::WorldRebuilder;
use sentinel_features::FeatureEngine;
use sentinel_analysis::Scorer;
use sentinel_report::{MatchReport, MatchMetadata, PlayerReport};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line args
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: example <match.dem>");
        return Ok(());
    }

    let path = PathBuf::from(&args[1]);
    println!("Analyzing: {:?}\n", path);

    // Step 1: Parse demo file
    println!("[1/6] Parsing demo file...");
    let adapter = sentinel_source2::Source2Adapter::from_file(&path)?;
    let meta = adapter.metadata();
    println!("  Map: {}", meta.map_name);
    println!("  Ticks: {} ({}s)", meta.total_ticks, meta.duration_seconds);

    // Step 2: Extract events
    println!("[2/6] Extracting events...");
    let events: Vec<_> = adapter.events().collect();
    println!("  Events: {}", events.len());

    // Step 3: Reconstruct world state
    println!("[3/6] Reconstructing world state...");
    let mut game_events = Vec::new();
    for event in &events {
        if let Some(ge) = convert_event(event) {
            game_events.push(ge);
        }
    }
    let mut rebuilder = WorldRebuilder::new();
    let tick_states = rebuilder.process_events(&game_events);
    let ctx = MatchContext::new(tick_states);
    println!("  Tick states: {}", ctx.states().len());

    // Step 4: Compute features
    println!("[4/6] Computing features...");
    let feature_engine = FeatureEngine::new();
    let players = adapter.player_ids();
    let mut all_feature_vectors = Vec::new();

    for &player in &players {
        let vectors = feature_engine.compute_match(&ctx, player);
        all_feature_vectors.extend(vectors);
    }
    println!("  Feature vectors: {}", all_feature_vectors.len());

    // Step 5: Run analysis
    println!("[5/6] Running analysis...");
    let scorer = Scorer::default_cs2();
    let mut report = MatchReport::new(MatchMetadata {
        demo_path: path.to_string_lossy().to_string(),
        map_name: meta.map_name.clone(),
        server_name: meta.server_name.clone(),
        total_rounds: 0,
        duration_seconds: meta.duration_seconds,
        tick_rate: meta.tick_rate,
    });

    for &player in &players {
        let fvs: Vec<&FeatureVector> = all_feature_vectors.iter()
            .filter(|fv| fv.player == player)
            .collect();
        if !fvs.is_empty() {
            let result = scorer.score_player(player, &fvs);
            let name = adapter.player_name(player)
                .unwrap_or_else(|| format!("Player_{}", player.as_u64()));

            report.add_player(PlayerReport {
                steam_id: player.as_u64(),
                name: name.clone(),
                team: "Unknown".to_string(),
                scores: result.overall_score.clone(),
                evidence: result.evidence.clone(),
                summary: format!("Score: {:.2}", result.overall_score.overall),
            });
        }
    }
    println!("  Players scored: {}", report.players.len());

    // Step 6: Generate report
    println!("[6/6] Generating report...");

    // Save JSON
    let json_path = path.with_extension("json");
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&json_path, &json)?;
    println!("  JSON: {:?}", json_path);

    // Save HTML
    let html_path = path.with_extension("html");
    let html = sentinel_report::html::HtmlReport::generate(&report);
    std::fs::write(&html_path, &html)?;
    println!("  HTML: {:?}", html_path);

    // Print summary
    println!("\n=== Analysis Complete ===");
    println!("Overall anomaly score: {:.2}", report.overall_anomaly);

    for player in &report.players {
        println!("\n  {}:", player.name);
        println!("    Score: {:.2}", player.scores.overall);
        for (cat, score) in &player.scores.categories {
            println!("    {}: {:.2}", cat, score);
        }
    }

    Ok(())
}

/// Convert a demo event to a game event
fn convert_event(event: &impl sentinel_core::source::DemoEvent) -> Option<sentinel_events::kinds::GameEvent> {
    use sentinel_events::kinds::{EventKind, EventValue, GameEvent};

    let kind = match event.kind() {
        sentinel_core::source::EventKind::PlayerDeath => EventKind::PlayerDeath,
        sentinel_core::source::EventKind::PlayerSpawn => EventKind::PlayerSpawn,
        sentinel_core::source::EventKind::WeaponFire => EventKind::WeaponFire,
        sentinel_core::source::EventKind::RoundStart => EventKind::RoundStart,
        sentinel_core::source::EventKind::RoundEnd => EventKind::RoundEnd,
        _ => return None,
    };

    let mut data = std::collections::BTreeMap::new();
    for (key, value) in event.data() {
        let sentinel_value = match value {
            sentinel_core::source::EventData::Int(v) => EventValue::Integer(*v),
            sentinel_core::source::EventData::Float(v) => EventValue::Float(*v),
            sentinel_core::source::EventData::String(v) => EventValue::String(v.clone()),
            sentinel_core::source::EventData::Bool(v) => EventValue::Boolean(*v),
            sentinel_core::source::EventData::PlayerId(v) => EventValue::PlayerId(v.as_u64()),
        };
        data.insert(key.clone(), sentinel_value);
    }

    Some(GameEvent { kind, tick: Tick(event.tick().0), data })
}
