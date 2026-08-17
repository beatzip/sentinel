//! Contract for corpus-backed comparisons between Sentinel scoring arms.

use serde::{Deserialize, Serialize};

/// A scoring arm that can be evaluated against the same verified corpus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelArm {
    Baseline,
    XGBoost,
    Transformer,
    Ensemble,
}

/// The primary decision metric to be calculated when a corpus is available.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationMetric {
    RocAuc,
    PrecisionAtFixedRecall,
    RecallAtFixedPrecision,
}

/// Reproducible comparison definition; it does not fabricate results without a verified corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbTestConfig {
    pub name: String,
    pub control: ModelArm,
    pub variants: Vec<ModelArm>,
    pub primary_metric: EvaluationMetric,
    pub minimum_verified_matches: usize,
    pub require_shared_corpus: bool,
}

impl AbTestConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.control != ModelArm::Baseline {
            return Err("A/B control must be the baseline scorer");
        }
        if self.variants.is_empty() {
            return Err("A/B test requires at least one model variant");
        }
        if self.variants.iter().any(|arm| *arm == self.control) {
            return Err("A/B variants must not repeat the control arm");
        }
        if self.minimum_verified_matches == 0 {
            return Err("A/B test requires verified matches");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_control_and_model_variant_are_accepted() {
        let config = AbTestConfig {
            name: "verified-corpus".to_string(),
            control: ModelArm::Baseline,
            variants: vec![ModelArm::XGBoost, ModelArm::Transformer],
            primary_metric: EvaluationMetric::RocAuc,
            minimum_verified_matches: 1,
            require_shared_corpus: true,
        };
        assert!(config.validate().is_ok());
    }
}
