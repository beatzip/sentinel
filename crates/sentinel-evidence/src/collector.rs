use std::collections::BTreeMap;

use sentinel_core::{Evidence, PlayerId, Tick};

/// Collects and manages evidence entries
pub struct EvidenceCollector {
    /// All collected evidence
    evidence: Vec<Evidence>,
    /// Evidence grouped by player
    by_player: BTreeMap<PlayerId, Vec<usize>>,
    /// Evidence grouped by feature
    by_feature: BTreeMap<String, Vec<usize>>,
}

impl EvidenceCollector {
    pub fn new() -> Self {
        Self {
            evidence: Vec::new(),
            by_player: BTreeMap::new(),
            by_feature: BTreeMap::new(),
        }
    }

    /// Add an evidence entry
    pub fn add(&mut self, evidence: Evidence) {
        let idx = self.evidence.len();

        // Index by player
        self.by_player.entry(evidence.player).or_default().push(idx);

        // Index by feature
        self.by_feature
            .entry(evidence.feature.clone())
            .or_default()
            .push(idx);

        self.evidence.push(evidence);
    }

    /// Get all evidence
    pub fn all(&self) -> &[Evidence] {
        &self.evidence
    }

    /// Get evidence for a specific player
    pub fn for_player(&self, player: PlayerId) -> Vec<&Evidence> {
        self.by_player
            .get(&player)
            .map(|indices| indices.iter().map(|&i| &self.evidence[i]).collect())
            .unwrap_or_default()
    }

    /// Get evidence for a specific feature
    pub fn for_feature(&self, feature: &str) -> Vec<&Evidence> {
        self.by_feature
            .get(feature)
            .map(|indices| indices.iter().map(|&i| &self.evidence[i]).collect())
            .unwrap_or_default()
    }

    /// Get evidence at a specific tick
    pub fn at_tick(&self, tick: Tick) -> Vec<&Evidence> {
        self.evidence.iter().filter(|e| e.tick == tick).collect()
    }

    /// Get significant evidence (above threshold)
    pub fn significant(&self, threshold: f64) -> Vec<&Evidence> {
        self.evidence
            .iter()
            .filter(|e| e.is_significant(threshold))
            .collect()
    }

    /// Get the total number of evidence entries
    pub fn len(&self) -> usize {
        self.evidence.len()
    }

    /// Check if there's any evidence
    pub fn is_empty(&self) -> bool {
        self.evidence.is_empty()
    }

    /// Get evidence grouped by player
    pub fn grouped_by_player(&self) -> BTreeMap<PlayerId, Vec<&Evidence>> {
        let mut result: BTreeMap<PlayerId, Vec<&Evidence>> = BTreeMap::new();

        for evidence in &self.evidence {
            result.entry(evidence.player).or_default().push(evidence);
        }

        result
    }

    /// Get the top N most anomalous evidence entries
    pub fn top_anomalous(&self, n: usize) -> Vec<&Evidence> {
        let mut sorted: Vec<&Evidence> = self.evidence.iter().collect();
        sorted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(n).collect()
    }
}

impl Default for EvidenceCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_evidence(tick: u32, player: u64, feature: &str, score: f64) -> Evidence {
        Evidence::new(
            Tick(tick),
            1,
            PlayerId::new(player),
            feature,
            score,
            "test reason",
        )
    }

    #[test]
    fn test_add_and_retrieve() {
        let mut collector = EvidenceCollector::new();
        collector.add(make_evidence(100, 1, "reaction_time", 0.9));
        collector.add(make_evidence(101, 2, "reaction_time", 0.8));

        assert_eq!(collector.len(), 2);

        let player1_evidence = collector.for_player(PlayerId::new(1));
        assert_eq!(player1_evidence.len(), 1);

        let player2_evidence = collector.for_player(PlayerId::new(2));
        assert_eq!(player2_evidence.len(), 1);
    }

    #[test]
    fn test_for_feature() {
        let mut collector = EvidenceCollector::new();
        collector.add(make_evidence(100, 1, "reaction_time", 0.9));
        collector.add(make_evidence(101, 1, "crosshair_error", 0.8));
        collector.add(make_evidence(102, 2, "reaction_time", 0.7));

        let reaction_evidence = collector.for_feature("reaction_time");
        assert_eq!(reaction_evidence.len(), 2);
    }

    #[test]
    fn test_top_anomalous() {
        let mut collector = EvidenceCollector::new();
        collector.add(make_evidence(100, 1, "feature_a", 0.5));
        collector.add(make_evidence(101, 1, "feature_b", 0.9));
        collector.add(make_evidence(102, 1, "feature_c", 0.7));

        let top = collector.top_anomalous(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].score, 0.9);
        assert_eq!(top[1].score, 0.7);
    }
}
