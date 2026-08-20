use std::path::Path;

use sentinel_core::{TickState, resolve_standard_player_fallback, source::DemoSource};
use sentinel_map::{MapData, Vec3 as MapVec3, loader};
use sentinel_report::{
    DEFAULT_SHOT_DAMAGE_LINK_WINDOW_TICKS, LinkedShotDamage, link_observed_shot_damage,
    replay::{
        OriginLineOfSight, ReplayData, ReplayFrame, ReplayPlayer, SpatialEvidenceReason,
        SpatialEvidenceStatus, SpatialShotEvidence, UnsupportedSpatialCapability, VisibilityPair,
    },
};
use sentinel_visibility::VisibilityEngine;
use sentinel_world::WorldRebuilder;

const FRAME_INTERVAL_TICKS: usize = 32;

fn spatial_evidence_for_links(
    states: &[TickState],
    map: &MapData,
    links: &[LinkedShotDamage],
) -> Vec<SpatialShotEvidence> {
    links
        .iter()
        .map(|link| {
            let unsupported_capabilities = vec![
                UnsupportedSpatialCapability::Hitboxes,
                UnsupportedSpatialCapability::PenetrationModel,
            ];
            if map.bvh.is_none() {
                return SpatialShotEvidence {
                    shot_tick: link.shot_tick,
                    damage_tick: link.damage_tick,
                    snapshot_tick: None,
                    attacker_id: link.attacker_id,
                    victim_id: link.victim_id,
                    status: SpatialEvidenceStatus::Unavailable,
                    reason: SpatialEvidenceReason::MissingMapCollision,
                    line_of_sight: OriginLineOfSight::Unknown,
                    attacker_origin: None,
                    victim_origin: None,
                    unsupported_capabilities,
                };
            }
            let state = states
                .partition_point(|state| state.tick.0 < link.shot_tick)
                .checked_sub(1)
                .and_then(|index| states.get(index));
            let Some(state) = state else {
                return SpatialShotEvidence {
                    shot_tick: link.shot_tick,
                    damage_tick: link.damage_tick,
                    snapshot_tick: None,
                    attacker_id: link.attacker_id,
                    victim_id: link.victim_id,
                    status: SpatialEvidenceStatus::Unavailable,
                    reason: SpatialEvidenceReason::MissingPlayerSnapshot,
                    line_of_sight: OriginLineOfSight::Unknown,
                    attacker_origin: None,
                    victim_origin: None,
                    unsupported_capabilities,
                };
            };
            let attacker = state
                .players
                .iter()
                .find(|player| player.id.as_u64() == link.attacker_id);
            let victim = state
                .players
                .iter()
                .find(|player| player.id.as_u64() == link.victim_id);
            let (Some(attacker), Some(victim)) = (attacker, victim) else {
                return SpatialShotEvidence {
                    shot_tick: link.shot_tick,
                    damage_tick: link.damage_tick,
                    snapshot_tick: Some(state.tick.0),
                    attacker_id: link.attacker_id,
                    victim_id: link.victim_id,
                    status: SpatialEvidenceStatus::Unavailable,
                    reason: SpatialEvidenceReason::MissingPlayerSnapshot,
                    line_of_sight: OriginLineOfSight::Unknown,
                    attacker_origin: None,
                    victim_origin: None,
                    unsupported_capabilities,
                };
            };
            let attacker_origin = [
                attacker.position.x,
                attacker.position.y,
                attacker.position.z,
            ];
            let victim_origin = [victim.position.x, victim.position.y, victim.position.z];
            if attacker_origin == [0.0, 0.0, 0.0] || victim_origin == [0.0, 0.0, 0.0] {
                return SpatialShotEvidence {
                    shot_tick: link.shot_tick,
                    damage_tick: link.damage_tick,
                    snapshot_tick: Some(state.tick.0),
                    attacker_id: link.attacker_id,
                    victim_id: link.victim_id,
                    status: SpatialEvidenceStatus::Unavailable,
                    reason: SpatialEvidenceReason::InvalidPosition,
                    line_of_sight: OriginLineOfSight::Unknown,
                    attacker_origin: Some(attacker_origin),
                    victim_origin: Some(victim_origin),
                    unsupported_capabilities,
                };
            }
            if !attacker.alive || !victim.alive {
                return SpatialShotEvidence {
                    shot_tick: link.shot_tick,
                    damage_tick: link.damage_tick,
                    snapshot_tick: Some(state.tick.0),
                    attacker_id: link.attacker_id,
                    victim_id: link.victim_id,
                    status: SpatialEvidenceStatus::Unavailable,
                    reason: SpatialEvidenceReason::DeadPlayer,
                    line_of_sight: OriginLineOfSight::Unknown,
                    attacker_origin: Some(attacker_origin),
                    victim_origin: Some(victim_origin),
                    unsupported_capabilities,
                };
            }
            let (Some(attacker_eye_offset), Some(victim_eye_offset)) =
                (attacker.skeleton.eye_offset_z, victim.skeleton.eye_offset_z)
            else {
                let mut unsupported_capabilities = unsupported_capabilities;
                unsupported_capabilities.push(UnsupportedSpatialCapability::EyePosition);
                return SpatialShotEvidence {
                    shot_tick: link.shot_tick,
                    damage_tick: link.damage_tick,
                    snapshot_tick: Some(state.tick.0),
                    attacker_id: link.attacker_id,
                    victim_id: link.victim_id,
                    status: SpatialEvidenceStatus::Unavailable,
                    reason: SpatialEvidenceReason::MissingEyePosition,
                    line_of_sight: OriginLineOfSight::Unknown,
                    attacker_origin: Some(attacker_origin),
                    victim_origin: Some(victim_origin),
                    unsupported_capabilities,
                };
            };
            if !attacker_eye_offset.is_finite()
                || !victim_eye_offset.is_finite()
                || attacker_eye_offset < 0.0
                || victim_eye_offset < 0.0
            {
                let mut unsupported_capabilities = unsupported_capabilities;
                unsupported_capabilities.push(UnsupportedSpatialCapability::EyePosition);
                return SpatialShotEvidence {
                    shot_tick: link.shot_tick,
                    damage_tick: link.damage_tick,
                    snapshot_tick: Some(state.tick.0),
                    attacker_id: link.attacker_id,
                    victim_id: link.victim_id,
                    status: SpatialEvidenceStatus::Unavailable,
                    reason: SpatialEvidenceReason::MissingEyePosition,
                    line_of_sight: OriginLineOfSight::Unknown,
                    attacker_origin: Some(attacker_origin),
                    victim_origin: Some(victim_origin),
                    unsupported_capabilities,
                };
            }
            let attacker_eye = MapVec3::new(
                attacker.position.x,
                attacker.position.y,
                attacker.position.z + attacker_eye_offset,
            );
            let victim_eye = MapVec3::new(
                victim.position.x,
                victim.position.y,
                victim.position.z + victim_eye_offset,
            );
            SpatialShotEvidence {
                shot_tick: link.shot_tick,
                damage_tick: link.damage_tick,
                snapshot_tick: Some(state.tick.0),
                attacker_id: link.attacker_id,
                victim_id: link.victim_id,
                status: SpatialEvidenceStatus::Available,
                reason: SpatialEvidenceReason::EyeToEyeLineOfSight,
                line_of_sight: if map.segment_blocked_3d(attacker_eye, victim_eye) {
                    OriginLineOfSight::BlockedByWorld
                } else {
                    OriginLineOfSight::Clear
                },
                attacker_origin: Some(attacker_origin),
                victim_origin: Some(victim_origin),
                unsupported_capabilities,
            }
        })
        .collect()
}

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
    let linked_shot_damage =
        link_observed_shot_damage(&shots, &damage, DEFAULT_SHOT_DAMAGE_LINK_WINDOW_TICKS);
    let mut rebuilder = WorldRebuilder::new();
    let states = rebuilder.process_events_with_snapshots(&game_events, &adapter.player_snapshots());
    let kills = rebuilder.take_kills();
    let metadata = adapter.metadata();
    let map = loader::load_map_by_name(&metadata.map_name).unwrap_or_else(MapData::dust2);
    let map_ref = &map;
    let spatial_evidence = spatial_evidence_for_links(&states, map_ref, &linked_shot_damage);
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
                    eye_offset_z: player.skeleton.eye_offset_z,
                    duck_amount: player.skeleton.duck_amount,
                    hitbox_set: player.skeleton.hitbox_set,
                    model_handle: player.skeleton.model_handle,
                    anim_graph_id: player.skeleton.anim_graph_id,
                    pose_recipe_version: player.skeleton.pose_recipe_version,
                    generic_hitbox_geometry: (player.id.as_u64() != 0
                        && [player.position.x, player.position.y, player.position.z]
                            != [0.0, 0.0, 0.0])
                    .then(|| {
                        resolve_standard_player_fallback(
                            player.position,
                            player.view_angles.yaw,
                            &player.skeleton,
                        )
                    }),
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
        rounds: super::build_round_contexts(
            adapter,
            &states,
            &kills,
            &game_events,
            &damage,
            &linked_shot_damage,
        ),
        shots,
        damage,
        linked_shot_damage,
        spatial_evidence,
        quality: Default::default(),
    };
    replay.quality = replay.assess_quality();
    let json = serde_json::to_string_pretty(&replay).map_err(|error| error.to_string())?;
    std::fs::write(output_path, json).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use sentinel_map::MapData;
    use sentinel_report::{
        LinkedShotDamage, ShotDamageLinkConfidence,
        replay::{
            OriginLineOfSight, ReplayData, ReplayFrame, SpatialEvidenceReason,
            SpatialEvidenceStatus, UnsupportedSpatialCapability,
        },
    };

    use super::spatial_evidence_for_links;

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
            linked_shot_damage: Vec::new(),
            spatial_evidence: Vec::new(),
            quality: Default::default(),
        };
        assert!(serde_json::to_string(&replay).unwrap().contains("de_dust2"));
    }

    #[test]
    fn spatial_trace_is_unavailable_without_3d_collision() {
        let links = vec![LinkedShotDamage {
            shot_tick: 10,
            damage_tick: 10,
            attacker_id: 1,
            victim_id: 2,
            weapon: "ak47".into(),
            shot_to_damage_ticks: 0,
            linkage_confidence: ShotDamageLinkConfidence::CandidateNearestPriorShot,
        }];
        let evidence = spatial_evidence_for_links(&[], &MapData::dust2(), &links);
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].status, SpatialEvidenceStatus::Unavailable);
        assert_eq!(
            evidence[0].reason,
            SpatialEvidenceReason::MissingMapCollision
        );
        assert_eq!(evidence[0].line_of_sight, OriginLineOfSight::Unknown);
        assert!(
            evidence[0]
                .unsupported_capabilities
                .contains(&UnsupportedSpatialCapability::Hitboxes)
        );
    }
}
