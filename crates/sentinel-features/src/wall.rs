use crate::traits::FeatureExt;
use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Tick};
use sentinel_visibility::VisibilityEngine;

/// Hidden tracking duration: time player tracks unseen enemy
pub struct HiddenTrackingDuration;

impl FeatureExt for HiddenTrackingDuration {
    fn name(&self) -> &str {
        "hidden_tracking_duration"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Wall
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.0),
        };
        let observer = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(0.0),
        };

        let target_team = match observer.team {
            sentinel_core::Team::Terrorist => sentinel_core::Team::CounterTerrorist,
            sentinel_core::Team::CounterTerrorist => sentinel_core::Team::Terrorist,
            _ => return FeatureResult::new(0.0),
        };

        let mut hidden_tracking = 0.0f64;

        // Check each enemy
        for enemy in state
            .players
            .iter()
            .filter(|p| p.team == target_team && p.alive)
        {
            // Check if crosshair is tracking this enemy
            let dx = enemy.position.x - observer.position.x;
            let dy = enemy.position.y - observer.position.y;
            let angle_to_enemy = dy.atan2(dx).to_degrees();
            let angle_diff = (angle_to_enemy - observer.view_angles.yaw).abs();
            let angle_diff = if angle_diff > 180.0 {
                360.0 - angle_diff
            } else {
                angle_diff
            };

            // If aiming within 15 degrees of enemy
            if angle_diff < 15.0 {
                // Check if enemy is visible using visibility engine
                let vis = VisibilityEngine::can_see(state, observer.id, enemy.id);

                // If enemy is NOT visible but player is tracking them
                if !vis.visible {
                    let distance = observer.position.distance_to(&enemy.position);
                    // Only count if enemy is far enough to be behind a wall
                    if distance > 500.0 {
                        hidden_tracking += 1.0 / 64.0; // One tick
                    }
                }
            }
        }

        FeatureResult::new(hidden_tracking.min(2.0))
            .with_metadata("unit".to_string(), "seconds".to_string())
    }
}

/// Information Availability Index: how much information does player have?
pub struct InformationAvailability;

impl FeatureExt for InformationAvailability {
    fn name(&self) -> &str {
        "information_availability"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Wall
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.0),
        };

        let vis_state = VisibilityEngine::get_visibility_state(state, player, tick);

        // Calculate information score based on:
        // - Visible enemies (direct sight)
        // - Audible enemies (sound)
        // - Radar information
        // - Teammate spots

        let visible_count = vis_state.visible_enemies.len() as f64;
        let audible_count = vis_state.audible_enemies.len() as f64;
        let radar_count = vis_state.radar_visible.len() as f64;
        let spotted_count = vis_state.spotted_by.len() as f64;

        // Weighted information score
        let info_score =
            (visible_count * 1.0 + audible_count * 0.5 + radar_count * 0.3 + spotted_count * 0.7)
                / 10.0;

        FeatureResult::new(info_score.min(1.0))
            .with_metadata("visible".to_string(), visible_count.to_string())
            .with_metadata("audible".to_string(), audible_count.to_string())
            .with_metadata("radar".to_string(), radar_count.to_string())
    }
}

/// Prefire rate: frequency of shooting before enemy is visible
pub struct PrefireRate;

impl FeatureExt for PrefireRate {
    fn name(&self) -> &str {
        "prefire_rate"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Wall
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.0),
        };
        let observer = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(0.0),
        };

        // Check if player is shooting (has a gun and is active)
        if !observer.weapon.is_gun() {
            return FeatureResult::new(0.0);
        }

        // Look back 5 seconds for recent kills/shots
        let window_start = tick.0.saturating_sub(5 * 64);
        let states = ctx.states();
        let start_idx = (window_start as usize).min(states.len());
        let end_idx = (tick.0 as usize).min(states.len());

        if start_idx >= end_idx {
            return FeatureResult::new(0.0);
        }

        // Count shots where target was not visible
        let mut prefire_shots = 0;
        let mut total_shots = 0;

        for state in &states[start_idx..end_idx] {
            if let Some(obs) = state.players.iter().find(|p| p.id == player)
                && obs.weapon.is_gun()
            {
                total_shots += 1;

                // Check if any enemy was visible at this tick
                let target_team = match obs.team {
                    sentinel_core::Team::Terrorist => sentinel_core::Team::CounterTerrorist,
                    sentinel_core::Team::CounterTerrorist => sentinel_core::Team::Terrorist,
                    _ => continue,
                };

                let any_visible = state
                    .players
                    .iter()
                    .filter(|p| p.team == target_team && p.alive)
                    .any(|enemy| VisibilityEngine::can_see(state, obs.id, enemy.id).visible);

                if !any_visible {
                    prefire_shots += 1;
                }
            }
        }

        let prefire_rate = if total_shots > 0 {
            prefire_shots as f64 / total_shots as f64
        } else {
            0.0
        };

        FeatureResult::new(prefire_rate.min(1.0))
            .with_metadata("prefire_shots".to_string(), prefire_shots.to_string())
            .with_metadata("total_shots".to_string(), total_shots.to_string())
    }
}

/// Rotation justification: does player rotate based on information?
/// MODIFIED: Uses solo playstyle to reduce false positives
pub struct RotationJustification;

impl FeatureExt for RotationJustification {
    fn name(&self) -> &str {
        "rotation_justification"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Wall
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.5),
        };

        let observer = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(0.5),
        };

        let my_team = observer.team;
        if my_team == sentinel_core::Team::Unassigned {
            return FeatureResult::new(0.5);
        }

        // Step 1: Calculate solo playstyle score
        let solo_score = Self::calculate_solo_score_static(observer, state, my_team);

        // Step 2: Look back 10 seconds for teammate deaths
        let window_start = tick.0.saturating_sub(10 * 64);
        let states = ctx.states();
        let start_idx = (window_start as usize).min(states.len());
        let end_idx = (tick.0 as usize).min(states.len());

        if start_idx >= end_idx {
            return FeatureResult::new(0.5);
        }

        // Count teammate deaths in window
        let teammate_deaths: usize = states[start_idx..end_idx]
            .iter()
            .filter(|s| s.players.iter().any(|p| p.team == my_team && !p.alive))
            .count();

        // Step 3: Calculate base rotation justification
        let base_justification = if teammate_deaths > 0 {
            0.7 // Good justification (player should rotate)
        } else {
            0.5 // Neutral (no deaths to react to)
        };

        // Step 4: Adjust based on solo playstyle
        // If player is a solo player, reduce the anomaly score
        // Solo players are EXPECTED to not rotate, so it's less suspicious
        let adjusted_justification = if solo_score > 0.7 {
            // High solo score = player consistently plays alone
            // This is expected behavior, not anomalous
            base_justification * (1.0 - solo_score * 0.5)
        } else {
            base_justification
        };

        FeatureResult::new(adjusted_justification)
            .with_metadata("teammate_deaths".to_string(), teammate_deaths.to_string())
            .with_metadata("solo_score".to_string(), format!("{:.2}", solo_score))
            .with_metadata(
                "base_justification".to_string(),
                format!("{:.2}", base_justification),
            )
    }
}

impl RotationJustification {
    /// Calculate solo playstyle score for a player (static method)
    fn calculate_solo_score_static(
        observer: &sentinel_core::PlayerState,
        state: &sentinel_core::TickState,
        my_team: sentinel_core::Team,
    ) -> f64 {
        let teammates: Vec<_> = state
            .players
            .iter()
            .filter(|p| p.team == my_team && p.id != observer.id && p.alive)
            .collect();

        if teammates.is_empty() {
            return 1.0; // Alone = maximum solo score
        }

        // Calculate average distance to teammates
        let avg_distance: f64 = teammates
            .iter()
            .map(|t| observer.position.distance_to(&t.position) as f64)
            .sum::<f64>()
            / teammates.len() as f64;

        // Check how many teammates are nearby (< 1000 units)
        let near_teammate_count = teammates
            .iter()
            .filter(|t| observer.position.distance_to(&t.position) < 1000.0)
            .count();

        let isolation_ratio = 1.0 - (near_teammate_count as f64 / teammates.len() as f64);

        // Check if player is the only one alive
        let alive_teammates = state
            .players
            .iter()
            .filter(|p| p.team == my_team && p.id != observer.id && p.alive)
            .count();
        let solo_alive = if alive_teammates == 0 { 1.0 } else { 0.0 };

        // Combine metrics
            (isolation_ratio * 0.5 + (avg_distance / 3000.0).min(1.0) * 0.3 + solo_alive * 0.2)
                .min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wall_features() {
        assert_eq!(HiddenTrackingDuration.name(), "hidden_tracking_duration");
        assert_eq!(InformationAvailability.name(), "information_availability");
        assert_eq!(PrefireRate.name(), "prefire_rate");
        assert_eq!(RotationJustification.name(), "rotation_justification");
        assert_eq!(HiddenTrackingDuration.category(), FeatureCategory::Wall);
    }
}
