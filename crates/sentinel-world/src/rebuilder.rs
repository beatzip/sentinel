use sentinel_core::{Angles, PlayerId, PlayerState, RoundPhase, Tick, TickState, Vec3, Weapon};
use sentinel_events::kinds::{EventKind, EventValue, GameEvent};

use crate::state::WorldState;

/// Rebuilds world state from a stream of game events
pub struct WorldRebuilder {
    /// Current world state
    world: WorldState,
    /// History of world states (one per tick)
    states: Vec<TickState>,
}

impl WorldRebuilder {
    /// Create a new rebuilder starting from tick 0
    pub fn new() -> Self {
        Self {
            world: WorldState::new(Tick(0)),
            states: Vec::new(),
        }
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

            self.world.add_kill(crate::state::KillEvent {
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
    }

    /// Handle weapon fire event
    fn handle_weapon_fire(&mut self, _event: &GameEvent) {
        // Update player's weapon state if needed
        // In a real implementation, this would track ammo, fire rate, etc.
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
}
