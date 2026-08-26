use serde::{Deserialize, Serialize};

use super::player::PlayerId;
use super::tick::Tick;

/// Grenade type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GrenadeType {
    Flash,
    Smoke,
    HE,
    Molotov,
    Incendiary,
    /// Observed fire effect where the source does not establish Molotov versus Incendiary.
    Inferno,
    Decoy,
}

/// State of a grenade or observed grenade effect in the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrenadeState {
    pub id: u32,
    pub grenade_type: GrenadeType,
    pub owner: Option<PlayerId>,
    pub position: super::player::Vec3,
    pub velocity: super::player::Vec3,
    /// Tick when a grenade throw was observed; absent for effect-only records.
    pub thrown_tick: Option<Tick>,
    /// Tick when a grenade detonated or an effect started.
    pub detonated_tick: Option<Tick>,
    pub start_tick: Option<Tick>,
    pub end_tick: Option<Tick>,
    /// Entity identifier from the demo for exact lifecycle pairing.
    pub entity_id: Option<u32>,
    pub active: bool,
    /// Observed effect state with no throw, trajectory, model, or collision telemetry.
    pub observed_effect_only: bool,
}

impl GrenadeState {
    pub fn time_since_thrown(&self, current_tick: Tick) -> Option<f32> {
        self.thrown_tick
            .map(|thrown_tick| (current_tick.0 - thrown_tick.0) as f32 / 64.0)
    }

    /// Time remaining until the effect expires in seconds.
    pub fn time_remaining(&self, current_tick: Tick) -> Option<f32> {
        self.end_tick
            .map(|end| (end.0 as f32 - current_tick.0 as f32) / 64.0)
    }

    pub fn is_timed(&self) -> bool {
        matches!(
            self.grenade_type,
            GrenadeType::Smoke
                | GrenadeType::Molotov
                | GrenadeType::Incendiary
                | GrenadeType::Inferno
        )
    }

    pub fn is_active_at(&self, current_tick: Tick) -> bool {
        if !self.active {
            return false;
        }
        if let Some(start) = self.start_tick
            && current_tick.0 < start.0
        {
            return false;
        }
        if let Some(end) = self.end_tick
            && current_tick.0 >= end.0
        {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::player::Vec3;
    use super::*;

    fn known_state(grenade_type: GrenadeType) -> GrenadeState {
        GrenadeState {
            id: 1,
            grenade_type,
            owner: None,
            position: Vec3::default(),
            velocity: Vec3::default(),
            thrown_tick: Some(Tick(100)),
            detonated_tick: Some(Tick(101)),
            start_tick: Some(Tick(101)),
            end_tick: Some(Tick(600)),
            entity_id: Some(42),
            active: true,
            observed_effect_only: false,
        }
    }

    #[test]
    fn test_timed_types_and_active_window() {
        assert!(known_state(GrenadeType::Smoke).is_timed());
        assert!(known_state(GrenadeType::Molotov).is_timed());
        assert!(known_state(GrenadeType::Incendiary).is_timed());
        assert!(known_state(GrenadeType::Inferno).is_timed());
        assert!(!known_state(GrenadeType::Flash).is_timed());

        let smoke = known_state(GrenadeType::Smoke);
        assert!(!smoke.is_active_at(Tick(100)));
        assert!(smoke.is_active_at(Tick(300)));
        assert!(!smoke.is_active_at(Tick(600)));
    }

    #[test]
    fn test_effect_only_state_keeps_unknown_throw_time() {
        let mut effect = known_state(GrenadeType::Inferno);
        effect.thrown_tick = None;
        effect.observed_effect_only = true;
        assert!(effect.time_since_thrown(Tick(300)).is_none());
        assert!(effect.is_active_at(Tick(300)));
    }

    #[test]
    fn test_timing_helpers() {
        let state = known_state(GrenadeType::Smoke);
        assert!((state.time_since_thrown(Tick(300)).unwrap() - 3.125).abs() < 0.001);
        assert!((state.time_remaining(Tick(300)).unwrap() - 4.6875).abs() < 0.001);
    }
}
