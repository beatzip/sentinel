use serde::{Deserialize, Serialize};

/// A tick number in the demo file. Ticks are the fundamental time unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct Tick(pub u32);

impl Tick {
    pub fn new(tick: u32) -> Self {
        Self(tick)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }

    /// Convert tick to seconds (assuming 64 tick rate)
    pub fn as_seconds(self) -> f64 {
        self.0 as f64 / 64.0
    }
}

impl std::fmt::Display for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tick:{}", self.0)
    }
}

/// Complete state snapshot at a specific tick
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickState {
    pub tick: Tick,
    pub players: Vec<super::player::PlayerState>,
    pub grenades: Vec<super::grenade::GrenadeState>,
    pub bomb: super::bomb::BombState,
    pub round: super::round::RoundState,
}
