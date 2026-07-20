use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Tick};
use crate::traits::FeatureExt;

/// Kill/death ratio computed from match state
pub struct KDRatio;

impl FeatureExt for KDRatio {
    fn name(&self) -> &str { "kd_ratio" }
    fn category(&self) -> FeatureCategory { FeatureCategory::General }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        // Count kills and deaths from evidence up to this tick
        let evidence = ctx.evidence();
        let kills = evidence.iter()
            .filter(|e| e.player == player && e.feature == "kill" && e.tick.0 <= tick.0)
            .count();
        let deaths = evidence.iter()
            .filter(|e| e.player == player && e.feature == "death" && e.tick.0 <= tick.0)
            .count();
        let kd = if deaths == 0 {
            kills as f64
        } else {
            kills as f64 / deaths as f64
        };
        FeatureResult::new(kd)
    }
}

/// Headshot percentage
pub struct HeadshotPercentage;

impl FeatureExt for HeadshotPercentage {
    fn name(&self) -> &str { "headshot_percentage" }
    fn category(&self) -> FeatureCategory { FeatureCategory::General }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let evidence = ctx.evidence();
        let total_kills = evidence.iter()
            .filter(|e| e.player == player && e.feature == "kill" && e.tick.0 <= tick.0)
            .count();
        let headshots = evidence.iter()
            .filter(|e| e.player == player && e.feature == "headshot" && e.tick.0 <= tick.0)
            .count();
        let percentage = if total_kills == 0 {
            0.4 // Default
        } else {
            headshots as f64 / total_kills as f64
        };
        FeatureResult::new(percentage)
    }
}

/// Survival time: average time alive per round
pub struct SurvivalTime;

impl FeatureExt for SurvivalTime {
    fn name(&self) -> &str { "survival_time" }
    fn category(&self) -> FeatureCategory { FeatureCategory::General }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(60.0),
        };
        let p = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(60.0),
        };
        // Time since round start
        let round_start = tick.0.saturating_sub(state.round.clock as u32 * 64 / 115 * 115);
        let survival = (tick.0 - round_start) as f64 / 64.0;
        FeatureResult::new(survival.min(115.0))
            .with_metadata("unit".to_string(), "seconds".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_general_features() {
        assert_eq!(KDRatio.name(), "kd_ratio");
        assert_eq!(HeadshotPercentage.name(), "headshot_percentage");
        assert_eq!(SurvivalTime.name(), "survival_time");
        assert_eq!(KDRatio.category(), FeatureCategory::General);
    }
}
