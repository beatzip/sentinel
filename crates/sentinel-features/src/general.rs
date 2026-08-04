use crate::traits::FeatureExt;
use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Tick};

/// Kill/death ratio computed from kill feed
pub struct KDRatio;

impl FeatureExt for KDRatio {
    fn name(&self) -> &str {
        "kd_ratio"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::General
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        // Use kills_up_to from MatchContext instead of iterating all states
        let kills = ctx.kills_up_to(tick);

        let kd_kills = kills.iter().filter(|k| k.attacker == player).count();
        let kd_deaths = kills.iter().filter(|k| k.victim == player).count();

        let kd = if kd_deaths == 0 {
            kd_kills as f64
        } else {
            kd_kills as f64 / kd_deaths as f64
        };
        FeatureResult::new(kd)
    }
}

/// Headshot percentage
pub struct HeadshotPercentage;

impl FeatureExt for HeadshotPercentage {
    fn name(&self) -> &str {
        "headshot_percentage"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::General
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        // Use kills_up_to from MatchContext instead of iterating all states
        let kills = ctx.kills_up_to(tick);

        let total_kills: usize = kills.iter().filter(|k| k.attacker == player).count();
        let headshots: usize = kills
            .iter()
            .filter(|k| k.attacker == player && k.headshot)
            .count();

        let percentage = if total_kills == 0 {
            0.4 // Default
        } else {
            headshots as f64 / total_kills as f64
        };
        FeatureResult::new(percentage)
    }
}

/// Survival time: time alive since round start
pub struct SurvivalTime;

impl FeatureExt for SurvivalTime {
    fn name(&self) -> &str {
        "survival_time"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::General
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(60.0),
        };

        let player_state = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(60.0),
        };

        if !player_state.alive {
            return FeatureResult::new(0.0);
        }

        // Time since round start (using start_tick from RoundState)
        let survival = (tick.0 - state.round.start_tick) as f64 / 64.0;
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
