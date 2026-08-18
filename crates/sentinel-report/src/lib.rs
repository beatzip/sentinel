pub mod html;
pub mod json;
pub mod markdown;
pub mod replay;

use serde::{Deserialize, Serialize};

use sentinel_core::{BehaviorScore, Evidence};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RiskLevel {
    #[default]
    Clean,
    Low,
    Moderate,
    High,
    Extreme,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum VerdictStatus {
    #[default]
    InsufficientHistory,
    Tentative,
    Standard,
    Strong,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SupportingMatch {
    pub report_id: String,
    pub map_name: String,
    pub overall_score: f64,
    pub evidence_count: usize,
    pub flagged: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfidenceAssessment {
    pub level: RiskLevel,
    pub status: VerdictStatus,
    pub independent_signals: usize,
    pub matches_observed: usize,
    pub flagged_matches: usize,
    pub recurrence: f64,
    pub supporting_matches: Vec<SupportingMatch>,
}

impl ConfidenceAssessment {
    pub fn assess(
        score: &BehaviorScore,
        matches_observed: usize,
        flagged_matches: usize,
        supporting_matches: Vec<SupportingMatch>,
    ) -> Self {
        let level = match score.overall {
            value if value < 0.2 => RiskLevel::Clean,
            value if value < 0.4 => RiskLevel::Low,
            value if value < 0.6 => RiskLevel::Moderate,
            value if value < 0.8 => RiskLevel::High,
            _ => RiskLevel::Extreme,
        };
        let rule_signals = score
            .categories
            .iter()
            .filter(|(name, value)| {
                !name.starts_with("learned_") && **value >= 0.6 && *name != "overall"
            })
            .count();
        let xgboost = score
            .categories
            .get("learned_xgboost")
            .copied()
            .unwrap_or(0.0);
        let temporal = score
            .categories
            .get("learned_temporal")
            .copied()
            .unwrap_or(0.0);
        let model_signals = usize::from(xgboost >= 0.6) + usize::from(temporal >= 0.6);
        let history_signal = usize::from(matches_observed >= 3 && flagged_matches >= 2);
        let independent_signals = rule_signals + model_signals + history_signal;
        let disagreement = (xgboost - temporal).abs() >= 0.35 && xgboost > 0.0 && temporal > 0.0;
        let status = if matches_observed < 2 {
            VerdictStatus::InsufficientHistory
        } else if disagreement
            || (matches_observed >= 3 && score.overall >= 0.6 && independent_signals < 2)
        {
            VerdictStatus::Tentative
        } else if score.overall >= 0.6 && independent_signals >= 3 {
            VerdictStatus::Strong
        } else {
            VerdictStatus::Standard
        };
        Self {
            level,
            status,
            independent_signals,
            matches_observed,
            flagged_matches,
            recurrence: if matches_observed == 0 {
                0.0
            } else {
                flagged_matches as f64 / matches_observed as f64
            },
            supporting_matches,
        }
    }
}

/// Immutable versions and fingerprints used to produce a report.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalysisProvenance {
    pub engine_version: String,
    pub demo_parser_version: String,
    pub demo_fingerprint: String,
    pub map_asset_version: String,
    pub feature_schema_version: String,
    pub xgboost_artifact_version: String,
    pub transformer_artifact_version: String,
}

/// Whether a stored report should be recomputed under a newer analysis stack.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReanalysisStatus {
    pub required: bool,
    pub reasons: Vec<String>,
}

impl AnalysisProvenance {
    pub fn reanalysis_status(&self, current: &Self) -> ReanalysisStatus {
        let checks = [
            (
                "engine_version",
                &self.engine_version,
                &current.engine_version,
            ),
            (
                "demo_parser_version",
                &self.demo_parser_version,
                &current.demo_parser_version,
            ),
            (
                "map_asset_version",
                &self.map_asset_version,
                &current.map_asset_version,
            ),
            (
                "feature_schema_version",
                &self.feature_schema_version,
                &current.feature_schema_version,
            ),
            (
                "xgboost_artifact_version",
                &self.xgboost_artifact_version,
                &current.xgboost_artifact_version,
            ),
            (
                "transformer_artifact_version",
                &self.transformer_artifact_version,
                &current.transformer_artifact_version,
            ),
        ];
        let reasons = checks
            .into_iter()
            .filter(|(_, recorded, active)| !recorded.is_empty() && recorded != active)
            .map(|(field, _, _)| field.to_string())
            .collect::<Vec<_>>();
        ReanalysisStatus {
            required: !reasons.is_empty(),
            reasons,
        }
    }
}

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

/// Public combat details tied to one roster-resolved kill.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RosterKill {
    pub tick: u32,
    pub attacker_id: u64,
    pub attacker_name: String,
    pub victim_id: u64,
    pub victim_name: String,
    pub assist_id: Option<u64>,
    pub assist_name: Option<String>,
    pub weapon: String,
    pub headshot: bool,
    pub wallbang: bool,
    pub through_smoke: bool,
}

/// One explicitly observed characteristic of a death; this never infers player intent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeathFact {
    Headshot,
    Wallbang,
    ThroughSmoke,
    Assisted,
}

/// Deterministic explanation built only from one roster-resolved kill event.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeathExplanation {
    pub tick: u32,
    pub attacker_name: String,
    pub victim_name: String,
    pub summary: String,
    pub facts: Vec<DeathFact>,
}

impl DeathExplanation {
    pub fn from_kill(kill: &RosterKill) -> Self {
        let mut facts = Vec::new();
        if kill.headshot {
            facts.push(DeathFact::Headshot);
        }
        if kill.wallbang {
            facts.push(DeathFact::Wallbang);
        }
        if kill.through_smoke {
            facts.push(DeathFact::ThroughSmoke);
        }
        if kill.assist_name.is_some() {
            facts.push(DeathFact::Assisted);
        }
        let qualifiers = facts
            .iter()
            .map(|fact| match fact {
                DeathFact::Headshot => "headshot",
                DeathFact::Wallbang => "wallbang",
                DeathFact::ThroughSmoke => "through-smoke",
                DeathFact::Assisted => "assisted",
            })
            .collect::<Vec<_>>();
        let suffix = (!qualifiers.is_empty()).then(|| format!(" ({})", qualifiers.join(", ")));
        Self {
            tick: kill.tick,
            attacker_name: kill.attacker_name.clone(),
            victim_name: kill.victim_name.clone(),
            summary: format!(
                "{} eliminated {} with {}{}",
                kill.attacker_name,
                kill.victim_name,
                kill.weapon,
                suffix.unwrap_or_default()
            ),
            facts,
        }
    }
}

/// A terminal, roster-resolved duel. Timing is exact for the kill event; wider shot history is
/// intentionally absent until the demo pipeline exports it as verified data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Encounter {
    pub round_number: u32,
    pub start_tick: u32,
    pub end_tick: u32,
    pub attacker_id: u64,
    pub attacker_name: String,
    pub defender_id: u64,
    pub defender_name: String,
    pub weapon: String,
    pub outcome: String,
    pub death_facts: Vec<DeathFact>,
}

impl Encounter {
    pub fn from_kill(round_number: u32, kill: &RosterKill) -> Self {
        let explanation = DeathExplanation::from_kill(kill);
        Self {
            round_number,
            start_tick: kill.tick,
            end_tick: kill.tick,
            attacker_id: kill.attacker_id,
            attacker_name: kill.attacker_name.clone(),
            defender_id: kill.victim_id,
            defender_name: kill.victim_name.clone(),
            weapon: kill.weapon.clone(),
            outcome: "attacker_kill".to_string(),
            death_facts: explanation.facts,
        }
    }
}

/// Factual summary that links a round result to its roster-resolved deaths.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoundStory {
    pub headline: String,
    pub result: String,
    pub deaths: Vec<DeathExplanation>,
}

impl RoundStory {
    pub fn from_facts(
        round_number: u32,
        t_score: u32,
        ct_score: u32,
        winner: Option<&str>,
        end_reason: Option<&str>,
        bomb_result: Option<&str>,
        kills: &[RosterKill],
    ) -> Self {
        let winner = winner.unwrap_or("unresolved");
        let mut outcomes = end_reason
            .into_iter()
            .chain(bomb_result)
            .collect::<Vec<_>>();
        if outcomes.is_empty() {
            outcomes.push("result recorded");
        }
        Self {
            headline: format!("Round {round_number}: {winner} ({t_score}-{ct_score})"),
            result: outcomes.join(" · "),
            deaths: kills.iter().map(DeathExplanation::from_kill).collect(),
        }
    }
}

/// Factual round context retained when the demo exposes each field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoundContext {
    pub round_number: u32,
    pub start_tick: u32,
    pub end_tick: u32,
    pub t_score: u32,
    pub ct_score: u32,
    pub winner: Option<String>,
    pub end_reason: Option<String>,
    pub bomb_result: Option<String>,
    pub buy_matchup: Option<String>,
    pub t_survivors: usize,
    pub ct_survivors: usize,
    pub kills: Vec<RosterKill>,
    /// Terminal duel ledger built from roster-resolved kills in this round.
    #[serde(default)]
    pub encounters: Vec<Encounter>,
    /// Deterministic narrative generated exclusively from this round's factual fields.
    #[serde(default)]
    pub story: RoundStory,
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
    /// Confidence policy and account-level recurrence from local Sentinel history.
    #[serde(default)]
    pub confidence: ConfidenceAssessment,
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
    /// Round-by-round factual context and roster-resolved kill feed.
    #[serde(default)]
    pub rounds: Vec<RoundContext>,
    /// Versions and content fingerprints used by this analysis.
    #[serde(default)]
    pub provenance: AnalysisProvenance,
    /// Set when this report is compared to a newer active provenance.
    #[serde(default)]
    pub reanalysis: ReanalysisStatus,
}

impl MatchReport {
    pub fn new(metadata: MatchMetadata) -> Self {
        Self {
            version: "1.0.0".to_string(),
            algorithm_version: "1.0.0".to_string(),
            metadata,
            players: Vec::new(),
            overall_anomaly: 0.0,
            rounds: Vec::new(),
            provenance: AnalysisProvenance::default(),
            reanalysis: ReanalysisStatus::default(),
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

#[cfg(test)]
mod provenance_tests {
    use super::*;

    #[test]
    fn changed_map_asset_requires_reanalysis() {
        let recorded = AnalysisProvenance {
            map_asset_version: "dust2-a".into(),
            ..Default::default()
        };
        let current = AnalysisProvenance {
            map_asset_version: "dust2-b".into(),
            ..Default::default()
        };
        let status = recorded.reanalysis_status(&current);
        assert!(status.required);
        assert_eq!(status.reasons, vec!["map_asset_version"]);
    }

    #[test]
    fn disagreement_is_tentative_when_models_diverge() {
        let mut score = BehaviorScore::new();
        score.overall = 0.7;
        score.categories.insert("learned_xgboost".into(), 0.9);
        score.categories.insert("learned_temporal".into(), 0.2);
        assert_eq!(
            ConfidenceAssessment::assess(&score, 4, 2, vec![]).status,
            VerdictStatus::Tentative
        );
    }

    #[test]
    fn round_story_uses_only_observed_kill_facts() {
        let kill = RosterKill {
            tick: 128,
            attacker_name: "Alpha".into(),
            victim_name: "Bravo".into(),
            weapon: "ak47".into(),
            headshot: true,
            wallbang: true,
            ..Default::default()
        };
        let story = RoundStory::from_facts(3, 2, 1, Some("Terrorist"), None, None, &[kill]);
        assert_eq!(story.headline, "Round 3: Terrorist (2-1)");
        assert!(story.deaths[0].facts.contains(&DeathFact::Headshot));
        assert!(story.deaths[0].facts.contains(&DeathFact::Wallbang));
        assert!(!story.deaths[0].summary.contains("intent"));
    }
}
