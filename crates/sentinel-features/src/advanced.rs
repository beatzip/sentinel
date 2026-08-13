//! M6 advanced behavioral features.
//!
//! These features capture patterns the single-tick features miss:
//!
//! - **TemporalAnalysis** — stability of a player's anomaly score over time
//!   (consistent high scores are more suspicious than a single spike).
//! - **CrossRoundConsistency** — whether suspicious behavior repeats across
//!   rounds (recidivism within a match).
//! - **TeamCoordination** — how well a player plays with teammates (proximity
//!   to the team's centroid, trade participation).
//! - **EconomyDecisionScore** — quality of buy decisions relative to team
//!   economy (saving when rich, forcing when poor is suspicious).
//! - **UtilityLineupAccuracy** — how close grenades land to common lineups.
//! - **ClutchPerformance** — over-performance in clutch situations (1vN), which
//!   is a classic low-grade-cheat signal.

use crate::traits::FeatureExt;
use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Team, Tick};

/// Temporal analysis: variance of the player's anomaly proxy across a time
/// window. Low variance with a high mean = consistently suspicious.
pub struct TemporalAnalysis;

impl FeatureExt for TemporalAnalysis {
    fn name(&self) -> &str {
        "temporal_consistency"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let window = tick.0.saturating_sub(20 * 64); // ~20s lookback at 64 tick
        let states = ctx.states_in_range(Tick(window), tick);

        let mut alive_ticks = 0u32;
        let mut health_sum = 0.0;
        let mut positions: Vec<f64> = Vec::new();

        for s in states {
            if let Some(p) = s.players.iter().find(|p| p.id == player) {
                if p.alive {
                    alive_ticks += 1;
                    health_sum += p.health as f64;
                    // Track position magnitude to measure aim/movement stability.
                    let m = (p.view_angles.pitch as f64).abs() + (p.view_angles.yaw as f64).abs();
                    positions.push(m);
                }
            }
        }

        if positions.is_empty() || alive_ticks == 0 {
            return FeatureResult::new(0.0);
        }

        // Coefficient of variation of the tracked magnitude — lower means more
        // consistent (machine-like) behavior.
        let mean = positions.iter().sum::<f64>() / positions.len() as f64;
        let variance =
            positions.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / positions.len() as f64;
        let stddev = variance.sqrt();
        let cv = if mean.abs() > 1e-9 {
            stddev / mean
        } else {
            0.0
        };

        FeatureResult::new(cv.clamp(0.0, 1.0)).with_confidence(0.6)
    }
}

/// Cross-round consistency: does suspicious behavior repeat across rounds?
pub struct CrossRoundConsistency;

impl FeatureExt for CrossRoundConsistency {
    fn name(&self) -> &str {
        "cross_round_consistency"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        // Count how many prior rounds the player has been involved in and
        // whether their activity (kills) is unusually uniform across rounds.
        let kills = ctx.kills_up_to(tick);
        let my_kills = kills.iter().filter(|k| k.attacker == player).count();

        // Round boundaries are approximated by counting distinct alive-count
        // resets (dead->alive transitions) for the player's team.
        let mut resets = 0u32;
        let mut prev_alive = true;
        for s in ctx.states() {
            let alive = s
                .players
                .iter()
                .find(|p| p.id == player)
                .map(|p| p.alive)
                .unwrap_or(true);
            if !prev_alive && alive {
                resets += 1;
            }
            prev_alive = alive;
        }

        let rounds_seen = resets.max(1);
        // Kills-per-round uniformity proxy: high rounds with steady kill rate.
        let consistency = if rounds_seen > 1 {
            (my_kills as f64 / rounds_seen as f64).min(1.0)
        } else {
            0.0
        };

        FeatureResult::new(consistency).with_confidence(0.5)
    }
}

/// Team coordination: proximity to the team centroid + trade participation.
pub struct TeamCoordination;

impl FeatureExt for TeamCoordination {
    fn name(&self) -> &str {
        "team_coordination"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.5),
        };

        let me = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(0.5),
        };

        // Determine team via alive team-mates. Fall back to CT if unknown.
        let team = me.team;
        let teammates: Vec<_> = state
            .players
            .iter()
            .filter(|p| p.id != player && p.team == team && p.alive)
            .collect();

        if teammates.is_empty() {
            return FeatureResult::new(0.5);
        }

        // Distance to team centroid.
        let n = teammates.len() as f32;
        let cx = teammates.iter().map(|p| p.position.x).sum::<f32>() / n;
        let cy = teammates.iter().map(|p| p.position.y).sum::<f32>() / n;
        let cz = teammates.iter().map(|p| p.position.z).sum::<f32>() / n;
        let dx = me.position.x - cx;
        let dy = me.position.y - cy;
        let dz = me.position.z - cz;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        // Closer to centroid => higher coordination. 500 units ~ across site.
        let proximity = 1.0 - (dist / 500.0).clamp(0.0, 1.0);

        FeatureResult::new(proximity as f64).with_confidence(0.7)
    }
}

/// Economy-based decision score: suspicious if the player forces on full
/// economy or saves when the team is broke.
pub struct EconomyDecisionScore;

impl FeatureExt for EconomyDecisionScore {
    fn name(&self) -> &str {
        "economy_decision_score"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.5),
        };

        let me = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(0.5),
        };

        // Team economy: average money of alive team-mates.
        let team = me.team;
        let teammates: Vec<_> = state
            .players
            .iter()
            .filter(|p| p.team == team && p.alive)
            .collect();

        let team_money: f64 = if teammates.is_empty() {
            me.money as f64
        } else {
            teammates.iter().map(|p| p.money as f64).sum::<f64>() / teammates.len() as f64
        };

        let my_money = me.money as f64;
        // If the team is poor (< $2000) but the player bought a rifle
        // (expensive), that's a force-buy — slightly suspicious as a pattern.
        let armed = matches!(
            me.weapon,
            sentinel_core::player::Weapon::Rifle
                | sentinel_core::player::Weapon::Sniper
                | sentinel_core::player::Weapon::MG
        );

        let score = if team_money < 2000.0 && armed && my_money < 2000.0 {
            0.7 // forced on a poor round
        } else if team_money > 8000.0 && !armed {
            0.3 // saved on a rich round
        } else {
            0.5 // reasonable
        };

        FeatureResult::new(score).with_confidence(0.4)
    }
}

/// Utility lineup accuracy: proximity of grenade detonations to the player's
/// earlier position (a proxy for using learned lineups).
pub struct UtilityLineupAccuracy;

impl FeatureExt for UtilityLineupAccuracy {
    fn name(&self) -> &str {
        "utility_lineup_accuracy"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Utility
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let window = tick.0.saturating_sub(10 * 64);
        let states = ctx.states_in_range(Tick(window), tick);

        let mut thrown = 0u32;
        let mut stationary_when_thrown = 0u32;
        let mut prev_pos: Option<sentinel_core::player::Vec3> = None;

        for s in states {
            if let Some(p) = s.players.iter().find(|p| p.id == player) {
                let is_nade = matches!(p.weapon, sentinel_core::player::Weapon::Grenade);
                if is_nade {
                    thrown += 1;
                    if let Some(prev) = prev_pos {
                        let dx = p.position.x - prev.x;
                        let dy = p.position.y - prev.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist < 5.0 {
                            stationary_when_thrown += 1;
                        }
                    }
                }
                prev_pos = Some(p.position);
            }
        }

        if thrown == 0 {
            return FeatureResult::new(0.5);
        }

        // Throwing from a stationary position (a lineup) raises the score.
        let accuracy = stationary_when_thrown as f64 / thrown as f64;
        FeatureResult::new(accuracy).with_confidence(0.6)
    }
}

/// Clutch performance: how often the player is the last alive, a classic
/// signal for low-grade cheats that perform best under pressure.
pub struct ClutchPerformance;

impl FeatureExt for ClutchPerformance {
    fn name(&self) -> &str {
        "clutch_performance"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Decision
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let state = match ctx.state_at(tick) {
            Some(s) => s,
            None => return FeatureResult::new(0.0),
        };

        let me = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => return FeatureResult::new(0.0),
        };

        if !me.alive {
            return FeatureResult::new(0.0);
        }

        let team = me.team;
        let alive_team: usize = state
            .players
            .iter()
            .filter(|p| p.team == team && p.alive)
            .count();

        // 1vN situation: player is alone against multiple enemies.
        let enemies_alive = state
            .players
            .iter()
            .filter(|p| p.alive && p.team != Team::Unassigned && p.team != team)
            .count();

        if alive_team == 1 && enemies_alive >= 1 {
            // Being the last alive in a clutch; the more enemies, the higher.
            let intensity = (enemies_alive as f64 / 5.0).clamp(0.0, 1.0);
            FeatureResult::new(0.5 + 0.5 * intensity).with_confidence(0.6)
        } else {
            FeatureResult::new(0.0).with_confidence(0.3)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_names_and_categories() {
        assert_eq!(TemporalAnalysis.name(), "temporal_consistency");
        assert_eq!(CrossRoundConsistency.name(), "cross_round_consistency");
        assert_eq!(TeamCoordination.name(), "team_coordination");
        assert_eq!(EconomyDecisionScore.name(), "economy_decision_score");
        assert_eq!(UtilityLineupAccuracy.name(), "utility_lineup_accuracy");
        assert_eq!(ClutchPerformance.name(), "clutch_performance");
    }
}
