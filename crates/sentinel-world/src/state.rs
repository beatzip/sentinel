use sentinel_core::{
    BombState, GrenadeState, GrenadeType, KillEvent, PlayerId, PlayerState, RoundPhase, RoundState,
    Tick,
};
use std::collections::BTreeMap;

/// Complete world state at a specific point in time
#[derive(Debug, Clone)]
pub struct WorldState {
    /// Current tick
    pub tick: Tick,
    /// All players
    pub players: BTreeMap<PlayerId, PlayerState>,
    /// All active grenades
    pub grenades: Vec<GrenadeState>,
    /// Bomb state
    pub bomb: BombState,
    /// Current round
    pub round: RoundState,
    /// Kill feed for current round
    pub kill_feed: Vec<KillEvent>,
}

impl WorldState {
    /// Create a new empty world state
    pub fn new(tick: Tick) -> Self {
        Self {
            tick,
            players: BTreeMap::new(),
            grenades: Vec::new(),
            bomb: BombState::Carried {
                carrier: PlayerId::new(0),
            },
            round: RoundState {
                round_number: 1,
                phase: RoundPhase::Freezetime,
                clock: 115.0,
                t_score: 0,
                ct_score: 0,
                winner: None,
                start_tick: 0,
            },
            kill_feed: Vec::new(),
        }
    }

    /// Get a player by ID
    pub fn player(&self, id: PlayerId) -> Option<&PlayerState> {
        self.players.get(&id)
    }

    /// Get all alive players
    pub fn alive_players(&self) -> Vec<&PlayerState> {
        self.players.values().filter(|p| p.alive).collect()
    }

    /// Get players on a specific team
    pub fn team_players(&self, team: sentinel_core::Team) -> Vec<&PlayerState> {
        self.players
            .values()
            .filter(|p| p.team == team && p.alive)
            .collect()
    }

    /// Get the number of alive players on each team
    pub fn alive_counts(&self) -> (usize, usize) {
        let t_count = self
            .players
            .values()
            .filter(|p| p.team == sentinel_core::Team::Terrorist && p.alive)
            .count();
        let ct_count = self
            .players
            .values()
            .filter(|p| p.team == sentinel_core::Team::CounterTerrorist && p.alive)
            .count();
        (t_count, ct_count)
    }

    /// Get all active grenades
    pub fn active_grenades(&self) -> Vec<&GrenadeState> {
        self.grenades.iter().filter(|g| g.active).collect()
    }

    /// Get grenades of a specific type
    pub fn grenades_by_type(&self, grenade_type: GrenadeType) -> Vec<&GrenadeState> {
        self.grenades
            .iter()
            .filter(|g| g.active && g.grenade_type == grenade_type)
            .collect()
    }

    /// Check if a player is visible from another player's position
    /// This is a simplified check - real implementation would use raycasting
    pub fn is_visible(&self, observer: PlayerId, target: PlayerId) -> bool {
        let observer = match self.player(observer) {
            Some(p) => p,
            None => return false,
        };
        let target = match self.player(target) {
            Some(p) => p,
            None => return false,
        };

        if !target.alive {
            return false;
        }

        // Simple distance-based visibility check
        let distance = observer.position.distance_to(&target.position);
        distance < 1000.0 // Simplified threshold
    }

    /// Get the closest enemy to a player
    pub fn closest_enemy(&self, player_id: PlayerId) -> Option<&PlayerState> {
        let player = self.player(player_id)?;
        let target_team = match player.team {
            sentinel_core::Team::Terrorist => sentinel_core::Team::CounterTerrorist,
            sentinel_core::Team::CounterTerrorist => sentinel_core::Team::Terrorist,
            sentinel_core::Team::Unassigned => return None,
        };

        self.team_players(target_team).into_iter().min_by(|a, b| {
            let dist_a = player.position.distance_to(&a.position);
            let dist_b = player.position.distance_to(&b.position);
            dist_a
                .partial_cmp(&dist_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Add a kill event to the kill feed
    pub fn add_kill(&mut self, kill: KillEvent) {
        self.kill_feed.push(kill);
    }

    /// Get all kills in the current round
    pub fn round_kills(&self) -> Vec<&KillEvent> {
        self.kill_feed.iter().collect()
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self::new(Tick(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{Team, Vec3};

    fn create_test_world() -> WorldState {
        let mut world = WorldState::new(Tick(100));

        let player1 = PlayerState {
            id: PlayerId::new(1),
            name: "Player1".to_string(),
            team: Team::Terrorist,
            position: Vec3::new(0.0, 0.0, 0.0),
            velocity: Vec3::default(),
            view_angles: sentinel_core::Angles::default(),
            weapon: sentinel_core::Weapon::Rifle,
            health: 100,
            armor: 100,
            money: 4500,
            flash_duration: 0.0,
            scoped: false,
            reloading: false,
            alive: true,
        };

        let player2 = PlayerState {
            id: PlayerId::new(2),
            name: "Player2".to_string(),
            team: Team::CounterTerrorist,
            position: Vec3::new(500.0, 0.0, 0.0),
            velocity: Vec3::default(),
            view_angles: sentinel_core::Angles::default(),
            weapon: sentinel_core::Weapon::Rifle,
            health: 100,
            armor: 100,
            money: 4500,
            flash_duration: 0.0,
            scoped: false,
            reloading: false,
            alive: true,
        };

        world.players.insert(player1.id, player1);
        world.players.insert(player2.id, player2);

        world
    }

    #[test]
    fn test_world_state_creation() {
        let world = WorldState::new(Tick(100));
        assert_eq!(world.tick, Tick(100));
        assert!(world.players.is_empty());
    }

    #[test]
    fn test_player_lookup() {
        let world = create_test_world();
        assert!(world.player(PlayerId::new(1)).is_some());
        assert!(world.player(PlayerId::new(999)).is_none());
    }

    #[test]
    fn test_alive_players() {
        let world = create_test_world();
        let alive = world.alive_players();
        assert_eq!(alive.len(), 2);
    }

    #[test]
    fn test_team_players() {
        let world = create_test_world();
        let t_players = world.team_players(Team::Terrorist);
        assert_eq!(t_players.len(), 1);
        assert_eq!(t_players[0].id, PlayerId::new(1));

        let ct_players = world.team_players(Team::CounterTerrorist);
        assert_eq!(ct_players.len(), 1);
        assert_eq!(ct_players[0].id, PlayerId::new(2));
    }

    #[test]
    fn test_alive_counts() {
        let world = create_test_world();
        let (t, ct) = world.alive_counts();
        assert_eq!(t, 1);
        assert_eq!(ct, 1);
    }
}
