use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Tick};

/// Trait for computing a single feature
pub trait FeatureExt: Send + Sync {
    /// The name of this feature
    fn name(&self) -> &str;

    /// The category this feature belongs to
    fn category(&self) -> FeatureCategory;

    /// Compute the feature value for a given player at a given tick
    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult;
}
