use std::collections::BTreeMap;

use sentinel_core::FeatureVector;

/// Per-round summary of one feature, suitable for temporal and cross-round analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct TemporalProfile {
    pub round_count: usize,
    pub mean: f64,
    pub variance: f64,
    pub trend: f64,
}

/// Calculates a compact per-round profile without introducing a time-series dependency.
pub fn profile_feature(vectors: &[FeatureVector], feature: &str) -> Option<TemporalProfile> {
    let mut rounds: BTreeMap<u32, Vec<f64>> = BTreeMap::new();
    for vector in vectors {
        if let Some(result) = vector.features.get(feature) {
            rounds.entry(vector.round).or_default().push(result.value);
        }
    }
    let means = rounds
        .values()
        .map(|values| values.iter().sum::<f64>() / values.len() as f64)
        .collect::<Vec<_>>();
    if means.is_empty() {
        return None;
    }
    let mean = means.iter().sum::<f64>() / means.len() as f64;
    let variance = means
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / means.len() as f64;
    Some(TemporalProfile {
        round_count: means.len(),
        mean,
        variance,
        trend: means.last().unwrap() - means.first().unwrap(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn profile_tracks_cross_round_trend() {
        let vectors = [
            FeatureVector {
                tick: sentinel_core::Tick(1),
                round: 1,
                player: sentinel_core::PlayerId::new(1),
                features: BTreeMap::from([(
                    "aim".to_string(),
                    sentinel_core::FeatureResult::new(0.2),
                )]),
            },
            FeatureVector {
                tick: sentinel_core::Tick(2),
                round: 2,
                player: sentinel_core::PlayerId::new(1),
                features: BTreeMap::from([(
                    "aim".to_string(),
                    sentinel_core::FeatureResult::new(0.8),
                )]),
            },
        ];
        let profile = profile_feature(&vectors, "aim").unwrap();
        assert_eq!(profile.round_count, 2);
        assert!((profile.trend - 0.6).abs() < f64::EPSILON);
    }
}
