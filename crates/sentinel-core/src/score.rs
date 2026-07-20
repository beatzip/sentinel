use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::feature::FeatureCategory;

/// Behavior scores for a player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorScore {
    /// Individual category scores (0.0 - 1.0)
    pub categories: BTreeMap<String, f64>,
    /// Overall anomaly score (0.0 - 1.0)
    pub overall: f64,
    /// Number of evidence items supporting this score
    pub evidence_count: usize,
}

impl BehaviorScore {
    pub fn new() -> Self {
        Self {
            categories: BTreeMap::new(),
            overall: 0.0,
            evidence_count: 0,
        }
    }

    /// Get score for a specific category
    pub fn category_score(&self, category: FeatureCategory) -> f64 {
        self.categories
            .get(&category.to_string())
            .copied()
            .unwrap_or(0.0)
    }

    /// Set score for a specific category
    pub fn set_category_score(&mut self, category: FeatureCategory, score: f64) {
        self.categories.insert(category.to_string(), score);
    }

    /// Compute overall score as weighted average of category scores
    pub fn compute_overall(&mut self) {
        if self.categories.is_empty() {
            self.overall = 0.0;
            return;
        }

        // Weights for each category (sum to 1.0)
        let weights = BTreeMap::from([
            ("aim".to_string(), 0.25),
            ("wall".to_string(), 0.25),
            ("movement".to_string(), 0.15),
            ("utility".to_string(), 0.10),
            ("decision".to_string(), 0.15),
            ("rotation".to_string(), 0.05),
            ("general".to_string(), 0.05),
        ]);

        let mut total_weight = 0.0;
        let mut weighted_sum = 0.0;

        for (category, &score) in &self.categories {
            let weight = weights.get(category).copied().unwrap_or(0.1);
            weighted_sum += score * weight;
            total_weight += weight;
        }

        self.overall = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };
    }

    /// Check if this score indicates suspicious behavior
    pub fn is_suspicious(&self, threshold: f64) -> bool {
        self.overall >= threshold
    }

    /// Get the most anomalous category
    pub fn most_anomalous_category(&self) -> Option<(&str, f64)> {
        self.categories
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(k, v)| (k.as_str(), *v))
    }
}

impl Default for BehaviorScore {
    fn default() -> Self {
        Self::new()
    }
}
