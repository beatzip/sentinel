use crate::MatchReport;

/// JSON report generator
pub struct JsonReport;

impl JsonReport {
    /// Generate a JSON report from match data
    pub fn generate(report: &MatchReport) -> String {
        serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
    }

    /// Generate a compact JSON report (single line)
    pub fn generate_compact(report: &MatchReport) -> String {
        serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MatchMetadata, MatchReport, PlayerReport};
    use sentinel_core::BehaviorScore;

    #[test]
    fn test_json_generation() {
        let metadata = MatchMetadata {
            demo_path: "test.dem".to_string(),
            map_name: "de_dust2".to_string(),
            server_name: "Test Server".to_string(),
            total_rounds: 30,
            duration_seconds: 1800.0,
            tick_rate: 64,
        };

        let mut report = MatchReport::new(metadata);

        let player_report = PlayerReport {
            steam_id: 12345,
            name: "TestPlayer".to_string(),
            team: "Terrorist".to_string(),
            scores: BehaviorScore::new(),
            evidence: Vec::new(),
            summary: "No anomalies detected".to_string(),
        };

        report.add_player(player_report);

        let json = JsonReport::generate(&report);
        assert!(json.contains("de_dust2"));
        assert!(json.contains("TestPlayer"));
    }
}
