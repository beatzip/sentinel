use serde::{Deserialize, Serialize};

use super::{LinkedShotDamage, ObservedDamage, ObservedShot, RoundContext};

/// Availability state for one candidate shot-to-damage spatial trace.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpatialEvidenceStatus {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OriginLineOfSight {
    Clear,
    BlockedByWorld,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpatialEvidenceReason {
    OriginToOriginLosOnly,
    EyeToEyeLineOfSight,
    MissingMapCollision,
    MissingPlayerSnapshot,
    InvalidPosition,
    MissingEyePosition,
    DeadPlayer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedSpatialCapability {
    EyePosition,
    Hitboxes,
    PenetrationModel,
}

/// A quality-gated world trace for a candidate linked shot and damage event.
///
/// It traces observed eye positions when the source exposes them. This is neither a hitbox
/// intersection nor a bullet-penetration calculation and cannot create a cheat verdict by itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpatialShotEvidence {
    pub shot_tick: u32,
    pub damage_tick: u32,
    pub snapshot_tick: Option<u32>,
    pub attacker_id: u64,
    pub victim_id: u64,
    pub status: SpatialEvidenceStatus,
    pub reason: SpatialEvidenceReason,
    pub line_of_sight: OriginLineOfSight,
    pub attacker_origin: Option<[f32; 3]>,
    pub victim_origin: Option<[f32; 3]>,
    #[serde(default)]
    pub unsupported_capabilities: Vec<UnsupportedSpatialCapability>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReplayQualityStatus {
    #[default]
    Unassessed,
    Sufficient,
    InsufficientEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplayQualityIssue {
    NoFrames,
    NoPlayerFrames,
    AllPositionsZero,
    AllYawZero,
    AllPitchZero,
    AllTeamsUnassigned,
    NoRoundRecords,
    NoVisiblePairs,
    NoShots,
    NoDamageEvents,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayQuality {
    pub status: ReplayQualityStatus,
    pub issues: Vec<ReplayQualityIssue>,
    pub player_samples: usize,
    pub nonzero_position_samples: usize,
    pub nonzero_yaw_samples: usize,
    pub nonzero_pitch_samples: usize,
}

/// Browser-friendly replay data, produced alongside a Sentinel report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayData {
    pub version: String,
    pub map: String,
    pub tick_rate: u32,
    pub frames: Vec<ReplayFrame>,
    /// Same round contexts as the match report for viewer-side evidence drill-down.
    #[serde(default)]
    pub rounds: Vec<RoundContext>,
    /// Every normalized weapon_fire event observed in this demo.
    #[serde(default)]
    pub shots: Vec<ObservedShot>,
    /// Every normalized player_hurt event observed in this demo.
    #[serde(default)]
    pub damage: Vec<ObservedDamage>,
    /// Candidate nearest-prior observed shot links for each observed damage event.
    #[serde(default)]
    pub linked_shot_damage: Vec<LinkedShotDamage>,
    /// Quality-gated origin-to-origin LOS facts for linked combat events.
    #[serde(default)]
    pub spatial_evidence: Vec<SpatialShotEvidence>,
    /// Gate that prevents anti-cheat interpretation when essential replay telemetry is absent.
    #[serde(default)]
    pub quality: ReplayQuality,
}

impl ReplayData {
    pub fn assess_quality(&self) -> ReplayQuality {
        let players = self
            .frames
            .iter()
            .flat_map(|frame| frame.players.iter())
            .filter(|player| player.steam_id != 0)
            .collect::<Vec<_>>();
        let player_samples = players.len();
        let nonzero_position_samples = players
            .iter()
            .filter(|player| player.x != 0.0 || player.y != 0.0 || player.z != 0.0)
            .count();
        let nonzero_yaw_samples = players.iter().filter(|player| player.yaw != 0.0).count();
        let nonzero_pitch_samples = players.iter().filter(|player| player.pitch != 0.0).count();
        let all_teams_unassigned =
            !players.is_empty() && players.iter().all(|player| player.team == "Unassigned");
        let has_visible_pairs = self
            .frames
            .iter()
            .any(|frame| !frame.visible_pairs.is_empty());
        let mut issues = Vec::new();
        if self.frames.is_empty() {
            issues.push(ReplayQualityIssue::NoFrames);
        }
        if player_samples == 0 {
            issues.push(ReplayQualityIssue::NoPlayerFrames);
        } else {
            if nonzero_position_samples == 0 {
                issues.push(ReplayQualityIssue::AllPositionsZero);
            }
            if nonzero_yaw_samples == 0 {
                issues.push(ReplayQualityIssue::AllYawZero);
            }
            if nonzero_pitch_samples == 0 {
                issues.push(ReplayQualityIssue::AllPitchZero);
            }
        }
        if all_teams_unassigned {
            issues.push(ReplayQualityIssue::AllTeamsUnassigned);
        }
        if self.rounds.is_empty() {
            issues.push(ReplayQualityIssue::NoRoundRecords);
        }
        if !has_visible_pairs {
            issues.push(ReplayQualityIssue::NoVisiblePairs);
        }
        if self.shots.is_empty() {
            issues.push(ReplayQualityIssue::NoShots);
        }
        if self.damage.is_empty() {
            issues.push(ReplayQualityIssue::NoDamageEvents);
        }
        ReplayQuality {
            status: if issues.is_empty() {
                ReplayQualityStatus::Sufficient
            } else {
                ReplayQualityStatus::InsufficientEvidence
            },
            issues,
            player_samples,
            nonzero_position_samples,
            nonzero_yaw_samples,
            nonzero_pitch_samples,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayFrame {
    pub tick: u32,
    pub round: u32,
    pub players: Vec<ReplayPlayer>,
    /// Directed pairs where the first player has a line of sight to the second.
    pub visible_pairs: Vec<VisibilityPair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayPlayer {
    pub steam_id: u64,
    pub name: String,
    pub team: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub health: i32,
    pub alive: bool,
    pub yaw: f32,
    pub pitch: f32,
    #[serde(default)]
    pub eye_offset_z: Option<f32>,
    #[serde(default)]
    pub duck_amount: Option<f32>,
    #[serde(default)]
    pub hitbox_set: Option<u8>,
    #[serde(default)]
    pub model_handle: Option<u64>,
    #[serde(default)]
    pub anim_graph_id: Option<u64>,
    #[serde(default)]
    pub pose_recipe_version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisibilityPair {
    pub observer: u64,
    pub target: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replay(player: ReplayPlayer) -> ReplayData {
        ReplayData {
            version: "1.1.0".into(),
            map: "de_test".into(),
            tick_rate: 64,
            frames: vec![ReplayFrame {
                tick: 64,
                round: 1,
                players: vec![player],
                visible_pairs: vec![VisibilityPair {
                    observer: 1,
                    target: 2,
                }],
            }],
            rounds: vec![RoundContext::default()],
            shots: vec![ObservedShot {
                tick: 64,
                shooter_id: 1,
                weapon: "ak47".into(),
                penetrated: 0,
                is_alt_fire: false,
            }],
            damage: vec![ObservedDamage {
                tick: 64,
                victim_id: 2,
                attacker_id: Some(1),
                weapon: "ak47".into(),
                dmg_health: 10,
                dmg_armor: 0,
                hitgroup: "chest".into(),
                dmg_health_real: 10,
            }],
            linked_shot_damage: vec![],
            spatial_evidence: vec![],
            quality: ReplayQuality::default(),
        }
    }

    #[test]
    fn zero_telemetry_is_insufficient_evidence() {
        let quality = replay(ReplayPlayer {
            steam_id: 1,
            name: "player".into(),
            team: "Unassigned".into(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
            health: 100,
            alive: true,
            yaw: 0.0,
            pitch: 0.0,
            eye_offset_z: None,
            duck_amount: None,
            hitbox_set: None,
            model_handle: None,
            anim_graph_id: None,
            pose_recipe_version: None,
        })
        .assess_quality();
        assert_eq!(quality.status, ReplayQualityStatus::InsufficientEvidence);
        assert!(
            quality
                .issues
                .contains(&ReplayQualityIssue::AllPositionsZero)
        );
        assert!(quality.issues.contains(&ReplayQualityIssue::AllPitchZero));
    }

    #[test]
    fn full_telemetry_is_sufficient() {
        let quality = replay(ReplayPlayer {
            steam_id: 1,
            name: "player".into(),
            team: "CounterTerrorist".into(),
            x: 1.0,
            y: 0.0,
            z: 0.0,
            health: 100,
            alive: true,
            yaw: 1.0,
            pitch: 1.0,
            eye_offset_z: None,
            duck_amount: None,
            hitbox_set: None,
            model_handle: None,
            anim_graph_id: None,
            pose_recipe_version: None,
        })
        .assess_quality();
        assert_eq!(quality.status, ReplayQualityStatus::Sufficient);
        assert!(quality.issues.is_empty());
    }
}
