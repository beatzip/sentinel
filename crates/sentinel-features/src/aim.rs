use crate::traits::FeatureExt;
use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Tick};

pub struct ReactionTime;

impl FeatureExt for ReactionTime {
    fn name(&self) -> &str {
        "reaction_time"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Aim
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.25),
        };
        let observer = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(0.25),
        };
        let target_team = match observer.team {
            sentinel_core::Team::Terrorist => sentinel_core::Team::CounterTerrorist,
            sentinel_core::Team::CounterTerrorist => sentinel_core::Team::Terrorist,
            _ => return FeatureResult::new(0.25),
        };
        let closest = state
            .players
            .iter()
            .filter(|p| p.team == target_team && p.alive)
            .min_by(|a, b| {
                let da = observer.position.distance_to(&a.position);
                let db = observer.position.distance_to(&b.position);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
        let closest = match closest {
            Some(p) => p,
            None => return FeatureResult::new(0.25),
        };
        let distance = observer.position.distance_to(&closest.position) as f64;
        let dx = closest.position.x - observer.position.x;
        let dy = closest.position.y - observer.position.y;
        let angle_to_target = (dy as f64).atan2(dx as f64).to_degrees();
        let angle_diff = (angle_to_target - observer.view_angles.yaw as f64).abs();
        let angle_diff = if angle_diff > 180.0 {
            360.0 - angle_diff
        } else {
            angle_diff
        };
        let base = 0.35;
        let distance_bonus = (distance / 4000.0).min(0.15);
        let angle_penalty = if angle_diff < 30.0 {
            0.0
        } else {
            angle_diff / 180.0 * 0.1
        };
        let reaction = (base - distance_bonus + angle_penalty).clamp(0.08, 0.5);
        FeatureResult::new(reaction)
            .with_metadata("distance".to_string(), format!("{distance:.0}"))
            .with_metadata("angle_diff".to_string(), format!("{angle_diff:.1}"))
    }
}

pub struct CrosshairPlacementError;

impl FeatureExt for CrosshairPlacementError {
    fn name(&self) -> &str {
        "crosshair_placement_error"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Aim
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(15.0),
        };
        let p = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(15.0),
        };
        let pitch_error = p.view_angles.pitch.abs() as f64;
        let error = pitch_error * 0.5;
        FeatureResult::new(error)
    }
}

pub struct AimVelocity;

impl FeatureExt for AimVelocity {
    fn name(&self) -> &str {
        "aim_velocity"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Aim
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let prev_tick = Tick(tick.0.saturating_sub(1));
        let current = ctx
            .state_at(tick)
            .and_then(|s| s.players.iter().find(|p| p.id == player));
        let previous = ctx
            .state_at(prev_tick)
            .and_then(|s| s.players.iter().find(|p| p.id == player));

        let velocity = match (current, previous) {
            (Some(curr), Some(prev)) => {
                let yaw_diff = (curr.view_angles.yaw - prev.view_angles.yaw).abs() as f64;
                let pitch_diff = (curr.view_angles.pitch - prev.view_angles.pitch).abs() as f64;
                (yaw_diff + pitch_diff) * 64.0
            }
            _ => 120.0,
        };
        FeatureResult::new(velocity)
    }
}

pub struct TrackingSmoothness;

impl FeatureExt for TrackingSmoothness {
    fn name(&self) -> &str {
        "tracking_smoothness"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Aim
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let window = 64u32;
        let start_tick = tick.0.saturating_sub(window);
        let mut velocities = Vec::new();

        for t in start_tick..tick.0 {
            let curr = ctx
                .state_at(Tick(t))
                .and_then(|s| s.players.iter().find(|p| p.id == player));
            let prev = ctx
                .state_at(Tick(t.saturating_sub(1)))
                .and_then(|s| s.players.iter().find(|p| p.id == player));
            if let (Some(c), Some(p)) = (curr, prev) {
                let yaw_diff = (c.view_angles.yaw - p.view_angles.yaw).abs() as f64;
                let pitch_diff = (c.view_angles.pitch - p.view_angles.pitch).abs() as f64;
                velocities.push(yaw_diff + pitch_diff);
            }
        }

        if velocities.len() < 2 {
            return FeatureResult::new(0.85);
        }

        let mean = velocities.iter().sum::<f64>() / velocities.len() as f64;
        let variance =
            velocities.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / velocities.len() as f64;
        let stddev = variance.sqrt();
        let smoothness = (1.0 - (stddev / 5.0)).clamp(0.0, 1.0);
        FeatureResult::new(smoothness)
    }
}

pub struct TargetSwitchSpeed;

impl FeatureExt for TargetSwitchSpeed {
    fn name(&self) -> &str {
        "target_switch_speed"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Aim
    }

    fn compute(&self, _ctx: &MatchContext, _tick: Tick, _player: PlayerId) -> FeatureResult {
        FeatureResult::new(0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_names() {
        assert_eq!(ReactionTime.name(), "reaction_time");
        assert_eq!(CrosshairPlacementError.name(), "crosshair_placement_error");
        assert_eq!(AimVelocity.name(), "aim_velocity");
        assert_eq!(TrackingSmoothness.name(), "tracking_smoothness");
        assert_eq!(TargetSwitchSpeed.name(), "target_switch_speed");
    }
}
