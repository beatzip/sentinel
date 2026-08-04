use serde::{Deserialize, Serialize};

use super::player::PlayerId;
use super::tick::Tick;

/// Grenade type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GrenadeType {
    Flash,
    Smoke,
    HE,
    Molotov,
    Incendiary,
    Decoy,
}

/// State of a grenade in the world.
///
/// For timed grenades (smokes, molotovs, incendiaries):
/// - `detonated_tick` = tick when grenade exploded/started burning
/// - `start_tick` = tick when grenade becomes active (detonate time)
/// - `end_tick` = tick when grenade expires (None if still active or not found)
/// - `entity_id` = unique entity identifier from demo for matching start/end events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrenadeState {
    pub id: u32,
    pub grenade_type: GrenadeType,
    pub owner: Option<PlayerId>,
    pub position: super::player::Vec3,
    pub velocity: super::player::Vec3,
    /// Tick when grenade was thrown
    pub thrown_tick: Tick,
    /// Tick when grenade detonated (exploded / started burning)
    pub detonated_tick: Option<Tick>,
    /// Start tick of the active effect (same as detonated_tick for smokes/infernos)
    pub start_tick: Option<Tick>,
    /// End tick when grenade effect expires (None if still active or not found)
    pub end_tick: Option<Tick>,
    /// Entity ID from demo (for matching start/end events per awpy pattern)
    pub entity_id: Option<u32>,
    pub active: bool,
}

impl GrenadeState {
    pub fn time_since_thrown(&self, current_tick: Tick) -> f32 {
        (current_tick.0 - self.thrown_tick.0) as f32 / 64.0
    }

    /// Time remaining until this grenade effect expires (in seconds).
    /// Returns None if the grenade doesn't have an end tick.
    pub fn time_remaining(&self, current_tick: Tick) -> Option<f32> {
        self.end_tick.map(|end| (end.0 as f32 - current_tick.0 as f32) / 64.0)
    }

    /// Whether this is a timed grenade (smoke, molotov, incendiary) that has a duration window.
    pub fn is_timed(&self) -> bool {
        matches!(self.grenade_type, GrenadeType::Smoke | GrenadeType::Molotov | GrenadeType::Incendiary)
    }

    /// Whether this grenade is currently active at the given tick.
    pub fn is_active_at(&self, current_tick: Tick) -> bool {
        if !self.active {
            return false;
        }
        // Check if we're within the effect window
        if let Some(start) = self.start_tick {
            if current_tick.0 < start.0 {
                return false;
            }
        }
        if let Some(end) = self.end_tick {
            if current_tick.0 >= end.0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::player::Vec3;

    #[test]
    fn test_is_timed() {
        let smoke = GrenadeState {
            id: 1,
            grenade_type: GrenadeType::Smoke,
            owner: None,
            position: Vec3::default(),
            velocity: Vec3::default(),
            thrown_tick: Tick(100),
            detonated_tick: Some(Tick(101)),
            start_tick: Some(Tick(101)),
            end_tick: Some(Tick(600)),
            entity_id: Some(42),
            active: true,
        };
        assert!(smoke.is_timed());

        let flash = GrenadeState {
            id: 2,
            grenade_type: GrenadeType::Flash,
            owner: None,
            position: Vec3::default(),
            velocity: Vec3::default(),
            thrown_tick: Tick(100),
            detonated_tick: Some(Tick(101)),
            start_tick: None,
            end_tick: None,
            entity_id: None,
            active: true,
        };
        assert!(!flash.is_timed());

        let molotov = GrenadeState {
            id: 3,
            grenade_type: GrenadeType::Molotov,
            owner: None,
            position: Vec3::default(),
            velocity: Vec3::default(),
            thrown_tick: Tick(100),
            detonated_tick: Some(Tick(101)),
            start_tick: Some(Tick(101)),
            end_tick: Some(Tick(400)),
            entity_id: Some(43),
            active: true,
        };
        assert!(molotov.is_timed());

        let incendiary = GrenadeState {
            id: 4,
            grenade_type: GrenadeType::Incendiary,
            owner: None,
            position: Vec3::default(),
            velocity: Vec3::default(),
            thrown_tick: Tick(100),
            detonated_tick: Some(Tick(101)),
            start_tick: Some(Tick(101)),
            end_tick: Some(Tick(350)),
            entity_id: Some(44),
            active: true,
        };
        assert!(incendiary.is_timed());
    }

    #[test]
    fn test_is_active_at() {
        let smoke = GrenadeState {
            id: 1,
            grenade_type: GrenadeType::Smoke,
            owner: None,
            position: Vec3::default(),
            velocity: Vec3::default(),
            thrown_tick: Tick(100),
            detonated_tick: Some(Tick(101)),
            start_tick: Some(Tick(101)),
            end_tick: Some(Tick(600)),
            entity_id: Some(42),
            active: true,
        };

        // Before start
        assert!(!smoke.is_active_at(Tick(100)));
        // During active
        assert!(smoke.is_active_at(Tick(300)));
        // After end
        assert!(!smoke.is_active_at(Tick(601)));
        // Inactive flag
        let mut dead_smoke = smoke.clone();
        dead_smoke.active = false;
        assert!(!dead_smoke.is_active_at(Tick(300)));
    }

    #[test]
    fn test_time_remaining() {
        let smoke = GrenadeState {
            id: 1,
            grenade_type: GrenadeType::Smoke,
            owner: None,
            position: Vec3::default(),
            velocity: Vec3::default(),
            thrown_tick: Tick(100),
            detonated_tick: Some(Tick(101)),
            start_tick: Some(Tick(101)),
            end_tick: Some(Tick(600)),
            entity_id: Some(42),
            active: true,
        };

        // At tick 300, (600-300) ticks remain = 300/64 = 4.6875 seconds
        let remaining = smoke.time_remaining(Tick(300));
        assert!(remaining.is_some());
        assert!((remaining.unwrap() - 4.6875).abs() < 0.001);

        // No end tick
        let mut no_end = smoke;
        no_end.end_tick = None;
        assert!(no_end.time_remaining(Tick(300)).is_none());
    }

    #[test]
    fn test_time_since_thrown() {
        let grenade = GrenadeState {
            id: 1,
            grenade_type: GrenadeType::Smoke,
            owner: None,
            position: Vec3::default(),
            velocity: Vec3::default(),
            thrown_tick: Tick(100),
            detonated_tick: Some(Tick(101)),
            start_tick: Some(Tick(101)),
            end_tick: Some(Tick(600)),
            entity_id: Some(42),
            active: true,
        };

        // At tick 300, 200 ticks passed = 200/64 = 3.125 seconds
        let elapsed = grenade.time_since_thrown(Tick(300));
        assert!((elapsed - 3.125).abs() < 0.001);
    }
}
