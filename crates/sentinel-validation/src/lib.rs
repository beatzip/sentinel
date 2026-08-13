pub mod calibration;
pub mod curves;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Label for a player in validation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlayerLabel {
    Legit,
    Cheater,
    Unknown,
}

/// Single player evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerEvaluation {
    pub steam_id: u64,
    pub name: String,
    pub team: String,
    pub label: PlayerLabel,
    pub overall_score: f64,
    pub category_scores: BTreeMap<String, f64>,
    pub evidence_count: usize,
    pub is_true_positive: bool,
    pub is_false_positive: bool,
    pub is_true_negative: bool,
    pub is_false_negative: bool,
}

/// Validation result for a single demo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoValidation {
    pub demo_path: String,
    pub map: String,
    pub players: Vec<PlayerEvaluation>,
    pub true_positives: usize,
    pub false_positives: usize,
    pub true_negatives: usize,
    pub false_negatives: usize,
}

/// Overall validation metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationMetrics {
    pub total_demos: usize,
    pub total_players: usize,
    pub total_cheaters: usize,
    pub total_legit: usize,
    pub true_positives: usize,
    pub false_positives: usize,
    pub true_negatives: usize,
    pub false_negatives: usize,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub false_positive_rate: f64,
    pub true_positive_rate: f64,
    pub accuracy: f64,
    pub score_distribution: ScoreDistribution,
    pub feature_importance: BTreeMap<String, f64>,
}

/// Score distribution for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreDistribution {
    pub bins: Vec<(f64, usize)>,
    pub mean: f64,
    pub median: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
}

/// Validation harness for running evaluations
pub struct ValidationHarness {
    demos: Vec<DemoValidation>,
    /// Score cutoff used to classify a player as a cheater. Currently the
    /// harness reports metrics over the full score distribution; this is kept
    /// so the threshold-gated classification path can be wired in without
    /// breaking the public constructor.
    #[expect(dead_code, reason = "preserved for threshold-gated classification")]
    threshold: f64,
}

impl ValidationHarness {
    pub fn new(threshold: f64) -> Self {
        Self {
            demos: Vec::new(),
            threshold,
        }
    }

    pub fn add_demo(&mut self, demo: DemoValidation) {
        self.demos.push(demo);
    }

    /// Borrow all demos recorded in the harness.
    pub fn demos(&self) -> &[DemoValidation] {
        &self.demos
    }

    pub fn compute_metrics(&self) -> ValidationMetrics {
        let mut tp = 0;
        let mut fp = 0;
        let mut tn = 0;
        let mut fn_ = 0;
        let mut total_players = 0;
        let mut total_cheaters = 0;
        let mut total_legit = 0;
        let mut all_scores = Vec::new();

        for demo in &self.demos {
            for player in &demo.players {
                total_players += 1;
                match player.label {
                    PlayerLabel::Cheater => total_cheaters += 1,
                    PlayerLabel::Legit => total_legit += 1,
                    _ => {}
                }
                if player.is_true_positive {
                    tp += 1;
                }
                if player.is_false_positive {
                    fp += 1;
                }
                if player.is_true_negative {
                    tn += 1;
                }
                if player.is_false_negative {
                    fn_ += 1;
                }
                all_scores.push(player.overall_score);
            }
        }

        let precision = if tp + fp > 0 {
            tp as f64 / (tp + fp) as f64
        } else {
            0.0
        };
        let recall = if tp + fn_ > 0 {
            tp as f64 / (tp + fn_) as f64
        } else {
            0.0
        };
        let f1 = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };
        let fpr = if fp + tn > 0 {
            fp as f64 / (fp + tn) as f64
        } else {
            0.0
        };
        let tpr = recall;
        let accuracy = if total_players > 0 {
            (tp + tn) as f64 / total_players as f64
        } else {
            0.0
        };

        let score_dist = Self::compute_distribution(&all_scores);
        let feature_imp = self.compute_feature_importance();

        ValidationMetrics {
            total_demos: self.demos.len(),
            total_players,
            total_cheaters,
            total_legit,
            true_positives: tp,
            false_positives: fp,
            true_negatives: tn,
            false_negatives: fn_,
            precision,
            recall,
            f1_score: f1,
            false_positive_rate: fpr,
            true_positive_rate: tpr,
            accuracy,
            score_distribution: score_dist,
            feature_importance: feature_imp,
        }
    }

    fn compute_distribution(scores: &[f64]) -> ScoreDistribution {
        if scores.is_empty() {
            return ScoreDistribution {
                bins: Vec::new(),
                mean: 0.0,
                median: 0.0,
                std_dev: 0.0,
                min: 0.0,
                max: 0.0,
            };
        }

        let mut sorted = scores.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
        let variance = sorted.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / sorted.len() as f64;
        let std_dev = variance.sqrt();

        let bin_count = 20;
        let min = sorted[0];
        let max = sorted[sorted.len() - 1];
        let bin_width = if max > min {
            (max - min) / bin_count as f64
        } else {
            0.01
        };

        let mut bins = Vec::new();
        for i in 0..bin_count {
            let threshold = min + bin_width * (i + 1) as f64;
            let count = sorted.iter().filter(|&&s| s <= threshold).count();
            bins.push((threshold, count));
        }

        ScoreDistribution {
            bins,
            mean,
            median: sorted[sorted.len() / 2],
            std_dev,
            min,
            max,
        }
    }

    fn compute_feature_importance(&self) -> BTreeMap<String, f64> {
        let mut importance = BTreeMap::new();

        for demo in &self.demos {
            for player in &demo.players {
                for cat in player.category_scores.keys() {
                    *importance.entry(cat.clone()).or_insert(0.0) += 1.0;
                }
            }
        }

        let total: f64 = importance.values().sum();
        if total > 0.0 {
            for val in importance.values_mut() {
                *val /= total;
            }
        }

        importance
    }

    pub fn summary(&self) -> String {
        let metrics = self.compute_metrics();

        format!(
            "=== Validation Report ===\n\n\
             Demos: {}\n\
             Players: {} (Cheaters: {}, Legit: {})\n\n\
             === Confusion Matrix ===\n\
             True Positives:  {}\n\
             False Positives: {}\n\
             True Negatives:  {}\n\
             False Negatives: {}\n\n\
             === Metrics ===\n\
             Precision:  {:.3}\n\
             Recall:     {:.3}\n\
             F1 Score:   {:.3}\n\
             FPR:        {:.3}\n\
             TPR:        {:.3}\n\
             Accuracy:   {:.3}\n\n\
             === Score Distribution ===\n\
             Mean:   {:.3}\n\
             Median: {:.3}\n\
             StdDev: {:.3}\n\
             Min:    {:.3}\n\
             Max:    {:.3}",
            metrics.total_demos,
            metrics.total_players,
            metrics.total_cheaters,
            metrics.total_legit,
            metrics.true_positives,
            metrics.false_positives,
            metrics.true_negatives,
            metrics.false_negatives,
            metrics.precision,
            metrics.recall,
            metrics.f1_score,
            metrics.false_positive_rate,
            metrics.true_positive_rate,
            metrics.accuracy,
            metrics.score_distribution.mean,
            metrics.score_distribution.median,
            metrics.score_distribution.std_dev,
            metrics.score_distribution.min,
            metrics.score_distribution.max
        )
    }
}

impl Default for ValidationHarness {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_harness() {
        let mut harness = ValidationHarness::new(0.5);

        let demo = DemoValidation {
            demo_path: "test.dem".to_string(),
            map: "de_dust2".to_string(),
            players: vec![
                PlayerEvaluation {
                    steam_id: 1,
                    name: "LegitPlayer".to_string(),
                    team: "Terrorist".to_string(),
                    label: PlayerLabel::Legit,
                    overall_score: 0.2,
                    category_scores: BTreeMap::new(),
                    evidence_count: 0,
                    is_true_positive: false,
                    is_false_positive: false,
                    is_true_negative: true,
                    is_false_negative: false,
                },
                PlayerEvaluation {
                    steam_id: 2,
                    name: "CheaterPlayer".to_string(),
                    team: "CounterTerrorist".to_string(),
                    label: PlayerLabel::Cheater,
                    overall_score: 0.8,
                    category_scores: BTreeMap::new(),
                    evidence_count: 10,
                    is_true_positive: true,
                    is_false_positive: false,
                    is_true_negative: false,
                    is_false_negative: false,
                },
            ],
            true_positives: 1,
            false_positives: 0,
            true_negatives: 1,
            false_negatives: 0,
        };

        harness.add_demo(demo);

        let metrics = harness.compute_metrics();
        assert_eq!(metrics.total_players, 2);
        assert_eq!(metrics.true_positives, 1);
        assert_eq!(metrics.true_negatives, 1);
        assert_eq!(metrics.precision, 1.0);
        assert_eq!(metrics.recall, 1.0);
    }
}
