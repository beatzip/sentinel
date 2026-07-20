use serde::{Deserialize, Serialize};

use super::player::Team;

/// Round phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoundPhase {
    Freezetime,
    Live,
    Over,
}

/// State of the current round
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundState {
    pub round_number: u32,
    pub phase: RoundPhase,
    pub clock: f32,
    pub t_score: u32,
    pub ct_score: u32,
    pub winner: Option<Team>,
}

impl RoundState {
    pub fn is_live(&self) -> bool {
        self.phase == RoundPhase::Live
    }
}
