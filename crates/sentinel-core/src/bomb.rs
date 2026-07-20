use serde::{Deserialize, Serialize};

use super::tick::Tick;

/// Bomb state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BombState {
    /// Not yet planted
    Carried { carrier: super::player::PlayerId },
    /// Dropped on the ground
    Dropped { position: super::player::Vec3 },
    /// Planted at a bombsite
    Planted {
        site: char,
        position: super::player::Vec3,
        planted_tick: Tick,
    },
    /// Defused
    Defused,
    /// Exploded
    Exploded,
}

impl BombState {
    pub fn is_planted(&self) -> bool {
        matches!(self, BombState::Planted { .. })
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            BombState::Carried { .. } | BombState::Dropped { .. } | BombState::Planted { .. }
        )
    }
}
