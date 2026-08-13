//! Threshold calibration and k-fold cross-validation.
//!
//! `calibrate_threshold` sweeps the decision threshold to find the value that
//! maximizes F1 (or a custom objective) over labelled scores. `cross_validate`
//! runs k-fold validation over a set of labelled demos and aggregates metrics.

use serde::{Deserialize, Serialize};

use crate::curves::{Curve, LabelledScore, pr_curve, roc_curve};
use crate::{DemoValidation, PlayerLabel, ValidationHarness};

/// Result of a threshold calibration sweep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationResult {
    /// Threshold that maximizes the objective.
    pub best_threshold: f64,
    /// Objective value (F1 by default) at the best threshold.
    pub best_objective: f64,
    /// Per-threshold metrics across the sweep.
    pub sweep: Vec<ThresholdPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdPoint {
    pub threshold: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

/// Objective function used during calibration.
pub trait CalibrationObjective: Send + Sync {
    fn score(&self, precision: f64, recall: f64) -> f64;
}

/// Maximize F1 score.
pub struct F1Objective;

impl CalibrationObjective for F1Objective {
    fn score(&self, precision: f64, recall: f64) -> f64 {
        if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        }
    }
}

/// Find the threshold (over a fine sweep) that maximizes the given objective.
///
/// `samples` are the (score, label) pairs. The sweep goes from 0.0 to 1.0 in
/// `steps` increments; at each threshold a player is predicted positive if
/// `score >= threshold`.
pub fn calibrate_threshold(samples: &[LabelledScore], steps: usize) -> CalibrationResult {
    calibrate_with_objective(samples, steps, &F1Objective)
}

/// Calibration with a custom objective.
pub fn calibrate_with_objective(
    samples: &[LabelledScore],
    steps: usize,
    objective: &dyn CalibrationObjective,
) -> CalibrationResult {
    let steps = steps.max(1);
    let mut sweep = Vec::with_capacity(steps + 1);
    let mut best = ThresholdPoint {
        threshold: 0.0,
        precision: 0.0,
        recall: 0.0,
        f1: 0.0,
    };

    for i in 0..=steps {
        let threshold = i as f64 / steps as f64;
        let (precision, recall) = precision_recall_at(samples, threshold);
        let f1 = objective.score(precision, recall);
        sweep.push(ThresholdPoint {
            threshold,
            precision,
            recall,
            f1,
        });
        if f1 >= best.f1 {
            best = ThresholdPoint {
                threshold,
                precision,
                recall,
                f1,
            };
        }
    }

    CalibrationResult {
        best_threshold: best.threshold,
        best_objective: best.f1,
        sweep,
    }
}

/// Compute precision/recall at a fixed threshold.
fn precision_recall_at(samples: &[LabelledScore], threshold: f64) -> (f64, f64) {
    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut positives = 0usize;
    for s in samples {
        if s.positive {
            positives += 1;
        }
        if s.score >= threshold {
            if s.positive {
                tp += 1;
            } else {
                fp += 1;
            }
        }
    }
    let precision = if tp + fp > 0 {
        tp as f64 / (tp + fp) as f64
    } else {
        0.0
    };
    let recall = if positives > 0 {
        tp as f64 / positives as f64
    } else {
        0.0
    };
    (precision, recall)
}

/// Result of a k-fold cross-validation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossValidationResult {
    pub k: usize,
    /// Per-fold metrics.
    pub folds: Vec<FoldMetrics>,
    /// Mean AUC-ROC across folds.
    pub mean_auc_roc: f64,
    /// Mean average-precision (PR AUC) across folds.
    pub mean_auc_pr: f64,
    /// Mean F1 at the calibrated threshold.
    pub mean_f1: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldMetrics {
    pub fold: usize,
    pub auc_roc: f64,
    pub auc_pr: f64,
    pub f1: f64,
    pub calibrated_threshold: f64,
}

/// Run k-fold cross-validation over a set of demos.
///
/// Each demo is treated as one fold unit (so `k` should be <= demos.len()).
/// For each fold: the other k-1 folds calibrate a threshold, which is then used
/// to evaluate the held-out fold, producing per-fold AUC-ROC, AUC-PR and F1.
pub fn cross_validate(demos: &[DemoValidation], k: usize) -> CrossValidationResult {
    let k = k.min(demos.len()).max(1);
    if demos.is_empty() {
        return CrossValidationResult {
            k,
            folds: Vec::new(),
            mean_auc_roc: 0.0,
            mean_auc_pr: 0.0,
            mean_f1: 0.0,
        };
    }

    let chunk = (demos.len() + k - 1) / k;
    let mut folds = Vec::with_capacity(k);

    for fold in 0..k {
        let start = fold * chunk;
        let end = (start + chunk).min(demos.len());
        if start >= demos.len() {
            break;
        }
        let test = &demos[start..end];
        let train: Vec<&DemoValidation> = demos
            .iter()
            .enumerate()
            .filter(|(i, _)| !(*i >= start && *i < end))
            .map(|(_, d)| d)
            .collect();

        let train_samples = collect_labelled_scores_iter(train.into_iter());
        let test_samples = collect_labelled_scores_iter(test.iter());

        if train_samples.is_empty() || test_samples.is_empty() {
            continue;
        }

        let cal = calibrate_threshold(&train_samples, 100);
        let roc = roc_curve(&test_samples);
        let pr = pr_curve(&test_samples);
        let (precision, recall) = precision_recall_at(&test_samples, cal.best_threshold);
        let f1 = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };

        folds.push(FoldMetrics {
            fold,
            auc_roc: roc.auc,
            auc_pr: pr.auc,
            f1,
            calibrated_threshold: cal.best_threshold,
        });
    }

    let mean = |f: &dyn Fn(&FoldMetrics) -> f64| -> f64 {
        if folds.is_empty() {
            0.0
        } else {
            folds.iter().map(f).sum::<f64>() / folds.len() as f64
        }
    };

    CrossValidationResult {
        k,
        mean_auc_roc: mean(&|f| f.auc_roc),
        mean_auc_pr: mean(&|f| f.auc_pr),
        mean_f1: mean(&|f| f.f1),
        folds,
    }
}

/// Collect labelled (score, positive) pairs from a set of demos.
/// Players labelled Cheater are positive; Legit are negative; Unknown skipped.
fn collect_labelled_scores_iter<'a, I>(demos: I) -> Vec<LabelledScore>
where
    I: IntoIterator<Item = &'a DemoValidation>,
{
    let mut out = Vec::new();
    for demo in demos {
        for player in &demo.players {
            match player.label {
                PlayerLabel::Cheater => out.push(LabelledScore::new(player.overall_score, true)),
                PlayerLabel::Legit => out.push(LabelledScore::new(player.overall_score, false)),
                PlayerLabel::Unknown => {}
            }
        }
    }
    out
}

/// Run the full validation suite on a harness: ROC, PR, per-feature importance,
/// threshold calibration, and a single-threshold confusion matrix.
pub fn evaluate(harness: &ValidationHarness) -> EvaluationReport {
    let samples: Vec<LabelledScore> = harness
        .demos()
        .iter()
        .flat_map(|d| d.players.iter())
        .filter_map(|p| match p.label {
            PlayerLabel::Cheater => Some(LabelledScore::new(p.overall_score, true)),
            PlayerLabel::Legit => Some(LabelledScore::new(p.overall_score, false)),
            PlayerLabel::Unknown => None,
        })
        .collect();

    let roc = roc_curve(&samples);
    let pr = pr_curve(&samples);
    let calibration = calibrate_threshold(&samples, 100);
    let metrics = harness.compute_metrics();

    EvaluationReport {
        roc,
        pr,
        calibration,
        metrics,
    }
}

/// Full evaluation report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationReport {
    pub roc: Curve,
    pub pr: Curve,
    pub calibration: CalibrationResult,
    pub metrics: crate::ValidationMetrics,
}

/// Per-map breakdown of validation metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerMapReport {
    pub map: String,
    pub players: usize,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub auc_roc: f64,
}

/// Compute per-map metrics by grouping demos by their `map` field.
pub fn per_map_analysis(demos: &[DemoValidation]) -> Vec<PerMapReport> {
    let mut groups: std::collections::BTreeMap<String, Vec<&DemoValidation>> =
        std::collections::BTreeMap::new();
    for d in demos {
        groups.entry(d.map.clone()).or_default().push(d);
    }

    groups
        .into_iter()
        .map(|(map, group)| {
            let players: usize = group.iter().map(|d| d.players.len()).sum();
            let samples = collect_labelled_scores_iter(group.into_iter());
            let (precision, recall) = precision_recall_at(&samples, 0.5);
            let f1 = if precision + recall > 0.0 {
                2.0 * precision * recall / (precision + recall)
            } else {
                0.0
            };
            let auc_roc = roc_curve(&samples).auc;
            PerMapReport {
                map,
                players,
                precision,
                recall,
                f1,
                auc_roc,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlayerEvaluation, PlayerLabel};

    fn demo(score: f64, label: PlayerLabel) -> DemoValidation {
        DemoValidation {
            demo_path: "t.dem".to_string(),
            map: "de_dust2".to_string(),
            players: vec![PlayerEvaluation {
                steam_id: 0,
                name: "p".to_string(),
                team: "T".to_string(),
                label,
                overall_score: score,
                category_scores: std::collections::BTreeMap::new(),
                evidence_count: 0,
                is_true_positive: false,
                is_false_positive: false,
                is_true_negative: false,
                is_false_negative: false,
            }],
            true_positives: 0,
            false_positives: 0,
            true_negatives: 0,
            false_negatives: 0,
        }
    }

    #[test]
    fn calibrate_finds_perfect_threshold() {
        let samples = vec![
            LabelledScore::new(0.9, true),
            LabelledScore::new(0.1, false),
        ];
        let cal = calibrate_threshold(&samples, 10);
        assert!(cal.best_objective > 0.99);
    }

    #[test]
    fn cross_validate_runs() {
        let demos = vec![
            demo(0.9, PlayerLabel::Cheater),
            demo(0.1, PlayerLabel::Legit),
            demo(0.8, PlayerLabel::Cheater),
            demo(0.2, PlayerLabel::Legit),
        ];
        let cv = cross_validate(&demos, 2);
        assert_eq!(cv.k, 2);
        // mean metrics should be finite
        assert!(cv.mean_auc_roc.is_finite());
    }

    #[test]
    fn per_map_groups_by_map() {
        let mut d1 = demo(0.9, PlayerLabel::Cheater);
        d1.map = "de_dust2".to_string();
        let mut d2 = demo(0.1, PlayerLabel::Legit);
        d2.map = "de_mirage".to_string();
        let report = per_map_analysis(&[d1, d2]);
        assert_eq!(report.len(), 2);
    }
}
