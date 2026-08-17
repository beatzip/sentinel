pub mod html;
pub mod json;
pub mod markdown;
pub mod replay;

use serde::{Deserialize, Serialize};

use sentinel_core::{BehaviorScore, Evidence};

/// Match metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchMetadata {
    /// Demo file path
    pub demo_path: String,
    /// Map name
    pub map_name: String,
    /// Server name
    pub server_name: String,
    /// Total rounds
    pub total_rounds: u32,
    /// Match duration in seconds
    pub duration_seconds: f64,
    /// Tick rate
    pub tick_rate: u32,
}

/// Player report data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerReport {
    /// Steam ID
    pub steam_id: u64,
    /// Player name
    pub name: String,
    /// Team
    pub team: String,
    /// Behavior scores
    pub scores: BehaviorScore,
    /// Evidence of anomalous behavior
    pub evidence: Vec<Evidence>,
    /// Human-readable summary
    pub summary: String,
}

/// Complete match report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReport {
    /// Report version
    pub version: String,
    /// Algorithm version used
    pub algorithm_version: String,
    /// Match metadata
    pub metadata: MatchMetadata,
    /// Player reports
    pub players: Vec<PlayerReport>,
    /// Overall match anomaly score
    pub overall_anomaly: f64,
}

impl MatchReport {
    pub fn new(metadata: MatchMetadata) -> Self {
        Self {
            version: "1.0.0".to_string(),
            algorithm_version: "1.0.0".to_string(),
            metadata,
            players: Vec::new(),
            overall_anomaly: 0.0,
        }
    }

    /// Add a player report
    pub fn add_player(&mut self, report: PlayerReport) {
        self.players.push(report);
        self.compute_overall_anomaly();
    }

    /// Compute overall anomaly score as average of player scores
    fn compute_overall_anomaly(&mut self) {
        if self.players.is_empty() {
            self.overall_anomaly = 0.0;
            return;
        }

        self.overall_anomaly =
            self.players.iter().map(|p| p.scores.overall).sum::<f64>() / self.players.len() as f64;
    }

    /// Get the most suspicious players
    pub fn most_suspicious(&self, n: usize) -> Vec<&PlayerReport> {
        let mut sorted = self.players.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| {
            b.scores
                .overall
                .partial_cmp(&a.scores.overall)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(n).collect()
    }
}
