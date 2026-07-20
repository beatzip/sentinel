use std::collections::BTreeMap;

use sentinel_core::{Evidence, MatchContext};

/// Links evidence to contextual information for explainability
pub struct EvidenceLinker;

impl EvidenceLinker {
    /// Enrich evidence with additional context from the match
    pub fn link(evidence: &mut Evidence, ctx: &MatchContext) {
        // Add visibility context
        if let Some(state) = ctx.state_at(evidence.tick) {
            // Check if the player was visible at this tick
            if let Some(player) = state.players.iter().find(|p| p.id == evidence.player) {
                evidence
                    .metadata
                    .insert("player_alive".to_string(), player.alive.to_string());
                evidence
                    .metadata
                    .insert("player_health".to_string(), player.health.to_string());
                evidence
                    .metadata
                    .insert("player_weapon".to_string(), format!("{:?}", player.weapon));
            }

            // Add round context
            evidence.metadata.insert(
                "round_number".to_string(),
                state.round.round_number.to_string(),
            );
            evidence.metadata.insert(
                "round_clock".to_string(),
                format!("{:.1}", state.round.clock),
            );
            evidence
                .metadata
                .insert("t_score".to_string(), state.round.t_score.to_string());
            evidence
                .metadata
                .insert("ct_score".to_string(), state.round.ct_score.to_string());
        }

        // Add tick timestamp in seconds
        evidence.metadata.insert(
            "tick_seconds".to_string(),
            format!("{:.2}", evidence.tick.as_seconds()),
        );
    }

    /// Link all evidence in a collection
    pub fn link_all(evidence: &mut [Evidence], ctx: &MatchContext) {
        for ev in evidence.iter_mut() {
            Self::link(ev, ctx);
        }
    }

    /// Generate a human-readable explanation for evidence
    pub fn explain(evidence: &Evidence) -> String {
        let tick_seconds = evidence
            .metadata
            .get("tick_seconds")
            .map(|s| s.as_str())
            .unwrap_or("?");

        let round = evidence
            .metadata
            .get("round_number")
            .map(|s| s.as_str())
            .unwrap_or("?");

        let weapon = evidence
            .metadata
            .get("player_weapon")
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        format!(
            "Round {}, tick {} ({}s): {} score {:.2} - {} [weapon: {}]",
            round,
            evidence.tick.0,
            tick_seconds,
            evidence.feature,
            evidence.score,
            evidence.reason,
            weapon
        )
    }

    /// Generate a summary explanation for a player's evidence
    pub fn summarize_player_evidence(evidence: &[&Evidence]) -> String {
        if evidence.is_empty() {
            return "No anomalous behavior detected.".to_string();
        }

        // Group by feature
        let mut by_feature: BTreeMap<&str, Vec<&Evidence>> = BTreeMap::new();
        for ev in evidence {
            by_feature.entry(ev.feature.as_str()).or_default().push(ev);
        }

        let mut summary = format!("Found {} evidence items:\n", evidence.len());

        for (feature, items) in &by_feature {
            let avg_score = items.iter().map(|e| e.score).sum::<f64>() / items.len() as f64;
            summary.push_str(&format!(
                "  - {}: {} occurrences, avg score {:.2}\n",
                feature,
                items.len(),
                avg_score
            ));
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{Evidence, PlayerId, Tick};

    #[test]
    fn test_explain() {
        let mut evidence = Evidence::new(
            Tick(1000),
            5,
            PlayerId::new(123),
            "reaction_time",
            0.95,
            "Suspiciously fast reaction",
        );
        evidence
            .metadata
            .insert("tick_seconds".to_string(), "15.62".to_string());
        evidence
            .metadata
            .insert("round_number".to_string(), "5".to_string());
        evidence
            .metadata
            .insert("player_weapon".to_string(), "Rifle".to_string());

        let explanation = EvidenceLinker::explain(&evidence);
        assert!(explanation.contains("Round 5"));
        assert!(explanation.contains("reaction_time"));
    }

    #[test]
    fn test_summarize_empty() {
        let summary = EvidenceLinker::summarize_player_evidence(&[]);
        assert!(summary.contains("No anomalous behavior"));
    }
}
