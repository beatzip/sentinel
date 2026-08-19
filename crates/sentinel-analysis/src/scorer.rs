use std::collections::BTreeMap;

use sentinel_core::{BehaviorScore, FeatureCategory, FeatureVector, Tick};

use crate::aggregation::BayesianAggregator;
use crate::baseline::BaselineSet;

/// Optional memory reference used to apply per-player recidivism adjustments.
/// Kept as a trait object so the analysis crate does not depend on the
/// `sentinel-memory` crate (avoids a circular dependency).
pub trait MemoryAdapter: Send + Sync {
    /// Recidivism-based score adjustment in roughly [-0.1, +0.2].
    fn recidivism_adjustment(&self, player: sentinel_core::PlayerId) -> f64;
}

/// Configuration for the behavior scorer
#[derive(Debug, Clone)]
pub struct ScorerConfig {
    /// Baseline distributions for features
    pub baselines: BaselineSet,
    /// Threshold for generating evidence (anomaly score must exceed this)
    pub evidence_threshold: f64,
    /// Minimum number of evidence items for a category to be included
    pub min_evidence_per_category: usize,
}

impl ScorerConfig {
    pub fn default_cs2() -> Self {
        Self {
            baselines: BaselineSet::default_cs2(),
            evidence_threshold: 0.6,
            min_evidence_per_category: 1,
        }
    }
}

impl Default for ScorerConfig {
    fn default() -> Self {
        Self::default_cs2()
    }
}

/// Result of scoring a single feature
#[derive(Debug, Clone)]
pub struct FeatureScore {
    /// Feature name
    pub name: String,
    /// Raw feature value
    pub value: f64,
    /// Anomaly score (0.0 - 1.0)
    pub anomaly_score: f64,
    /// Z-score (standard deviations from mean)
    pub z_score: f64,
    /// Whether this feature is considered anomalous
    pub is_anomalous: bool,
}

/// Result of scoring a player
#[derive(Debug, Clone)]
pub struct PlayerScoreResult {
    /// Player ID
    pub player: sentinel_core::PlayerId,
    /// Tick when scoring was performed
    pub tick: Tick,
    /// Per-feature scores
    pub feature_scores: BTreeMap<String, FeatureScore>,
    /// Per-category anomaly scores
    pub category_scores: BTreeMap<String, f64>,
    /// Overall behavior score
    pub overall_score: BehaviorScore,
    /// Evidence entries generated during scoring
    pub evidence: Vec<sentinel_core::Evidence>,
}

/// The behavior scorer analyzes feature vectors and produces behavior scores
pub struct Scorer {
    config: ScorerConfig,
    memory: Option<Box<dyn MemoryAdapter>>,
}

impl Scorer {
    pub fn new(config: ScorerConfig) -> Self {
        Self {
            config,
            memory: None,
        }
    }

    pub fn default_cs2() -> Self {
        Self::new(ScorerConfig::default_cs2())
    }

    /// Attach a memory adapter. The scorer will use the memory's recidivism
    /// signal to raise scores for players with a chronic anomaly history,
    /// improving detection of marginal cheaters.
    pub fn with_memory(mut self, memory: Box<dyn MemoryAdapter>) -> Self {
        self.memory = Some(memory);
        self
    }

    /// Score a single feature vector
    pub fn score_feature_vector(&self, fv: &FeatureVector) -> Vec<FeatureScore> {
        fv.features
            .iter()
            .filter_map(|(name, result)| {
                if result.confidence <= 0.0 {
                    return None;
                }
                self.config.baselines.get(name).map(|baseline| {
                    let z_score = baseline.z_score(result.value);
                    let anomaly_score = baseline.anomaly_score(result.value);
                    let is_anomalous = anomaly_score >= self.config.evidence_threshold;

                    FeatureScore {
                        name: name.clone(),
                        value: result.value,
                        anomaly_score,
                        z_score,
                        is_anomalous,
                    }
                })
            })
            .collect()
    }

    /// Score all feature vectors for a player and produce a behavior score
    pub fn score_player(
        &self,
        player: sentinel_core::PlayerId,
        feature_vectors: &[&FeatureVector],
    ) -> PlayerScoreResult {
        let mut all_feature_scores: BTreeMap<String, Vec<FeatureScore>> = BTreeMap::new();

        // Score each feature vector and collect evidence
        let mut evidence = Vec::new();

        for fv in feature_vectors {
            for fs in self.score_feature_vector(fv) {
                // Generate evidence for anomalous features
                if fs.is_anomalous {
                    let reason = self.generate_evidence_reason(&fs);
                    let ev = sentinel_core::Evidence::new(
                        fv.tick,
                        fv.round,
                        player,
                        fs.name.clone(),
                        fs.anomaly_score,
                        reason,
                    );
                    evidence.push(ev);
                }

                all_feature_scores
                    .entry(fs.name.clone())
                    .or_default()
                    .push(fs);
            }
        }

        // Compute per-feature aggregate scores
        let feature_scores: BTreeMap<String, FeatureScore> = all_feature_scores
            .iter()
            .map(|(name, scores)| {
                let avg_anomaly =
                    scores.iter().map(|s| s.anomaly_score).sum::<f64>() / scores.len() as f64;
                let avg_z = scores.iter().map(|s| s.z_score).sum::<f64>() / scores.len() as f64;
                let avg_value = scores.iter().map(|s| s.value).sum::<f64>() / scores.len() as f64;

                (
                    name.clone(),
                    FeatureScore {
                        name: name.clone(),
                        value: avg_value,
                        anomaly_score: avg_anomaly,
                        z_score: avg_z,
                        is_anomalous: avg_anomaly >= self.config.evidence_threshold,
                    },
                )
            })
            .collect();

        // Compute category scores
        let category_scores = self.compute_category_scores(&feature_scores);

        // Compute overall score
        let overall_score = self.compute_overall_score(player, &category_scores, &feature_scores);

        let tick = feature_vectors.first().map(|fv| fv.tick).unwrap_or(Tick(0));

        PlayerScoreResult {
            player,
            tick,
            feature_scores,
            category_scores,
            overall_score,
            evidence,
        }
    }

    /// Blend learned scores only after feature-level evidence is generated.
    pub fn apply_learned_scores(
        &self,
        result: &mut PlayerScoreResult,
        xgboost_score: f64,
        transformer_score: f64,
    ) {
        let baseline_score = result.overall_score.overall;
        result
            .overall_score
            .categories
            .insert("learned_xgboost".to_string(), xgboost_score);
        result
            .overall_score
            .categories
            .insert("learned_temporal".to_string(), transformer_score);
        result.overall_score.overall =
            (baseline_score * 0.35 + xgboost_score * 0.40 + transformer_score * 0.25)
                .clamp(0.0, 1.0);
    }

    /// Generate human-readable reason for evidence
    fn generate_evidence_reason(&self, score: &FeatureScore) -> String {
        match score.name.as_str() {
            "reaction_time" => format!(
                "Reaction time {:.3}s is {:.1}σ from mean (anomaly: {:.2})",
                score.value, score.z_score, score.anomaly_score
            ),
            "crosshair_placement_error" => format!(
                "Crosshair error {:.1}° is {:.1}σ from mean (anomaly: {:.2})",
                score.value, score.z_score, score.anomaly_score
            ),
            "aim_velocity" => format!(
                "Aim velocity {:.1} deg/s is {:.1}σ from mean (anomaly: {:.2})",
                score.value, score.z_score, score.anomaly_score
            ),
            "tracking_smoothness" => format!(
                "Tracking smoothness {:.3} is {:.1}σ from mean (anomaly: {:.2})",
                score.value, score.z_score, score.anomaly_score
            ),
            "hidden_tracking_duration" => format!(
                "Hidden tracking {:.2}s is {:.1}σ from mean (anomaly: {:.2})",
                score.value, score.z_score, score.anomaly_score
            ),
            "prefire_rate" => format!(
                "Prefire rate {:.3} is {:.1}σ from mean (anomaly: {:.2})",
                score.value, score.z_score, score.anomaly_score
            ),
            _ => format!(
                "{}: value {:.3}, {:.1}σ from mean (anomaly: {:.2})",
                score.name, score.value, score.z_score, score.anomaly_score
            ),
        }
    }

    /// Compute scores for each category
    fn compute_category_scores(
        &self,
        feature_scores: &BTreeMap<String, FeatureScore>,
    ) -> BTreeMap<String, f64> {
        let mut category_scores: BTreeMap<String, Vec<f64>> = BTreeMap::new();

        // Map feature names to categories
        let feature_categories = self.get_feature_categories();

        for (name, score) in feature_scores {
            if !score.is_anomalous {
                continue;
            }
            if let Some(category) = feature_categories.get(name.as_str()) {
                category_scores
                    .entry(category.to_string())
                    .or_default()
                    .push(score.anomaly_score);
            }
        }

        // Average anomaly scores within each category
        category_scores
            .into_iter()
            .map(|(category, scores)| {
                let avg = scores.iter().sum::<f64>() / scores.len() as f64;
                (category, avg)
            })
            .collect()
    }

    /// Compute overall score from category scores
    fn compute_overall_score(
        &self,
        player: sentinel_core::PlayerId,
        category_scores: &BTreeMap<String, f64>,
        feature_scores: &BTreeMap<String, FeatureScore>,
    ) -> BehaviorScore {
        let mut score = BehaviorScore::new();

        // Set category scores
        for (category, &cat_score) in category_scores {
            let feature_category = match category.as_str() {
                "aim" => FeatureCategory::Aim,
                "wall" => FeatureCategory::Wall,
                "movement" => FeatureCategory::Movement,
                "utility" => FeatureCategory::Utility,
                "decision" => FeatureCategory::Decision,
                "rotation" => FeatureCategory::Rotation,
                "general" => FeatureCategory::General,
                _ => continue,
            };
            score.set_category_score(feature_category, cat_score);
        }

        // Compute overall using Bayesian aggregation
        let mut overall = BayesianAggregator::combine_categories(category_scores);

        // Apply memory-based recidivism adjustment: players with a chronic
        // anomaly history get a boost; players with a clean history get a
        // small discount. This is what surfaces marginal cheaters who stay
        // just under single-match thresholds.
        if let Some(memory) = &self.memory {
            overall = (overall + memory.recidivism_adjustment(player)).clamp(0.0, 1.0);
        }

        score.overall = overall;

        // Count evidence items (anomalous features)
        score.evidence_count = feature_scores.values().filter(|s| s.is_anomalous).count();

        score
    }

    /// Map feature names to categories
    fn get_feature_categories(&self) -> BTreeMap<&str, &str> {
        BTreeMap::from([
            ("reaction_time", "aim"),
            ("crosshair_placement_error", "aim"),
            ("aim_velocity", "aim"),
            ("flick_distance", "aim"),
            ("tracking_smoothness", "aim"),
            ("target_switch_speed", "aim"),
            ("aim_snap_distance", "aim"),
            ("hidden_tracking_duration", "wall"),
            ("wallbang_accuracy", "wall"),
            ("prefire_rate", "wall"),
            ("info_kills_ratio", "wall"),
            ("through_smoke_kills", "wall"),
            ("movement_smoothness", "movement"),
            ("counter_strafe_accuracy", "movement"),
            ("path_efficiency", "movement"),
            ("bunny_hop_rate", "movement"),
            ("trade_kill_timing", "decision"),
            ("rotation_speed", "decision"),
            ("positioning_score", "decision"),
            ("flash_assist_rate", "utility"),
            ("smoke_timing", "utility"),
            ("nade_usage_rate", "utility"),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{FeatureResult, PlayerId};

    #[test]
    fn test_score_feature_vector() {
        let scorer = Scorer::default_cs2();

        let mut features = BTreeMap::new();
        features.insert(
            "reaction_time".to_string(),
            FeatureResult::new(0.25), // Average value
        );
        features.insert(
            "crosshair_placement_error".to_string(),
            FeatureResult::new(15.0), // Average value
        );

        let fv = FeatureVector {
            tick: Tick(100),
            round: 1,
            player: PlayerId::new(1),
            features,
        };

        let scores = scorer.score_feature_vector(&fv);
        assert_eq!(scores.len(), 2);

        // Average values should have low anomaly scores
        for score in &scores {
            assert!(score.anomaly_score < 0.5);
        }
    }

    #[test]
    fn test_score_anomalous_features() {
        let scorer = Scorer::default_cs2();

        let mut features = BTreeMap::new();
        // Very fast reaction time (suspicious)
        features.insert("reaction_time".to_string(), FeatureResult::new(0.05));

        let fv = FeatureVector {
            tick: Tick(100),
            round: 1,
            player: PlayerId::new(1),
            features,
        };

        let scores = scorer.score_feature_vector(&fv);

        // Reaction time should be flagged as anomalous (very fast)
        assert_eq!(scores.len(), 1);
        assert!(scores[0].anomaly_score > 0.6);
    }

    #[test]
    fn test_unavailable_feature_does_not_create_score() {
        let scorer = Scorer::default_cs2();
        let mut features = BTreeMap::new();
        features.insert(
            "aim_velocity".to_string(),
            FeatureResult::new(0.0).with_confidence(0.0),
        );
        let fv = FeatureVector {
            tick: Tick(100),
            round: 0,
            player: PlayerId::new(1),
            features,
        };

        assert!(scorer.score_feature_vector(&fv).is_empty());
    }

    #[test]
    fn test_score_player() {
        let scorer = Scorer::default_cs2();

        let mut features = BTreeMap::new();
        features.insert("reaction_time".to_string(), FeatureResult::new(0.25));

        let fv = FeatureVector {
            tick: Tick(100),
            round: 1,
            player: PlayerId::new(1),
            features,
        };

        let result = scorer.score_player(PlayerId::new(1), &[&fv]);
        assert_eq!(result.player, PlayerId::new(1));
        assert!(result.category_scores.is_empty());
        assert_eq!(result.overall_score.overall, 0.0);
    }
}
