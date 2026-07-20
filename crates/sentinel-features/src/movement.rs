use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Tick};
use crate::traits::FeatureExt;

pub struct MovementSmoothness;

impl FeatureExt for MovementSmoothness {
    fn name(&self) -> &str { "movement_smoothness" }
    fn category(&self) -> FeatureCategory { FeatureCategory::Movement }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let window = 32u32;
        let start = tick.0.saturating_sub(window);
        let mut angles = Vec::new();

        for t in start..=tick.0 {
            if let Some(state) = ctx.state_at(Tick(t)) {
                if let Some(p) = state.players.iter().find(|p| p.id == player) {
                    if p.velocity.length() > 10.0 {
                        let angle = p.velocity.y.atan2(p.velocity.x) as f64;
                        angles.push(angle);
                    }
                }
            }
        }

        if angles.len() < 2 {
            return FeatureResult::new(0.8);
        }

        let mean = angles.iter().sum::<f64>() / angles.len() as f64;
        let variance = angles.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / angles.len() as f64;
        let stddev = variance.sqrt();
        let smoothness = (1.0 - stddev / 3.0).clamp(0.0, 1.0);
        FeatureResult::new(smoothness)
    }
}

pub struct CounterStrafeAccuracy;

impl FeatureExt for CounterStrafeAccuracy {
    fn name(&self) -> &str { "counter_strafe_accuracy" }
    fn category(&self) -> FeatureCategory { FeatureCategory::Movement }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.7),
        };
        let p = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(0.7),
        };
        let speed = p.velocity.length() as f64;
        let accuracy = if speed < 20.0 {
            0.9
        } else if speed < 50.0 {
            0.6
        } else {
            0.2
        };
        FeatureResult::new(accuracy)
    }
}

pub struct PathEfficiency;

impl FeatureExt for PathEfficiency {
    fn name(&self) -> &str { "path_efficiency" }
    fn category(&self) -> FeatureCategory { FeatureCategory::Movement }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let window = 64u32;
        let start = tick.0.saturating_sub(window);

        let pos_start = ctx.state_at(Tick(start))
            .and_then(|s| s.players.iter().find(|p| p.id == player))
            .map(|p| p.position);
        let pos_end = ctx.state_at(tick)
            .and_then(|s| s.players.iter().find(|p| p.id == player))
            .map(|p| p.position);

        match (pos_start, pos_end) {
            (Some(start_pos), Some(end_pos)) => {
                let displacement = start_pos.distance_to(&end_pos) as f64;
                let mut path_len = 0.0f64;
                for t in start..=tick.0 {
                    if let Some(state) = ctx.state_at(Tick(t)) {
                        if let Some(p) = state.players.iter().find(|p| p.id == player) {
                            path_len += p.velocity.length() as f64 / 64.0;
                        }
                    }
                }
                if path_len > 0.0 {
                    let efficiency = (displacement / path_len).clamp(0.0, 1.0);
                    FeatureResult::new(efficiency)
                } else {
                    FeatureResult::new(0.75)
                }
            }
            _ => FeatureResult::new(0.75),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movement_features() {
        assert_eq!(MovementSmoothness.name(), "movement_smoothness");
        assert_eq!(CounterStrafeAccuracy.name(), "counter_strafe_accuracy");
        assert_eq!(PathEfficiency.name(), "path_efficiency");
    }
}
