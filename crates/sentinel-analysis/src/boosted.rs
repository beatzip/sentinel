use std::collections::BTreeMap;

use sentinel_core::FeatureVector;

use crate::ml::{AnomalyModel, ModelError};

/// One labeled vector for supervised feature-combination training.
#[derive(Debug, Clone)]
pub struct LabeledVector {
    pub vector: FeatureVector,
    /// `0.0` for legitimate and `1.0` for confirmed anomalous behavior.
    pub label: f64,
}

#[derive(Debug, Clone)]
struct DecisionStump {
    feature: String,
    threshold: f64,
    left_weight: f64,
    right_weight: f64,
}

/// Small, deterministic gradient-boosted decision-stump model.
///
/// It is intentionally dependency-free. It supplies the feature-combination
/// contract needed by labeled M4 data while preserving a future upgrade path to
/// a full XGBoost-compatible trainer.
pub struct GradientBoostedStumps {
    rounds: usize,
    learning_rate: f64,
    base_logit: f64,
    stumps: Vec<DecisionStump>,
    trained: bool,
}

impl GradientBoostedStumps {
    pub fn new(rounds: usize, learning_rate: f64) -> Self {
        Self {
            rounds,
            learning_rate,
            base_logit: 0.0,
            stumps: Vec::new(),
            trained: false,
        }
    }

    pub fn train_labeled(&mut self, samples: &[LabeledVector]) -> Result<(), ModelError> {
        if samples.len() < 2 {
            return Err(ModelError::TrainingFailed(
                "At least two labeled vectors are required".to_string(),
            ));
        }
        if samples
            .iter()
            .any(|sample| !(0.0..=1.0).contains(&sample.label))
        {
            return Err(ModelError::InvalidData(
                "Labels must be between 0.0 and 1.0".to_string(),
            ));
        }

        let labels = samples
            .iter()
            .map(|sample| sample.label)
            .collect::<Vec<_>>();
        let mean_label = labels.iter().sum::<f64>() / labels.len() as f64;
        self.base_logit = logit(mean_label);
        self.stumps.clear();

        let features = samples
            .iter()
            .flat_map(|sample| sample.vector.features.keys().cloned())
            .collect::<std::collections::BTreeSet<_>>();
        if features.is_empty() {
            return Err(ModelError::InvalidData("No numeric features".to_string()));
        }

        let mut predictions = vec![self.base_logit; samples.len()];
        for _ in 0..self.rounds {
            let residuals = labels
                .iter()
                .zip(&predictions)
                .map(|(label, prediction)| label - sigmoid(*prediction))
                .collect::<Vec<_>>();
            let Some(stump) = best_stump(samples, &features, &residuals) else {
                break;
            };
            for (prediction, sample) in predictions.iter_mut().zip(samples) {
                *prediction += self.learning_rate * stump_value(&stump, &sample.vector);
            }
            self.stumps.push(stump);
        }
        self.trained = !self.stumps.is_empty();
        if self.trained {
            Ok(())
        } else {
            Err(ModelError::TrainingFailed(
                "Training could not split labeled data".to_string(),
            ))
        }
    }

    pub fn feature_importance(&self) -> BTreeMap<String, f64> {
        let mut importance = BTreeMap::new();
        for stump in &self.stumps {
            *importance.entry(stump.feature.clone()).or_insert(0.0) +=
                (stump.right_weight - stump.left_weight).abs();
        }
        let total = importance.values().sum::<f64>();
        if total > 0.0 {
            for value in importance.values_mut() {
                *value /= total;
            }
        }
        importance
    }

    fn score(&self, vector: &FeatureVector) -> f64 {
        sigmoid(
            self.base_logit
                + self.learning_rate
                    * self
                        .stumps
                        .iter()
                        .map(|stump| stump_value(stump, vector))
                        .sum::<f64>(),
        )
    }
}

impl Default for GradientBoostedStumps {
    fn default() -> Self {
        Self::new(24, 0.2)
    }
}

impl AnomalyModel for GradientBoostedStumps {
    fn train(&mut self, _vectors: &[FeatureVector]) -> Result<(), ModelError> {
        Err(ModelError::InvalidData(
            "Gradient boosting requires labeled vectors; call train_labeled".to_string(),
        ))
    }

    fn predict(&self, vector: &FeatureVector) -> Result<f64, ModelError> {
        self.trained
            .then(|| self.score(vector))
            .ok_or(ModelError::ModelNotTrained)
    }

    fn name(&self) -> &str {
        "gradient_boosted_stumps"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }
}

fn best_stump(
    samples: &[LabeledVector],
    features: &std::collections::BTreeSet<String>,
    residuals: &[f64],
) -> Option<DecisionStump> {
    let mut best: Option<(f64, DecisionStump)> = None;
    for feature in features {
        let mut values = samples
            .iter()
            .map(|sample| {
                sample
                    .vector
                    .features
                    .get(feature)
                    .map_or(0.0, |result| result.value)
            })
            .collect::<Vec<_>>();
        values.sort_by(f64::total_cmp);
        let threshold = values[values.len() / 2];
        let (left, right): (Vec<_>, Vec<_>) =
            samples.iter().zip(residuals).partition(|(sample, _)| {
                sample
                    .vector
                    .features
                    .get(feature)
                    .map_or(0.0, |result| result.value)
                    < threshold
            });
        if left.is_empty() || right.is_empty() {
            continue;
        }
        let left_weight =
            left.iter().map(|(_, residual)| **residual).sum::<f64>() / left.len() as f64;
        let right_weight =
            right.iter().map(|(_, residual)| **residual).sum::<f64>() / right.len() as f64;
        let gain =
            left.len() as f64 * left_weight.powi(2) + right.len() as f64 * right_weight.powi(2);
        let stump = DecisionStump {
            feature: feature.clone(),
            threshold,
            left_weight,
            right_weight,
        };
        if best.as_ref().is_none_or(|(best_gain, _)| gain > *best_gain) {
            best = Some((gain, stump));
        }
    }
    best.map(|(_, stump)| stump)
}

fn stump_value(stump: &DecisionStump, vector: &FeatureVector) -> f64 {
    if vector
        .features
        .get(&stump.feature)
        .map_or(0.0, |result| result.value)
        < stump.threshold
    {
        stump.left_weight
    } else {
        stump.right_weight
    }
}

fn sigmoid(value: f64) -> f64 {
    1.0 / (1.0 + (-value).exp())
}

fn logit(value: f64) -> f64 {
    let value = value.clamp(0.001, 0.999);
    (value / (1.0 - value)).ln()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{FeatureResult, PlayerId, Tick};

    fn sample(value: f64, label: f64) -> LabeledVector {
        LabeledVector {
            vector: FeatureVector {
                tick: Tick(1),
                round: 1,
                player: PlayerId::new(value as u64),
                features: BTreeMap::from([("tracking".to_string(), FeatureResult::new(value))]),
            },
            label,
        }
    }

    #[test]
    fn boosted_stumps_rank_labeled_anomalies_higher() {
        let mut model = GradientBoostedStumps::new(12, 0.3);
        let training = [
            sample(0.1, 0.0),
            sample(0.2, 0.0),
            sample(0.8, 1.0),
            sample(0.9, 1.0),
        ];
        model.train_labeled(&training).unwrap();
        assert!(
            model.predict(&sample(0.9, 1.0).vector).unwrap()
                > model.predict(&sample(0.1, 0.0).vector).unwrap()
        );
        assert_eq!(model.feature_importance()["tracking"], 1.0);
    }
}
