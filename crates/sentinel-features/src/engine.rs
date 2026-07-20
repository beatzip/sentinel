use std::collections::BTreeMap;

use sentinel_core::{FeatureVector, MatchContext, Tick};

use crate::traits::FeatureExt;

/// Engine that computes all features for a player
pub struct FeatureEngine {
    features: Vec<Box<dyn FeatureExt>>,
}

impl FeatureEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            features: Vec::new(),
        };

        // Register aim features
        engine.register(crate::aim::ReactionTime);
        engine.register(crate::aim::CrosshairPlacementError);
        engine.register(crate::aim::AimVelocity);
        engine.register(crate::aim::TrackingSmoothness);
        engine.register(crate::aim::TargetSwitchSpeed);

        // Register movement features
        engine.register(crate::movement::MovementSmoothness);
        engine.register(crate::movement::CounterStrafeAccuracy);
        engine.register(crate::movement::PathEfficiency);

        // Register wall/info features (visibility-based)
        engine.register(crate::wall::HiddenTrackingDuration);
        engine.register(crate::wall::InformationAvailability);
        engine.register(crate::wall::PrefireRate);
        engine.register(crate::wall::RotationJustification);

        // Register decision features (including solo playstyle)
        engine.register(crate::decision::TradeKillTiming);
        engine.register(crate::decision::RotationSpeed);
        engine.register(crate::decision::SoloPlaystyleIndex);
        engine.register(crate::decision::TeamProximityScore);
        engine.register(crate::decision::TradeKillParticipation);
        engine.register(crate::decision::UtilitySupportRate);

        // Register utility features
        engine.register(crate::utility::FlashAssistRate);
        engine.register(crate::utility::NadeUsageRate);

        // Register general features
        engine.register(crate::general::KDRatio);
        engine.register(crate::general::HeadshotPercentage);
        engine.register(crate::general::SurvivalTime);

        engine
    }

    pub fn register(&mut self, feature: impl FeatureExt + 'static) {
        self.features.push(Box::new(feature));
    }

    /// Compute all features for a player at a given tick
    pub fn compute_all(
        &self,
        ctx: &MatchContext,
        tick: Tick,
        player: sentinel_core::PlayerId,
    ) -> FeatureVector {
        let mut features = BTreeMap::new();

        for feature in &self.features {
            let result = feature.compute(ctx, tick, player);
            features.insert(feature.name().to_string(), result);
        }

        FeatureVector {
            tick,
            round: ctx.current_round(),
            player,
            features,
        }
    }

    /// Compute features for all ticks in a match
    pub fn compute_match(
        &self,
        ctx: &MatchContext,
        player: sentinel_core::PlayerId,
    ) -> Vec<FeatureVector> {
        let mut vectors = Vec::new();

        for state in ctx.states() {
            let fv = self.compute_all(ctx, state.tick, player);
            vectors.push(fv);
        }

        vectors
    }
}

impl Default for FeatureEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = FeatureEngine::new();
        assert!(!engine.features.is_empty());
    }
}
