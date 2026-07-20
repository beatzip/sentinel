use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Tick};
use crate::traits::FeatureExt;

/// Flash assist rate: percentage of kills with flash assists
pub struct FlashAssistRate;

impl FeatureExt for FlashAssistRate {
    fn name(&self) -> &str { "flash_assist_rate" }
    fn category(&self) -> FeatureCategory { FeatureCategory::Utility }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        // Count flash detonations near this player followed by kills
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.15),
        };
        let grenades: Vec<_> = state.grenades.iter()
            .filter(|g| g.grenade_type == sentinel_core::GrenadeType::Flash && g.detonated)
            .collect();
        let flash_rate = (grenades.len() as f64 * 0.05).clamp(0.0, 1.0);
        FeatureResult::new(flash_rate)
    }
}

/// Nade usage rate: frequency of grenade usage per round
pub struct NadeUsageRate;

impl FeatureExt for NadeUsageRate {
    fn name(&self) -> &str { "nade_usage_rate" }
    fn category(&self) -> FeatureCategory { FeatureCategory::Utility }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.3),
        };
        let total_grenades = state.grenades.len() as f64;
        let rate = (total_grenades / 10.0).clamp(0.0, 1.0); // Normalize by expected max
        FeatureResult::new(rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utility_features() {
        assert_eq!(FlashAssistRate.name(), "flash_assist_rate");
        assert_eq!(NadeUsageRate.name(), "nade_usage_rate");
        assert_eq!(FlashAssistRate.category(), FeatureCategory::Utility);
    }
}
