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
        engine.register(crate::advanced::EconomyShare);
        engine.register(crate::advanced::ClutchPressure);

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
            round: ctx
                .state_at(tick)
                .map_or(0, |state| state.round.round_number),
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

        for tick in Self::feature_ticks(ctx) {
            let fv = self.compute_all(ctx, tick, player);
            vectors.push(fv);
        }

        vectors
    }

    fn feature_ticks(ctx: &MatchContext) -> impl Iterator<Item = Tick> + '_ {
        let has_live_phase = ctx.states().iter().any(|state| state.round.is_live());
        ctx.states()
            .iter()
            .filter(move |state| !has_live_phase || state.round.is_live())
            .map(|state| state.tick)
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
    use sentinel_core::{BombState, PlayerId, RoundPhase, RoundState, TickState};

    #[test]
    fn test_engine_creation() {
        let engine = FeatureEngine::new();
        assert!(!engine.features.is_empty());
    }

    #[test]
    fn compute_match_skips_non_live_ticks_when_live_phase_is_known() {
        let state = |tick, phase| TickState {
            tick: Tick(tick),
            players: Vec::new(),
            grenades: Vec::new(),
            bomb: BombState::Carried {
                carrier: PlayerId::new(0),
            },
            round: RoundState {
                round_number: 1,
                phase,
                clock: 0.0,
                t_score: 0,
                ct_score: 0,
                winner: None,
                start_tick: 0,
            },
        };
        let ctx = MatchContext::new(vec![
            state(10, RoundPhase::Warmup),
            state(20, RoundPhase::Freezetime),
            state(30, RoundPhase::Live),
            state(40, RoundPhase::Over),
        ]);

        assert_eq!(
            FeatureEngine::feature_ticks(&ctx).collect::<Vec<_>>(),
            vec![Tick(30)]
        );
    }
}
