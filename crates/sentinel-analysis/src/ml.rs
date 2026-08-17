use std::collections::BTreeMap;

use sentinel_core::FeatureVector;

/// Trait for ML-based anomaly detection models
pub trait AnomalyModel: Send + Sync {
    /// Train the model on feature vectors
    fn train(&mut self, vectors: &[FeatureVector]) -> Result<(), ModelError>;

    /// Predict anomaly score for a feature vector
    fn predict(&self, vector: &FeatureVector) -> Result<f64, ModelError>;

    /// Get model name
    fn name(&self) -> &str;

    /// Get model version
    fn version(&self) -> &str;
}

/// Model errors
#[derive(Debug, Clone)]
pub enum ModelError {
    TrainingFailed(String),
    PredictionFailed(String),
    InvalidData(String),
    ModelNotTrained,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::TrainingFailed(msg) => write!(f, "Training failed: {}", msg),
            ModelError::PredictionFailed(msg) => write!(f, "Prediction failed: {}", msg),
            ModelError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
            ModelError::ModelNotTrained => write!(f, "Model not trained"),
        }
    }
}

impl std::error::Error for ModelError {}

/// Isolation Forest anomaly detection model
pub struct IsolationForest {
    /// Number of trees
    n_trees: usize,
    /// Maximum depth of each tree
    max_depth: usize,
    /// Trained trees
    trees: Vec<IsolationNode>,
    feature_names: Vec<String>,
    bounds: Vec<(f64, f64)>,
    sample_size: usize,
    /// Whether the model is trained
    trained: bool,
}

enum IsolationNode {
    Branch {
        feature_index: usize,
        split_value: f64,
        left: Box<IsolationNode>,
        right: Box<IsolationNode>,
    },
    Leaf {
        depth: usize,
    },
}

impl IsolationForest {
    pub fn new(n_trees: usize, max_depth: usize) -> Self {
        Self {
            n_trees,
            max_depth,
            trees: Vec::new(),
            feature_names: Vec::new(),
            bounds: Vec::new(),
            sample_size: 0,
            trained: false,
        }
    }

    /// Compute anomaly score where higher values are more anomalous.
    fn compute_score(&self, vector: &FeatureVector) -> f64 {
        if self.trees.is_empty() {
            return 0.5;
        }
        if self.bounds.iter().enumerate().any(|(index, (min, max))| {
            let value = self.feature_value(vector, index);
            value < *min || value > *max
        }) {
            return 1.0;
        }

        let average_depth = self
            .trees
            .iter()
            .map(|tree| self.path_length(tree, vector) as f64)
            .sum::<f64>()
            / self.trees.len() as f64;

        let c = self.average_path_length(self.sample_size);
        if c == 0.0 {
            0.5
        } else {
            2.0_f64.powf(-average_depth / c)
        }
    }

    fn path_length(&self, node: &IsolationNode, vector: &FeatureVector) -> usize {
        match node {
            IsolationNode::Leaf { depth } => *depth,
            IsolationNode::Branch {
                feature_index,
                split_value,
                left,
                right,
            } => {
                if self.feature_value(vector, *feature_index) < *split_value {
                    self.path_length(left, vector)
                } else {
                    self.path_length(right, vector)
                }
            }
        }
    }

    fn feature_value(&self, vector: &FeatureVector, index: usize) -> f64 {
        self.feature_names
            .get(index)
            .and_then(|name| vector.features.get(name))
            .map_or(0.0, |result| result.value)
    }

    fn build_tree(&self, samples: &[Vec<f64>], depth: usize, tree_index: usize) -> IsolationNode {
        if depth >= self.max_depth || samples.len() <= 1 || self.feature_names.is_empty() {
            return IsolationNode::Leaf { depth };
        }
        let feature_index = (tree_index + depth) % self.feature_names.len();
        let min = samples
            .iter()
            .map(|sample| sample[feature_index])
            .fold(f64::INFINITY, f64::min);
        let max = samples
            .iter()
            .map(|sample| sample[feature_index])
            .fold(f64::NEG_INFINITY, f64::max);
        if min >= max {
            return IsolationNode::Leaf { depth };
        }
        let split_value = min + (max - min) * [0.25, 0.5, 0.75][(tree_index + depth) % 3];
        let (left_samples, right_samples): (Vec<_>, Vec<_>) = samples
            .iter()
            .cloned()
            .partition(|sample| sample[feature_index] < split_value);
        if left_samples.is_empty() || right_samples.is_empty() {
            return IsolationNode::Leaf { depth };
        }
        IsolationNode::Branch {
            feature_index,
            split_value,
            left: Box::new(self.build_tree(&left_samples, depth + 1, tree_index)),
            right: Box::new(self.build_tree(&right_samples, depth + 1, tree_index)),
        }
    }

    fn average_path_length(&self, n: usize) -> f64 {
        if n <= 1 {
            return 0.0;
        }
        2.0 * (n as f64 - 1.0).ln() - 2.0 * (n as f64 - 1.0) / n as f64 + 1.0
    }
}

impl AnomalyModel for IsolationForest {
    fn train(&mut self, vectors: &[FeatureVector]) -> Result<(), ModelError> {
        if vectors.is_empty() {
            return Err(ModelError::TrainingFailed("No training data".to_string()));
        }

        self.feature_names = vectors
            .iter()
            .flat_map(|vector| vector.features.keys().cloned())
            .collect();
        self.feature_names.sort();
        self.feature_names.dedup();
        if self.feature_names.is_empty() {
            return Err(ModelError::InvalidData("No numeric features".to_string()));
        }
        let samples = vectors
            .iter()
            .map(|vector| {
                self.feature_names
                    .iter()
                    .map(|name| vector.features.get(name).map_or(0.0, |result| result.value))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        self.bounds = (0..self.feature_names.len())
            .map(|index| {
                (
                    samples
                        .iter()
                        .map(|sample| sample[index])
                        .fold(f64::INFINITY, f64::min),
                    samples
                        .iter()
                        .map(|sample| sample[index])
                        .fold(f64::NEG_INFINITY, f64::max),
                )
            })
            .collect();
        self.trees.clear();
        self.sample_size = samples.len();
        for tree_index in 0..self.n_trees {
            self.trees.push(self.build_tree(&samples, 0, tree_index));
        }

        self.trained = true;
        Ok(())
    }

    fn predict(&self, vector: &FeatureVector) -> Result<f64, ModelError> {
        if !self.trained {
            return Err(ModelError::ModelNotTrained);
        }
        Ok(self.compute_score(vector))
    }

    fn name(&self) -> &str {
        "isolation_forest"
    }

    fn version(&self) -> &str {
        "1.1.0"
    }
}

/// Simple statistical model using z-scores
pub struct StatisticalModel {
    /// Feature means
    means: BTreeMap<String, f64>,
    /// Feature standard deviations
    stddevs: BTreeMap<String, f64>,
    trained: bool,
}

impl StatisticalModel {
    pub fn new() -> Self {
        Self {
            means: BTreeMap::new(),
            stddevs: BTreeMap::new(),
            trained: false,
        }
    }

    fn compute_score(&self, vector: &FeatureVector) -> f64 {
        if self.means.is_empty() {
            return 0.0;
        }

        let mut total_z = 0.0;
        let mut count = 0;

        for (name, result) in &vector.features {
            if let (Some(&mean), Some(&stddev)) = (self.means.get(name), self.stddevs.get(name))
                && stddev > 0.0
            {
                let z = (result.value - mean).abs() / stddev;
                total_z += z;
                count += 1;
            }
        }

        if count == 0 {
            return 0.0;
        }

        let avg_z = total_z / count as f64;

        // Convert z-score to anomaly score using sigmoid
        1.0 / (1.0 + (-avg_z + 2.0).exp())
    }
}

impl Default for StatisticalModel {
    fn default() -> Self {
        Self::new()
    }
}

impl AnomalyModel for StatisticalModel {
    fn train(&mut self, vectors: &[FeatureVector]) -> Result<(), ModelError> {
        if vectors.is_empty() {
            return Err(ModelError::TrainingFailed("No training data".to_string()));
        }

        // Compute means and stddevs for each feature
        let mut feature_values: BTreeMap<String, Vec<f64>> = BTreeMap::new();

        for fv in vectors {
            for (name, result) in &fv.features {
                feature_values
                    .entry(name.clone())
                    .or_default()
                    .push(result.value);
            }
        }

        for (name, values) in feature_values {
            let n = values.len() as f64;
            let mean = values.iter().sum::<f64>() / n;
            let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
            let stddev = variance.sqrt();

            self.means.insert(name.clone(), mean);
            self.stddevs.insert(name, stddev);
        }

        self.trained = true;
        Ok(())
    }

    fn predict(&self, vector: &FeatureVector) -> Result<f64, ModelError> {
        if !self.trained {
            return Err(ModelError::ModelNotTrained);
        }
        Ok(self.compute_score(vector))
    }

    fn name(&self) -> &str {
        "statistical"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }
}

impl Default for IsolationForest {
    fn default() -> Self {
        Self::new(100, 10)
    }
}

/// Ensemble model combining multiple models
pub struct EnsembleModel {
    models: Vec<Box<dyn AnomalyModel>>,
    weights: Vec<f64>,
}

impl EnsembleModel {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            weights: Vec::new(),
        }
    }

    pub fn add_model(&mut self, model: Box<dyn AnomalyModel>, weight: f64) {
        self.models.push(model);
        self.weights.push(weight);
    }
}

impl Default for EnsembleModel {
    fn default() -> Self {
        Self::new()
    }
}

impl AnomalyModel for EnsembleModel {
    fn train(&mut self, vectors: &[FeatureVector]) -> Result<(), ModelError> {
        for model in &mut self.models {
            model.train(vectors)?;
        }
        Ok(())
    }

    fn predict(&self, vector: &FeatureVector) -> Result<f64, ModelError> {
        if self.models.is_empty() {
            return Err(ModelError::ModelNotTrained);
        }

        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for (model, &weight) in self.models.iter().zip(&self.weights) {
            if let Ok(score) = model.predict(vector) {
                weighted_sum += score * weight;
                total_weight += weight;
            }
        }

        if total_weight == 0.0 {
            Ok(0.0)
        } else {
            Ok(weighted_sum / total_weight)
        }
    }

    fn name(&self) -> &str {
        "ensemble"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{PlayerId, Tick};
    use std::collections::BTreeMap;

    fn make_feature_vector(value: f64) -> FeatureVector {
        let mut features = BTreeMap::new();
        features.insert(
            "test_feature".to_string(),
            sentinel_core::FeatureResult::new(value),
        );

        FeatureVector {
            tick: Tick(100),
            round: 1,
            player: PlayerId::new(1),
            features,
        }
    }

    #[test]
    fn test_statistical_model() {
        let mut model = StatisticalModel::new();

        // Train on normal values
        let vectors: Vec<FeatureVector> = (0..100)
            .map(|i| make_feature_vector(10.0 + (i as f64 * 0.1)))
            .collect();

        model.train(&vectors).unwrap();

        // Predict on normal value
        let normal = make_feature_vector(10.5);
        let score = model.predict(&normal).unwrap();
        assert!(score < 0.5);

        // Predict on anomalous value
        let anomalous = make_feature_vector(100.0);
        let score = model.predict(&anomalous).unwrap();
        assert!(score > 0.5);
    }

    #[test]
    fn test_isolation_forest() {
        let mut model = IsolationForest::new(10, 5);

        let vectors: Vec<FeatureVector> = (0..50).map(|i| make_feature_vector(i as f64)).collect();

        model.train(&vectors).unwrap();

        let test = make_feature_vector(25.0);
        let normal_score = model.predict(&test).unwrap();
        let anomaly_score = model.predict(&make_feature_vector(100.0)).unwrap();
        assert!((0.0..=1.0).contains(&normal_score));
        assert!(anomaly_score > normal_score);
    }

    #[test]
    fn test_ensemble_model() {
        let mut ensemble = EnsembleModel::new();
        ensemble.add_model(Box::new(StatisticalModel::new()), 0.5);
        ensemble.add_model(Box::new(IsolationForest::new(10, 5)), 0.5);

        let vectors: Vec<FeatureVector> = (0..50).map(|i| make_feature_vector(i as f64)).collect();

        ensemble.train(&vectors).unwrap();

        let test = make_feature_vector(25.0);
        let score = ensemble.predict(&test).unwrap();
        assert!((0.0..=1.0).contains(&score));
    }
}
