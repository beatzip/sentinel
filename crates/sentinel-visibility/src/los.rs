use sentinel_core::{GrenadeType, PlayerId, PlayerState, Tick, TickState, Vec3, Weapon};
use sentinel_map::{MapData, Vec2};

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
    /// Check if observer can see target at given tick (uses default map)
    pub fn can_see(state: &TickState, observer: PlayerId, target: PlayerId) -> VisibilityResult {
        use sentinel_map::MapData;
        let map = MapData::dust2(); // Default map for backward compatibility
        Self::can_see_with_map(state, observer, target, &map)
    }

    /// Check if observer can see target at given tick with map data
    pub fn can_see_with_map(
        state: &TickState,
        observer: PlayerId,
        target: PlayerId,
        map: &MapData,
    ) -> VisibilityResult {
        let obs = match state.players.iter().find(|p| p.id == observer) {
            Some(p) => p,
            None => {
                return VisibilityResult {
                    visible: false,
                    reason: VisibilityReason::NoLineOfSight,
                    distance: f32::MAX,
                };
            }
        };

        let tgt = match state.players.iter().find(|p| p.id == target) {
            Some(p) => p,
            None => {
                return VisibilityResult {
                    visible: false,
                    reason: VisibilityReason::NoLineOfSight,
                    distance: f32::MAX,
                };
            }
        };

        if !tgt.alive {
            return VisibilityResult {
                visible: false,
                reason: VisibilityReason::TargetDead,
                distance: obs.position.distance_to(&tgt.position),
            };
        }

        let distance = obs.position.distance_to(&tgt.position);

        // Check if observer is flashed
        if obs.flash_duration > 0.0 {
            return VisibilityResult {
                visible: false,
                reason: VisibilityReason::Flashed,
                distance,
            };
        }

        // Check max weapon range
        if !Self::in_weapon_range(obs, distance) {
            return VisibilityResult {
                visible: false,
                reason: VisibilityReason::TooFar,
                distance,
            };
        }

        // Check angle of view (FOV ~120 degrees)
        if !Self::in_field_of_view(obs, tgt) {
            return VisibilityResult {
                visible: false,
                reason: VisibilityReason::NoLineOfSight,
                distance,
            };
        }

        // Check line of sight through walls (raycasting)
        if Self::is_line_blocked(obs, tgt, map) {
            return VisibilityResult {
                visible: false,
                reason: VisibilityReason::ThroughWall,
                distance,
            };
        }

        // Check if target is behind smoke
        if Self::is_behind_smoke(tgt, state) {
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

        // Wall attenuation - now uses proper raycasting
        // Use a default map for backward compatibility
        let wall_attenuation = if Self::is_line_blocked(observer, target, &MapData::dust2()) {
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
    pub fn radar_knowledge(state: &TickState, player: PlayerId, map: &MapData) -> RadarInfo {
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

        let my_team = player_state.team;
        if my_team == sentinel_core::Team::Unassigned {
            return RadarInfo {
                visible: false,
                reason: "No team".to_string(),
                spotted_by: Vec::new(),
            };
        }

        // In CS2, radar shows:
        // 1. Teammates (always visible)
        // 2. Enemies spotted by teammates
        // 3. Enemies that fire unsuppressed weapons
        // 4. Enemies that make loud sounds
        //
        // Here we populate `spotted_by` with teammate IDs who can see enemies
        // (i.e., teammates that could provide intel to the player).

        let mut spotted_by = Vec::new();
        for teammate in &state.players {
            if teammate.team == my_team && teammate.id != player && teammate.alive {
                // Check if this teammate can see any enemies
                for other in &state.players {
                    if other.team != my_team && other.alive {
                        let vis = Self::can_see_with_map(state, teammate.id, other.id, map);
                        if vis.visible {
                            if !spotted_by.contains(&teammate.id) {
                                spotted_by.push(teammate.id);
                            }
                            break; // teammate spotted at least one enemy, move on
                        }
                    }
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
        map: &MapData,
    ) -> PlayerVisibilityState {
        let mut visible_enemies = Vec::new();
        let mut audible_enemies = Vec::new();
        let mut radar_visible = Vec::new();

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
            let vis_result = Self::can_see_with_map(state, player, other.id, map);
            if vis_result.visible {
                visible_enemies.push(other.id);
            }

            // Check if audible
            let audio_result = Self::can_hear(state, player, other.id);
            if audio_result.audible {
                audible_enemies.push(other.id);
            }
        }

        // Check radar: teammates that can see enemies (intel) + all alive teammates
        let radar_info = Self::radar_knowledge(state, player, map);
        for teammate_id in &radar_info.spotted_by {
            radar_visible.push(*teammate_id);
        }
        // Also add all alive teammates (always on radar in CS2)
        if let Some(team) = player_team {
            for other in &state.players {
                if other.team == team && other.id != player && other.alive && !radar_visible.contains(&other.id) {
                    radar_visible.push(other.id);
                }
            }
        }

        // Use radar_knowledge result for spotted_by (teammates that spotted enemies)
        let spotted_by = radar_info.spotted_by;

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

    /// Check if line of sight is blocked by walls (2D raycasting, or 3D BVH if available).
    /// 
    /// Priority order:
    /// 1. If both positions are inside nav mesh areas that are walkable-connected → not blocked
    /// 2. If BVH available (from .tri files), use 3D raycasting for most accurate results
    /// 3. Fallback to 2D wall checking
    fn is_line_blocked(observer: &PlayerState, target: &PlayerState, map: &MapData) -> bool {
        // Step 1: Try nav mesh connectivity check (fast + accurate for walkable paths)
        let from_2d = Vec2::new(observer.position.x, observer.position.y);
        let to_2d = Vec2::new(target.position.x, target.position.y);
        
        if let Some(from_area) = map.find_area_2d(from_2d) {
            if let Some(to_area) = map.find_area_2d(to_2d) {
                // Both inside nav areas — check connectivity
                if from_area == to_area {
                    // Same area = definitely visible
                    return false;
                }
                if map.can_walk_between(from_area, to_area) {
                    // Connected areas: do a direct wall check on the XY projection
                    // (connection may go around corners, but a clear direct line is visible)
                    if !map.line_blocked(from_2d, to_2d) {
                        return false;
                    }
                    // Line is blocked by walls even though areas are connected
                    // Fall through to BVH/2D checking for more accuracy
                }
            }
        }
        
        // Step 2: If we have a BVH (from .tri files), use 3D raycasting for accurate results
        if let Some(ref _bvh) = map.bvh {
            // Use 3D raycasting
            let from_map: sentinel_map::Vec3 = observer.position.into();
            let direction: sentinel_map::Vec3 = (target.position - observer.position).normalize().into();
            
            return map.line_blocked_3d(from_map, direction);
        }
        
        // Step 3: Fallback to 2D raycasting (ignore Z for wall checking)
        map.line_blocked(from_2d, to_2d)
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
    use sentinel_map::{MapData, Vec2};

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
                start_tick: 0,
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
            detonated_tick: Some(Tick(95)),
            start_tick: Some(Tick(95)),
            end_tick: Some(Tick(600)), // 8 seconds at 64 tick
            entity_id: Some(42),
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
        let map = MapData::dust2();
        let vis_state = VisibilityEngine::get_visibility_state(&state, PlayerId::new(1), Tick(100), &map);

        assert_eq!(vis_state.player_id, PlayerId::new(1));
        assert!(!vis_state.visible_enemies.is_empty());
        assert!(!vis_state.audible_enemies.is_empty());
    }

    // ==================== Wall Detection Tests ====================

    #[test]
    fn test_line_blocked_with_wall() {
        let state = create_test_state();
        let map = MapData::dust2();
        
        // Player at (0, 0), target at (500, 0)
        // Dust2 has a wall at x=1400 from y=2500 to y=3200
        // This shouldn't be blocked since we're at lower y values
        let result = VisibilityEngine::can_see_with_map(&state, PlayerId::new(1), PlayerId::new(2), &map);
        // The default dust2 walls are at high y values, this line shouldn't be blocked
        // But let's test with a line that crosses a wall
        assert!(result.visible || result.reason != VisibilityReason::ThroughWall);
    }

    #[test]
    fn test_line_blocked_true() {
        // Create a custom state with players positioned to cross a wall
        let player1 = PlayerState {
            id: PlayerId::new(1),
            name: "Player1".to_string(),
            team: sentinel_core::Team::Terrorist,
            position: Vec3::new(1300.0, 2800.0, 0.0), // Inside A site
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
            position: Vec3::new(1500.0, 2800.0, 0.0), // North of A site wall
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

        let state = TickState {
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
                start_tick: 0,
            },
        };

        let map = MapData::dust2();
        let result = VisibilityEngine::can_see_with_map(&state, PlayerId::new(1), PlayerId::new(2), &map);
        
        // The line from (1300, 2800) to (1500, 2800) crosses the wall at x=1400
        assert!(result.visible == false || result.reason == VisibilityReason::ThroughWall,
            "Expected ThroughWall but got: {:?}", result.reason);
    }

    #[test]
    fn test_line_not_blocked() {
        // Create a custom state with players positioned to NOT cross any wall
        let player1 = PlayerState {
            id: PlayerId::new(1),
            name: "Player1".to_string(),
            team: sentinel_core::Team::Terrorist,
            position: Vec3::new(-1000.0, -500.0, 0.0),
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
            position: Vec3::new(-500.0, -500.0, 0.0),
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

        let state = TickState {
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
                start_tick: 0,
            },
        };

        let map = MapData::dust2();
        let result = VisibilityEngine::can_see_with_map(&state, PlayerId::new(1), PlayerId::new(2), &map);
        
        // This line shouldn't cross any walls
        assert!(result.visible, "Expected visible but got reason: {:?}", result.reason);
    }

    #[test]
    fn test_wall_segment_intersection() {
        // Test the line_blocked function directly
        use sentinel_map::MapData;
        
        let map = MapData::dust2();
        
        // Create a line that should cross the mid wall at x=-200
        let from = Vec2::new(-300.0, 1500.0);
        let to = Vec2::new(-100.0, 1500.0);
        
        // This crosses the mid wall but let's check the actual walls
        // The mid wall is from (-200, 1000) to (-200, 2000)
        let blocked = map.line_blocked(from, to);
        // Our line goes from y=1500, crossing x=-200 at y=1500
        // The mid wall is from y=1000 to y=2000 at x=-200
        // So this should be blocked
        assert!(blocked, "Line from (-300,1500) to (-100,1500) should cross mid wall");
        
        // Create a line that doesn't cross any wall
        let from2 = Vec2::new(-500.0, -500.0);
        let to2 = Vec2::new(-300.0, -500.0);
        
        let blocked2 = map.line_blocked(from2, to2);
        assert!(!blocked2, "Line from (-500,-500) to (-300,-500) should not cross any wall");
    }

    #[test]
    fn test_segments_intersect() {
        // Test segment intersection logic
        let a1 = Vec2::new(0.0, 0.0);
        let a2 = Vec2::new(10.0, 0.0);
        let b1 = Vec2::new(5.0, -5.0);
        let b2 = Vec2::new(5.0, 5.0);
        
        // Horizontal segment from (0,0) to (10,0)
        // Vertical segment from (5,-5) to (5,5)
        // These should intersect at (5, 0)
        let blocked = sentinel_map::data::segments_intersect(a1, a2, b1, b2);
        assert!(blocked, "Segments should intersect");
        
        // Non-intersecting segments
        let a3 = Vec2::new(0.0, 0.0);
        let a4 = Vec2::new(5.0, 0.0);
        let c1 = Vec2::new(0.0, 10.0);
        let c2 = Vec2::new(10.0, 10.0);
        
        let blocked2 = sentinel_map::data::segments_intersect(a3, a4, c1, c2);
        assert!(!blocked2, "Segments should not intersect");
    }

    #[test]
    fn test_mirage_wall_detection() {
        let map = MapData::mirage();
        
        // Check if line_blocked works with mirage map
        let from = Vec2::new(0.0, 2000.0);
        let to = Vec2::new(0.0, 3000.0);
        
        // This is exactly on the mirage wall
        let blocked = map.line_blocked(from, to);
        assert!(blocked, "Line should be on or very close to wall");
    }

    #[test]
    fn test_inferno_wall_detection() {
        let map = MapData::inferno();
        
        // Check if line_blocked works with inferno map
        let from = Vec2::new(-500.0, 2000.0);
        let to = Vec2::new(500.0, 2000.0);
        
        // This crosses the inferno mid wall at y=2000
        let blocked = map.line_blocked(from, to);
        assert!(blocked, "Line should cross inferno mid wall");
    }
}

