use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use sentinel_core::source::{
    DemoEvent, DemoSource, EventData, EventKind, MatchMetadata, PlayerSnapshot, RoundInfo, Team,
    WeaponKind,
};
use sentinel_core::{PlayerId, Tick};
use source2_demo::prelude::*;

pub struct Source2Adapter {
    metadata: MatchMetadata,
    events: Vec<Source2Event>,
    player_snapshots: Vec<Source2PlayerSnapshot>,
    rounds: Vec<Source2Round>,
    player_names: HashMap<PlayerId, String>,
    player_teams: HashMap<PlayerId, Team>,
}

#[derive(Debug, Clone)]
pub struct Source2Event {
    tick: Tick,
    kind: EventKind,
    data: Vec<(String, EventData)>,
}

#[derive(Debug, Clone)]
pub struct Source2PlayerSnapshot {
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
pub struct Source2Round {
    number: u32,
    winner: Option<Team>,
    start_tick: Tick,
    end_tick: Tick,
}

#[derive(Default)]
struct DemoCollector {
    events: Vec<Source2Event>,
    player_names: HashMap<PlayerId, String>,
    player_teams: HashMap<PlayerId, Team>,
    player_snapshots: Vec<Source2PlayerSnapshot>,
    current_tick: u32,
    entity_to_player: HashMap<u32, PlayerId>,
}

#[observer]
#[uses_all]
impl DemoCollector {
    #[on_tick_start]
    fn handle_tick_start(&mut self, ctx: &Context) -> ObserverResult {
        self.current_tick = ctx.tick();
        Ok(())
    }

    #[on_game_event]
    fn handle_game_event(&mut self, _ctx: &Context, event: &GameEvent) -> ObserverResult {
        let kind = match event.name() {
            "player_death" => EventKind::PlayerDeath,
            "player_spawn" => EventKind::PlayerSpawn,
            "player_hurt" => EventKind::PlayerHurt,
            "player_sound" => EventKind::PlayerSound,
            "weapon_fire" => EventKind::WeaponFire,
            "round_start" => EventKind::RoundStart,
            "round_end" => EventKind::RoundEnd,
            "bomb_plant" => EventKind::BombPlant,
            "bomb_defuse" => EventKind::BombDefuse,
            "smokegrenade_detonate" => EventKind::SmokeDetonate,
            "smokegrenade_expired" => EventKind::SmokeExpired,
            "flashbang_detonate" => EventKind::FlashDetonate,
            "hegrenade_detonate" => EventKind::HEDetonate,
            "molotov_detonate" => EventKind::MolotovDetonate,
            "inferno_startburn" => EventKind::InfernoStart,
            "inferno_expire" => EventKind::InfernoExpire,
            _ => return Ok(()),
        };

        let tick = Tick(self.current_tick);
        let mut data = Vec::new();

        // Extract common fields
        for key in &["attacker", "userid", "weapon", "team", "name", "winner"] {
            if let Ok(val) = event.get_value(key) {
                let event_val = match val {
                    EventValue::Int(v) => EventData::Int(*v as i64),
                    EventValue::U64(v) => EventData::PlayerId(PlayerId::new(*v)),
                    EventValue::String(s) => EventData::String(s.clone()),
                    EventValue::Float(v) => EventData::Float(*v as f64),
                    EventValue::Bool(v) => EventData::Bool(*v),
                    EventValue::Byte(v) => EventData::Int(*v as i64),
                };
                data.push((key.to_string(), event_val));
            }
        }

        // Also extract grenade-specific fields
        for key in &["entityid", "x", "y", "z"] {
            if let Ok(val) = event.get_value(key) {
                let event_val = match val {
                    EventValue::Int(v) => EventData::Int(*v as i64),
                    EventValue::U64(v) => EventData::Int(*v as i64),
                    EventValue::Float(v) => EventData::Float(*v as f64),
                    _ => continue,
                };
                data.push((key.to_string(), event_val));
            }
        }

        // Extract player_hurt specific fields
        if event.name() == "player_hurt" {
            for key in &[
                "dmg_health",
                "dmg_armor",
                "hitgroup",
                "victim_health",
                "victim_armor",
            ] {
                if let Ok(val) = event.get_value(key) {
                    let event_val = match val {
                        EventValue::Int(v) => EventData::Int(*v as i64),
                        EventValue::Float(v) => EventData::Float(*v as f64),
                        _ => continue,
                    };
                    data.push((key.to_string(), event_val));
                }
            }
        }

        // Extract weapon_fire specific fields
        if event.name() == "weapon_fire" {
            for key in &["penetrated", "is_alt_fire"] {
                if let Ok(val) = event.get_value(key) {
                    let event_val = match val {
                        EventValue::Int(v) => EventData::Int(*v as i64),
                        EventValue::Bool(v) => EventData::Bool(*v),
                        _ => continue,
                    };
                    data.push((key.to_string(), event_val));
                }
            }
        }

        // Track player names/teams from spawn events
        if event.name() == "player_spawn"
            && let Some((_, EventData::PlayerId(id))) = data.iter().find(|(k, _)| k == "userid")
        {
            let pid = *id;
            if let Some((_, EventData::String(name))) = data.iter().find(|(k, _)| k == "name") {
                self.player_names.insert(pid, name.clone());
            }
            if let Some((_, EventData::Int(t))) = data.iter().find(|(k, _)| k == "team") {
                let team = match t {
                    2 => Team::Terrorist,
                    3 => Team::CounterTerrorist,
                    _ => Team::Unassigned,
                };
                self.player_teams.insert(pid, team);
            }
        }

        self.events.push(Source2Event { tick, kind, data });
        Ok(())
    }

    #[on_entity]
    fn handle_entity(
        &mut self,
        _ctx: &Context,
        event: EntityEvents,
        entity: &Entity,
    ) -> ObserverResult {
        // Only process entity updates
        if event != EntityEvents::Updated {
            return Ok(());
        }

        let class_name = entity.class().name();
        let tick = Tick(self.current_tick);

        // Player controller - extract name and team
        if class_name == "CCSPlayerController"
            && let Ok(val) = entity.get_property("m_steamID")
            && let Ok(steam_id) = val.try_into()
        {
            let player_id = PlayerId::new(steam_id);
            self.entity_to_player.insert(entity.index(), player_id);

            // Get player name
            if let Ok(name_val) = entity.get_property("m_iszPlayerName") {
                let name: String = name_val.try_into().unwrap_or_default();
                if !name.is_empty() {
                    self.player_names.insert(player_id, name);
                }
            }

            // Get team
            if let Ok(team_val) = entity.get_property("m_iTeamNum") {
                let team_num: i32 = team_val.try_into().unwrap_or(0);
                let team = match team_num {
                    2 => Team::Terrorist,
                    3 => Team::CounterTerrorist,
                    _ => Team::Unassigned,
                };
                self.player_teams.insert(player_id, team);
            }
        }

        // Player pawn - extract position, health, etc.
        if class_name == "CCSPlayerPawn" {
            // Get player ID from entity-to-player mapping or from steamID
            let player_id = if let Some(&id) = self.entity_to_player.get(&entity.index()) {
                id
            } else if let Ok(val) = entity.get_property("m_steamID") {
                if let Ok(steam_id) = val.try_into() {
                    PlayerId::new(steam_id)
                } else {
                    return Ok(());
                }
            } else {
                return Ok(());
            };

            // Extract position (try multiple property paths)
            let x = self
                .get_f32(entity, "CBodyComponent.m_vecAbsOrigin.x")
                .or_else(|| self.get_f32(entity, "m_vecAbsOrigin.x"))
                .unwrap_or(0.0);
            let y = self
                .get_f32(entity, "CBodyComponent.m_vecAbsOrigin.y")
                .or_else(|| self.get_f32(entity, "m_vecAbsOrigin.y"))
                .unwrap_or(0.0);
            let z = self
                .get_f32(entity, "CBodyComponent.m_vecAbsOrigin.z")
                .or_else(|| self.get_f32(entity, "m_vecAbsOrigin.z"))
                .unwrap_or(0.0);

            // Extract velocity
            let vx = self.get_f32(entity, "m_vecVelocity.x").unwrap_or(0.0);
            let vy = self.get_f32(entity, "m_vecVelocity.y").unwrap_or(0.0);
            let vz = self.get_f32(entity, "m_vecVelocity.z").unwrap_or(0.0);

            // Extract view angles
            let pitch = self.get_f32(entity, "m_angEyeAngles.x").unwrap_or(0.0);
            let yaw = self.get_f32(entity, "m_angEyeAngles.y").unwrap_or(0.0);
            let roll = self.get_f32(entity, "m_angEyeAngles.z").unwrap_or(0.0);

            // Extract health
            let health = self.get_i32(entity, "m_iHealth").unwrap_or(100);

            // Extract armor
            let armor = self.get_i32(entity, "m_ArmorValue").unwrap_or(0);

            // Extract scoped state
            let scoped = self.get_bool(entity, "m_bIsScoped").unwrap_or(false);

            // Extract alive state (m_lifeState: 0 = alive, others = dead)
            let alive = self
                .get_i32(entity, "m_lifeState")
                .map(|v| v == 0)
                .unwrap_or(true);

            // Create snapshot
            self.player_snapshots.push(Source2PlayerSnapshot {
                player_id,
                tick,
                x,
                y,
                z,
                vx,
                vy,
                vz,
                pitch,
                yaw,
                roll,
                health,
                armor,
                weapon: WeaponKind::Unknown,
                alive,
                scoped,
            });
        }

        Ok(())
    }

    /// Safely get an f32 property from an entity
    fn get_f32(&self, entity: &Entity, name: &str) -> Option<f32> {
        entity.get_property(name).ok()?.try_into().ok()
    }

    /// Safely get an i32 property from an entity
    fn get_i32(&self, entity: &Entity, name: &str) -> Option<i32> {
        entity.get_property(name).ok()?.try_into().ok()
    }

    /// Safely get a bool property from an entity
    fn get_bool(&self, entity: &Entity, name: &str) -> Option<bool> {
        entity.get_property(name).ok()?.try_into().ok()
    }

    /// Safely get a String property from an entity
    #[expect(dead_code, reason = "helper for future String property extraction")]
    fn get_string(&self, entity: &Entity, name: &str) -> Option<String> {
        entity.get_property(name).ok()?.try_into().ok()
    }
}

impl Source2Adapter {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("Failed to read: {}", e))?;
        Self::from_bytes(&bytes, path)
    }

    pub fn from_bytes(bytes: &[u8], path: &Path) -> Result<Self, String> {
        let mut parser = Parser::new(bytes).map_err(|e| format!("Parser error: {}", e))?;
        let rc: Rc<RefCell<DemoCollector>> = parser.register_observer();
        parser
            .run_to_end()
            .map_err(|e| format!("Parse error: {}", e))?;

        let c = rc.borrow();

        // Debug: print event types
        eprintln!("Collected {} events", c.events.len());
        let mut event_counts = HashMap::new();
        for e in &c.events {
            *event_counts.entry(format!("{:?}", e.kind)).or_insert(0) += 1;
        }
        for (kind, count) in &event_counts {
            eprintln!("  {}: {}", kind, count);
        }
        // Debug: print first player_spawn and player_death events
        if let Some(spawn) = c.events.iter().find(|e| e.kind == EventKind::PlayerSpawn) {
            eprintln!("First spawn event data:");
            for (key, value) in &spawn.data {
                eprintln!("  {}: {:?}", key, value);
            }
        }
        if let Some(death) = c.events.iter().find(|e| e.kind == EventKind::PlayerDeath) {
            eprintln!("First death event data:");
            for (key, value) in &death.data {
                eprintln!("  {}: {:?}", key, value);
            }
        }

        // Extract unique player IDs from events
        let mut player_ids = HashSet::new();
        for event in &c.events {
            for (_, value) in &event.data {
                if let EventData::PlayerId(id) = value {
                    player_ids.insert(*id);
                }
                if let EventData::Int(id) = value
                    && *id > 0
                    && *id < 1000
                {
                    player_ids.insert(PlayerId::new(*id as u64));
                }
            }
        }
        eprintln!("Found {} unique player IDs", player_ids.len());

        // Create player_names from extracted IDs
        let mut player_names = c.player_names.clone();
        let mut player_teams = c.player_teams.clone();
        for &id in &player_ids {
            player_names
                .entry(id)
                .or_insert_with(|| format!("Player_{}", id.as_u64()));
            player_teams.entry(id).or_insert(Team::Unassigned);
        }
        eprintln!(
            "Player names: {:?}",
            c.player_names.keys().collect::<Vec<_>>()
        );

        let metadata = MatchMetadata {
            demo_path: path.to_string_lossy().to_string(),
            map_name: String::new(),
            server_name: String::new(),
            total_ticks: c.current_tick,
            tick_rate: 64,
            duration_seconds: c.current_tick as f64 / 64.0,
        };

        let mut rounds = Vec::new();
        let starts: Vec<_> = c
            .events
            .iter()
            .filter(|e| e.kind == EventKind::RoundStart)
            .collect();
        let ends: Vec<_> = c
            .events
            .iter()
            .filter(|e| e.kind == EventKind::RoundEnd)
            .collect();
        for (i, s) in starts.iter().enumerate() {
            let end = ends
                .get(i)
                .map(|e| e.tick)
                .unwrap_or(Tick(metadata.total_ticks));
            let winner = ends.get(i).and_then(|e| {
                e.data.iter().find_map(|(k, v)| {
                    if k == "winner" {
                        if let EventData::Int(t) = v {
                            match t {
                                2 => Some(Team::Terrorist),
                                3 => Some(Team::CounterTerrorist),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            });
            rounds.push(Source2Round {
                number: i as u32 + 1,
                winner,
                start_tick: s.tick,
                end_tick: end,
            });
        }

        Ok(Self {
            metadata,
            events: c.events.clone(),
            player_snapshots: c.player_snapshots.clone(),
            rounds,
            player_names,
            player_teams,
        })
    }
}

impl DemoEvent for Source2Event {
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

impl PlayerSnapshot for Source2PlayerSnapshot {
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

impl RoundInfo for Source2Round {
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

impl DemoSource for Source2Adapter {
    type Event = Source2Event;
    type PlayerSnapshot = Source2PlayerSnapshot;
    type RoundInfo = Source2Round;
    fn metadata(&self) -> MatchMetadata {
        self.metadata.clone()
    }
    fn events(&self) -> impl Iterator<Item = Self::Event> {
        self.events.iter().cloned()
    }
    fn players_at_tick(&self, tick: Tick) -> Vec<Self::PlayerSnapshot> {
        self.player_snapshots
            .iter()
            .filter(|s| s.tick == tick)
            .cloned()
            .collect()
    }
    fn player_snapshots(&self) -> Vec<Self::PlayerSnapshot> {
        self.player_snapshots.clone()
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
        self.player_names.keys().cloned().collect()
    }
    fn player_name(&self, id: PlayerId) -> Option<String> {
        self.player_names.get(&id).cloned()
    }
    fn player_team(&self, id: PlayerId) -> Option<Team> {
        self.player_teams.get(&id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_source2_adapter_compiles() {
        let a = Source2Adapter {
            metadata: MatchMetadata {
                demo_path: "test.dem".into(),
                map_name: "de_dust2".into(),
                server_name: "Test".into(),
                total_ticks: 6400,
                tick_rate: 64,
                duration_seconds: 100.0,
            },
            events: vec![],
            player_snapshots: vec![],
            rounds: vec![],
            player_names: HashMap::new(),
            player_teams: HashMap::new(),
        };
        assert_eq!(a.tick_count(), 6400);
    }
}
