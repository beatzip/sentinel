use serde::{Deserialize, Serialize};

/// Browser-friendly replay data, produced alongside a Sentinel report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayData {
    pub version: String,
    pub map: String,
    pub tick_rate: u32,
    pub frames: Vec<ReplayFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayFrame {
    pub tick: u32,
    pub round: u32,
    pub players: Vec<ReplayPlayer>,
    /// Directed pairs where the first player has a line of sight to the second.
    pub visible_pairs: Vec<VisibilityPair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayPlayer {
    pub steam_id: u64,
    pub name: String,
    pub team: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub health: i32,
    pub alive: bool,
    pub yaw: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisibilityPair {
    pub observer: u64,
    pub target: u64,
}
