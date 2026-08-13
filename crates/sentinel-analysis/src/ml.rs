//! Machine-learning models for anomaly detection.
//!
//! Implements three families used by Sentinel:
//!
//! - **IsolationForest** — real recursive isolation trees that partition the
//!   feature space. Samples that isolate in few splits score as anomalous.
//!   Deterministic (seeded) so results are reproducible.
//! - **BoostedStumps** — an XGBoost-style ensemble of decision stumps trained
//!   with gradient boosting on z-scored feature deviations, combining many
//!   weak learners into one anomaly score.
//! - **TemporalTransformer** — a scaffold for attention-over-time modeling of
//!   a player's feature-vector sequence (placeholder positional encoding +
//!   single self-attention head), kept lightweight and dependency-free.
//!
//! Also provides an **ABTest** framework to compare two models on a labelled
//! set and report which is better.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

/// A feature matrix: rows are samples, columns are feature values in a stable
/// column order. Built from heterogeneous `FeatureVector`s so models operate on
/// fixed-width numeric data.
#[derive(Debug, Clone, Default)]
pub struct FeatureMatrix {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<f64>>,
}

impl FeatureMatrix {
    pub fn from_vectors(vectors: &[FeatureVector]) -> Self {
        // Collect the union of feature names in a stable sorted order.
        let mut col_set = std::collections::BTreeSet::new();
        for fv in vectors {
            for name in fv.features.keys() {
                col_set.insert(name.clone());
            }
        }
        let columns: Vec<String> = col_set.into_iter().collect();
        let col_index: BTreeMap<String, usize> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| (c.clone(), i))
            .collect();

        let mut rows = Vec::with_capacity(vectors.len());
        for fv in vectors {
            let mut row = vec![0.0_f64; columns.len()];
            for (name, result) in &fv.features {
                if let Some(&idx) = col_index.get(name) {
                    row[idx] = result.value;
                }
            }
            rows.push(row);
        }
        Self { columns, rows }
    }

    pub fn n_features(&self) -> usize {
        self.columns.len()
    }

    pub fn n_samples(&self) -> usize {
        self.rows.len()
    }
}

/// Simple deterministic PRNG (xorshift) so training is reproducible across
/// machines — Sentinel's "deterministic" design principle.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub fn next_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() as usize) % bound
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() as f64 / u64::MAX as f64).clamp(0.0, 1.0)
    }
}

/// A single isolation tree node.
#[derive(Debug, Clone)]
#[expect(
    dead_code,
    reason = "isolation-tree node fields retained for completeness"
)]
enum IsolationNode {
    Branch {
        feature: usize,
        split: f64,
        left: Box<IsolationNode>,
        right: Box<IsolationNode>,
    },
    Leaf {
        depth: usize,
        size: usize,
    },
}

/// A single isolation tree.
#[derive(Debug, Clone)]
pub struct IsolationTree {
    root: IsolationNode,
}

impl IsolationTree {
    /// Build a tree by recursively partitioning `rows` on random features
    /// and random split points until max depth or a single sample remains.
    fn build(rows: &[(usize, Vec<f64>)], depth: usize, max_depth: usize, rng: &mut Rng) -> Self {
        if rows.is_empty() {
            return Self {
                root: IsolationNode::Leaf { depth, size: 0 },
            };
        }
        if depth >= max_depth || rows.len() <= 1 {
            return Self {
                root: IsolationNode::Leaf {
                    depth,
                    size: rows.len(),
                },
            };
        }
        let n_features = rows[0].1.len();
        if n_features == 0 {
            return Self {
                root: IsolationNode::Leaf {
                    depth,
                    size: rows.len(),
                },
            };
        }

        let feature = rng.next_usize(n_features);
        let vals: Vec<f64> = rows.iter().map(|(_, r)| r[feature]).collect();
        let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if (max - min).abs() < 1e-12 {
            return Self {
                root: IsolationNode::Leaf {
                    depth,
                    size: rows.len(),
                },
            };
        }
        let split = min + rng.next_f64() * (max - min);

        let mut left_rows = Vec::new();
        let mut right_rows = Vec::new();
        for (i, row) in rows {
            if row[feature] < split {
                left_rows.push((*i, row.clone()));
            } else {
                right_rows.push((*i, row.clone()));
            }
        }

        Self {
            root: IsolationNode::Branch {
                feature,
                split,
                left: Box::new(Self::build(&left_rows, depth + 1, max_depth, rng).root),
                right: Box::new(Self::build(&right_rows, depth + 1, max_depth, rng).root),
            },
        }
    }

    /// Path length to isolate a sample (with leaf-size adjustment).
    fn path_length(&self, row: &[f64]) -> f64 {
        Self::walk(&self.root, row, 0)
    }

    fn walk(node: &IsolationNode, row: &[f64], depth: usize) -> f64 {
        match node {
            IsolationNode::Leaf { size, .. } => depth as f64 + Self::expected_path(*size),
            IsolationNode::Branch {
                feature,
                split,
                left,
                right,
            } => {
                if row.get(*feature).copied().unwrap_or(0.0) < *split {
                    Self::walk(left, row, depth + 1)
                } else {
                    Self::walk(right, row, depth + 1)
                }
            }
        }
    }

    /// Expected path length for a leaf of size n: 2 H(n-1) - 2(n-1)/n where
    /// H is the harmonic number, approximated by ln.
    fn expected_path(n: usize) -> f64 {
        if n <= 1 {
            return 0.0;
        }
        let n = n as f64;
        2.0 * ((n - 1.0).ln() + 0.5772156649) - 2.0 * (n - 1.0) / n
    }
}

/// Isolation Forest anomaly detection model.
pub struct IsolationForest {
    n_trees: usize,
    max_depth: usize,
    sample_size: usize,
    seed: u64,
    trees: Vec<IsolationTree>,
    trained: bool,
    /// c(n) normalization constant computed from sample size.
    c_norm: f64,
}

impl IsolationForest {
    pub fn new(n_trees: usize, max_depth: usize) -> Self {
        Self {
            n_trees,
            max_depth,
            sample_size: 256,
            seed: 0xC0FFEE,
            trees: Vec::new(),
            trained: false,
            c_norm: 1.0,
        }
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_sample_size(mut self, sample_size: usize) -> Self {
        self.sample_size = sample_size.max(2);
        self
    }

    fn compute_score(&self, row: &[f64]) -> f64 {
        if self.trees.is_empty() {
            return 0.5;
        }
        let avg: f64 =
            self.trees.iter().map(|t| t.path_length(row)).sum::<f64>() / self.trees.len() as f64;
        let c = self.c_norm;
        if c == 0.0 {
            0.5
        } else {
            // s = 2^(-avg/c); 1 = most anomalous, 0.5 = average.
            2.0_f64.powf(-avg / c)
        }
    }
}

impl Default for IsolationForest {
    fn default() -> Self {
        Self::new(100, 10)
    }
}

impl AnomalyModel for IsolationForest {
    fn train(&mut self, vectors: &[FeatureVector]) -> Result<(), ModelError> {
        if vectors.is_empty() {
            return Err(ModelError::TrainingFailed("No training data".to_string()));
        }

        let matrix = FeatureMatrix::from_vectors(vectors);
        if matrix.n_features() == 0 {
            return Err(ModelError::InvalidData(
                "Feature vectors have no features".to_string(),
            ));
        }

        let sample_size = self.sample_size.min(matrix.n_samples());
        self.c_norm = IsolationTree::expected_path(sample_size);
        let mut rng = Rng::new(self.seed);

        self.trees.clear();
        for _ in 0..self.n_trees {
            // Sample `sample_size` rows (with replacement) for this tree.
            let mut sample: Vec<(usize, Vec<f64>)> = Vec::with_capacity(sample_size);
            for _ in 0..sample_size {
                let i = rng.next_usize(matrix.n_samples());
                sample.push((i, matrix.rows[i].clone()));
            }
            self.trees
                .push(IsolationTree::build(&sample, 0, self.max_depth, &mut rng));
        }

        self.trained = true;
        Ok(())
    }

    fn predict(&self, vector: &FeatureVector) -> Result<f64, ModelError> {
        if !self.trained {
            return Err(ModelError::ModelNotTrained);
        }
        // Project the feature vector into the model's column order.
        let row = self.project(vector);
        Ok(self.compute_score(&row))
    }

    fn name(&self) -> &str {
        "isolation_forest"
    }

    fn version(&self) -> &str {
        "2.0.0"
    }
}

impl IsolationForest {
    fn project(&self, _vector: &FeatureVector) -> Vec<f64> {
        // The column order is not retained after training; we rebuild from the
        // incoming vector using its own keys sorted. For prediction we rely on
        // the model being trained on the same feature schema, so we project
        // the vector into a sorted-key row.
        let mut keys: Vec<&String> = _vector.features.keys().collect();
        keys.sort();
        keys.iter().map(|k| _vector.features[*k].value).collect()
    }
}

/// Simple statistical model using z-scores.
pub struct StatisticalModel {
    means: BTreeMap<String, f64>,
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

/// A single decision stump: splits one feature at a threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionStump {
    pub feature: String,
    pub threshold: f64,
    pub left_score: f64,
    pub right_score: f64,
}

impl DecisionStump {
    fn predict(&self, value: f64) -> f64 {
        if value < self.threshold {
            self.left_score
        } else {
            self.right_score
        }
    }
}

/// XGBoost-style ensemble of decision stumps trained with gradient boosting.
///
/// Each round fits a stump to the residual (the gradient of a logistic loss
/// w.r.t. the current prediction) and adds it with a learning rate. Labels are
/// 1.0 for anomalous samples, 0.0 for normal.
pub struct BoostedStumps {
    stumps: Vec<DecisionStump>,
    learning_rate: f64,
    trained: bool,
}

impl BoostedStumps {
    pub fn new(learning_rate: f64) -> Self {
        Self {
            stumps: Vec::new(),
            learning_rate,
            trained: false,
        }
    }

    fn fit_stump(
        features: &BTreeMap<String, Vec<f64>>,
        residuals: &[f64],
    ) -> Option<DecisionStump> {
        let n = residuals.len();
        if n == 0 {
            return None;
        }
        let mut best: Option<DecisionStump> = None;
        let mut best_loss = f64::INFINITY;

        for (name, values) in features {
            let mut indexed: Vec<(f64, f64)> = values
                .iter()
                .copied()
                .zip(residuals.iter().copied())
                .collect();
            indexed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            // Try splits between consecutive distinct values.
            for i in 1..n {
                if indexed[i].0 == indexed[i - 1].0 {
                    continue;
                }
                let threshold = (indexed[i].0 + indexed[i - 1].0) / 2.0;
                let (left, right): (Vec<f64>, Vec<f64>) = indexed
                    .iter()
                    .map(|(_, r)| *r)
                    .partition(|_| indexed[i - 1].0 < threshold);
                if left.is_empty() || right.is_empty() {
                    continue;
                }
                let left_mean = left.iter().sum::<f64>() / left.len() as f64;
                let right_mean = right.iter().sum::<f64>() / right.len() as f64;
                // Loss = sum of squared residuals to the local means.
                let loss: f64 = left.iter().map(|r| (r - left_mean).powi(2)).sum::<f64>()
                    + right.iter().map(|r| (r - right_mean).powi(2)).sum::<f64>();
                if loss < best_loss {
                    best_loss = loss;
                    best = Some(DecisionStump {
                        feature: name.clone(),
                        threshold,
                        left_score: left_mean,
                        right_score: right_mean,
                    });
                }
            }
        }
        best
    }
}

impl Default for BoostedStumps {
    fn default() -> Self {
        Self::new(0.1)
    }
}

impl AnomalyModel for BoostedStumps {
    fn train(&mut self, vectors: &[FeatureVector]) -> Result<(), ModelError> {
        if vectors.is_empty() {
            return Err(ModelError::TrainingFailed("No training data".to_string()));
        }

        // Columnar feature storage.
        let mut features: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for fv in vectors {
            for (name, result) in &fv.features {
                features.entry(name.clone()).or_default().push(result.value);
            }
        }
        // Pad missing values with 0.
        let n = vectors.len();
        for vals in features.values_mut() {
            while vals.len() < n {
                vals.push(0.0);
            }
        }

        if features.is_empty() {
            return Err(ModelError::InvalidData(
                "Feature vectors have no features".to_string(),
            ));
        }

        // Labels: treat samples whose mean z-score (proxy) is high as positive.
        // Since we may not have labels here, we synthesize targets from
        // outlying-ness so the boosting has signal.
        let mut means = BTreeMap::new();
        let mut stds = BTreeMap::new();
        for (name, vals) in &features {
            let m = vals.iter().sum::<f64>() / vals.len() as f64;
            let var = vals.iter().map(|v| (v - m).powi(2)).sum::<f64>() / vals.len() as f64;
            means.insert(name.clone(), m);
            stds.insert(name.clone(), var.sqrt());
        }
        let mut labels = vec![0.0_f64; n];
        for (i, fv) in vectors.iter().enumerate() {
            let mut zsum = 0.0;
            let mut zc = 0;
            for (name, result) in &fv.features {
                if let Some(&s) = stds.get(name) {
                    if s > 0.0 {
                        zsum += ((result.value - means[name]).abs()) / s;
                        zc += 1;
                    }
                }
            }
            let avg_z = if zc > 0 { zsum / zc as f64 } else { 0.0 };
            // Logistic target from z-score.
            labels[i] = 1.0 / (1.0 + (-(avg_z - 2.0)).exp());
        }

        let n_rounds = 50;
        let mut preds = vec![0.5_f64; n];
        self.stumps.clear();
        for _ in 0..n_rounds {
            // Gradient of logloss: residual = label - pred.
            let residuals: Vec<f64> = (0..n).map(|i| labels[i] - preds[i]).collect();
            let stump = match Self::fit_stump(&features, &residuals) {
                Some(s) => s,
                None => break,
            };
            // Update predictions for each sample.
            for i in 0..n {
                let v = features
                    .get(&stump.feature)
                    .and_then(|vals| vals.get(i).copied())
                    .unwrap_or(0.0);
                preds[i] += self.learning_rate * stump.predict(v);
            }
            self.stumps.push(stump);
        }

        self.trained = true;
        Ok(())
    }

    fn predict(&self, vector: &FeatureVector) -> Result<f64, ModelError> {
        if !self.trained {
            return Err(ModelError::ModelNotTrained);
        }
        let mut score = 0.0;
        for stump in &self.stumps {
            let v = vector.get_value(&stump.feature).unwrap_or(0.0);
            score += self.learning_rate * stump.predict(v);
        }
        // Normalize via logistic so output is in [0,1].
        Ok(1.0 / (1.0 + (-score).exp()))
    }

    fn name(&self) -> &str {
        "boosted_stumps"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }
}

/// Temporal Transformer scaffold: a dependency-free single self-attention head
/// over a player's feature-vector sequence. This is a lightweight placeholder
/// for a full transformer-based temporal model; it computes attention weights
/// and returns a sequence-level anomaly score.
pub struct TemporalTransformer {
    embedding_dim: usize,
    trained: bool,
    /// Learned query/key projection weights (random-init for the scaffold).
    weights: Vec<Vec<f64>>,
    bias: f64,
}

impl TemporalTransformer {
    pub fn new(embedding_dim: usize) -> Self {
        Self {
            embedding_dim,
            trained: false,
            weights: Vec::new(),
            bias: 0.0,
        }
    }

    /// Train on a *sequence* of feature vectors for a single player. For the
    /// scaffold we just learn mean statistics to center the attention.
    fn train_sequence(&mut self, sequence: &[FeatureVector]) {
        if sequence.is_empty() {
            return;
        }
        let matrix = FeatureMatrix::from_vectors(sequence);
        self.embedding_dim = matrix.n_features();
        let dim = self.embedding_dim.max(1);
        // Simple identity-ish weights (diagonal bias) so attention is stable.
        self.weights = (0..dim)
            .map(|i| (0..dim).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
            .collect();
        // Bias = mean anomaly proxy across the sequence.
        let mut sum = 0.0;
        for row in &matrix.rows {
            let m = row.iter().sum::<f64>() / row.len().max(1) as f64;
            sum += m;
        }
        self.bias = sum / matrix.n_samples().max(1) as f64;
        self.trained = true;
    }

    /// Score a sequence: higher variance of attention-weighted embeddings is
    /// treated as more anomalous (machine-like repetition reduces variance).
    fn score_sequence(&self, sequence: &[FeatureVector]) -> f64 {
        if !self.trained || sequence.is_empty() {
            return 0.5;
        }
        let matrix = FeatureMatrix::from_vectors(sequence);
        let dim = self.embedding_dim.min(matrix.n_features());
        if dim == 0 {
            return 0.5;
        }
        // Attention: dot product of each row with the (bias) query vector.
        let query: Vec<f64> = (0..dim).map(|_| self.bias).collect();
        let mut attention = Vec::with_capacity(matrix.n_samples());
        for row in &matrix.rows {
            let dot: f64 = (0..dim).map(|i| row[i] * query[i]).sum();
            attention.push(dot);
        }
        // Softmax over attention to get weights.
        let max = attention.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = attention.iter().map(|a| (a - max).exp()).collect();
        let exp_sum: f64 = exps.iter().sum();
        let weights: Vec<f64> = exps.iter().map(|e| e / exp_sum.max(1e-12)).collect();

        // Weighted embedding and its variance across time — low variance (high
        // repetition) is more anomalous.
        let mut weighted = vec![0.0_f64; dim];
        for (i, row) in matrix.rows.iter().enumerate() {
            for j in 0..dim {
                weighted[j] += weights[i] * row[j];
            }
        }
        let mean = weighted.iter().sum::<f64>() / dim as f64;
        let var = weighted.iter().map(|w| (w - mean).powi(2)).sum::<f64>() / dim as f64;
        // Map variance to [0,1]: low variance -> high anomaly.
        1.0 / (1.0 + var.sqrt())
    }
}

impl Default for TemporalTransformer {
    fn default() -> Self {
        Self::new(0)
    }
}

impl AnomalyModel for TemporalTransformer {
    fn train(&mut self, vectors: &[FeatureVector]) -> Result<(), ModelError> {
        if vectors.is_empty() {
            return Err(ModelError::TrainingFailed("No training data".to_string()));
        }
        self.train_sequence(vectors);
        Ok(())
    }

    fn predict(&self, vector: &FeatureVector) -> Result<f64, ModelError> {
        if !self.trained {
            return Err(ModelError::ModelNotTrained);
        }
        Ok(self.score_sequence(std::slice::from_ref(vector)))
    }

    fn name(&self) -> &str {
        "temporal_transformer"
    }

    fn version(&self) -> &str {
        "0.1.0"
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

/// A labelled sample for A/B testing: the model's score and the true label.
#[derive(Debug, Clone, Copy)]
pub struct LabeledSample {
    pub score: f64,
    pub positive: bool,
}

/// Result of comparing two models via A/B test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestResult {
    pub model_a_name: String,
    pub model_b_name: String,
    pub auc_a: f64,
    pub auc_b: f64,
    /// +1 if B is better, -1 if A is better, 0 if tied.
    pub winner: i8,
    pub delta_auc: f64,
}

/// A/B testing framework: compares two models on the same labelled set and
/// reports which has higher AUC-ROC.
pub struct ABTest;

impl ABTest {
    /// Compare two models by name + their labelled score sets.
    pub fn compare(
        name_a: &str,
        scores_a: &[LabeledSample],
        name_b: &str,
        scores_b: &[LabeledSample],
    ) -> ABTestResult {
        let auc_a = Self::auc(scores_a);
        let auc_b = Self::auc(scores_b);
        let delta = auc_b - auc_a;
        let winner = if delta > 1e-9 {
            1
        } else if delta < -1e-9 {
            -1
        } else {
            0
        };
        ABTestResult {
            model_a_name: name_a.to_string(),
            model_b_name: name_b.to_string(),
            auc_a,
            auc_b,
            winner,
            delta_auc: delta,
        }
    }

    /// Rank-based AUC: probability that a positive sample outranks a negative.
    fn auc(samples: &[LabeledSample]) -> f64 {
        let pos: Vec<f64> = samples
            .iter()
            .filter(|s| s.positive)
            .map(|s| s.score)
            .collect();
        let neg: Vec<f64> = samples
            .iter()
            .filter(|s| !s.positive)
            .map(|s| s.score)
            .collect();
        if pos.is_empty() || neg.is_empty() {
            return 0.5;
        }
        let mut wins = 0.0;
        let mut ties = 0.0;
        for &p in &pos {
            for &n in &neg {
                if p > n {
                    wins += 1.0;
                } else if (p - n).abs() < 1e-12 {
                    ties += 1.0;
                }
            }
        }
        (wins + 0.5 * ties) / (pos.len() * neg.len()) as f64
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

    fn make_two_feature_vector(a: f64, b: f64) -> FeatureVector {
        let mut features = BTreeMap::new();
        features.insert("a".to_string(), sentinel_core::FeatureResult::new(a));
        features.insert("b".to_string(), sentinel_core::FeatureResult::new(b));
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
        let vectors: Vec<FeatureVector> = (0..100)
            .map(|i| make_feature_vector(10.0 + (i as f64 * 0.1)))
            .collect();
        model.train(&vectors).unwrap();
        let normal = make_feature_vector(10.5);
        assert!(model.predict(&normal).unwrap() < 0.5);
        let anomalous = make_feature_vector(100.0);
        assert!(model.predict(&anomalous).unwrap() > 0.5);
    }

    #[test]
    fn isolation_forest_flags_outlier() {
        let mut model = IsolationForest::new(20, 6).with_sample_size(32);
        // 40 normal clustered samples + 1 outlier.
        let mut vectors: Vec<FeatureVector> = (0..40)
            .map(|i| make_two_feature_vector(5.0, i as f64 * 0.01))
            .collect();
        vectors.push(make_two_feature_vector(90.0, 90.0));
        model.train(&vectors).unwrap();

        let normal = make_two_feature_vector(5.0, 0.2);
        let outlier = make_two_feature_vector(90.0, 90.0);
        let s_normal = model.predict(&normal).unwrap();
        let s_outlier = model.predict(&outlier).unwrap();
        // Outlier should score more anomalous (higher) than the cluster.
        assert!(
            s_outlier > s_normal,
            "outlier={s_outlier} normal={s_normal}"
        );
    }

    #[test]
    fn boosted_stumps_train_and_predict() {
        let mut model = BoostedStumps::new(0.3);
        let vectors: Vec<FeatureVector> = (0..60)
            .map(|i| make_feature_vector(10.0 + (i as f64 * 0.1)))
            .collect();
        model.train(&vectors).unwrap();
        let score = model.predict(&make_feature_vector(10.5)).unwrap();
        assert!((0.0..=1.0).contains(&score));
        let outlier = model.predict(&make_feature_vector(500.0)).unwrap();
        assert!((0.0..=1.0).contains(&outlier));
    }

    #[test]
    fn temporal_transformer_runs_on_sequence() {
        let mut model = TemporalTransformer::new(0);
        let seq: Vec<FeatureVector> = (0..10).map(|i| make_feature_vector(i as f64)).collect();
        model.train(&seq).unwrap();
        let s = model.predict(&make_feature_vector(5.0)).unwrap();
        assert!((0.0..=1.0).contains(&s));
    }

    #[test]
    fn ensemble_combines_models() {
        let mut ensemble = EnsembleModel::new();
        ensemble.add_model(Box::new(StatisticalModel::new()), 0.5);
        ensemble.add_model(Box::new(IsolationForest::new(10, 5)), 0.5);
        let vectors: Vec<FeatureVector> = (0..50).map(|i| make_feature_vector(i as f64)).collect();
        ensemble.train(&vectors).unwrap();
        let score = ensemble.predict(&make_feature_vector(25.0)).unwrap();
        assert!((0.0..=1.0).contains(&score));
    }

    #[test]
    fn ab_test_picks_better_model() {
        // Model A is near-random (AUC 0.5), model B perfectly separates.
        let a = vec![
            LabeledSample {
                score: 0.5,
                positive: true,
            },
            LabeledSample {
                score: 0.5,
                positive: false,
            },
            LabeledSample {
                score: 0.6,
                positive: true,
            },
            LabeledSample {
                score: 0.4,
                positive: false,
            },
        ];
        let b = vec![
            LabeledSample {
                score: 0.9,
                positive: true,
            },
            LabeledSample {
                score: 0.1,
                positive: false,
            },
            LabeledSample {
                score: 0.8,
                positive: true,
            },
            LabeledSample {
                score: 0.2,
                positive: false,
            },
        ];
        let res = ABTest::compare("stat", &a, "forest", &b);
        assert_eq!(res.winner, 1);
        assert!(res.auc_b > res.auc_a);
    }

    #[test]
    fn rng_is_deterministic() {
        let mut r1 = Rng::new(42);
        let mut r2 = Rng::new(42);
        for _ in 0..10 {
            assert_eq!(r1.next_u64(), r2.next_u64());
        }
    }
}
