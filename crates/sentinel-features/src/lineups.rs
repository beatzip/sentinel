//! Versioned utility-lineup contracts. The repository intentionally ships no production lineups:
//! every entry must be sourced and reviewed outside of code before being loaded.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UtilityGrenade {
    Smoke,
    Flash,
    He,
    Molotov,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UtilitySide {
    Terrorist,
    CounterTerrorist,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct LineupPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl LineupPoint {
    fn distance_to(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// A reviewed map-specific lineup. `source` must identify the local review/source record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtilityLineup {
    pub id: String,
    pub map_name: String,
    pub side: UtilitySide,
    pub grenade: UtilityGrenade,
    pub origin: LineupPoint,
    pub landing: LineupPoint,
    pub tags: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtilityLineupLibrary {
    pub version: u32,
    #[serde(default)]
    pub lineups: Vec<UtilityLineup>,
}

impl Default for UtilityLineupLibrary {
    fn default() -> Self {
        Self {
            version: 1,
            lineups: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UtilityLineupMatch<'a> {
    pub lineup: &'a UtilityLineup,
    pub origin_distance: f32,
    pub landing_distance: f32,
}

impl UtilityLineupLibrary {
    /// Returns the nearest reviewed lineup only when both origin and landing are within tolerance.
    pub fn match_lineup(
        &self,
        map_name: &str,
        side: UtilitySide,
        grenade: UtilityGrenade,
        origin: LineupPoint,
        landing: LineupPoint,
        tolerance: f32,
    ) -> Option<UtilityLineupMatch<'_>> {
        self.lineups
            .iter()
            .filter(|lineup| {
                lineup.map_name == map_name && lineup.side == side && lineup.grenade == grenade
            })
            .filter_map(|lineup| {
                let origin_distance = lineup.origin.distance_to(origin);
                let landing_distance = lineup.landing.distance_to(landing);
                (origin_distance <= tolerance && landing_distance <= tolerance).then_some(
                    UtilityLineupMatch {
                        lineup,
                        origin_distance,
                        landing_distance,
                    },
                )
            })
            .min_by(|left, right| {
                (left.origin_distance + left.landing_distance)
                    .total_cmp(&(right.origin_distance + right.landing_distance))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_only_reviewed_lineup_within_both_tolerances() {
        let library = UtilityLineupLibrary {
            version: 1,
            lineups: vec![UtilityLineup {
                id: "fixture".into(),
                map_name: "de_fixture".into(),
                side: UtilitySide::Terrorist,
                grenade: UtilityGrenade::Smoke,
                origin: LineupPoint::default(),
                landing: LineupPoint {
                    x: 10.0,
                    ..Default::default()
                },
                tags: Vec::new(),
                source: "test".into(),
            }],
        };
        assert!(
            library
                .match_lineup(
                    "de_fixture",
                    UtilitySide::Terrorist,
                    UtilityGrenade::Smoke,
                    LineupPoint {
                        x: 0.5,
                        ..Default::default()
                    },
                    LineupPoint {
                        x: 10.5,
                        ..Default::default()
                    },
                    1.0,
                )
                .is_some()
        );
        assert!(
            library
                .match_lineup(
                    "de_fixture",
                    UtilitySide::Terrorist,
                    UtilityGrenade::Smoke,
                    LineupPoint {
                        x: 3.0,
                        ..Default::default()
                    },
                    LineupPoint {
                        x: 10.5,
                        ..Default::default()
                    },
                    1.0,
                )
                .is_none()
        );
    }
}
