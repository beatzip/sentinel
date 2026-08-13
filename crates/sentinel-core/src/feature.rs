use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::tick::Tick;

/// Category of a feature
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum FeatureCategory {
    Aim,
    Wall,
    Movement,
    Utility,
    Decision,
    Rotation,
    General,
}

impl std::fmt::Display for FeatureCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeatureCategory::Aim => write!(f, "aim"),
            FeatureCategory::Wall => write!(f, "wall"),
            FeatureCategory::Movement => write!(f, "movement"),
            FeatureCategory::Utility => write!(f, "utility"),
            FeatureCategory::Decision => write!(f, "decision"),
            FeatureCategory::Rotation => write!(f, "rotation"),
            FeatureCategory::General => write!(f, "general"),
        }
    }
}

/// Result of computing a single feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureResult {
    pub value: f64,
    pub confidence: f64,
    pub metadata: BTreeMap<String, String>,
}

impl FeatureResult {
    pub fn new(value: f64) -> Self {
        Self {
            value,
            confidence: 1.0,
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// A feature vector containing all computed features for a player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    pub tick: Tick,
    pub round: u32,
    pub player: super::player::PlayerId,
    pub features: BTreeMap<String, FeatureResult>,
}

impl FeatureVector {
    pub fn get(&self, name: &str) -> Option<&FeatureResult> {
        self.features.get(name)
    }

    pub fn get_value(&self, name: &str) -> Option<f64> {
        self.features.get(name).map(|r| r.value)
    }

    pub fn len(&self) -> usize {
        self.features.len()
    }

    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }
}

/// Trait for computing a single feature
pub trait Feature: Send + Sync {
    /// The name of this feature
    fn name(&self) -> &str;

    /// The category this feature belongs to
    fn category(&self) -> FeatureCategory;

    /// Compute the feature value for a given player at a given tick
    fn compute(&self, ctx: &super::world::MatchContext, tick: Tick) -> FeatureResult;
}

/// A collection of features that can be computed together
pub struct FeatureRegistry {
    features: Vec<Box<dyn Feature>>,
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self {
            features: Vec::new(),
        }
    }

    pub fn register(&mut self, feature: Box<dyn Feature>) {
        self.features.push(feature);
    }

    pub fn features(&self) -> &[Box<dyn Feature>] {
        &self.features
    }

    pub fn len(&self) -> usize {
        self.features.len()
    }

    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    /// Compute all features for a player at a given tick
    pub fn compute_all(
        &self,
        ctx: &super::world::MatchContext,
        tick: Tick,
        player: super::player::PlayerId,
    ) -> FeatureVector {
        let mut features = BTreeMap::new();

        for feature in &self.features {
            let result = feature.compute(ctx, tick);
            features.insert(feature.name().to_string(), result);
        }

        FeatureVector {
            tick,
            round: ctx.current_round(),
            player,
            features,
        }
    }
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        Self::new()
    }
}
