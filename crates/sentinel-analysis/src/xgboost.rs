use std::collections::BTreeSet;
use std::path::Path;

use sentinel_core::FeatureVector;
use sequoia_boost::prelude::{BoostedModel, DMatrix, TrainingParams, TreeMethod, train};

use crate::ml::ModelError;
use crate::{AnomalyModel, LabeledVector};

/// Native XGBoost-compatible binary classifier trained from verified player labels.
pub struct XgBoostModel {
    feature_names: Vec<String>,
    model: Option<BoostedModel>,
}

impl XgBoostModel {
    pub fn new() -> Self {
        Self {
            feature_names: Vec::new(),
            model: None,
        }
    }

    pub fn train_labeled(&mut self, samples: &[LabeledVector]) -> Result<(), ModelError> {
        if samples.len() < 2 {
            return Err(ModelError::TrainingFailed(
                "At least two labeled vectors are required".to_string(),
            ));
        }
        self.feature_names = samples
            .iter()
            .flat_map(|sample| sample.vector.features.keys().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if self.feature_names.is_empty() {
            return Err(ModelError::InvalidData("No numeric features".to_string()));
        }
        let labels = samples
            .iter()
            .map(|sample| sample.label as f32)
            .collect::<Vec<_>>();
        if labels.iter().any(|label| !(0.0..=1.0).contains(label)) {
            return Err(ModelError::InvalidData(
                "Labels must be between 0.0 and 1.0".to_string(),
            ));
        }
        let matrix = self
            .matrix(
                &samples
                    .iter()
                    .map(|sample| &sample.vector)
                    .collect::<Vec<_>>(),
            )?
            .with_labels(&labels)
            .map_err(|error| ModelError::TrainingFailed(error.to_string()))?;
        let params = TrainingParams::builder()
            .objective("binary:logistic")
            .tree_method(TreeMethod::Hist)
            .max_depth(5)
            .eta(0.12)
            .min_child_weight(0.0)
            .lambda(0.0)
            .build()
            .map_err(|error| ModelError::TrainingFailed(error.to_string()))?;
        self.model = Some(
            train(&params, &matrix, 120)
                .map_err(|error| ModelError::TrainingFailed(error.to_string()))?,
        );
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<(), ModelError> {
        self.model
            .as_ref()
            .ok_or(ModelError::ModelNotTrained)?
            .save_binary(path)
            .map_err(|error| ModelError::PredictionFailed(error.to_string()))
    }

    pub fn load(path: &Path, feature_names: Vec<String>) -> Result<Self, ModelError> {
        let model = BoostedModel::load_binary(path)
            .map_err(|error| ModelError::PredictionFailed(error.to_string()))?;
        if feature_names.is_empty() {
            return Err(ModelError::InvalidData(
                "XGBoost feature metadata is empty".to_string(),
            ));
        }
        Ok(Self {
            feature_names,
            model: Some(model),
        })
    }

    pub fn feature_names(&self) -> &[String] {
        &self.feature_names
    }

    fn matrix(&self, vectors: &[&FeatureVector]) -> Result<DMatrix, ModelError> {
        let values = vectors
            .iter()
            .flat_map(|vector| {
                self.feature_names.iter().map(move |name| {
                    vector.features.get(name).map_or(0.0, |result| result.value) as f32
                })
            })
            .collect::<Vec<_>>();
        DMatrix::from_dense(&values, vectors.len(), self.feature_names.len())
            .map_err(|error| ModelError::PredictionFailed(error.to_string()))
    }
}

impl Default for XgBoostModel {
    fn default() -> Self {
        Self::new()
    }
}

impl AnomalyModel for XgBoostModel {
    fn train(&mut self, _vectors: &[FeatureVector]) -> Result<(), ModelError> {
        Err(ModelError::InvalidData(
            "XGBoost requires verified labels; call train_labeled".to_string(),
        ))
    }

    fn predict(&self, vector: &FeatureVector) -> Result<f64, ModelError> {
        let model = self.model.as_ref().ok_or(ModelError::ModelNotTrained)?;
        model
            .predict(&self.matrix(&[vector])?)
            .map_err(|error| ModelError::PredictionFailed(error.to_string()))
            .and_then(|scores| {
                scores.first().copied().map(f64::from).ok_or_else(|| {
                    ModelError::PredictionFailed("XGBoost returned no score".to_string())
                })
            })
    }

    fn name(&self) -> &str {
        "xgboost"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

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
    fn xgboost_ranks_verified_anomaly_higher() {
        let samples = [
            sample(0.05, 0.0),
            sample(0.15, 0.0),
            sample(0.8, 1.0),
            sample(0.95, 1.0),
        ];
        let mut model = XgBoostModel::new();
        model.train_labeled(&samples).unwrap();
        assert!(
            model.predict(&samples[3].vector).unwrap() > model.predict(&samples[0].vector).unwrap()
        );
        let path = std::env::temp_dir().join("sentinel_xgboost_test.sqb");
        model.save(&path).unwrap();
        let loaded = XgBoostModel::load(&path, model.feature_names().to_vec()).unwrap();
        assert!(
            loaded.predict(&samples[3].vector).unwrap()
                > loaded.predict(&samples[0].vector).unwrap()
        );
        let _ = std::fs::remove_file(path);
    }
}
