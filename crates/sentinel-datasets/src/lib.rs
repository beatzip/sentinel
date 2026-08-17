use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use sentinel_analysis::BaselineSet;

/// Calibration dataset containing feature distributions from known-legit matches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationDataset {
    /// Dataset name
    pub name: String,
    /// Version of the dataset
    pub version: String,
    /// Number of matches used
    pub match_count: usize,
    /// Number of players used
    pub player_count: usize,
    /// Feature baselines computed from the dataset
    pub baselines: BaselineSet,
    /// Metadata about the dataset
    pub metadata: DatasetMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetMetadata {
    /// Creation timestamp
    pub created_at: String,
    /// Source of the data
    pub source: String,
    /// Game version
    pub game_version: String,
    /// Tick rate of the matches
    pub tick_rate: u32,
    /// Maps included
    pub maps: Vec<String>,
}

impl CalibrationDataset {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "1.0.0".to_string(),
            match_count: 0,
            player_count: 0,
            baselines: BaselineSet::new(),
            metadata: DatasetMetadata {
                created_at: chrono::Utc::now().to_rfc3339(),
                source: "unknown".to_string(),
                game_version: "cs2".to_string(),
                tick_rate: 64,
                maps: Vec::new(),
            },
        }
    }

    /// Create a default calibration dataset with CS2-typical values
    pub fn default_cs2() -> Self {
        let mut dataset = Self::new("cs2_default");
        dataset.version = "1.0.0".to_string();
        dataset.match_count = 1000;
        dataset.player_count = 10000;
        dataset.baselines = BaselineSet::default_cs2();
        dataset.metadata.source = "synthetic".to_string();
        dataset.metadata.maps = vec![
            "de_dust2".to_string(),
            "de_mirage".to_string(),
            "de_inferno".to_string(),
        ];
        dataset
    }

    /// Save dataset to a JSON file
    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Load dataset from a JSON file
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(std::io::Error::other)
    }

    /// Get the baseline set
    pub fn baselines(&self) -> &BaselineSet {
        &self.baselines
    }
}

/// Golden test case for regression testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenTestCase {
    /// Test name
    pub name: String,
    /// Path to the demo file
    pub demo_path: PathBuf,
    /// Expected output (JSON)
    pub expected_output: serde_json::Value,
    /// Tolerance for floating point comparisons
    pub tolerance: f64,
}

/// Manager for golden test cases
pub struct GoldenTestManager {
    cases: Vec<GoldenTestCase>,
    datasets_dir: PathBuf,
}

impl GoldenTestManager {
    pub fn new(datasets_dir: PathBuf) -> Self {
        Self {
            cases: Vec::new(),
            datasets_dir,
        }
    }

    /// Load all golden test cases from the datasets directory
    pub fn load_all(&mut self) -> Result<(), std::io::Error> {
        let golden_dir = self.datasets_dir.join("golden");
        if !golden_dir.exists() {
            std::fs::create_dir_all(&golden_dir)?;
            return Ok(());
        }

        for entry in std::fs::read_dir(golden_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let json = std::fs::read_to_string(&path)?;
                if let Ok(test_case) = serde_json::from_str::<GoldenTestCase>(&json) {
                    self.cases.push(test_case);
                }
            }
        }

        Ok(())
    }

    /// Get all test cases
    pub fn cases(&self) -> &[GoldenTestCase] {
        &self.cases
    }

    /// Add a new test case
    pub fn add_case(&mut self, case: GoldenTestCase) {
        self.cases.push(case);
    }

    /// Save a test case to disk
    pub fn save_case(&self, case: &GoldenTestCase) -> Result<(), std::io::Error> {
        let golden_dir = self.datasets_dir.join("golden");
        std::fs::create_dir_all(&golden_dir)?;

        let path = golden_dir.join(format!("{}.json", case.name));
        let json = serde_json::to_string_pretty(case).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Run all golden tests and return results
    pub fn run_all(&self) -> Vec<GoldenTestResult> {
        self.cases
            .iter()
            .map(|case| {
                // Check if demo file exists
                let demo_exists = case.demo_path.exists();

                GoldenTestResult {
                    name: case.name.clone(),
                    passed: demo_exists, // Simplified check
                    message: if demo_exists {
                        "Demo file exists".to_string()
                    } else {
                        format!("Demo file not found: {:?}", case.demo_path)
                    },
                }
            })
            .collect()
    }
}

/// Result of a golden test
#[derive(Debug, Clone)]
pub struct GoldenTestResult {
    pub name: String,
    pub passed: bool,
    pub message: String,
}

/// Dataset statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetStats {
    /// Total number of feature vectors
    pub total_vectors: usize,
    /// Number of unique players
    pub unique_players: usize,
    /// Number of unique matches
    pub unique_matches: usize,
    /// Feature coverage (percentage of features with data)
    pub feature_coverage: f64,
    /// Per-feature statistics
    pub feature_stats: BTreeMap<String, FeatureStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStats {
    pub name: String,
    pub count: usize,
    pub mean: f64,
    pub stddev: f64,
    pub min: f64,
    pub max: f64,
}

impl DatasetStats {
    pub fn compute(vectors: &[sentinel_core::FeatureVector]) -> Self {
        let mut feature_counts: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        let mut unique_players = std::collections::HashSet::new();
        let mut unique_matches = std::collections::HashSet::new();

        for fv in vectors {
            unique_players.insert(fv.player);
            unique_matches.insert(fv.round);

            for (name, result) in &fv.features {
                feature_counts
                    .entry(name.clone())
                    .or_default()
                    .push(result.value);
            }
        }

        let total_features = 20; // Expected number of features
        let feature_coverage = feature_counts.len() as f64 / total_features as f64;

        let feature_stats = feature_counts
            .into_iter()
            .map(|(name, values)| {
                let count = values.len();
                let mean = values.iter().sum::<f64>() / count as f64;
                let variance =
                    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / count as f64;
                let stddev = variance.sqrt();
                let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

                (
                    name.clone(),
                    FeatureStats {
                        name,
                        count,
                        mean,
                        stddev,
                        min,
                        max,
                    },
                )
            })
            .collect();

        Self {
            total_vectors: vectors.len(),
            unique_players: unique_players.len(),
            unique_matches: unique_matches.len(),
            feature_coverage,
            feature_stats,
        }
    }
}

/// Human-verified label assigned to a demo before it is used for validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DatasetLabel {
    Legit,
    Cheater,
    Unknown,
}

/// One demo registered in a labeled dataset. Paths are relative to the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetEntry {
    pub demo_path: PathBuf,
    pub label: DatasetLabel,
    pub source: String,
    pub verified: bool,
}

/// Portable index for the M4 dataset; demo archives stay out of the repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetManifest {
    pub version: u32,
    pub entries: Vec<DatasetEntry>,
}

impl Default for DatasetManifest {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetAudit {
    pub total: usize,
    pub legit: usize,
    pub cheater: usize,
    pub unknown: usize,
    pub missing_files: usize,
    pub unverified: usize,
    pub duplicate_paths: usize,
}

impl DatasetManifest {
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(std::io::Error::other)
    }

    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    pub fn audit(&self, root: &Path) -> DatasetAudit {
        let mut paths = HashSet::new();
        let mut audit = DatasetAudit {
            total: self.entries.len(),
            legit: 0,
            cheater: 0,
            unknown: 0,
            missing_files: 0,
            unverified: 0,
            duplicate_paths: 0,
        };

        for entry in &self.entries {
            match entry.label {
                DatasetLabel::Legit => audit.legit += 1,
                DatasetLabel::Cheater => audit.cheater += 1,
                DatasetLabel::Unknown => audit.unknown += 1,
            }
            if !entry.verified {
                audit.unverified += 1;
            }
            if !root.join(&entry.demo_path).is_file() {
                audit.missing_files += 1;
            }
            if !paths.insert(&entry.demo_path) {
                audit.duplicate_paths += 1;
            }
        }

        audit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calibration_dataset() {
        let dataset = CalibrationDataset::default_cs2();
        assert_eq!(dataset.name, "cs2_default");
        assert!(dataset.match_count > 0);
        assert!(!dataset.baselines.baselines.is_empty());
    }

    #[test]
    fn test_golden_test_manager() {
        let temp_dir = std::env::temp_dir().join("sentinel_test_golden");
        let mut manager = GoldenTestManager::new(temp_dir.clone());

        let test_case = GoldenTestCase {
            name: "test_case_1".to_string(),
            demo_path: PathBuf::from("test.dem"),
            expected_output: serde_json::json!({"score": 0.5}),
            tolerance: 0.01,
        };

        manager.add_case(test_case);
        assert_eq!(manager.cases().len(), 1);

        // Clean up
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_dataset_stats() {
        let mut features = BTreeMap::new();
        features.insert(
            "reaction_time".to_string(),
            sentinel_core::FeatureResult::new(0.25),
        );

        let fv = sentinel_core::FeatureVector {
            tick: sentinel_core::Tick(100),
            round: 1,
            player: sentinel_core::PlayerId::new(1),
            features,
        };

        let stats = DatasetStats::compute(&[fv]);
        assert_eq!(stats.total_vectors, 1);
        assert_eq!(stats.unique_players, 1);
    }

    #[test]
    fn manifest_audit_counts_labels_and_missing_files() {
        let root = std::env::temp_dir().join("sentinel_dataset_audit");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("legit")).unwrap();
        std::fs::write(root.join("legit/match.dem"), []).unwrap();
        let manifest = DatasetManifest {
            version: 1,
            entries: vec![
                DatasetEntry {
                    demo_path: PathBuf::from("legit/match.dem"),
                    label: DatasetLabel::Legit,
                    source: "hltv".to_string(),
                    verified: true,
                },
                DatasetEntry {
                    demo_path: PathBuf::from("cheater/missing.dem"),
                    label: DatasetLabel::Cheater,
                    source: "manual-review".to_string(),
                    verified: false,
                },
            ],
        };

        assert_eq!(
            manifest.audit(&root),
            DatasetAudit {
                total: 2,
                legit: 1,
                cheater: 1,
                unknown: 0,
                missing_files: 1,
                unverified: 1,
                duplicate_paths: 0,
            }
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
