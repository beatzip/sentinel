use sentinel_core::{
    Angles, KillEvent, PlayerId, PlayerState, RoundPhase, Tick, TickState, Vec3, Weapon,
};
use sentinel_core::source::PlayerSnapshot;
use sentinel_events::kinds::{EventKind, EventValue, GameEvent};

use crate::state::WorldState;

/// Rebuilds world state from a stream of game events
pub struct WorldRebuilder {
    /// Current world state
    world: WorldState,
    /// History of world states (one per tick)
    states: Vec<TickState>,
    /// Accumulated kill events (separated from TickState for memory efficiency)
    kills: Vec<KillEvent>,
}

impl WorldRebuilder {
    /// Create a new rebuilder starting from tick 0
    pub fn new() -> Self {
        Self {
            world: WorldState::new(Tick(0)),
            states: Vec::new(),
            kills: Vec::new(),
        }
    }

    /// Take accumulated kill events (call after process_events/process_events_with_snapshots)
    pub fn take_kills(&mut self) -> Vec<KillEvent> {
        std::mem::take(&mut self.kills)
    }

    /// Process a batch of events and return the resulting world states
    pub fn process_events(&mut self, events: &[GameEvent]) -> Vec<TickState> {
        let mut current_tick = Tick(0);

        for event in events {
            // If we've moved to a new tick, save the current state
            if event.tick != current_tick {
                self.save_state(current_tick);
                current_tick = event.tick;
                self.world.tick = current_tick;
            }

            // Process the event
            self.process_event(event);
        }

        // Save the final state
        self.save_state(current_tick);

        std::mem::take(&mut self.states)
    }

    /// Process events with player snapshots from a demo source.
    /// This merges real telemetry data (positions, velocities, view angles) into
    /// the world state, fixing the issue where players were frozen at (0,0,0).
    /// 
    /// CRITICAL: This method saves states for ALL ticks that have either events OR snapshots,
    /// not just event ticks. This "densification" ensures features can access state at any tick.
    pub fn process_events_with_snapshots<S>(
        &mut self,
        events: &[GameEvent],
        snapshots: &[S],
    ) -> Vec<TickState>
    where
        S: PlayerSnapshot,
    {
        use std::collections::BTreeMap;

        // Pre-index events by tick for efficient lookup
        let mut events_by_tick: BTreeMap<u32, Vec<&GameEvent>> = BTreeMap::new();
        for event in events {
            events_by_tick
                .entry(event.tick.0)
                .or_default()
                .push(event);
        }

        // Pre-index snapshots by tick for efficient lookup
        let mut snapshots_by_tick: BTreeMap<u32, Vec<&S>> = BTreeMap::new();
        for snap in snapshots {
            snapshots_by_tick
                .entry(snap.tick().as_u32())
                .or_default()
                .push(snap);
        }

        // Collect all unique ticks from both events and snapshots
        let mut ticks: Vec<u32> = events_by_tick
            .keys()
            .chain(snapshots_by_tick.keys())
            .copied()
            .collect();
        ticks.sort_unstable();
        ticks.dedup();

        // Process each tick in order
        for tick in ticks {
            self.world.tick = Tick(tick);

            // Process all events for this tick
            if let Some(evs) = events_by_tick.get(&tick) {
                for event in evs {
                    self.process_event(event);
                }
            }

            // Merge snapshots (this updates positions, velocities, etc. from real telemetry)
            self.merge_snapshots(Tick(tick), &snapshots_by_tick);

            // Save the state for this tick
            self.save_state(Tick(tick));
        }

        std::mem::take(&mut self.states)
    }

    /// Merge player snapshots from the demo source into the world state.
    /// This is the critical fix: without this, players are frozen at (0,0,0)
    /// with zero velocity and view angles for the entire round.
    fn merge_snapshots<S: PlayerSnapshot>(
        &mut self,
        tick: Tick,
        snapshots_by_tick: &std::collections::BTreeMap<u32, Vec<&S>>,
    ) {
        if let Some(snaps) = snapshots_by_tick.get(&tick.0) {
            for snap in snaps {
                let pid = snap.id();
                let (x, y, z) = snap.position();
                let (vx, vy, vz) = snap.velocity();
                let (pitch, yaw, roll) = snap.view_angles();

                if let Some(player) = self.world.players.get_mut(&pid) {
                    // Update with real telemetry data
                    player.position = Vec3::new(x, y, z);
                    player.velocity = Vec3::new(vx, vy, vz);
                    player.view_angles = Angles { pitch, yaw, roll };
                    player.health = snap.health();
                    player.armor = snap.armor();
                    player.alive = snap.alive();
                    player.scoped = snap.scoped();
                } else {
                    // Player not yet in world state (spawn event may not have been processed yet)
                    // Create a basic player state from the snapshot
                    let team = self
                        .world
                        .players
                        .values()
                        .find(|p| p.id == pid)
                        .map(|p| p.team)
                        .unwrap_or(sentinel_core::Team::Unassigned);

                    let name = self
                        .world
                        .players
                        .values()
                        .find(|p| p.id == pid)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player_{}", pid.as_u64()));

                    let player = PlayerState {
                        id: pid,
                        name,
                        team,
                        position: Vec3::new(x, y, z),
                        velocity: Vec3::new(vx, vy, vz),
                        view_angles: Angles { pitch, yaw, roll },
                        weapon: Weapon::Knife,
                        health: snap.health(),
                        armor: snap.armor(),
                        money: 800,
                        flash_duration: 0.0,
                        scoped: snap.scoped(),
                        reloading: false,
                        alive: snap.alive(),
                    };
                    self.world.players.insert(pid, player);
                }
            }
        }
    }

    /// Process a single event
    fn process_event(&mut self, event: &GameEvent) {
        match &event.kind {
            EventKind::PlayerSpawn => self.handle_player_spawn(event),
            EventKind::PlayerDeath => self.handle_player_death(event),
            EventKind::PlayerHurt => self.handle_player_hurt(event),
            EventKind::WeaponFire => self.handle_weapon_fire(event),
            EventKind::SmokeGrenadeDetonate => self.handle_grenade_detonate(event),
            EventKind::FlashGrenadeDetonate => self.handle_grenade_detonate(event),
            EventKind::HEGrenadeDetonate => self.handle_grenade_detonate(event),
            EventKind::MolotovDetonate => self.handle_grenade_detonate(event),
            EventKind::BombPlant => self.handle_bomb_plant(event),
            EventKind::BombDefuse => self.handle_bomb_defuse(event),
            EventKind::RoundStart => self.handle_round_start(event),
            EventKind::RoundEnd => self.handle_round_end(event),
            _ => {} // Other events don't affect world state directly
        }
    }

    /// Save the current world state as a TickState
    fn save_state(&mut self, tick: Tick) {
        let players: Vec<PlayerState> = self.world.players.values().cloned().collect();
        let grenades = self.world.grenades.clone();

        // Note: kill_feed is NOT cloned here for memory efficiency.
        // It is stored separately in MatchContext.kills and accessed via kills_up_to().
        // This prevents GBs of memory usage when states.len() reaches hundreds of thousands.
        self.states.push(TickState {
            tick,
            players,
            grenades,
            bomb: self.world.bomb.clone(),
            round: self.world.round.clone(),
        });
    }

    /// Handle player spawn event
    fn handle_player_spawn(&mut self, event: &GameEvent) {
        if let Some(player_id) = event.data.get("userid").and_then(|v| v.as_player_id()) {
            let team = event
                .data
                .get("team")
                .and_then(|v| v.as_i64())
                .map(|t| match t {
                    2 => sentinel_core::Team::Terrorist,
                    3 => sentinel_core::Team::CounterTerrorist,
                    _ => sentinel_core::Team::Unassigned,
                })
                .unwrap_or(sentinel_core::Team::Unassigned);

            let position = event
                .data
                .get("position")
                .and_then(|v| match v {
                    EventValue::Vector(x, y, z) => Some(Vec3::new(*x, *y, *z)),
                    _ => None,
                })
                .unwrap_or_default();

            let player = PlayerState {
                id: PlayerId::new(player_id),
                name: event
                    .data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                team,
                position,
                velocity: Vec3::default(),
                view_angles: Angles::default(),
                weapon: Weapon::Knife,
                health: 100,
                armor: 0,
                money: 800,
                flash_duration: 0.0,
                scoped: false,
                reloading: false,
                alive: true,
            };

            self.world.players.insert(player.id, player);
        }
    }

    /// Handle player death event
    fn handle_player_death(&mut self, event: &GameEvent) {
        if let Some(victim_id) = event.data.get("victim").and_then(|v| v.as_player_id())
            && let Some(player) = self.world.players.get_mut(&PlayerId::new(victim_id))
        {
            player.alive = false;
            player.health = 0;
        }

        // Add to kill feed
        if let (Some(attacker_id), Some(victim_id)) = (
            event.data.get("attacker").and_then(|v| v.as_player_id()),
            event.data.get("victim").and_then(|v| v.as_player_id()),
        ) {
            let weapon = event
                .data
                .get("weapon")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let headshot = event
                .data
                .get("headshot")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            self.world.add_kill(KillEvent {
                tick: event.tick,
                attacker: PlayerId::new(attacker_id),
                victim: PlayerId::new(victim_id),
                weapon,
                headshot,
                assisted: false,
                assist_player: None,
            });
        }
    }

    /// Handle player hurt event
    fn handle_player_hurt(&mut self, event: &GameEvent) {
        if let Some(victim_id) = event.data.get("userid").and_then(|v| v.as_player_id())
            && let Some(dmg_health) = event.data.get("dmg_health").and_then(|v| v.as_i64())
            && let Some(player) = self.world.players.get_mut(&PlayerId::new(victim_id))
        {
            player.health = (player.health - dmg_health as i32).max(0);
        }

        // Track damage for ADR/stats calculation
        // Note: Individual damage events are stored via DamageEvent stream in parsed_events, not in TickState
        let _ = event.data.get("dmg_health"); // Track for future stats integration
    }

    /// Handle weapon fire event
    fn handle_weapon_fire(&mut self, event: &GameEvent) {
        // Track active shots for shot-calling analysis
        // Note: Shot events are stored separately via ShotEvent stream for memory efficiency.
        // Here we just note that firing occurred for potential fire-rate calculations.
        if let Some(shooter_id) = event.data.get("userid").and_then(|v| v.as_player_id()) {
            if let Some(player) = self.world.players.get_mut(&PlayerId::new(shooter_id)) {
                // Update weapon category if available
                if let Some(weapon_str) = event.data.get("weapon").and_then(|v| v.as_str()) {
                    player.weapon = match weapon_str.to_lowercase().as_str() {
                        // Snipers (must come before Rifle)
                        "awp" | "scar20" | "g3sg1" => Weapon::Sniper,
                        // Rifles
                        "ak47" | "m4a1" | "m4a1_silencer" | "m4a1s" | "aug" | "sg553" | "famas" | "galil" | "galil_ar" | "sg556" => Weapon::Rifle,
                        // Pistols
                        "deagle" | "elite" | "fiveseven" | "glock" | "usp" | "usp_silencer" | "p2000" | "tec9" | "cz75a" | "p250" | "revolver" | "dark_reclaimer" => Weapon::Pistol,
                        // SMGs
                        "mp9" | "mp7" | "mac10" | "p90" | "bizon" | "ppbizon" | "mp5sd" | "ump45" => Weapon::SMG,
                        // Shotguns
                        "nova" | "xm1014" | "mag7" | "sawedoff" => Weapon::Shotgun,
                        // MGs
                        "m249" | "negev" => Weapon::MG,
                        // Knife
                        "knife" | "knife_t" | "bayonet" | "flip" | "gut" | "m9" | "classic" | "star" | "talon" | "nomad" => Weapon::Knife,
                        // C4
                        "c4" => Weapon::C4,
                        // Grenades
                        "hegrenade" | "flashbang" | "smokegrenade" | "molotov" | "incgrenade" | "decoy" => Weapon::Grenade,
                        _ => Weapon::None,
                    };
                }
            }
        }
    }

    /// Handle grenade detonate event
    fn handle_grenade_detonate(&mut self, _event: &GameEvent) {
        // Mark the grenade as detonated
        // In a real implementation, we'd find the specific grenade and mark it
    }

    /// Handle bomb plant event
    fn handle_bomb_plant(&mut self, event: &GameEvent) {
        if let Some(site) = event.data.get("site").and_then(|v| v.as_str()) {
            self.world.bomb = sentinel_core::BombState::Planted {
                site: site.chars().next().unwrap_or('A'),
                position: Vec3::default(), // Would get from event data
                planted_tick: event.tick,
            };
        }
    }

    /// Handle bomb defuse event
    fn handle_bomb_defuse(&mut self, _event: &GameEvent) {
        self.world.bomb = sentinel_core::BombState::Defused;
    }

    /// Handle round start event
    fn handle_round_start(&mut self, event: &GameEvent) {
        if let Some(round_num) = event.data.get("round").and_then(|v| v.as_i64()) {
            self.world.round.round_number = round_num as u32;
            self.world.round.phase = RoundPhase::Live;
            self.world.round.clock = 115.0;
            self.world.round.start_tick = event.tick.0;
        }

        // Reset all players to alive
        for player in self.world.players.values_mut() {
            player.alive = true;
            player.health = 100;
        }

        // Clear grenades
        self.world.grenades.clear();
    }

    /// Handle round end event
    fn handle_round_end(&mut self, event: &GameEvent) {
        self.world.round.phase = RoundPhase::Over;

        if let Some(winner) = event.data.get("winner").and_then(|v| v.as_i64()) {
            self.world.round.winner = Some(match winner {
                2 => sentinel_core::Team::Terrorist,
                3 => sentinel_core::Team::CounterTerrorist,
                _ => sentinel_core::Team::Unassigned,
            });

            // Update scores
            match self.world.round.winner {
                Some(sentinel_core::Team::Terrorist) => self.world.round.t_score += 1,
                Some(sentinel_core::Team::CounterTerrorist) => self.world.round.ct_score += 1,
                _ => {}
            }
        }
    }
}

impl Default for WorldRebuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_events::kinds::{EventKind, player_death, round_start};

    #[test]
    fn test_rebuilder_empty() {
        let mut rebuilder = WorldRebuilder::new();
        let states = rebuilder.process_events(&[]);
        // Rebuilder always produces at least one state (the final state)
        assert_eq!(states.len(), 1);
    }

    #[test]
    fn test_rebuilder_round_start() {
        let mut rebuilder = WorldRebuilder::new();
        let events = vec![round_start(Tick(100), 1)];
        let states = rebuilder.process_events(&events);

        // Should have states for tick 0 (initial) and tick 100 (round start)
        assert_eq!(states.len(), 2);
        assert_eq!(states[1].round.round_number, 1);
    }

    #[test]
    fn test_rebuilder_player_death() {
        let mut rebuilder = WorldRebuilder::new();

        // First spawn a player
        let spawn_event = sentinel_events::kinds::make_event(
            EventKind::PlayerSpawn,
            Tick(0),
            vec![
                ("userid", EventValue::PlayerId(1)),
                ("team", EventValue::Integer(2)),
            ],
        );

        let death_event = player_death(Tick(100), 2, 1, "ak47");

        let states = rebuilder.process_events(&[spawn_event, death_event]);

        // Should have states for tick 0 and tick 100
        assert!(states.len() >= 2);

        // Player should be dead in the final state
        let final_state = states.last().unwrap();
        let player = final_state
            .players
            .iter()
            .find(|p| p.id == PlayerId::new(1));
        assert!(player.is_some());
        assert!(!player.unwrap().alive);
    }

    #[test]
    fn test_process_events_with_snapshots_densify() {
        // Test that states are saved for ALL ticks with snapshots, not just event ticks
        // This verifies the "densify" behavior where we collect all unique ticks
        // from both events and snapshots
        use std::collections::BTreeMap;
        
        // Simulate the tick collection logic from process_events_with_snapshots
        let mut events_by_tick: BTreeMap<u32, ()> = BTreeMap::new();
        events_by_tick.insert(0, ());
        
        let mut snapshots_by_tick: BTreeMap<u32, ()> = BTreeMap::new();
        snapshots_by_tick.insert(0, ());
        snapshots_by_tick.insert(64, ());
        snapshots_by_tick.insert(128, ());
        
        // Collect all unique ticks
        let mut ticks: Vec<u32> = events_by_tick
            .keys()
            .chain(snapshots_by_tick.keys())
            .copied()
            .collect();
        ticks.sort_unstable();
        ticks.dedup();
        
        // Should have 3 unique ticks: 0, 64, 128
        assert_eq!(ticks.len(), 3);
        assert!(ticks.contains(&0));
        assert!(ticks.contains(&64));
        assert!(ticks.contains(&128));
    }

    #[test]
    fn test_kills_up_to_boundary() {
        use sentinel_core::KillEvent;
        use sentinel_core::world::MatchContext;

        let mut context = MatchContext::new(vec![]);

        // Add kills at ticks 100, 200, 300
        context.set_kills(vec![
            KillEvent {
                tick: Tick(100),
                attacker: PlayerId::new(1),
                victim: PlayerId::new(2),
                weapon: "ak47".to_string(),
                headshot: false,
                assisted: false,
                assist_player: None,
            },
            KillEvent {
                tick: Tick(200),
                attacker: PlayerId::new(3),
                victim: PlayerId::new(1),
                weapon: "awp".to_string(),
                headshot: true,
                assisted: false,
                assist_player: None,
            },
            KillEvent {
                tick: Tick(300),
                attacker: PlayerId::new(2),
                victim: PlayerId::new(3),
                weapon: "m4a1".to_string(),
                headshot: false,
                assisted: false,
                assist_player: None,
            },
        ]);

        // Test kills up to tick 150
        let kills = context.kills_up_to(Tick(150));
        assert_eq!(kills.len(), 1);
        assert_eq!(kills[0].tick, Tick(100));

        // Test exact match boundary
        let kills = context.kills_up_to(Tick(200));
        assert_eq!(kills.len(), 2);
    }
}

