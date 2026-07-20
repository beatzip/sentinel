use serde::{Deserialize, Serialize};

use sentinel_core::Vec3;

/// 2D line segment for wall detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallSegment {
    pub start: Vec2,
    pub end: Vec2,
    pub material: Material,
}

/// 2D point for geometry
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Material type for visibility calculations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Material {
    Solid,
    Glass,
    Wood,
    Metal,
    Flesh,
}

/// Spawn point for a team
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnPoint {
    pub position: Vec3,
    pub team: SpawnTeam,
    pub angle: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpawnTeam {
    T,
    CT,
}

/// Bombsite definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bombsite {
    pub name: char,
    pub center: Vec2,
    pub polygon: Vec<Vec2>,
    pub radius: f32,
}

/// Navigation mesh node for pathfinding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavNode {
    pub id: u32,
    pub center: Vec3,
    pub connections: Vec<u32>,
}

/// Complete map data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapData {
    /// Map name
    pub name: String,
    /// Map boundaries (min_x, min_y, max_x, max_y)
    pub bounds: (f32, f32, f32, f32),
    /// Wall segments for line-of-sight
    pub walls: Vec<WallSegment>,
    /// Spawn points
    pub spawns: Vec<SpawnPoint>,
    /// Bombsites
    pub bombsites: Vec<Bombsite>,
    /// Navigation nodes
    pub nav_nodes: Vec<NavNode>,
}

impl MapData {
    /// Check if a line between two points intersects any wall
    pub fn line_blocked(&self, from: Vec2, to: Vec2) -> bool {
        self.walls
            .iter()
            .any(|wall| segments_intersect(from, to, wall.start, wall.end))
    }

    /// Get the closest spawn point to a position
    pub fn closest_spawn(&self, pos: Vec3, team: SpawnTeam) -> Option<&SpawnPoint> {
        self.spawns
            .iter()
            .filter(|s| s.team == team)
            .min_by(|a, b| {
                let dist_a = pos.distance_to(&Vec3::new(a.position.x, a.position.y, a.position.z));
                let dist_b = pos.distance_to(&Vec3::new(b.position.x, b.position.y, b.position.z));
                dist_a
                    .partial_cmp(&dist_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get bombsite by name
    pub fn bombsite(&self, name: char) -> Option<&Bombsite> {
        self.bombsites.iter().find(|b| b.name == name)
    }

    /// Check if a position is inside a bombsite
    pub fn in_bombsite(&self, pos: Vec2) -> Option<&Bombsite> {
        self.bombsites.iter().find(|b| {
            let dx = pos.x - b.center.x;
            let dy = pos.y - b.center.y;
            (dx * dx + dy * dy).sqrt() <= b.radius
        })
    }

    /// Create a default map for de_dust2
    pub fn dust2() -> Self {
        Self {
            name: "de_dust2".to_string(),
            bounds: (-2476.0, -1261.0, 2123.0, 4549.0),
            walls: Self::dust2_walls(),
            spawns: Self::dust2_spawns(),
            bombsites: Self::dust2_bombsites(),
            nav_nodes: Vec::new(),
        }
    }

    /// Create a default map for de_mirage
    pub fn mirage() -> Self {
        Self {
            name: "de_mirage".to_string(),
            bounds: (-3230.0, -3165.0, 1870.0, 4515.0),
            walls: Self::mirage_walls(),
            spawns: Self::mirage_spawns(),
            bombsites: Self::mirage_bombsites(),
            nav_nodes: Vec::new(),
        }
    }

    /// Create a default map for de_inferno
    pub fn inferno() -> Self {
        Self {
            name: "de_inferno".to_string(),
            bounds: (-2087.0, -1117.0, 2912.0, 4523.0),
            walls: Self::inferno_walls(),
            spawns: Self::inferno_spawns(),
            bombsites: Self::inferno_bombsites(),
            nav_nodes: Vec::new(),
        }
    }

    fn dust2_walls() -> Vec<WallSegment> {
        // Simplified dust2 walls - major structures only
        vec![
            // A site walls
            WallSegment {
                start: Vec2::new(1400.0, 2500.0),
                end: Vec2::new(1400.0, 3200.0),
                material: Material::Solid,
            },
            WallSegment {
                start: Vec2::new(1400.0, 3200.0),
                end: Vec2::new(1800.0, 3200.0),
                material: Material::Solid,
            },
            // B site walls
            WallSegment {
                start: Vec2::new(-1800.0, 2500.0),
                end: Vec2::new(-1800.0, 3200.0),
                material: Material::Solid,
            },
            WallSegment {
                start: Vec2::new(-1800.0, 3200.0),
                end: Vec2::new(-1400.0, 3200.0),
                material: Material::Solid,
            },
            // Mid walls
            WallSegment {
                start: Vec2::new(-200.0, 1000.0),
                end: Vec2::new(-200.0, 2000.0),
                material: Material::Solid,
            },
            WallSegment {
                start: Vec2::new(200.0, 1000.0),
                end: Vec2::new(200.0, 2000.0),
                material: Material::Solid,
            },
        ]
    }

    fn dust2_spawns() -> Vec<SpawnPoint> {
        vec![
            // T spawns
            SpawnPoint {
                position: Vec3::new(-500.0, 1000.0, 0.0),
                team: SpawnTeam::T,
                angle: 90.0,
            },
            SpawnPoint {
                position: Vec3::new(-400.0, 1000.0, 0.0),
                team: SpawnTeam::T,
                angle: 90.0,
            },
            SpawnPoint {
                position: Vec3::new(-300.0, 1000.0, 0.0),
                team: SpawnTeam::T,
                angle: 90.0,
            },
            // CT spawns
            SpawnPoint {
                position: Vec3::new(0.0, 3500.0, 0.0),
                team: SpawnTeam::CT,
                angle: -90.0,
            },
            SpawnPoint {
                position: Vec3::new(100.0, 3500.0, 0.0),
                team: SpawnTeam::CT,
                angle: -90.0,
            },
            SpawnPoint {
                position: Vec3::new(200.0, 3500.0, 0.0),
                team: SpawnTeam::CT,
                angle: -90.0,
            },
        ]
    }

    fn dust2_bombsites() -> Vec<Bombsite> {
        vec![
            Bombsite {
                name: 'A',
                center: Vec2::new(1600.0, 2900.0),
                polygon: vec![
                    Vec2::new(1400.0, 2700.0),
                    Vec2::new(1800.0, 2700.0),
                    Vec2::new(1800.0, 3100.0),
                    Vec2::new(1400.0, 3100.0),
                ],
                radius: 250.0,
            },
            Bombsite {
                name: 'B',
                center: Vec2::new(-1600.0, 2900.0),
                polygon: vec![
                    Vec2::new(-1800.0, 2700.0),
                    Vec2::new(-1400.0, 2700.0),
                    Vec2::new(-1400.0, 3100.0),
                    Vec2::new(-1800.0, 3100.0),
                ],
                radius: 250.0,
            },
        ]
    }

    fn mirage_walls() -> Vec<WallSegment> {
        vec![
            WallSegment {
                start: Vec2::new(0.0, 2000.0),
                end: Vec2::new(0.0, 3000.0),
                material: Material::Solid,
            },
            WallSegment {
                start: Vec2::new(-1000.0, 2500.0),
                end: Vec2::new(1000.0, 2500.0),
                material: Material::Solid,
            },
        ]
    }

    fn mirage_spawns() -> Vec<SpawnPoint> {
        vec![
            SpawnPoint {
                position: Vec3::new(-500.0, 500.0, 0.0),
                team: SpawnTeam::T,
                angle: 90.0,
            },
            SpawnPoint {
                position: Vec3::new(0.0, 3500.0, 0.0),
                team: SpawnTeam::CT,
                angle: -90.0,
            },
        ]
    }

    fn mirage_bombsites() -> Vec<Bombsite> {
        vec![
            Bombsite {
                name: 'A',
                center: Vec2::new(500.0, 2500.0),
                polygon: vec![
                    Vec2::new(300.0, 2300.0),
                    Vec2::new(700.0, 2300.0),
                    Vec2::new(700.0, 2700.0),
                    Vec2::new(300.0, 2700.0),
                ],
                radius: 200.0,
            },
            Bombsite {
                name: 'B',
                center: Vec2::new(-500.0, 2500.0),
                polygon: vec![
                    Vec2::new(-700.0, 2300.0),
                    Vec2::new(-300.0, 2300.0),
                    Vec2::new(-300.0, 2700.0),
                    Vec2::new(-700.0, 2700.0),
                ],
                radius: 200.0,
            },
        ]
    }

    fn inferno_walls() -> Vec<WallSegment> {
        vec![
            WallSegment {
                start: Vec2::new(0.0, 1500.0),
                end: Vec2::new(0.0, 2500.0),
                material: Material::Solid,
            },
            WallSegment {
                start: Vec2::new(-800.0, 2000.0),
                end: Vec2::new(800.0, 2000.0),
                material: Material::Solid,
            },
        ]
    }

    fn inferno_spawns() -> Vec<SpawnPoint> {
        vec![
            SpawnPoint {
                position: Vec3::new(-400.0, 400.0, 0.0),
                team: SpawnTeam::T,
                angle: 90.0,
            },
            SpawnPoint {
                position: Vec3::new(0.0, 3000.0, 0.0),
                team: SpawnTeam::CT,
                angle: -90.0,
            },
        ]
    }

    fn inferno_bombsites() -> Vec<Bombsite> {
        vec![
            Bombsite {
                name: 'A',
                center: Vec2::new(400.0, 2200.0),
                polygon: vec![
                    Vec2::new(200.0, 2000.0),
                    Vec2::new(600.0, 2000.0),
                    Vec2::new(600.0, 2400.0),
                    Vec2::new(200.0, 2400.0),
                ],
                radius: 200.0,
            },
            Bombsite {
                name: 'B',
                center: Vec2::new(-400.0, 2200.0),
                polygon: vec![
                    Vec2::new(-600.0, 2000.0),
                    Vec2::new(-200.0, 2000.0),
                    Vec2::new(-200.0, 2400.0),
                    Vec2::new(-600.0, 2400.0),
                ],
                radius: 200.0,
            },
        ]
    }
}

/// Check if two line segments intersect
fn segments_intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
    let d1 = direction(b1, b2, a1);
    let d2 = direction(b1, b2, a2);
    let d3 = direction(a1, a2, b1);
    let d4 = direction(a1, a2, b2);

    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

fn direction(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dust2_creation() {
        let map = MapData::dust2();
        assert_eq!(map.name, "de_dust2");
        assert!(!map.walls.is_empty());
        assert!(!map.spawns.is_empty());
        assert_eq!(map.bombsites.len(), 2);
    }

    #[test]
    fn test_line_blocked() {
        let map = MapData::dust2();
        // Line crossing a wall should be blocked
        let blocked = map.line_blocked(Vec2::new(1300.0, 2800.0), Vec2::new(1500.0, 2800.0));
        assert!(blocked);
    }

    #[test]
    fn test_bombsite_lookup() {
        let map = MapData::dust2();
        assert!(map.bombsite('A').is_some());
        assert!(map.bombsite('B').is_some());
        assert!(map.bombsite('C').is_none());
    }

    #[test]
    fn test_in_bombsite() {
        let map = MapData::dust2();
        let in_a = map.in_bombsite(Vec2::new(1600.0, 2900.0));
        assert!(in_a.is_some());
        assert_eq!(in_a.unwrap().name, 'A');
    }
}
