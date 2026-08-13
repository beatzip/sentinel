//! Precision-Recall and ROC curve computation, plus AUC-ROC.
//!
//! These operate on a set of (score, is_positive) pairs and produce the points
//! needed to draw PR/ROC curves and compute summary metrics.

use serde::{Deserialize, Serialize};

/// A single point on a curve (threshold, precision/recall or FPR/TPR).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurvePoint {
    pub threshold: f64,
    pub x: f64,
    pub y: f64,
}

/// A full PR / ROC curve with its AUC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Curve {
    pub kind: String,
    pub points: Vec<CurvePoint>,
    pub auc: f64,
}

/// A labelled score used to build curves.
#[derive(Debug, Clone, Copy)]
pub struct LabelledScore {
    pub score: f64,
    pub positive: bool,
}

impl LabelledScore {
    pub fn new(score: f64, positive: bool) -> Self {
        Self { score, positive }
    }
}

/// Build a Precision-Recall curve and compute average precision (AUC).
///
/// Sweeps the score threshold from high to low. At each distinct threshold the
/// curve records precision and recall.
pub fn pr_curve(samples: &[LabelledScore]) -> Curve {
    if samples.is_empty() {
        return Curve {
            kind: "pr".to_string(),
            points: Vec::new(),
            auc: 0.0,
        };
    }

    let total_positive = samples.iter().filter(|s| s.positive).count() as f64;
    if total_positive == 0.0 {
        return Curve {
            kind: "pr".to_string(),
            points: Vec::new(),
            auc: 0.0,
        };
    }

    let mut sorted: Vec<LabelledScore> = samples.to_vec();
    // Sort descending by score so sweeping the threshold high->low grows the
    // set of predicted positives.
    sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut points = Vec::new();
    let mut tp = 0usize;
    let mut fp = 0usize;

    for s in &sorted {
        if s.positive {
            tp += 1;
        } else {
            fp += 1;
        }
        let precision = tp as f64 / (tp + fp) as f64;
        let recall = tp as f64 / total_positive;
        points.push(CurvePoint {
            threshold: s.score,
            x: recall,
            y: precision,
        });
    }

    let auc = average_precision(&points, recall_weights(&sorted, total_positive));

    Curve {
        kind: "pr".to_string(),
        points,
        auc,
    }
}

/// Build a ROC curve (FPR vs TPR) and compute AUC via the trapezoidal rule.
pub fn roc_curve(samples: &[LabelledScore]) -> Curve {
    if samples.is_empty() {
        return Curve {
            kind: "roc".to_string(),
            points: Vec::new(),
            auc: 0.0,
        };
    }

    let total_positive = samples.iter().filter(|s| s.positive).count() as f64;
    let total_negative = samples.len() as f64 - total_positive;
    if total_positive == 0.0 || total_negative == 0.0 {
        return Curve {
            kind: "roc".to_string(),
            points: Vec::new(),
            auc: 0.0,
        };
    }

    let mut sorted: Vec<LabelledScore> = samples.to_vec();
    sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut points = vec![CurvePoint {
        threshold: 1.0,
        x: 0.0,
        y: 0.0,
    }];

    let mut tp = 0usize;
    let mut fp = 0usize;

    for s in &sorted {
        if s.positive {
            tp += 1;
        } else {
            fp += 1;
        }
        let tpr = tp as f64 / total_positive;
        let fpr = fp as f64 / total_negative;
        points.push(CurvePoint {
            threshold: s.score,
            x: fpr,
            y: tpr,
        });
    }

    // AUC via trapezoidal rule over the sorted ROC points.
    let auc = trapezoid_area(&points);

    Curve {
        kind: "roc".to_string(),
        points,
        auc,
    }
}

/// Average precision using the step-wise interpolation: AP = sum over k of
/// (R_k - R_{k-1}) * P_k, where R is recall and P is precision.
fn average_precision(points: &[CurvePoint], recall_deltas: Vec<f64>) -> f64 {
    let mut ap = 0.0;
    let mut prev_recall = 0.0;
    for (i, p) in points.iter().enumerate() {
        let delta = p.x - prev_recall;
        let _ = &recall_deltas; // retained for clarity of the AP formula
        ap += delta * p.y;
        prev_recall = p.x;
        // Guard against index drift in extreme degenerate cases.
        if i + 1 == points.len() {
            break;
        }
    }
    ap
}

/// Recall deltas per sample (kept for AP alternative formulations).
fn recall_weights(_samples: &[LabelledScore], _total_positive: f64) -> Vec<f64> {
    Vec::new()
}

/// Trapezoidal area under a set of (x, y) points sorted by x ascending.
fn trapezoid_area(points: &[CurvePoint]) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let mut area = 0.0;
    for w in points.windows(2) {
        let dx = w[1].x - w[0].x;
        area += dx * (w[0].y + w[1].y) / 2.0;
    }
    area
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_classifier_roc_auc_is_one() {
        let samples = vec![
            LabelledScore::new(0.9, true),
            LabelledScore::new(0.8, true),
            LabelledScore::new(0.4, false),
            LabelledScore::new(0.2, false),
        ];
        let roc = roc_curve(&samples);
        assert!((roc.auc - 1.0).abs() < 1e-9, "auc was {}", roc.auc);
    }

    #[test]
    fn random_classifier_roc_auc_near_half() {
        // Scores uncorrelated with labels -> AUC near 0.5.
        let samples = vec![
            LabelledScore::new(0.9, true),
            LabelledScore::new(0.8, false),
            LabelledScore::new(0.2, true),
            LabelledScore::new(0.3, false),
        ];
        let roc = roc_curve(&samples);
        assert!((roc.auc - 0.5).abs() < 1e-9, "auc was {}", roc.auc);
    }

    #[test]
    fn pr_curve_perfect_classifier_ap_one() {
        let samples = vec![
            LabelledScore::new(0.9, true),
            LabelledScore::new(0.8, true),
            LabelledScore::new(0.4, false),
            LabelledScore::new(0.2, false),
        ];
        let pr = pr_curve(&samples);
        assert!((pr.auc - 1.0).abs() < 1e-9, "ap was {}", pr.auc);
    }

    #[test]
    fn empty_inputs_yield_empty_curves() {
        assert!(roc_curve(&[]).points.is_empty());
        assert!(pr_curve(&[]).points.is_empty());
    }
}
