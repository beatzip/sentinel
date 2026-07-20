use std::collections::BTreeMap;

/// Bayesian aggregation of multiple evidence scores
pub struct BayesianAggregator;

impl BayesianAggregator {
    /// Combine multiple independent evidence scores using Bayesian updating.
    /// Each score is treated as a probability of the hypothesis being true.
    /// Returns a combined probability.
    pub fn combine_scores(scores: &[f64]) -> f64 {
        if scores.is_empty() {
            return 0.0;
        }

        // Start with prior probability of 0.5 (unknown)
        let prior = 0.5;

        // Convert each score to likelihood ratio
        // score = P(evidence | hypothesis) / P(evidence | ¬hypothesis)
        let mut posterior = prior;

        for &score in scores {
            // Clamp score to avoid extreme values
            let p = score.clamp(0.01, 0.99);

            // Likelihood ratio: how much more likely is the evidence under hypothesis
            let likelihood_ratio = p / (1.0 - p);

            // Update posterior using Bayes' theorem
            let prior_odds = posterior / (1.0 - posterior);
            let posterior_odds = prior_odds * likelihood_ratio;
            posterior = posterior_odds / (1.0 + posterior_odds);
        }

        posterior
    }

    /// Combine scores with weights (weighted Bayesian)
    pub fn combine_weighted(scores: &[(f64, f64)]) -> f64 {
        if scores.is_empty() {
            return 0.0;
        }

        let prior = 0.5;
        let mut posterior = prior;

        for &(score, weight) in scores {
            let p = score.clamp(0.01, 0.99);

            // Apply weight to the likelihood ratio
            let lr = p / (1.0 - p);
            let weighted_lr = lr.powf(weight);

            let prior_odds = posterior / (1.0 - posterior);
            let posterior_odds = prior_odds * weighted_lr;
            posterior = posterior_odds / (1.0 + posterior_odds);
        }

        posterior
    }

    /// Combine category scores with predefined weights
    pub fn combine_categories(category_scores: &BTreeMap<String, f64>) -> f64 {
        let weights = BTreeMap::from([
            ("aim".to_string(), 0.25),
            ("wall".to_string(), 0.25),
            ("movement".to_string(), 0.15),
            ("utility".to_string(), 0.10),
            ("decision".to_string(), 0.15),
            ("rotation".to_string(), 0.05),
            ("general".to_string(), 0.05),
        ]);

        let weighted_scores: Vec<(f64, f64)> = category_scores
            .iter()
            .filter_map(|(category, &score)| weights.get(category).map(|&weight| (score, weight)))
            .collect();

        Self::combine_weighted(&weighted_scores)
    }

    /// Compute confidence based on number of evidence items
    pub fn evidence_confidence(evidence_count: usize) -> f64 {
        // Confidence increases with more evidence, approaching 1.0
        // Formula: 1 - e^(-k*n) where k controls saturation
        let k = 0.3;
        1.0 - (-k * evidence_count as f64).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combine_scores_empty() {
        assert_eq!(BayesianAggregator::combine_scores(&[]), 0.0);
    }

    #[test]
    fn test_combine_scores_single() {
        let score = BayesianAggregator::combine_scores(&[0.8]);
        assert!((score - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_combine_scores_multiple() {
        // Two pieces of evidence, each 0.7
        let score = BayesianAggregator::combine_scores(&[0.7, 0.7]);
        // Should be higher than either individual score
        assert!(score > 0.7);
        assert!(score < 0.95);
    }

    #[test]
    fn test_combine_weighted() {
        let scores = vec![(0.8, 1.0), (0.6, 0.5)];
        let result = BayesianAggregator::combine_weighted(&scores);
        assert!(result > 0.5);
        assert!(result < 0.9);
    }

    #[test]
    fn test_combine_categories() {
        let mut scores = BTreeMap::new();
        scores.insert("aim".to_string(), 0.3);
        scores.insert("wall".to_string(), 0.9);
        scores.insert("movement".to_string(), 0.2);

        let result = BayesianAggregator::combine_categories(&scores);
        assert!(result > 0.3);
        assert!(result < 0.8);
    }

    #[test]
    fn test_evidence_confidence() {
        assert!(BayesianAggregator::evidence_confidence(0) < 0.1);
        assert!(BayesianAggregator::evidence_confidence(5) > 0.5);
        assert!(BayesianAggregator::evidence_confidence(20) > 0.9);
    }
}
