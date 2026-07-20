use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::player::PlayerId;
use super::tick::Tick;

/// Evidence of anomalous behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// The tick where this evidence was observed
    pub tick: Tick,
    /// The round number
    pub round: u32,
    /// The player this evidence is about
    pub player: PlayerId,
    /// The feature that triggered this evidence
    pub feature: String,
    /// The score for this evidence (0.0 - 1.0)
    pub score: f64,
    /// Confidence in this evidence (0.0 - 1.0)
    pub confidence: f64,
    /// Human-readable explanation
    pub reason: String,
    /// Additional context
    pub metadata: BTreeMap<String, String>,
}

impl Evidence {
    pub fn new(
        tick: Tick,
        round: u32,
        player: PlayerId,
        feature: impl Into<String>,
        score: f64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            tick,
            round,
            player,
            feature: feature.into(),
            score,
            confidence: 1.0,
            reason: reason.into(),
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

    /// Check if this evidence is above a given threshold
    pub fn is_significant(&self, threshold: f64) -> bool {
        self.score >= threshold && self.confidence >= 0.5
    }
}
