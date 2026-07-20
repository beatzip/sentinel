use crate::traits::FeatureExt;
use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Tick};

pub struct RotationReactionTime;

impl FeatureExt for RotationReactionTime {
    fn name(&self) -> &str {
        "rotation_reaction_time"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Rotation
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(2.0),
        };
        let p = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(2.0),
        };
        let lookback = tick.0.saturating_sub(3 * 64);
        if let Some(prev) = ctx.state_at(Tick(lookback)) {
            if let Some(prev_p) = prev.players.iter().find(|pp| pp.id == player) {
                let yaw_change = (p.view_angles.yaw - prev_p.view_angles.yaw).abs();
                let yaw_change = if yaw_change > 180.0 {
                    360.0 - yaw_change
                } else {
                    yaw_change
                };
                if yaw_change > 90.0 {
                    let time = 3.0 - (yaw_change as f64 / 180.0);
                    return FeatureResult::new(time.max(0.5));
                }
            }
        }
        FeatureResult::new(2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotation_features() {
        assert_eq!(RotationReactionTime.name(), "rotation_reaction_time");
    }
}
