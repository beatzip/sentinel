use crate::traits::FeatureExt;
use sentinel_core::{FeatureCategory, FeatureResult, MatchContext, PlayerId, Tick};

fn angular_delta_degrees(current: f32, previous: f32) -> f64 {
    (f64::from(current) - f64::from(previous) + 180.0).rem_euclid(360.0) - 180.0
}

fn unavailable(reason: &'static str) -> FeatureResult {
    FeatureResult::new(0.0)
        .with_confidence(0.0)
        .with_metadata("availability", reason)
}

/// Reaction time requires a visibility onset linked to an observed shot.
pub struct ReactionTime;

impl FeatureExt for ReactionTime {
    fn name(&self) -> &str {
        "reaction_time"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Aim
    }

    fn compute(&self, _ctx: &MatchContext, _tick: Tick, _player: PlayerId) -> FeatureResult {
        unavailable("requires_visibility_to_shot_linkage")
    }
}

/// Head placement cannot be inferred from player origins without hitbox or bone telemetry.
pub struct CrosshairPlacementError;

impl FeatureExt for CrosshairPlacementError {
    fn name(&self) -> &str {
        "crosshair_placement_error"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Aim
    }

    fn compute(&self, _ctx: &MatchContext, _tick: Tick, _player: PlayerId) -> FeatureResult {
        unavailable("requires_head_hitboxes")
    }
}

pub struct AimVelocity;

impl FeatureExt for AimVelocity {
    fn name(&self) -> &str {
        "aim_velocity"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Aim
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let current = ctx.state_at(tick).and_then(|state| {
            state
                .players
                .iter()
                .find(|candidate| candidate.id == player)
        });
        let previous_state = ctx.state_before(tick);

        let velocity = match (current, previous_state) {
            (Some(current), Some(previous_state)) => {
                let Some(previous) = previous_state
                    .players
                    .iter()
                    .find(|candidate| candidate.id == player)
                else {
                    return unavailable("missing_history");
                };
                let tick_delta = tick.0.saturating_sub(previous_state.tick.0);
                if tick_delta == 0
                    || (current.view_angles.pitch == 0.0
                        && current.view_angles.yaw == 0.0
                        && previous.view_angles.pitch == 0.0
                        && previous.view_angles.yaw == 0.0)
                {
                    return unavailable("unavailable_angles");
                }
                let yaw_delta =
                    angular_delta_degrees(current.view_angles.yaw, previous.view_angles.yaw);
                let pitch_delta =
                    angular_delta_degrees(current.view_angles.pitch, previous.view_angles.pitch);
                (yaw_delta.abs() + pitch_delta.abs()) * 64.0 / f64::from(tick_delta)
            }
            _ => return unavailable("missing_history"),
        };
        FeatureResult::new(velocity)
    }
}

pub struct TrackingSmoothness;

impl FeatureExt for TrackingSmoothness {
    fn name(&self) -> &str {
        "tracking_smoothness"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Aim
    }

    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        let mut velocities = Vec::new();
        let start_tick = tick.0.saturating_sub(64);

        for current_tick in start_tick.saturating_add(1)..=tick.0 {
            let current = ctx.state_at(Tick(current_tick)).and_then(|state| {
                state
                    .players
                    .iter()
                    .find(|candidate| candidate.id == player)
            });
            let previous = ctx.state_before(Tick(current_tick)).and_then(|state| {
                state
                    .players
                    .iter()
                    .find(|candidate| candidate.id == player)
            });
            if let (Some(current), Some(previous)) = (current, previous)
                && (current.view_angles.pitch != 0.0
                    || current.view_angles.yaw != 0.0
                    || previous.view_angles.pitch != 0.0
                    || previous.view_angles.yaw != 0.0)
            {
                let yaw_delta =
                    angular_delta_degrees(current.view_angles.yaw, previous.view_angles.yaw);
                let pitch_delta =
                    angular_delta_degrees(current.view_angles.pitch, previous.view_angles.pitch);
                velocities.push(yaw_delta.abs() + pitch_delta.abs());
            }
        }

        if velocities.len() < 2 {
            return unavailable("missing_angle_history");
        }

        let mean = velocities.iter().sum::<f64>() / velocities.len() as f64;
        let variance = velocities
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / velocities.len() as f64;
        FeatureResult::new((1.0 - variance.sqrt() / 5.0).clamp(0.0, 1.0))
    }
}

/// Target switch speed needs observed target identities at fire or damage time.
pub struct TargetSwitchSpeed;

impl FeatureExt for TargetSwitchSpeed {
    fn name(&self) -> &str {
        "target_switch_speed"
    }

    fn category(&self) -> FeatureCategory {
        FeatureCategory::Aim
    }

    fn compute(&self, _ctx: &MatchContext, _tick: Tick, _player: PlayerId) -> FeatureResult {
        unavailable("requires_target_switch_identity")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_names() {
        assert_eq!(ReactionTime.name(), "reaction_time");
        assert_eq!(CrosshairPlacementError.name(), "crosshair_placement_error");
        assert_eq!(AimVelocity.name(), "aim_velocity");
        assert_eq!(TrackingSmoothness.name(), "tracking_smoothness");
        assert_eq!(TargetSwitchSpeed.name(), "target_switch_speed");
    }

    #[test]
    fn yaw_delta_wraps_across_zero() {
        assert!((angular_delta_degrees(1.0, 359.0) - 2.0).abs() < 0.001);
        assert!((angular_delta_degrees(359.0, 1.0) + 2.0).abs() < 0.001);
    }
}
