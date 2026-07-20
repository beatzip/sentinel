use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Statistical baseline for a single feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureBaseline {
    /// Feature name
    pub name: String,
    /// Mean value from calibration dataset
    pub mean: f64,
    /// Standard deviation from calibration dataset
    pub stddev: f64,
    /// 95th percentile (for anomaly detection)
    pub p95: f64,
    /// 99th percentile (for high-confidence anomalies)
    pub p99: f64,
    /// Number of samples used to compute this baseline
    pub sample_count: usize,
}

impl FeatureBaseline {
    pub fn new(name: impl Into<String>, mean: f64, stddev: f64) -> Self {
        Self {
            name: name.into(),
            mean,
            stddev,
            p95: mean + 1.645 * stddev,
            p99: mean + 2.326 * stddev,
            sample_count: 0,
        }
    }

    /// Compute z-score for a given value
    pub fn z_score(&self, value: f64) -> f64 {
        if self.stddev == 0.0 {
            return 0.0;
        }
        (value - self.mean) / self.stddev
    }

    /// Compute anomaly score (0.0 - 1.0) from z-score
    /// Higher z-score = more anomalous
    pub fn anomaly_score(&self, value: f64) -> f64 {
        let z = self.z_score(value).abs();
        // Sigmoid-like transformation to map z-score to [0, 1]
        1.0 / (1.0 + (-z + 2.0).exp())
    }

    /// Check if a value is anomalous at a given confidence level
    pub fn is_anomalous(&self, value: f64, confidence: f64) -> bool {
        let threshold = if confidence >= 0.99 {
            self.p99
        } else if confidence >= 0.95 {
            self.p95
        } else {
            self.mean + 1.0 * self.stddev
        };

        (value - self.mean).abs() > (threshold - self.mean).abs()
    }
}

/// Collection of baselines for all features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineSet {
    /// Feature name -> baseline
    pub baselines: BTreeMap<String, FeatureBaseline>,
    /// Version of the calibration data
    pub version: String,
}

impl BaselineSet {
    pub fn new() -> Self {
        Self {
            baselines: BTreeMap::new(),
            version: "1.0.0".to_string(),
        }
    }

    /// Add a baseline for a feature
    pub fn add(&mut self, baseline: FeatureBaseline) {
        self.baselines.insert(baseline.name.clone(), baseline);
    }

    /// Get baseline for a feature
    pub fn get(&self, feature_name: &str) -> Option<&FeatureBaseline> {
        self.baselines.get(feature_name)
    }

    /// Compute anomaly scores for all features
    pub fn score_all(&self, values: &BTreeMap<String, f64>) -> BTreeMap<String, f64> {
        values
            .iter()
            .filter_map(|(name, &value)| {
                self.get(name).map(|baseline| {
                    let score = baseline.anomaly_score(value);
                    (name.clone(), score)
                })
            })
            .collect()
    }

    /// Create a default baseline set with typical CS2 values
    pub fn default_cs2() -> Self {
        let mut set = Self::new();

        // Aim features
        set.add(FeatureBaseline::new("reaction_time", 0.25, 0.08));
        set.add(FeatureBaseline::new("crosshair_placement_error", 15.0, 8.0));
        set.add(FeatureBaseline::new("aim_velocity", 120.0, 40.0));
        set.add(FeatureBaseline::new("flick_distance", 25.0, 15.0));
        set.add(FeatureBaseline::new("tracking_smoothness", 0.85, 0.1));
        set.add(FeatureBaseline::new("target_switch_speed", 0.3, 0.1));

        // Wall/info features
        set.add(FeatureBaseline::new("hidden_tracking_duration", 0.5, 0.3));
        set.add(FeatureBaseline::new("wallbang_accuracy", 0.15, 0.1));
        set.add(FeatureBaseline::new("prefire_rate", 0.1, 0.08));
        set.add(FeatureBaseline::new("info_kills_ratio", 0.3, 0.15));

        // Movement features
        set.add(FeatureBaseline::new("movement_smoothness", 0.8, 0.12));
        set.add(FeatureBaseline::new("counter_strafe_accuracy", 0.7, 0.15));
        set.add(FeatureBaseline::new("path_efficiency", 0.75, 0.1));

        // Decision features
        set.add(FeatureBaseline::new("trade_kill_timing", 3.0, 1.5));
        set.add(FeatureBaseline::new("rotation_speed", 5.0, 2.0));
        set.add(FeatureBaseline::new("solo_playstyle_index", 0.3, 0.25));
        set.add(FeatureBaseline::new("team_proximity_score", 0.6, 0.2));
        set.add(FeatureBaseline::new("trade_kill_participation", 0.5, 0.2));
        set.add(FeatureBaseline::new("utility_support_rate", 0.3, 0.15));

        // Information/visibility features
        set.add(FeatureBaseline::new("information_availability", 0.3, 0.2));
        set.add(FeatureBaseline::new("rotation_justification", 0.5, 0.2));

        set
    }
}

impl Default for BaselineSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_z_score() {
        let baseline = FeatureBaseline::new("test", 100.0, 10.0);
        assert!((baseline.z_score(100.0) - 0.0).abs() < 0.001);
        assert!((baseline.z_score(110.0) - 1.0).abs() < 0.001);
        assert!((baseline.z_score(90.0) - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_anomaly_score() {
        let baseline = FeatureBaseline::new("test", 100.0, 10.0);
        // Mean should have low anomaly score
        let score_mean = baseline.anomaly_score(100.0);
        assert!(score_mean < 0.5);

        // 3 standard deviations away should have moderate-high anomaly score
        let score_anomaly = baseline.anomaly_score(130.0);
        assert!(score_anomaly > 0.7);

        // 4 standard deviations away should have high anomaly score
        let score_extreme = baseline.anomaly_score(140.0);
        assert!(score_extreme > 0.8);
    }

    #[test]
    fn test_baseline_set() {
        let mut set = BaselineSet::new();
        set.add(FeatureBaseline::new("feature_a", 50.0, 5.0));
        set.add(FeatureBaseline::new("feature_b", 100.0, 10.0));

        assert!(set.get("feature_a").is_some());
        assert!(set.get("feature_c").is_none());

        let mut values = BTreeMap::new();
        values.insert("feature_a".to_string(), 50.0);
        values.insert("feature_b".to_string(), 130.0);

        let scores = set.score_all(&values);
        assert_eq!(scores.len(), 2);
        assert!(scores["feature_a"] < scores["feature_b"]);
    }
}
