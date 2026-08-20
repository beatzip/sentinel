use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs::File, io::Read, path::Path};

use sentinel_core::{TickState, resolve_standard_player_fallback, source::DemoSource};
use sentinel_map::{MapData, Vec3 as MapVec3, loader};
use sentinel_report::{
    DEFAULT_SHOT_DAMAGE_LINK_WINDOW_TICKS, LinkedShotDamage, link_observed_shot_damage,
    replay::{
        ApproximateSpatialStatus, OriginLineOfSight, PlayerSpatialApproximate, ReplayData,
        ReplayFrame, ReplayPlayer, SpatialEvidenceReason, SpatialEvidenceStatus,
        SpatialShotEvidence, UnsupportedSpatialCapability, VerifiedModelMapping, VisibilityPair,
    },
};
use sentinel_visibility::VisibilityEngine;
use sentinel_world::WorldRebuilder;

const FRAME_INTERVAL_TICKS: usize = 32;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifiedModelMappingManifest {
    schema_version: u8,
    demo_sha256: String,
    game_build: String,
    mappings: Vec<VerifiedModelMappingManifestEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifiedModelMappingManifestEntry {
    model_handle: u64,
    hitbox_set: u8,
    pose_recipe_version: i32,
    asset_path: String,
    asset_sha256: String,
    resource_file: String,
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 65_536];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Unable to hash {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn verified_model_mappings(
    manifest_path: &Path,
    demo_path: &Path,
    observed: &BTreeSet<(u64, u8, i32)>,
) -> Result<Vec<VerifiedModelMapping>, String> {
    let json = std::fs::read_to_string(manifest_path)
        .map_err(|error| format!("Unable to read {}: {error}", manifest_path.display()))?;
    let manifest: VerifiedModelMappingManifest = serde_json::from_str(&json)
        .map_err(|error| format!("Invalid verified model manifest: {error}"))?;
    if manifest.schema_version != 1 {
        return Err("Verified model manifest must use schema_version 1".to_string());
    }
    if manifest.game_build.trim().is_empty() || !valid_sha256(&manifest.demo_sha256) {
        return Err(
            "Verified model manifest requires game_build and a SHA-256 demo identity".to_string(),
        );
    }
    if sha256_file(demo_path)? != manifest.demo_sha256.to_ascii_lowercase() {
        return Err(
            "Verified model manifest demo_sha256 does not match the exported demo".to_string(),
        );
    }
    if manifest.mappings.is_empty() {
        return Err("Verified model manifest contains no mappings".to_string());
    }
    let manifest_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut seen = BTreeSet::new();
    manifest
        .mappings
        .into_iter()
        .map(|entry| {
            let identity = (entry.model_handle, entry.hitbox_set, entry.pose_recipe_version);
            if !observed.contains(&identity) {
                return Err(format!(
                    "Verified model mapping {identity:?} was not observed in the exported replay"
                ));
            }
            if !seen.insert(identity) {
                return Err(format!("Verified model manifest duplicates mapping {identity:?}"));
            }
            if entry.asset_path.trim().is_empty()
                || Path::new(&entry.asset_path).is_absolute()
                || !valid_sha256(&entry.asset_sha256)
            {
                return Err("Verified model mapping requires a logical asset path and SHA-256 asset identity".to_string());
            }
            let resource_file = manifest_root.join(entry.resource_file);
            if sha256_file(&resource_file)? != entry.asset_sha256.to_ascii_lowercase() {
                return Err(format!(
                    "Verified model mapping asset hash does not match {}",
                    resource_file.display()
                ));
            }
            Ok(VerifiedModelMapping {
                model_handle: entry.model_handle,
                hitbox_set: entry.hitbox_set,
                pose_recipe_version: entry.pose_recipe_version,
                game_build: manifest.game_build.clone(),
                asset_path: entry.asset_path,
                asset_sha256: entry.asset_sha256.to_ascii_lowercase(),
                mapping_source: "verified_manifest".to_string(),
            })
        })
        .collect()
}

fn observed_model_identities(frames: &[ReplayFrame]) -> BTreeSet<(u64, u8, i32)> {
    frames
        .iter()
        .flat_map(|frame| frame.players.iter())
        .filter_map(|player| {
            Some((
                player.model_handle?,
                player.hitbox_set?,
                player.pose_recipe_version?,
            ))
        })
        .collect()
}

fn approximate_spatial_records(frames: &[ReplayFrame]) -> Vec<PlayerSpatialApproximate> {
    frames
        .iter()
        .flat_map(|frame| {
            frame.players.iter().filter_map(move |player| {
                let hitboxes = player.generic_hitbox_geometry.clone()?;
                Some(PlayerSpatialApproximate {
                    record_type: "player_spatial_approximate".to_string(),
                    tick: frame.tick,
                    round: frame.round,
                    player_id: player.steam_id,
                    status: ApproximateSpatialStatus::Available,
                    usage_scope: "exploratory_functional".to_string(),
                    evidence_allowed: false,
                    source: hitboxes.source,
                    confidence: hitboxes.confidence,
                    hitboxes,
                })
            })
        })
        .collect()
}

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
    export_with_verified_model_manifest(demo_path, output_path, None)
}

pub fn export_with_verified_model_manifest(
    demo_path: &Path,
    output_path: &Path,
    manifest_path: Option<&Path>,
) -> Result<(), String> {
    let adapter = sentinel_source2::Source2Adapter::from_file(demo_path)
        .map_err(|error| format!("Unable to parse demo: {error}"))?;
    export_adapter_with_verified_model_manifest(&adapter, output_path, demo_path, manifest_path)
}

pub fn export_adapter(
    adapter: &sentinel_source2::Source2Adapter,
    output_path: &Path,
) -> Result<(), String> {
    export_adapter_with_verified_model_manifest(adapter, output_path, Path::new(""), None)
}

fn export_adapter_with_verified_model_manifest(
    adapter: &sentinel_source2::Source2Adapter,
    output_path: &Path,
    demo_path: &Path,
    manifest_path: Option<&Path>,
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
    let frames: Vec<ReplayFrame> = states
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
    let approximate_spatial = approximate_spatial_records(&frames);
    let verified_model_mappings = manifest_path
        .map(|path| verified_model_mappings(path, demo_path, &observed_model_identities(&frames)))
        .transpose()?
        .unwrap_or_default();
    let mut replay = ReplayData {
        version: "1.3.0".to_string(),
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
        approximate_spatial,
        verified_model_mappings,
        quality: Default::default(),
    };
    replay.quality = replay.assess_quality();
    let json = serde_json::to_string_pretty(&replay).map_err(|error| error.to_string())?;
    std::fs::write(output_path, json).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use sentinel_core::{
        HitboxGeometryConfidence, HitboxGeometrySource, SkeletonMetadata, Vec3,
        resolve_standard_player_fallback,
    };
    use sentinel_map::MapData;
    use sentinel_report::{
        LinkedShotDamage, ShotDamageLinkConfidence,
        replay::{
            ApproximateSpatialStatus, OriginLineOfSight, ReplayData, ReplayFrame, ReplayPlayer,
            SpatialEvidenceReason, SpatialEvidenceStatus, UnsupportedSpatialCapability,
        },
    };

    use super::{
        approximate_spatial_records, sha256_file, spatial_evidence_for_links,
        verified_model_mappings,
    };

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
            approximate_spatial: Vec::new(),
            verified_model_mappings: Vec::new(),
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

    #[test]
    fn approximate_records_are_isolated_from_evidence_contract() {
        let hitboxes = resolve_standard_player_fallback(
            Vec3::new(1.0, 2.0, 3.0),
            90.0,
            &SkeletonMetadata::default(),
        );
        let records = approximate_spatial_records(&[ReplayFrame {
            tick: 64,
            round: 2,
            players: vec![ReplayPlayer {
                steam_id: 1,
                name: "player".into(),
                team: "CounterTerrorist".into(),
                x: 1.0,
                y: 2.0,
                z: 3.0,
                health: 100,
                alive: true,
                yaw: 90.0,
                pitch: 0.0,
                eye_offset_z: None,
                duck_amount: None,
                hitbox_set: None,
                model_handle: None,
                anim_graph_id: None,
                pose_recipe_version: None,
                generic_hitbox_geometry: Some(hitboxes),
            }],
            visible_pairs: Vec::new(),
        }]);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record_type, "player_spatial_approximate");
        assert_eq!(records[0].status, ApproximateSpatialStatus::Available);
        assert_eq!(records[0].usage_scope, "exploratory_functional");
        assert!(!records[0].evidence_allowed);
        assert_eq!(records[0].source, HitboxGeometrySource::GenericFallback);
        assert_eq!(records[0].confidence, HitboxGeometryConfidence::Approximate);
    }

    #[test]
    fn verified_model_manifest_requires_matching_demo_asset_and_observed_identity() {
        use std::{collections::BTreeSet, fs};

        let root =
            std::env::temp_dir().join(format!("sentinel-model-manifest-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let demo = root.join("match.dem");
        let asset = root.join("player.vmdl_c");
        fs::write(&demo, b"observed demo bytes").unwrap();
        fs::write(&asset, b"verified asset bytes").unwrap();
        let manifest = root.join("mapping.json");
        fs::write(
            &manifest,
            serde_json::json!({
                "schema_version": 1,
                "demo_sha256": sha256_file(&demo).unwrap(),
                "game_build": "verified-test-build",
                "mappings": [{
                    "model_handle": 77,
                    "hitbox_set": 0,
                    "pose_recipe_version": 2,
                    "asset_path": "models/player/test.vmdl_c",
                    "asset_sha256": sha256_file(&asset).unwrap(),
                    "resource_file": "player.vmdl_c"
                }]
            })
            .to_string(),
        )
        .unwrap();
        let observed = BTreeSet::from([(77, 0, 2)]);
        let mappings = verified_model_mappings(&manifest, &demo, &observed).unwrap();
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].mapping_source, "verified_manifest");
        assert!(verified_model_mappings(&manifest, &demo, &BTreeSet::new()).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
