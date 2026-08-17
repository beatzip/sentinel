use std::collections::BTreeSet;

use sentinel_core::FeatureVector;
use serde::{Deserialize, Serialize};

use crate::ml::ModelError;

/// Verified time-ordered feature sequence for one player in one match.
#[derive(Debug, Clone)]
pub struct LabeledSequence {
    pub vectors: Vec<FeatureVector>,
    pub label: f64,
}

/// Compact trainable single-head Transformer encoder for temporal anomaly classification.
///
/// It learns query, key and value projections plus a classifier head through
/// binary cross-entropy. The small architecture keeps CPU training practical
/// for the current Sentinel corpus while retaining the standard Transformer Q/K/V contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalTransformer {
    feature_names: Vec<String>,
    query: Vec<f64>,
    key: Vec<f64>,
    value: Vec<f64>,
    output: f64,
    bias: f64,
    learning_rate: f64,
    epochs: usize,
    trained: bool,
}

impl TemporalTransformer {
    pub fn new(epochs: usize, learning_rate: f64) -> Self {
        Self {
            feature_names: Vec::new(),
            query: Vec::new(),
            key: Vec::new(),
            value: Vec::new(),
            output: 0.1,
            bias: 0.0,
            learning_rate,
            epochs,
            trained: false,
        }
    }

    pub fn train_labeled(&mut self, sequences: &[LabeledSequence]) -> Result<(), ModelError> {
        if sequences.len() < 2 || sequences.iter().any(|sequence| sequence.vectors.is_empty()) {
            return Err(ModelError::TrainingFailed(
                "At least two non-empty labeled sequences are required".to_string(),
            ));
        }
        if sequences
            .iter()
            .any(|sequence| !(0.0..=1.0).contains(&sequence.label))
        {
            return Err(ModelError::InvalidData(
                "Labels must be between 0.0 and 1.0".to_string(),
            ));
        }
        self.feature_names = sequences
            .iter()
            .flat_map(|sequence| sequence.vectors.iter())
            .flat_map(|vector| vector.features.keys().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if self.feature_names.is_empty() {
            return Err(ModelError::InvalidData("No numeric features".to_string()));
        }
        let dimensions = self.feature_names.len();
        self.query = vec![1.0 / dimensions as f64; dimensions];
        self.key = vec![1.0 / dimensions as f64; dimensions];
        self.value = vec![1.0 / dimensions as f64; dimensions];
        self.output = 0.1;
        self.bias = 0.0;

        for _ in 0..self.epochs {
            for sequence in sequences {
                self.update(sequence);
            }
        }
        self.trained = true;
        Ok(())
    }

    pub fn predict_sequence(&self, vectors: &[FeatureVector]) -> Result<f64, ModelError> {
        if !self.trained {
            return Err(ModelError::ModelNotTrained);
        }
        if vectors.is_empty() {
            return Err(ModelError::InvalidData("Sequence is empty".to_string()));
        }
        Ok(sigmoid(self.forward(vectors).0))
    }

    pub fn load(path: &std::path::Path) -> Result<Self, ModelError> {
        let json = std::fs::read_to_string(path)
            .map_err(|error| ModelError::PredictionFailed(error.to_string()))?;
        let model = serde_json::from_str::<Self>(&json)
            .map_err(|error| ModelError::PredictionFailed(error.to_string()))?;
        if !model.trained || model.feature_names.is_empty() {
            return Err(ModelError::InvalidData(
                "Transformer artifact is not trained".to_string(),
            ));
        }
        Ok(model)
    }

    fn update(&mut self, sequence: &LabeledSequence) {
        let inputs = self.inputs(&sequence.vectors);
        let (logit, attention, q_last, keys, values, context) = self.forward_inputs(&inputs);
        let error = sigmoid(logit) - sequence.label;
        let scale = (self.feature_names.len() as f64).sqrt().max(1.0);
        let mut grad_query = vec![0.0; self.feature_names.len()];
        let mut grad_key = vec![0.0; self.feature_names.len()];
        let mut grad_value = vec![0.0; self.feature_names.len()];
        let mut attention_gradients = values
            .iter()
            .map(|value| error * self.output * value)
            .collect::<Vec<_>>();
        let weighted = attention
            .iter()
            .zip(&attention_gradients)
            .map(|(attention, gradient)| attention * gradient)
            .sum::<f64>();
        let score_gradients = attention
            .iter()
            .zip(&attention_gradients)
            .map(|(attention, gradient)| attention * (gradient - weighted))
            .collect::<Vec<_>>();
        let grad_q = score_gradients
            .iter()
            .zip(&keys)
            .map(|(gradient, key)| gradient * key / scale)
            .sum::<f64>();
        for ((input, value), (score_gradient, key)) in inputs
            .iter()
            .zip(&values)
            .zip(score_gradients.iter().zip(&keys))
        {
            let grad_v = error * self.output * attention[grad_value_index(input, &inputs)];
            for (index, feature) in input.iter().enumerate() {
                grad_value[index] += grad_v * feature;
                grad_key[index] += score_gradient * q_last / scale * feature;
            }
            let _ = value;
            let _ = key;
        }
        for (index, feature) in inputs.last().unwrap().iter().enumerate() {
            grad_query[index] += grad_q * feature;
        }
        self.output -= self.learning_rate * error * context;
        self.bias -= self.learning_rate * error;
        for index in 0..self.feature_names.len() {
            self.query[index] -= self.learning_rate * grad_query[index];
            self.key[index] -= self.learning_rate * grad_key[index];
            self.value[index] -= self.learning_rate * grad_value[index];
        }
        attention_gradients.clear();
    }

    fn forward(&self, vectors: &[FeatureVector]) -> (f64, Vec<f64>, f64, Vec<f64>, Vec<f64>, f64) {
        self.forward_inputs(&self.inputs(vectors))
    }

    fn forward_inputs(&self, inputs: &[Vec<f64>]) -> (f64, Vec<f64>, f64, Vec<f64>, Vec<f64>, f64) {
        let q_last = dot(inputs.last().unwrap(), &self.query);
        let keys = inputs
            .iter()
            .map(|input| dot(input, &self.key))
            .collect::<Vec<_>>();
        let values = inputs
            .iter()
            .map(|input| dot(input, &self.value))
            .collect::<Vec<_>>();
        let scale = (self.feature_names.len() as f64).sqrt().max(1.0);
        let attention = softmax(keys.iter().map(|key| q_last * key / scale).collect());
        let context = attention
            .iter()
            .zip(&values)
            .map(|(weight, value)| weight * value)
            .sum::<f64>();
        (
            self.output * context + self.bias,
            attention,
            q_last,
            keys,
            values,
            context,
        )
    }

    fn inputs(&self, vectors: &[FeatureVector]) -> Vec<Vec<f64>> {
        vectors
            .iter()
            .map(|vector| {
                self.feature_names
                    .iter()
                    .map(|name| vector.features.get(name).map_or(0.0, |result| result.value))
                    .collect()
            })
            .collect()
    }
}

impl Default for TemporalTransformer {
    fn default() -> Self {
        Self::new(80, 0.02)
    }
}

fn grad_value_index(input: &[f64], inputs: &[Vec<f64>]) -> usize {
    inputs
        .iter()
        .position(|candidate| candidate.as_slice() == input)
        .unwrap_or(0)
}
fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}
fn sigmoid(value: f64) -> f64 {
    1.0 / (1.0 + (-value).exp())
}
fn softmax(logits: Vec<f64>) -> Vec<f64> {
    let maximum = logits.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let values = logits
        .iter()
        .map(|value| (value - maximum).exp())
        .collect::<Vec<_>>();
    let sum = values.iter().sum::<f64>();
    values.into_iter().map(|value| value / sum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{FeatureResult, PlayerId, Tick};
    use std::collections::BTreeMap;
    fn vector(tick: u32, value: f64) -> FeatureVector {
        FeatureVector {
            tick: Tick(tick),
            round: 1,
            player: PlayerId::new(1),
            features: BTreeMap::from([("tracking".to_string(), FeatureResult::new(value))]),
        }
    }
    #[test]
    fn transformer_learns_temporal_anomaly_sequences() {
        let legit = LabeledSequence {
            vectors: vec![vector(1, 0.1), vector(2, 0.15), vector(3, 0.12)],
            label: 0.0,
        };
        let cheater = LabeledSequence {
            vectors: vec![vector(1, 0.8), vector(2, 0.95), vector(3, 1.0)],
            label: 1.0,
        };
        let mut model = TemporalTransformer::new(200, 0.05);
        model
            .train_labeled(&[legit.clone(), cheater.clone()])
            .unwrap();
        assert!(
            model.predict_sequence(&cheater.vectors).unwrap()
                > model.predict_sequence(&legit.vectors).unwrap()
        );
        let path = std::env::temp_dir().join("sentinel_transformer_test.json");
        std::fs::write(&path, serde_json::to_string(&model).unwrap()).unwrap();
        let loaded = TemporalTransformer::load(&path).unwrap();
        assert!(
            loaded.predict_sequence(&cheater.vectors).unwrap()
                > loaded.predict_sequence(&legit.vectors).unwrap()
        );
        let _ = std::fs::remove_file(path);
    }
}
