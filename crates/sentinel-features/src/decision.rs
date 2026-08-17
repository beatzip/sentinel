use crate::traits::FeatureExt;
use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Tick};

/// Trade kill timing: time between teammate death and trade kill
pub struct TradeKillTiming;

impl FeatureExt for TradeKillTiming {
    fn name(&self) -> &str {
        "trade_kill_timing"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, _player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(3.0),
        };
        let _observer = match state.players.iter().find(|p| p.id == _player) {
            Some(p) => p,
            None => return FeatureResult::new(3.0),
        };
        let window_start = tick.0.saturating_sub(5 * 64);
        let mut teammate_death_tick: Option<u32> = None;

        for t in (window_start..=tick.0).rev() {
            if let Some(state) = ctx.state_at(Tick(t)) {
                let dead_count = state.players.iter().filter(|pp| !pp.alive).count();
                if dead_count > 0 && teammate_death_tick.is_none() {
                    teammate_death_tick = Some(t);
                }
            }
        }

        if let Some(death_tick) = teammate_death_tick {
            let timing = (tick.0 as f64 - death_tick as f64) / 64.0;
            FeatureResult::new(timing.clamp(0.0, 10.0))
        } else {
            FeatureResult::new(3.0)
        }
    }
}

/// Rotation speed: time to rotate to help teammate
pub struct RotationSpeed;

impl FeatureExt for RotationSpeed {
    fn name(&self) -> &str {
        "rotation_speed"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let lookback = tick.0.saturating_sub(2 * 64);
        let curr = ctx
            .state_at(tick)
            .and_then(|s| s.players.iter().find(|p| p.id == player));
        let prev = ctx
            .state_at(Tick(lookback))
            .and_then(|s| s.players.iter().find(|p| p.id == player));

        if let (Some(c), Some(p)) = (curr, prev) {
            let distance_moved = p.position.distance_to(&c.position) as f64;
            let speed = distance_moved / 2.0;
            let rotation_time = if speed > 200.0 {
                2.0
            } else if speed > 100.0 {
                4.0
            } else {
                6.0
            };
            FeatureResult::new(rotation_time)
        } else {
            FeatureResult::new(5.0)
        }
    }
}

/// Solo Playstyle Index: how isolated does this player play?
/// High score = player rarely stays near teammates
/// This should REDUCE the anomaly score for rotation_justification
pub struct SoloPlaystyleIndex;

impl FeatureExt for SoloPlaystyleIndex {
    fn name(&self) -> &str {
        "solo_playstyle_index"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.5),
        };

        let observer = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(0.5),
        };

        let my_team = observer.team;
        if my_team == sentinel_core::Team::Unassigned {
            return FeatureResult::new(0.5);
        }

        let teammates: Vec<_> = state
            .players
            .iter()
            .filter(|p| p.team == my_team && p.id != player && p.alive)
            .collect();

        if teammates.is_empty() {
            return FeatureResult::new(1.0);
        }

        let avg_distance: f64 = teammates
            .iter()
            .map(|t| observer.position.distance_to(&t.position) as f64)
            .sum::<f64>()
            / teammates.len() as f64;

        let near_teammate_count = teammates
            .iter()
            .filter(|t| observer.position.distance_to(&t.position) < 1000.0)
            .count();

        let isolation_ratio = 1.0 - (near_teammate_count as f64 / teammates.len() as f64);

        let alive_teammates = state
            .players
            .iter()
            .filter(|p| p.team == my_team && p.id != player && p.alive)
            .count();
        let solo_alive = if alive_teammates == 0 { 1.0 } else { 0.0 };

        let solo_score =
            (isolation_ratio * 0.5 + (avg_distance / 3000.0).min(1.0) * 0.3 + solo_alive * 0.2)
                .min(1.0);

        FeatureResult::new(solo_score)
            .with_metadata("avg_distance".to_string(), format!("{avg_distance:.0}"))
            .with_metadata(
                "isolation_ratio".to_string(),
                format!("{isolation_ratio:.2}"),
            )
            .with_metadata("alive_teammates".to_string(), alive_teammates.to_string())
    }
}

/// Team Proximity Score: how close does player stay to teammates?
pub struct TeamProximityScore;

impl FeatureExt for TeamProximityScore {
    fn name(&self) -> &str {
        "team_proximity_score"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.5),
        };

        let observer = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(0.5),
        };

        let my_team = observer.team;
        if my_team == sentinel_core::Team::Unassigned {
            return FeatureResult::new(0.5);
        }

        let teammates: Vec<_> = state
            .players
            .iter()
            .filter(|p| p.team == my_team && p.id != player && p.alive)
            .collect();

        if teammates.is_empty() {
            return FeatureResult::new(0.5);
        }

        let avg_distance: f64 = teammates
            .iter()
            .map(|t| observer.position.distance_to(&t.position) as f64)
            .sum::<f64>()
            / teammates.len() as f64;

        let proximity = (1.0 - (avg_distance / 2000.0).min(1.0)).max(0.0);

        FeatureResult::new(proximity)
            .with_metadata("avg_distance".to_string(), format!("{avg_distance:.0}"))
            .with_metadata("teammate_count".to_string(), teammates.len().to_string())
    }
}

/// Trade Kill Participation: does player trade kills with teammates?
pub struct TradeKillParticipation;

impl FeatureExt for TradeKillParticipation {
    fn name(&self) -> &str {
        "trade_kill_participation"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, _player: PlayerId) -> FeatureResult {
        let window_start = tick.0.saturating_sub(10 * 64);
        let window_states = ctx.states_in_range(Tick(window_start), tick);

        if window_states.is_empty() {
            return FeatureResult::new(0.5);
        }

        let mut teammate_deaths = 0;
        let trades = 0;

        for state in window_states {
            for p in &state.players {
                if p.team == sentinel_core::Team::Unassigned || p.id == _player {
                    continue;
                }
                if !p.alive {
                    teammate_deaths += 1;
                }
            }
        }

        let participation = if teammate_deaths > 0 {
            trades as f64 / teammate_deaths as f64
        } else {
            0.5
        };

        FeatureResult::new(participation.min(1.0))
            .with_metadata("teammate_deaths".to_string(), teammate_deaths.to_string())
            .with_metadata("trades".to_string(), trades.to_string())
    }
}

/// Utility Support Rate: does player use utility to help teammates?
pub struct UtilitySupportRate;

impl FeatureExt for UtilitySupportRate {
    fn name(&self) -> &str {
        "utility_support_rate"
    }
    fn category(&self) -> FeatureCategory {
        FeatureCategory::Utility
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, _player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.3),
        };

        let _player_ref = match state.players.iter().find(|p| p.id == _player) {
            Some(p) => p,
            None => return FeatureResult::new(0.3),
        };

        let window_start = tick.0.saturating_sub(30 * 64);
        let window_states = ctx.states_in_range(Tick(window_start), tick);

        if window_states.is_empty() {
            return FeatureResult::new(0.3);
        }

        let flash_assists: usize = window_states
            .iter()
            .filter(|s| {
                s.grenades.iter().any(|g| {
                    g.grenade_type == sentinel_core::GrenadeType::Flash
                        && g.detonated_tick.is_some()
                        && g.owner == Some(_player)
                })
            })
            .count();

        let support_rate = (flash_assists as f64 * 0.1).min(1.0);

        FeatureResult::new(support_rate)
            .with_metadata("flash_assists".to_string(), flash_assists.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_features() {
        assert_eq!(TradeKillTiming.name(), "trade_kill_timing");
        assert_eq!(RotationSpeed.name(), "rotation_speed");
        assert_eq!(SoloPlaystyleIndex.name(), "solo_playstyle_index");
        assert_eq!(TeamProximityScore.name(), "team_proximity_score");
        assert_eq!(TradeKillParticipation.name(), "trade_kill_participation");
        assert_eq!(UtilitySupportRate.name(), "utility_support_rate");
        assert_eq!(SoloPlaystyleIndex.category(), FeatureCategory::Decision);
    }
}
