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

/// State of a grenade in the world
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrenadeState {
    pub id: u32,
    pub grenade_type: GrenadeType,
    pub owner: Option<PlayerId>,
    pub position: super::player::Vec3,
    pub velocity: super::player::Vec3,
    pub thrown_tick: Tick,
    pub detonated: bool,
    pub detonated_tick: Option<Tick>,
    pub active: bool,
}

impl GrenadeState {
    pub fn time_since_thrown(&self, current_tick: Tick) -> f32 {
        (current_tick.0 - self.thrown_tick.0) as f32 / 64.0
    }
}
