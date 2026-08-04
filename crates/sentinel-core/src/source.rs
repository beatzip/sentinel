use std::path::Path;

use crate::{PlayerId, Tick};

/// A unified interface for demo file sources.
///
/// This trait abstracts away the specifics of different game replay formats.
/// Sentinel's core pipeline operates exclusively through this interface,
/// so changing the underlying parser library never touches analysis code.
///
/// Implementations:
/// - `MockSource` — for testing with synthetic data
/// - `Source2Adapter` — wraps demoparser2 for CS2 .dem files
/// - Future: CS:GO, Deadlock, Valorant, etc.
pub trait DemoSource {
    /// Type representing a single game event
    type Event: DemoEvent;

    /// Type representing a player snapshot at a tick
    type PlayerSnapshot: PlayerSnapshot;

    /// Type representing a round
    type RoundInfo: RoundInfo;

    /// Metadata about the match
    fn metadata(&self) -> MatchMetadata;

    /// Iterator over all game events in chronological order
    fn events(&self) -> impl Iterator<Item = Self::Event>;

    /// Player snapshots at a specific tick
    fn players_at_tick(&self, tick: Tick) -> Vec<Self::PlayerSnapshot>;

    /// All player snapshots across all ticks (for bulk processing)
    fn player_snapshots(&self) -> Vec<Self::PlayerSnapshot>;

    /// All rounds in the match
    fn rounds(&self) -> &[Self::RoundInfo];

    /// Total number of ticks
    fn tick_count(&self) -> u32;

    /// Tick rate (ticks per second)
    fn tick_rate(&self) -> u32;

    /// All unique player IDs in the match
    fn player_ids(&self) -> Vec<PlayerId>;

    /// Player name by ID
    fn player_name(&self, id: PlayerId) -> Option<String>;

    /// Player team by ID
    fn player_team(&self, id: PlayerId) -> Option<Team>;
}

/// Metadata about a match
#[derive(Debug, Clone)]
pub struct MatchMetadata {
    pub demo_path: String,
    pub map_name: String,
    pub server_name: String,
    pub total_ticks: u32,
    pub tick_rate: u32,
    pub duration_seconds: f64,
}

/// Player team
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Team {
    Terrorist,
    CounterTerrorist,
    Unassigned,
}

/// A game event from a demo source
pub trait DemoEvent {
    fn tick(&self) -> Tick;
    fn kind(&self) -> EventKind;
    fn data(&self) -> &[(String, EventData)];
}

/// Event kind enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    PlayerSpawn,
    PlayerDeath,
    PlayerHurt,
    PlayerSound,
    WeaponFire,
    RoundStart,
    RoundEnd,
    BombPlant,
    BombDefuse,
    SmokeDetonate,
    SmokeExpired,
    FlashDetonate,
    HEDetonate,
    MolotovDetonate,
    InfernoStart,
    InfernoExpire,
}

/// Event data values
#[derive(Debug, Clone)]
pub enum EventData {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    PlayerId(PlayerId),
}

/// A player snapshot at a specific tick
pub trait PlayerSnapshot {
    fn id(&self) -> PlayerId;
    fn tick(&self) -> Tick;
    fn position(&self) -> (f32, f32, f32);
    fn velocity(&self) -> (f32, f32, f32);
    fn view_angles(&self) -> (f32, f32, f32);
    fn health(&self) -> i32;
    fn armor(&self) -> i32;
    fn weapon(&self) -> WeaponKind;
    fn alive(&self) -> bool;
    fn scoped(&self) -> bool;
}

/// Round information
pub trait RoundInfo {
    fn number(&self) -> u32;
    fn winner(&self) -> Option<Team>;
    fn start_tick(&self) -> Tick;
    fn end_tick(&self) -> Tick;
}

/// Weapon categories for feature computation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponKind {
    Knife,
    Pistol,
    SMG,
    Rifle,
    Sniper,
    Shotgun,
    MG,
    Grenade,
    C4,
    DefuseKit,
    Unknown,
}

impl WeaponKind {
    pub fn is_gun(&self) -> bool {
        matches!(
            self,
            Self::Pistol | Self::SMG | Self::Rifle | Self::Sniper | Self::Shotgun | Self::MG
        )
    }
}

/// A mock demo source for testing
pub struct MockSource {
    metadata: MatchMetadata,
    events: Vec<MockEvent>,
    players: Vec<MockPlayer>,
    rounds: Vec<MockRound>,
}

#[derive(Debug, Clone)]
pub struct MockEvent {
    tick: Tick,
    kind: EventKind,
    data: Vec<(String, EventData)>,
}

#[derive(Debug, Clone)]
pub struct MockPlayer {
    id: PlayerId,
    name: String,
    team: Team,
    snapshots: Vec<MockSnapshot>,
}

#[derive(Debug, Clone)]
pub struct MockSnapshot {
    player_id: PlayerId,
    tick: Tick,
    x: f32,
    y: f32,
    z: f32,
    vx: f32,
    vy: f32,
    vz: f32,
    pitch: f32,
    yaw: f32,
    roll: f32,
    health: i32,
    armor: i32,
    weapon: WeaponKind,
    alive: bool,
    scoped: bool,
}

#[derive(Debug, Clone)]
pub struct MockRound {
    number: u32,
    winner: Option<Team>,
    start_tick: Tick,
    end_tick: Tick,
}

impl MockEvent {
    pub fn new(tick: u32, kind: EventKind, data: Vec<(String, EventData)>) -> Self {
        Self {
            tick: Tick(tick),
            kind,
            data,
        }
    }
}

impl DemoEvent for MockEvent {
    fn tick(&self) -> Tick {
        self.tick
    }
    fn kind(&self) -> EventKind {
        self.kind.clone()
    }
    fn data(&self) -> &[(String, EventData)] {
        &self.data
    }
}

impl PlayerSnapshot for MockSnapshot {
    fn id(&self) -> PlayerId {
        self.player_id
    }
    fn tick(&self) -> Tick {
        self.tick
    }
    fn position(&self) -> (f32, f32, f32) {
        (self.x, self.y, self.z)
    }
    fn velocity(&self) -> (f32, f32, f32) {
        (self.vx, self.vy, self.vz)
    }
    fn view_angles(&self) -> (f32, f32, f32) {
        (self.pitch, self.yaw, self.roll)
    }
    fn health(&self) -> i32 {
        self.health
    }
    fn armor(&self) -> i32 {
        self.armor
    }
    fn weapon(&self) -> WeaponKind {
        self.weapon
    }
    fn alive(&self) -> bool {
        self.alive
    }
    fn scoped(&self) -> bool {
        self.scoped
    }
}

impl RoundInfo for MockRound {
    fn number(&self) -> u32 {
        self.number
    }
    fn winner(&self) -> Option<Team> {
        self.winner
    }
    fn start_tick(&self) -> Tick {
        self.start_tick
    }
    fn end_tick(&self) -> Tick {
        self.end_tick
    }
}

impl MockSource {
    pub fn new() -> Self {
        Self {
            metadata: MatchMetadata {
                demo_path: "mock.dem".to_string(),
                map_name: "de_dust2".to_string(),
                server_name: "Mock Server".to_string(),
                total_ticks: 6400,
                tick_rate: 64,
                duration_seconds: 100.0,
            },
            events: Vec::new(),
            players: Vec::new(),
            rounds: Vec::new(),
        }
    }

    pub fn add_player(&mut self, player: MockPlayer) {
        self.players.push(player);
    }

    pub fn add_event(&mut self, event: MockEvent) {
        self.events.push(event);
    }

    pub fn add_round(&mut self, round: MockRound) {
        self.rounds.push(round);
    }
}

impl Default for MockSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoSource for MockSource {
    type Event = MockEvent;
    type PlayerSnapshot = MockSnapshot;
    type RoundInfo = MockRound;

    fn metadata(&self) -> MatchMetadata {
        self.metadata.clone()
    }
    fn events(&self) -> impl Iterator<Item = Self::Event> {
        self.events.iter().cloned()
    }
    fn players_at_tick(&self, tick: Tick) -> Vec<Self::PlayerSnapshot> {
        self.players
            .iter()
            .flat_map(|p| p.snapshots.iter().filter(move |s| s.tick == tick).cloned())
            .collect()
    }
    fn player_snapshots(&self) -> Vec<Self::PlayerSnapshot> {
        self.players
            .iter()
            .flat_map(|p| p.snapshots.iter().cloned())
            .collect()
    }
    fn rounds(&self) -> &[Self::RoundInfo] {
        &self.rounds
    }
    fn tick_count(&self) -> u32 {
        self.metadata.total_ticks
    }
    fn tick_rate(&self) -> u32 {
        self.metadata.tick_rate
    }
    fn player_ids(&self) -> Vec<PlayerId> {
        self.players.iter().map(|p| p.id).collect()
    }
    fn player_name(&self, id: PlayerId) -> Option<String> {
        self.players
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
    }
    fn player_team(&self, id: PlayerId) -> Option<Team> {
        self.players.iter().find(|p| p.id == id).map(|p| p.team)
    }
}

/// Load a DemoSource from a .dem file path.
/// Currently returns MockSource; real implementation wraps demoparser2.
pub fn load_demo(_path: &Path) -> Result<MockSource, String> {
    Ok(MockSource::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_source() {
        let mut src = MockSource::new();
        src.add_player(MockPlayer {
            id: PlayerId::new(1),
            name: "Player1".to_string(),
            team: Team::Terrorist,
            snapshots: vec![MockSnapshot {
                player_id: PlayerId::new(1),
                tick: Tick(100),
                x: 0.0,
                y: 0.0,
                z: 0.0,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                pitch: 0.0,
                yaw: 0.0,
                roll: 0.0,
                health: 100,
                armor: 100,
                weapon: WeaponKind::Rifle,
                alive: true,
                scoped: false,
            }],
        });

        assert_eq!(src.player_ids().len(), 1);
        assert_eq!(
            src.player_name(PlayerId::new(1)),
            Some("Player1".to_string())
        );
        assert_eq!(src.players_at_tick(Tick(100)).len(), 1);
    }
}
