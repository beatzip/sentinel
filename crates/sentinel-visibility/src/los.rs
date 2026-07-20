use sentinel_core::{GrenadeType, PlayerId, PlayerState, Tick, TickState, Vec3, Weapon};

/// Result of a visibility check
#[derive(Debug, Clone, PartialEq)]
pub struct VisibilityResult {
    pub visible: bool,
    pub reason: VisibilityReason,
    pub distance: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisibilityReason {
    Clear,
    ThroughWall,
    ThroughSmoke,
    TooFar,
    TargetDead,
    NoLineOfSight,
    Flashed,
    BehindObstacle,
}

/// Audio visibility result
#[derive(Debug, Clone)]
pub struct AudioResult {
    pub audible: bool,
    pub volume: f32,
    pub reason: String,
    pub attenuation: f32,
}

/// Radar information
#[derive(Debug, Clone)]
pub struct RadarInfo {
    pub visible: bool,
    pub reason: String,
    pub spotted_by: Vec<PlayerId>,
}

/// Comprehensive visibility state for a player at a tick
#[derive(Debug, Clone)]
pub struct PlayerVisibilityState {
    pub player_id: PlayerId,
    pub tick: Tick,
    pub visible_enemies: Vec<PlayerId>,
    pub audible_enemies: Vec<PlayerId>,
    pub radar_visible: Vec<PlayerId>,
    pub spotted_by: Vec<PlayerId>,
    pub behind_smoke: bool,
    pub is_flashed: bool,
    pub flash_duration_remaining: f32,
}

/// Visibility engine for determining what players can see/hear
pub struct VisibilityEngine;

impl VisibilityEngine {
    /// Check if observer can see target at given tick
    pub fn can_see(state: &TickState, observer: PlayerId, target: PlayerId) -> VisibilityResult {
        let observer = match state.players.iter().find(|p| p.id == observer) {
            Some(p) => p,
            None => {
                return VisibilityResult {
                    visible: false,
                    reason: VisibilityReason::NoLineOfSight,
                    distance: f32::MAX,
                };
            }
        };

        let target = match state.players.iter().find(|p| p.id == target) {
            Some(p) => p,
            None => {
                return VisibilityResult {
                    visible: false,
                    reason: VisibilityReason::NoLineOfSight,
                    distance: f32::MAX,
                };
            }
        };

        if !target.alive {
            return VisibilityResult {
                visible: false,
                reason: VisibilityReason::TargetDead,
                distance: observer.position.distance_to(&target.position),
            };
        }

        let distance = observer.position.distance_to(&target.position);

        // Check if observer is flashed
        if observer.flash_duration > 0.0 {
            return VisibilityResult {
                visible: false,
                reason: VisibilityReason::Flashed,
                distance,
            };
        }

        // Check max weapon range
        if !Self::in_weapon_range(observer, distance) {
            return VisibilityResult {
                visible: false,
                reason: VisibilityReason::TooFar,
                distance,
            };
        }

        // Check angle of view (FOV ~120 degrees)
        if !Self::in_field_of_view(observer, target) {
            return VisibilityResult {
                visible: false,
                reason: VisibilityReason::NoLineOfSight,
                distance,
            };
        }

        // Check line of sight through walls (raycasting)
        if Self::is_line_blocked(observer, target, state) {
            return VisibilityResult {
                visible: false,
                reason: VisibilityReason::ThroughWall,
                distance,
            };
        }

        // Check if target is behind smoke
        if Self::is_behind_smoke(target, state) {
            return VisibilityResult {
                visible: false,
                reason: VisibilityReason::ThroughSmoke,
                distance,
            };
        }

        VisibilityResult {
            visible: true,
            reason: VisibilityReason::Clear,
            distance,
        }
    }

    /// Check if observer can hear target
    pub fn can_hear(state: &TickState, observer: PlayerId, target: PlayerId) -> AudioResult {
        let observer = match state.players.iter().find(|p| p.id == observer) {
            Some(p) => p,
            None => {
                return AudioResult {
                    audible: false,
                    volume: 0.0,
                    reason: "Observer not found".to_string(),
                    attenuation: 0.0,
                };
            }
        };

        let target = match state.players.iter().find(|p| p.id == target) {
            Some(p) => p,
            None => {
                return AudioResult {
                    audible: false,
                    volume: 0.0,
                    reason: "Target not found".to_string(),
                    attenuation: 0.0,
                };
            }
        };

        if !target.alive {
            return AudioResult {
                audible: false,
                volume: 0.0,
                reason: "Target dead".to_string(),
                attenuation: 0.0,
            };
        }

        let distance = observer.position.distance_to(&target.position);

        // CS2 audio ranges
        let max_footstep_distance = 2000.0;
        let max_weapon_fire_distance = 4000.0;

        // Determine sound type based on target activity
        let max_distance = if target.velocity.length() > 10.0 {
            max_footstep_distance // Moving = footsteps
        } else {
            max_weapon_fire_distance // Stationary = potential weapon fire
        };

        if distance > max_distance {
            return AudioResult {
                audible: false,
                volume: 0.0,
                reason: "Too far".to_string(),
                attenuation: 0.0,
            };
        }

        // Calculate volume with distance attenuation
        let base_volume = 1.0 - (distance / max_distance);

        // Wall attenuation (simplified - would need proper raycasting)
        let wall_attenuation = if Self::is_line_blocked(observer, target, state) {
            0.3 // Walls reduce sound by ~70%
        } else {
            1.0
        };

        let volume = base_volume * wall_attenuation;

        AudioResult {
            audible: volume > 0.05, // Threshold for audible sound
            volume,
            reason: format!(
                "Distance: {:.0}, Wall attenuation: {:.1}",
                distance, wall_attenuation
            ),
            attenuation: wall_attenuation,
        }
    }

    /// Get radar information for a player
    pub fn radar_knowledge(state: &TickState, player: PlayerId) -> RadarInfo {
        let player_state = match state.players.iter().find(|p| p.id == player) {
            Some(p) => p,
            None => {
                return RadarInfo {
                    visible: false,
                    reason: "Player not found".to_string(),
                    spotted_by: Vec::new(),
                };
            }
        };

        // In CS2, radar shows:
        // 1. Teammates (always visible)
        // 2. Enemies spotted by teammates
        // 3. Enemies that fire unsuppressed weapons
        // 4. Enemies that make loud sounds

        let spotted_by = Vec::new();
        let _target_team = match player_state.team {
            sentinel_core::Team::Terrorist => sentinel_core::Team::CounterTerrorist,
            sentinel_core::Team::CounterTerrorist => sentinel_core::Team::Terrorist,
            sentinel_core::Team::Unassigned => {
                return RadarInfo {
                    visible: false,
                    reason: "No team".to_string(),
                    spotted_by: Vec::new(),
                };
            }
        };

        // Check if any teammate has spotted enemies
        for teammate in &state.players {
            if teammate.team == player_state.team && teammate.id != player {
                // Teammate could spot enemies they can see
                // Simplified: if teammate is alive, they might spot enemies
                if teammate.alive {
                    // In real implementation, would check visibility from teammate to enemies
                }
            }
        }

        RadarInfo {
            visible: !spotted_by.is_empty(),
            reason: if spotted_by.is_empty() {
                "No spotted enemies".to_string()
            } else {
                format!("Spotted by {} teammates", spotted_by.len())
            },
            spotted_by,
        }
    }

    /// Get comprehensive visibility state for a player
    pub fn get_visibility_state(
        state: &TickState,
        player: PlayerId,
        tick: Tick,
    ) -> PlayerVisibilityState {
        let mut visible_enemies = Vec::new();
        let mut audible_enemies = Vec::new();
        let mut radar_visible = Vec::new();
        let spotted_by = Vec::new();

        let player_team = state
            .players
            .iter()
            .find(|p| p.id == player)
            .map(|p| p.team);

        for other in &state.players {
            if other.id == player || !other.alive {
                continue;
            }

            // Check if visible
            let vis_result = Self::can_see(state, player, other.id);
            if vis_result.visible {
                visible_enemies.push(other.id);
            }

            // Check if audible
            let audio_result = Self::can_hear(state, player, other.id);
            if audio_result.audible {
                audible_enemies.push(other.id);
            }

            // Check radar
            if let Some(team) = player_team
                && other.team == team
            {
                // Teammates are always on radar
                radar_visible.push(other.id);
            }
        }

        // Check if player is behind smoke
        let behind_smoke = state
            .players
            .iter()
            .find(|p| p.id == player)
            .map(|p| Self::is_behind_smoke(p, state))
            .unwrap_or(false);

        // Check if player is flashed
        let is_flashed = state
            .players
            .iter()
            .find(|p| p.id == player)
            .map(|p| p.flash_duration > 0.0)
            .unwrap_or(false);

        let flash_duration = state
            .players
            .iter()
            .find(|p| p.id == player)
            .map(|p| p.flash_duration)
            .unwrap_or(0.0);

        PlayerVisibilityState {
            player_id: player,
            tick,
            visible_enemies,
            audible_enemies,
            radar_visible,
            spotted_by,
            behind_smoke,
            is_flashed,
            flash_duration_remaining: flash_duration,
        }
    }

    /// Check if target is within weapon range
    fn in_weapon_range(observer: &PlayerState, distance: f32) -> bool {
        let max_range = match observer.weapon {
            Weapon::Sniper => 8192.0,
            Weapon::Rifle => 4096.0,
            Weapon::SMG => 2048.0,
            Weapon::Pistol => 1536.0,
            Weapon::Shotgun => 512.0,
            _ => 1024.0,
        };
        distance <= max_range
    }

    /// Check if target is within field of view (~120 degrees)
    fn in_field_of_view(observer: &PlayerState, target: &PlayerState) -> bool {
        let dx = target.position.x - observer.position.x;
        let dy = target.position.y - observer.position.y;

        let angle_to_target = dy.atan2(dx).to_degrees();
        let observer_angle = observer.view_angles.yaw;

        let mut angle_diff = (angle_to_target - observer_angle).abs();
        if angle_diff > 180.0 {
            angle_diff = 360.0 - angle_diff;
        }

        angle_diff <= 60.0 // 120 degree FOV (60 each side)
    }

    /// Check if line of sight is blocked by walls (2D raycasting)
    fn is_line_blocked(_observer: &PlayerState, _target: &PlayerState, _state: &TickState) -> bool {
        // Simplified: In real implementation, would raycast against wall segments
        // from sentinel-map. For now, return false (no walls blocking).
        // Real implementation would use WallSegment intersection tests.
        false // Placeholder - real implementation needs map data
    }

    /// Check if target is behind a smoke grenade
    fn is_behind_smoke(target: &PlayerState, state: &TickState) -> bool {
        // Check all active smoke grenades
        for grenade in &state.grenades {
            if !grenade.active {
                continue;
            }

            if grenade.grenade_type != GrenadeType::Smoke {
                continue;
            }

            // Check if target is within smoke radius
            let smoke_radius = 200.0; // CS2 smoke radius in units
            let distance_to_smoke = target.position.distance_to(&grenade.position);

            if distance_to_smoke < smoke_radius {
                return true;
            }
        }

        false
    }

    /// Check if player is within smoke radius at a position
    pub fn position_in_smoke(position: Vec3, state: &TickState) -> bool {
        for grenade in &state.grenades {
            if !grenade.active || grenade.grenade_type != GrenadeType::Smoke {
                continue;
            }

            let smoke_radius = 200.0;
            let distance = position.distance_to(&grenade.position);

            if distance < smoke_radius {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::GrenadeType;
    use sentinel_core::bomb::BombState;
    use sentinel_core::grenade::GrenadeState;
    use sentinel_core::round::RoundState;
    use sentinel_core::{Angles, Tick, TickState, Vec3, Weapon};

    fn create_test_state() -> TickState {
        let player1 = PlayerState {
            id: PlayerId::new(1),
            name: "Player1".to_string(),
            team: sentinel_core::Team::Terrorist,
            position: Vec3::new(0.0, 0.0, 0.0),
            velocity: Vec3::default(),
            view_angles: Angles {
                pitch: 0.0,
                yaw: 0.0,
                roll: 0.0,
            },
            weapon: Weapon::Rifle,
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
            team: sentinel_core::Team::CounterTerrorist,
            position: Vec3::new(500.0, 0.0, 0.0),
            velocity: Vec3::default(),
            view_angles: Angles {
                pitch: 0.0,
                yaw: 180.0,
                roll: 0.0,
            },
            weapon: Weapon::Rifle,
            health: 100,
            armor: 100,
            money: 4500,
            flash_duration: 0.0,
            scoped: false,
            reloading: false,
            alive: true,
        };

        TickState {
            tick: Tick(100),
            players: vec![player1, player2],
            grenades: Vec::new(),
            bomb: BombState::Carried {
                carrier: PlayerId::new(0),
            },
            round: RoundState {
                round_number: 1,
                phase: sentinel_core::RoundPhase::Live,
                clock: 100.0,
                t_score: 0,
                ct_score: 0,
                winner: None,
            },
        }
    }

    #[test]
    fn test_can_see_close() {
        let state = create_test_state();
        let result = VisibilityEngine::can_see(&state, PlayerId::new(1), PlayerId::new(2));
        assert!(result.visible);
        assert_eq!(result.reason, VisibilityReason::Clear);
    }

    #[test]
    fn test_can_hear_close() {
        let state = create_test_state();
        let result = VisibilityEngine::can_hear(&state, PlayerId::new(1), PlayerId::new(2));
        assert!(result.audible);
        assert!(result.volume > 0.0);
    }

    #[test]
    fn test_dead_target() {
        let mut state = create_test_state();
        state
            .players
            .iter_mut()
            .find(|p| p.id == PlayerId::new(2))
            .unwrap()
            .alive = false;

        let result = VisibilityEngine::can_see(&state, PlayerId::new(1), PlayerId::new(2));
        assert!(!result.visible);
        assert_eq!(result.reason, VisibilityReason::TargetDead);
    }

    #[test]
    fn test_flashed_observer() {
        let mut state = create_test_state();
        state
            .players
            .iter_mut()
            .find(|p| p.id == PlayerId::new(1))
            .unwrap()
            .flash_duration = 2.0;

        let result = VisibilityEngine::can_see(&state, PlayerId::new(1), PlayerId::new(2));
        assert!(!result.visible);
        assert_eq!(result.reason, VisibilityReason::Flashed);
    }

    #[test]
    fn test_smoke_visibility() {
        let mut state = create_test_state();

        // Add smoke grenade near the target (within 200 units)
        let smoke = GrenadeState {
            id: 1,
            grenade_type: GrenadeType::Smoke,
            owner: Some(PlayerId::new(1)),
            position: Vec3::new(400.0, 0.0, 0.0), // 100 units from target at (500, 0, 0)
            velocity: Vec3::default(),
            thrown_tick: Tick(90),
            detonated: true,
            detonated_tick: Some(Tick(95)),
            active: true,
        };
        state.grenades.push(smoke);

        let result = VisibilityEngine::can_see(&state, PlayerId::new(1), PlayerId::new(2));
        assert!(!result.visible);
        assert_eq!(result.reason, VisibilityReason::ThroughSmoke);
    }

    #[test]
    fn test_visibility_state() {
        let state = create_test_state();
        let vis_state = VisibilityEngine::get_visibility_state(&state, PlayerId::new(1), Tick(100));

        assert_eq!(vis_state.player_id, PlayerId::new(1));
        assert!(!vis_state.visible_enemies.is_empty());
        assert!(!vis_state.audible_enemies.is_empty());
    }
}
