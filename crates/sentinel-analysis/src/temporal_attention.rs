use std::collections::BTreeMap;

use sentinel_core::FeatureVector;

use crate::ml::ModelError;

/// Output of a temporal attention pass for one player's ordered feature sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct TemporalSequenceScore {
    pub anomaly_score: f64,
    pub transition_score: f64,
    pub attention: Vec<f64>,
}

/// Dependency-free temporal attention baseline.
///
/// The encoder normalizes features against learned baselines, compares the
/// newest token to a recency-weighted context window, and exposes attention
/// weights for evidence review. It deliberately keeps the same sequence input
/// contract needed by a future learned Transformer implementation.
pub struct TemporalAttentionModel {
    context_window: usize,
    means: BTreeMap<String, f64>,
    stddevs: BTreeMap<String, f64>,
    trained: bool,
}

impl TemporalAttentionModel {
    pub fn new(context_window: usize) -> Self {
        Self {
            context_window: context_window.max(2),
            means: BTreeMap::new(),
            stddevs: BTreeMap::new(),
            trained: false,
        }
    }

    pub fn train(&mut self, vectors: &[FeatureVector]) -> Result<(), ModelError> {
        if vectors.is_empty() {
            return Err(ModelError::TrainingFailed(
                "No sequence vectors supplied".to_string(),
            ));
        }
        let mut values: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for vector in vectors {
            for (name, result) in &vector.features {
                values.entry(name.clone()).or_default().push(result.value);
            }
        }
        if values.is_empty() {
            return Err(ModelError::InvalidData("No numeric features".to_string()));
        }
        self.means.clear();
        self.stddevs.clear();
        for (name, feature_values) in values {
            let mean = feature_values.iter().sum::<f64>() / feature_values.len() as f64;
            let variance = feature_values
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f64>()
                / feature_values.len() as f64;
            self.means.insert(name.clone(), mean);
            self.stddevs.insert(name, variance.sqrt().max(0.001));
        }
        self.trained = true;
        Ok(())
    }

    pub fn score_sequence(
        &self,
        sequence: &[FeatureVector],
    ) -> Result<TemporalSequenceScore, ModelError> {
        if !self.trained {
            return Err(ModelError::ModelNotTrained);
        }
        if sequence.len() < 2 {
            return Err(ModelError::InvalidData(
                "At least two ordered vectors are required".to_string(),
            ));
        }
        let start = sequence.len().saturating_sub(self.context_window);
        let context = &sequence[start..sequence.len() - 1];
        let current = self.embedding(sequence.last().unwrap());
        let history = context
            .iter()
            .map(|vector| self.embedding(vector))
            .collect::<Vec<_>>();
        let attention = softmax(
            history
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    -(*value - current).abs() - (history.len() - index) as f64 * 0.15
                })
                .collect(),
        );
        let expected = history
            .iter()
            .zip(&attention)
            .map(|(value, weight)| value * weight)
            .sum::<f64>();
        let transition_score = (current - expected).abs();
        let anomaly_score = sigmoid((current.abs() + transition_score) / 2.0 - 1.2);
        Ok(TemporalSequenceScore {
            anomaly_score,
            transition_score,
            attention,
        })
    }

    fn embedding(&self, vector: &FeatureVector) -> f64 {
        let mut total = 0.0;
        let mut count = 0;
        for (name, result) in &vector.features {
            if let (Some(mean), Some(stddev)) = (self.means.get(name), self.stddevs.get(name)) {
                total += (result.value - mean) / stddev;
                count += 1;
            }
        }
        if count == 0 {
            0.0
        } else {
            total / count as f64
        }
    }
}

impl Default for TemporalAttentionModel {
    fn default() -> Self {
        Self::new(32)
    }
}

fn softmax(logits: Vec<f64>) -> Vec<f64> {
    let maximum = logits.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let weights = logits
        .iter()
        .map(|value| (value - maximum).exp())
        .collect::<Vec<_>>();
    let sum = weights.iter().sum::<f64>();
    weights.into_iter().map(|weight| weight / sum).collect()
}

fn sigmoid(value: f64) -> f64 {
    1.0 / (1.0 + (-value).exp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{FeatureResult, PlayerId, Tick};

    fn vector(tick: u32, value: f64) -> FeatureVector {
        FeatureVector {
            tick: Tick(tick),
            round: 1,
            player: PlayerId::new(1),
            features: BTreeMap::from([("tracking".to_string(), FeatureResult::new(value))]),
        }
    }

    #[test]
    fn temporal_attention_flags_a_large_behavior_change() {
        let training = (0..20)
            .map(|tick| vector(tick, 0.2 + tick as f64 * 0.01))
            .collect::<Vec<_>>();
        let mut model = TemporalAttentionModel::new(4);
        model.train(&training).unwrap();
        let stable = [vector(21, 0.31), vector(22, 0.32), vector(23, 0.33)];
        let jump = [vector(21, 0.31), vector(22, 0.32), vector(23, 2.0)];
        assert!(
            model.score_sequence(&jump).unwrap().anomaly_score
                > model.score_sequence(&stable).unwrap().anomaly_score
        );
        assert!(
            (model
                .score_sequence(&jump)
                .unwrap()
                .attention
                .iter()
                .sum::<f64>()
                - 1.0)
                .abs()
                < f64::EPSILON
        );
    }
}
