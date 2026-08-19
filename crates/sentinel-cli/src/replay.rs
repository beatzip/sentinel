use std::path::Path;

use sentinel_core::source::DemoSource;
use sentinel_map::{MapData, loader};
use sentinel_report::replay::{ReplayData, ReplayFrame, ReplayPlayer, VisibilityPair};
use sentinel_visibility::VisibilityEngine;
use sentinel_world::WorldRebuilder;

const FRAME_INTERVAL_TICKS: usize = 32;

pub fn export(demo_path: &Path, output_path: &Path) -> Result<(), String> {
    let adapter = sentinel_source2::Source2Adapter::from_file(demo_path)
        .map_err(|error| format!("Unable to parse demo: {error}"))?;
    export_adapter(&adapter, output_path)
}

pub fn export_adapter(
    adapter: &sentinel_source2::Source2Adapter,
    output_path: &Path,
) -> Result<(), String> {
    let events = adapter.events().collect::<Vec<_>>();
    let game_events = events
        .iter()
        .filter_map(super::convert_demo_event)
        .collect::<Vec<_>>();
    let (shots, damage) = super::observed_combat_events(&game_events);
    let mut rebuilder = WorldRebuilder::new();
    let states = rebuilder.process_events_with_snapshots(&game_events, &adapter.player_snapshots());
    let kills = rebuilder.take_kills();
    let metadata = adapter.metadata();
    let map = loader::load_map_by_name(&metadata.map_name).unwrap_or_else(MapData::dust2);
    let map_ref = &map;
    let frames = states
        .iter()
        .step_by(FRAME_INTERVAL_TICKS)
        .map(|state| {
            let players = state
                .players
                .iter()
                .map(|player| ReplayPlayer {
                    steam_id: player.id.as_u64(),
                    name: player.name.clone(),
                    team: format!("{:?}", player.team),
                    x: player.position.x,
                    y: player.position.y,
                    z: player.position.z,
                    health: player.health,
                    alive: player.alive,
                    yaw: player.view_angles.yaw,
                    pitch: player.view_angles.pitch,
                })
                .collect::<Vec<_>>();
            let visible_pairs = state
                .players
                .iter()
                .filter(|observer| observer.alive)
                .flat_map(|observer| {
                    state
                        .players
                        .iter()
                        .filter(move |target| target.alive && target.team != observer.team)
                        .filter_map(move |target| {
                            VisibilityEngine::can_see_with_map(
                                state,
                                observer.id,
                                target.id,
                                map_ref,
                            )
                            .visible
                            .then_some(VisibilityPair {
                                observer: observer.id.as_u64(),
                                target: target.id.as_u64(),
                            })
                        })
                })
                .collect();
            ReplayFrame {
                tick: state.tick.0,
                round: state.round.round_number,
                players,
                visible_pairs,
            }
        })
        .collect();
    let mut replay = ReplayData {
        version: "1.1.0".to_string(),
        map: metadata.map_name,
        tick_rate: metadata.tick_rate,
        frames,
        rounds: super::build_round_contexts(adapter, &states, &kills, &game_events, &damage),
        shots,
        damage,
        quality: Default::default(),
    };
    replay.quality = replay.assess_quality();
    let json = serde_json::to_string_pretty(&replay).map_err(|error| error.to_string())?;
    std::fs::write(output_path, json).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use sentinel_report::replay::{ReplayData, ReplayFrame};

    #[test]
    fn replay_contract_serializes() {
        let replay = ReplayData {
            version: "1.0.0".to_string(),
            map: "de_dust2".to_string(),
            tick_rate: 64,
            frames: vec![ReplayFrame {
                tick: 64,
                round: 1,
                players: Vec::new(),
                visible_pairs: Vec::new(),
            }],
            rounds: Vec::new(),
            shots: Vec::new(),
            damage: Vec::new(),
            quality: Default::default(),
        };
        assert!(serde_json::to_string(&replay).unwrap().contains("de_dust2"));
    }
}
