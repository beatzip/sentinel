use serde::{Deserialize, Serialize};

use super::player::PlayerId;
use super::tick::Tick;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillEvent {
    pub tick: Tick,
    pub attacker: PlayerId,
    pub victim: PlayerId,
    pub weapon: String,
    pub headshot: bool,
    pub assisted: bool,
    pub assist_player: Option<PlayerId>,
    #[serde(default)]
    pub wallbang: bool,
    #[serde(default)]
    pub through_smoke: bool,
}
