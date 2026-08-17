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
    pub auc_roc: f64,
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

/// Metrics at one score cutoff for ROC and precision/recall charts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdMetrics {
    pub threshold: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub false_positive_rate: f64,
}

/// One held-out fold evaluated with a threshold learned from the remaining demos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossValidationFold {
    pub fold: usize,
    pub threshold: f64,
    pub metrics: ValidationMetrics,
}

/// Validation harness for running evaluations
pub struct ValidationHarness {
    demos: Vec<DemoValidation>,
    /// Score cutoff used to classify a labeled player as a cheater.
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

    /// Returns score cutoffs suitable for ROC and precision/recall charts.
    pub fn threshold_curve(&self) -> Vec<ThresholdMetrics> {
        let mut thresholds = self
            .demos
            .iter()
            .flat_map(|demo| &demo.players)
            .filter(|player| player.label != PlayerLabel::Unknown)
            .map(|player| player.overall_score)
            .collect::<Vec<_>>();
        thresholds.sort_by(f64::total_cmp);
        thresholds.dedup();
        thresholds
            .into_iter()
            .rev()
            .map(|threshold| {
                let metrics = self.metrics_at(threshold);
                ThresholdMetrics {
                    threshold,
                    precision: metrics.precision,
                    recall: metrics.recall,
                    f1_score: metrics.f1_score,
                    false_positive_rate: metrics.false_positive_rate,
                }
            })
            .collect()
    }

    /// Selects the threshold with the highest F1 score; ties prefer fewer false positives.
    pub fn calibrate_threshold(&self) -> Option<ThresholdMetrics> {
        self.threshold_curve().into_iter().max_by(|left, right| {
            left.f1_score.total_cmp(&right.f1_score).then_with(|| {
                right
                    .false_positive_rate
                    .total_cmp(&left.false_positive_rate)
            })
        })
    }

    /// Returns metrics grouped by map at the harness threshold.
    pub fn per_map_metrics(&self) -> BTreeMap<String, ValidationMetrics> {
        let mut by_map: BTreeMap<String, Vec<DemoValidation>> = BTreeMap::new();
        for demo in &self.demos {
            by_map
                .entry(demo.map.clone())
                .or_default()
                .push(demo.clone());
        }
        by_map
            .into_iter()
            .map(|(map, demos)| {
                (
                    map,
                    Self::with_demos(self.threshold, demos).compute_metrics(),
                )
            })
            .collect()
    }

    /// Splits demos by insertion order and evaluates each held-out fold.
    pub fn cross_validate(&self, folds: usize) -> Vec<CrossValidationFold> {
        if folds < 2 || self.demos.len() < folds {
            return Vec::new();
        }

        (0..folds)
            .filter_map(|fold| {
                let training = self
                    .demos
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| index % folds != fold)
                    .map(|(_, demo)| demo.clone())
                    .collect::<Vec<_>>();
                let threshold = Self::with_demos(self.threshold, training)
                    .calibrate_threshold()?
                    .threshold;
                let held_out = self
                    .demos
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| index % folds == fold)
                    .map(|(_, demo)| demo.clone())
                    .collect::<Vec<_>>();

                Some(CrossValidationFold {
                    fold,
                    threshold,
                    metrics: Self::with_demos(threshold, held_out).compute_metrics(),
                })
            })
            .collect()
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
                    PlayerLabel::Cheater => {
                        total_cheaters += 1;
                        if player.overall_score >= self.threshold {
                            tp += 1;
                        } else {
                            fn_ += 1;
                        }
                    }
                    PlayerLabel::Legit => {
                        total_legit += 1;
                        if player.overall_score >= self.threshold {
                            fp += 1;
                        } else {
                            tn += 1;
                        }
                    }
                    PlayerLabel::Unknown => {}
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
        let auc_roc = self.compute_auc_roc();

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
            auc_roc,
            score_distribution: score_dist,
            feature_importance: feature_imp,
        }
    }

    fn metrics_at(&self, threshold: f64) -> ValidationMetrics {
        Self::with_demos(threshold, self.demos.clone()).compute_metrics()
    }

    fn with_demos(threshold: f64, demos: Vec<DemoValidation>) -> Self {
        Self { demos, threshold }
    }

    fn compute_auc_roc(&self) -> f64 {
        let mut scores = self
            .demos
            .iter()
            .flat_map(|demo| &demo.players)
            .filter_map(|player| match player.label {
                PlayerLabel::Cheater => Some((player.overall_score, true)),
                PlayerLabel::Legit => Some((player.overall_score, false)),
                PlayerLabel::Unknown => None,
            })
            .collect::<Vec<_>>();
        let positives = scores.iter().filter(|(_, positive)| *positive).count();
        let negatives = scores.len() - positives;
        if positives == 0 || negatives == 0 {
            return 0.0;
        }

        scores.sort_by(|left, right| left.0.total_cmp(&right.0));
        let mut positive_rank_sum = 0.0;
        let mut start = 0;
        while start < scores.len() {
            let mut end = start + 1;
            while end < scores.len() && scores[end].0 == scores[start].0 {
                end += 1;
            }
            let average_rank = (start + 1 + end) as f64 / 2.0;
            positive_rank_sum += scores[start..end]
                .iter()
                .filter(|(_, positive)| *positive)
                .count() as f64
                * average_rank;
            start = end;
        }

        (positive_rank_sum - (positives * (positives + 1) / 2) as f64)
            / (positives * negatives) as f64
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
             Accuracy:   {:.3}\n\
             AUC-ROC:    {:.3}\n\n\
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
            metrics.auc_roc,
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

        harness.add_demo(demo.clone());
        harness.add_demo(demo);

        let metrics = harness.compute_metrics();
        assert_eq!(metrics.total_players, 4);
        assert_eq!(metrics.true_positives, 2);
        assert_eq!(metrics.true_negatives, 2);
        assert_eq!(metrics.precision, 1.0);
        assert_eq!(metrics.recall, 1.0);
        assert_eq!(metrics.auc_roc, 1.0);
        assert_eq!(harness.threshold_curve().len(), 2);
        assert_eq!(harness.calibrate_threshold().unwrap().threshold, 0.8);
        assert_eq!(harness.per_map_metrics()["de_dust2"].total_demos, 2);
        assert!(
            harness
                .cross_validate(2)
                .iter()
                .all(|fold| fold.metrics.f1_score == 1.0)
        );
    }
}
