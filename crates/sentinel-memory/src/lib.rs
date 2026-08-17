//! Persistent memory and self-learning baselines for Sentinel AI.
//!
//! `Memory` accumulates statistics from every analyzed demo:
//!
//! - **Learned baselines**: per-feature running mean/stddev over all observed
//!   players, updated incrementally with Welford's algorithm. These replace the
//!   hardcoded `default_cs2` baselines once enough samples have been seen,
//!   giving the scorer an empirical notion of "normal" play.
//! - **Player profiles**: per-SteamID aggregate feature values and anomaly
//!   history across matches. A player that consistently deviates from the
//!   learned baselines accumulates a `recidivism` signal, which raises their
//!   score — this is what lets Sentinel catch *marginal* cheaters who stay just
//!   under single-match thresholds.
//!
//! Everything is serialized to a single JSON file so the tool stays trivial to
//! launch: no database, no server.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use sentinel_analysis::BaselineSet;
use sentinel_analysis::baseline::FeatureBaseline;
use sentinel_core::{FeatureVector, PlayerId};

/// Minimum number of per-feature samples before learned baselines are trusted
/// over the hardcoded defaults.
pub const MIN_SAMPLES_FOR_LEARNED: usize = 50;
const MAX_SUPPORTING_MATCHES: usize = 20;

/// A locally analyzed match that supports an account-level recurrence read.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SupportingMatch {
    pub report_id: String,
    pub map_name: String,
    pub overall_score: f64,
    pub evidence_count: usize,
    pub flagged: bool,
}

/// Local-only account history derived from Sentinel evidence, never K/D or external profiles.
#[derive(Debug, Clone, Default)]
pub struct AccountHistory {
    pub matches_observed: usize,
    pub flagged_matches: usize,
    pub supporting_matches: Vec<SupportingMatch>,
}

/// Online accumulator for a single feature's statistics (Welford's algorithm).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureAccumulator {
    pub count: usize,
    pub mean: f64,
    pub m2: f64,
    pub min: f64,
    pub max: f64,
}

impl FeatureAccumulator {
    pub fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }

    /// Observe a new sample, updating running statistics.
    pub fn observe(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
        if value < self.min {
            self.min = value;
        }
        if value > self.max {
            self.max = value;
        }
    }

    /// Population standard deviation.
    pub fn stddev(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        (self.m2 / self.count as f64).sqrt()
    }

    /// Merge another accumulator into this one (parallel/streaming merge).
    pub fn merge(&mut self, other: &Self) {
        if other.count == 0 {
            return;
        }
        if self.count == 0 {
            *self = other.clone();
            return;
        }
        let n_a = self.count as f64;
        let n_b = other.count as f64;
        let n = n_a + n_b;
        let delta = other.mean - self.mean;
        self.mean += delta * n_b / n;
        self.m2 += other.m2 + delta * delta * n_a * n_b / n;
        self.count = n as usize;
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    /// Convert to a snapshot baseline.
    pub fn to_baseline(&self, name: &str) -> FeatureBaseline {
        let stddev = self.stddev();
        FeatureBaseline {
            name: name.to_string(),
            mean: self.mean,
            stddev,
            p95: self.mean + 1.645 * stddev,
            p99: self.mean + 2.326 * stddev,
            sample_count: self.count,
        }
    }
}

/// Per-player aggregate observations across matches.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerProfile {
    /// Number of matches this player has been observed in.
    pub matches_observed: usize,
    /// Running mean of each feature's per-match average value.
    pub feature_means: BTreeMap<String, FeatureAccumulator>,
    /// Running mean of the player's overall anomaly score per match.
    pub overall_score_acc: FeatureAccumulator,
    /// Number of matches where the player exceeded the evidence threshold.
    pub flagged_matches: usize,
    /// Sum of evidence items across all observed matches.
    pub total_evidence: usize,
    /// Recent locally analyzed matches with enough evidence to support recurrence review.
    #[serde(default)]
    pub supporting_matches: Vec<SupportingMatch>,
}

impl PlayerProfile {
    pub fn record_match(
        &mut self,
        report_id: &str,
        map_name: &str,
        overall_score: f64,
        evidence_count: usize,
        feature_averages: &BTreeMap<String, f64>,
        flagged: bool,
    ) {
        self.matches_observed += 1;
        self.overall_score_acc.observe(overall_score);
        self.total_evidence += evidence_count;
        if flagged {
            self.flagged_matches += 1;
        }
        if flagged || overall_score >= 0.5 {
            self.supporting_matches.push(SupportingMatch {
                report_id: report_id.to_string(),
                map_name: map_name.to_string(),
                overall_score,
                evidence_count,
                flagged,
            });
            self.supporting_matches.sort_by(|a, b| {
                b.overall_score
                    .partial_cmp(&a.overall_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            self.supporting_matches.truncate(MAX_SUPPORTING_MATCHES);
        }
        for (name, &value) in feature_averages {
            self.feature_means
                .entry(name.clone())
                .or_default()
                .observe(value);
        }
    }

    /// Recidivism index in [0, 1]: how consistently this player is anomalous
    /// across observed matches. 0 = never flagged, 1 = flagged every match.
    pub fn recidivism(&self) -> f64 {
        if self.matches_observed == 0 {
            return 0.0;
        }
        self.flagged_matches as f64 / self.matches_observed as f64
    }
}

/// A per-player result from analyzing one match, fed into the memory store.
#[derive(Debug, Clone)]
pub struct MatchObservation {
    /// Stable local report identifier.
    pub report_id: String,
    /// Map recorded by the analyzed demo.
    pub map_name: String,
    /// Player this observation is about.
    pub player: PlayerId,
    /// Overall anomaly score for the player in this match.
    pub overall_score: f64,
    /// Number of evidence items generated for the player.
    pub evidence_count: usize,
    /// Per-feature average values for the player in this match.
    pub feature_averages: BTreeMap<String, f64>,
    /// Whether the player exceeded the flag threshold in this match.
    pub flagged: bool,
}

/// The full persistent memory store.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Memory {
    /// Per-feature accumulators over ALL observed players (the "normal" baseline).
    pub global_baselines: BTreeMap<String, FeatureAccumulator>,
    /// Per-player profiles.
    pub players: BTreeMap<u64, PlayerProfile>,
    /// Number of demos analyzed into this memory.
    pub demos_analyzed: usize,
    /// Schema version for forward-compatible deserialization.
    pub version: String,
    /// When the memory was last updated (RFC3339).
    pub updated_at: String,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            ..Default::default()
        }
    }

    /// Default on-disk location: `sentinel_memory.json` in the current dir.
    pub fn default_path() -> PathBuf {
        PathBuf::from("sentinel_memory.json")
    }

    /// Load memory from a JSON file, returning an empty memory if the file
    /// does not exist (first run).
    pub fn load(path: &Path) -> Result<Self, MemoryError> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let json = std::fs::read_to_string(path)
            .map_err(|e| MemoryError::Io(format!("read {}: {}", path.display(), e)))?;
        let mut mem: Self = serde_json::from_str(&json)
            .map_err(|e| MemoryError::Corrupt(format!("parse {}: {}", path.display(), e)))?;
        if mem.version.is_empty() {
            mem.version = env!("CARGO_PKG_VERSION").to_string();
        }
        Ok(mem)
    }

    /// Save memory to a JSON file.
    pub fn save(&self, path: &Path) -> Result<(), MemoryError> {
        let mut copy = self.clone();
        copy.updated_at = chrono::Utc::now().to_rfc3339();
        let json = serde_json::to_string_pretty(&copy)
            .map_err(|e| MemoryError::Serialize(e.to_string()))?;
        std::fs::write(path, json)
            .map_err(|e| MemoryError::Io(format!("write {}: {}", path.display(), e)))?;
        Ok(())
    }

    /// Observe a set of feature vectors from a single match: update the global
    /// learned baselines and the per-player profiles.
    ///
    /// `per_player_results` maps player id to (overall score, evidence count,
    /// per-feature averages, whether the player was flagged). Pass `None` for
    /// the averages/flag if you only want to feed baselines.
    pub fn observe_match(
        &mut self,
        all_vectors: &[FeatureVector],
        per_player_results: &[MatchObservation],
    ) {
        self.demos_analyzed += 1;

        // Update global baselines from every observed feature value.
        for fv in all_vectors {
            for (name, result) in &fv.features {
                self.global_baselines
                    .entry(name.clone())
                    .or_default()
                    .observe(result.value);
            }
        }

        // Update player profiles.
        for obs in per_player_results {
            let profile = self.players.entry(obs.player.as_u64()).or_default();
            profile.record_match(
                &obs.report_id,
                &obs.map_name,
                obs.overall_score,
                obs.evidence_count,
                &obs.feature_averages,
                obs.flagged,
            );
        }
    }

    /// Account-level history drawn only from locally computed Sentinel evidence.
    pub fn account_history(&self, player: PlayerId) -> AccountHistory {
        let Some(profile) = self.players.get(&player.as_u64()) else {
            return AccountHistory::default();
        };
        AccountHistory {
            matches_observed: profile.matches_observed,
            flagged_matches: profile.flagged_matches,
            supporting_matches: profile.supporting_matches.clone(),
        }
    }

    /// Build a `BaselineSet` from learned statistics, falling back to the
    /// hardcoded CS2 defaults for any feature with too few samples.
    pub fn learned_baselines(&self) -> BaselineSet {
        let defaults = BaselineSet::default_cs2();
        let mut set = BaselineSet::new();
        for (name, acc) in &self.global_baselines {
            if acc.count >= MIN_SAMPLES_FOR_LEARNED {
                set.add(acc.to_baseline(name));
            } else if let Some(default) = defaults.get(name) {
                set.add(default.clone());
            }
        }
        // Ensure all default features are present even if unobserved.
        for (name, baseline) in &defaults.baselines {
            if !set.baselines.contains_key(name) {
                set.add(baseline.clone());
            }
        }
        set
    }

    /// Whether the memory has enough data to trust learned baselines.
    pub fn has_learned(&self) -> bool {
        self.global_baselines
            .values()
            .any(|acc| acc.count >= MIN_SAMPLES_FOR_LEARNED)
    }

    /// Recidivism-based score adjustment in [-0.2, +0.2].
    ///
    /// Players seen as anomalous across multiple past matches get a boost;
    /// players with a clean history get a small discount. This is what makes
    /// marginal cheaters — who dodge single-match thresholds — detectable over
    /// time.
    pub fn recidivism_adjustment(&self, player: PlayerId) -> f64 {
        let Some(profile) = self.players.get(&player.as_u64()) else {
            return 0.0;
        };
        if profile.matches_observed < 2 {
            return 0.0;
        }
        let r = profile.recidivism();
        // r in [0,1]: map to adjustment in [-0.1, +0.2].
        // Clean history (r=0) -> -0.1 (more trust), chronic (r=1) -> +0.2.
        (r * 0.3) - 0.1
    }

    /// Human-readable summary for the CLI `memory` command.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("Demos analyzed: {}\n", self.demos_analyzed));
        s.push_str(&format!("Players tracked: {}\n", self.players.len()));
        let learned = self
            .global_baselines
            .iter()
            .filter(|(_, a)| a.count >= MIN_SAMPLES_FOR_LEARNED)
            .count();
        s.push_str(&format!(
            "Features with learned baselines: {}/{}\n",
            learned,
            self.global_baselines.len()
        ));
        s.push_str(&format!("Memory version: {}\n", self.version));
        s.push_str(&format!("Last updated: {}", self.updated_at));

        // Top recurring suspects.
        let mut suspects: Vec<_> = self
            .players
            .iter()
            .filter(|(_, p)| p.matches_observed >= 2)
            .collect();
        suspects.sort_by(|a, b| {
            b.1.recidivism()
                .partial_cmp(&a.1.recidivism())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if !suspects.is_empty() {
            s.push_str("\nRecurring suspects (recidivism):\n");
            for (id, p) in suspects.iter().take(10) {
                s.push_str(&format!(
                    "  Player {}: {} matches, {} flagged, recidivism {:.2}\n",
                    id,
                    p.matches_observed,
                    p.flagged_matches,
                    p.recidivism()
                ));
            }
        }
        s
    }
}

/// Errors produced by the memory store.
#[derive(Debug, Clone)]
pub enum MemoryError {
    Io(String),
    Corrupt(String),
    Serialize(String),
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::Io(m) => write!(f, "memory io: {m}"),
            MemoryError::Corrupt(m) => write!(f, "memory corrupt: {m}"),
            MemoryError::Serialize(m) => write!(f, "memory serialize: {m}"),
        }
    }
}

impl std::error::Error for MemoryError {}

impl sentinel_analysis::MemoryAdapter for Memory {
    fn recidivism_adjustment(&self, player: sentinel_core::PlayerId) -> f64 {
        Memory::recidivism_adjustment(self, player)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{FeatureResult, Tick};
    use std::collections::BTreeMap;

    fn fv(player: u64, value: f64) -> FeatureVector {
        let mut features = BTreeMap::new();
        features.insert("reaction_time".to_string(), FeatureResult::new(value));
        FeatureVector {
            tick: Tick(1),
            round: 1,
            player: PlayerId::new(player),
            features,
        }
    }

    #[test]
    fn welford_accumulator_tracks_mean_stddev() {
        let mut acc = FeatureAccumulator::new();
        for v in [10.0, 12.0, 9.0, 11.0, 8.0] {
            acc.observe(v);
        }
        assert!((acc.mean - 10.0).abs() < 1e-9);
        // population stddev of those 5 values is ~1.4142
        assert!((acc.stddev() - 2.0_f64.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn observing_updates_baselines_and_profiles() {
        let mut mem = Memory::new();
        let vectors: Vec<_> = (0..60)
            .map(|i| fv(i % 10, 0.25 + i as f64 * 0.001))
            .collect();
        let results = vec![MatchObservation {
            report_id: "fixture".into(),
            map_name: "de_dust2".into(),
            player: PlayerId::new(1),
            overall_score: 0.8,
            evidence_count: 3,
            feature_averages: {
                let mut m = BTreeMap::new();
                m.insert("reaction_time".to_string(), 0.26);
                m
            },
            flagged: true,
        }];
        mem.observe_match(&vectors, &results);

        assert_eq!(mem.demos_analyzed, 1);
        assert!(mem.has_learned());
        let profile = mem.players.get(&1).unwrap();
        assert_eq!(profile.matches_observed, 1);
        assert!(profile.flagged_matches == 1);
        assert_eq!(profile.supporting_matches.len(), 1);
        assert!((profile.recidivism() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn recidivism_adjustment_is_neutral_for_new_players() {
        let mem = Memory::new();
        assert_eq!(mem.recidivism_adjustment(PlayerId::new(999)), 0.0);
    }

    #[test]
    fn learned_baselines_fall_back_to_defaults_for_sparse_features() {
        let mut mem = Memory::new();
        // one sample — below MIN_SAMPLES_FOR_LEARNED
        mem.observe_match(&[fv(1, 0.25)], &[]);
        let set = mem.learned_baselines();
        // reaction_time should fall back to default
        assert!(set.get("reaction_time").is_some());
        assert!(set.get("aim_velocity").is_some()); // default present
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("sentinel_mem_test");
        let path = dir.join("mem.json");
        std::fs::create_dir_all(&dir).unwrap();
        let mut mem = Memory::new();
        mem.observe_match(&[fv(1, 0.25)], &[]);
        mem.save(&path).unwrap();
        let loaded = Memory::load(&path).unwrap();
        assert_eq!(loaded.demos_analyzed, 1);
        assert!(loaded.global_baselines.contains_key("reaction_time"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn account_history_links_back_to_local_reports() {
        let mut mem = Memory::new();
        mem.observe_match(
            &[],
            &[MatchObservation {
                report_id: "match-1".into(),
                map_name: "de_mirage".into(),
                player: PlayerId::new(7),
                overall_score: 0.8,
                evidence_count: 3,
                feature_averages: BTreeMap::new(),
                flagged: true,
            }],
        );
        let history = mem.account_history(PlayerId::new(7));
        assert_eq!(history.matches_observed, 1);
        assert_eq!(history.supporting_matches[0].report_id, "match-1");
    }
}
