use crate::traits::FeatureExt;
use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Tick};

/// Relative buy power within the alive team. A low value marks an eco-constrained decision.
pub struct EconomyShare;

impl FeatureExt for EconomyShare {
    fn name(&self) -> &str {
        "economy_share"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let Some(state) = ctx.state_at(tick) else {
            return FeatureResult::new(0.5);
        };
        let Some(observer) = state
            .players
            .iter()
            .find(|candidate| candidate.id == player)
        else {
            return FeatureResult::new(0.5);
        };
        let teammates = state
            .players
            .iter()
            .filter(|candidate| candidate.team == observer.team && candidate.alive)
            .collect::<Vec<_>>();
        if teammates.is_empty() {
            return FeatureResult::new(0.5);
        }
        let team_average = teammates
            .iter()
            .map(|candidate| candidate.money)
            .sum::<i32>() as f64
            / teammates.len() as f64;
        let share = if team_average > 0.0 {
            (observer.money as f64 / team_average).min(2.0) / 2.0
        } else {
            0.0
        };
        FeatureResult::new(share)
            .with_metadata("money".to_string(), observer.money.to_string())
            .with_metadata("team_average".to_string(), format!("{team_average:.0}"))
    }
}

/// Pressure in a last-alive-player situation, normalized by remaining opponents.
pub struct ClutchPressure;

impl FeatureExt for ClutchPressure {
    fn name(&self) -> &str {
        "clutch_pressure"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let Some(state) = ctx.state_at(tick) else {
            return FeatureResult::new(0.0);
        };
        let Some(observer) = state
            .players
            .iter()
            .find(|candidate| candidate.id == player)
        else {
            return FeatureResult::new(0.0);
        };
        let alive_teammates = state
            .players
            .iter()
            .filter(|candidate| candidate.team == observer.team && candidate.alive)
            .count();
        let opponents = state
            .players
            .iter()
            .filter(|candidate| candidate.team != observer.team && candidate.alive)
            .count();
        let pressure = if observer.alive && alive_teammates == 1 {
            (opponents as f64 / 5.0).min(1.0)
        } else {
            0.0
        };
        FeatureResult::new(pressure)
            .with_metadata("opponents_alive".to_string(), opponents.to_string())
            .with_metadata("is_clutch".to_string(), (pressure > 0.0).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advanced_feature_metadata_is_stable() {
        assert_eq!(EconomyShare.name(), "economy_share");
        assert_eq!(ClutchPressure.name(), "clutch_pressure");
        assert_eq!(ClutchPressure.category(), FeatureCategory::Decision);
    }
}
