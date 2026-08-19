use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use crate::{
    MapData, Material, NavNode, SpawnPoint, SpawnTeam, Vec2, Vec3, WallSegment, compute_bbox2d,
};

/// Represents a corner point from awpy nav format
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Vector3Dict {
    x: f64,
    y: f64,
    z: f64,
}

impl From<Vector3Dict> for Vec3 {
    fn from(v: Vector3Dict) -> Self {
        Vec3::new(v.x as f32, v.y as f32, v.z as f32)
    }
}

/// Check if two 2D edges are approximately equal (within tolerance)
/// Used in nav loading to detect shared polygon edges between areas
fn edges_equal(e1: &(Vec2, Vec2), e2: &(Vec2, Vec2)) -> bool {
    const TOL: f32 = 1.0; // 1 unit tolerance
    (e1.0.x - e2.0.x).abs() < TOL
        && (e1.0.y - e2.0.y).abs() < TOL
        && (e1.1.x - e2.1.x).abs() < TOL
        && (e1.1.y - e2.1.y).abs() < TOL
}

/// Navigation area from awpy nav JSON format
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NavAreaDict {
    area_id: u32,
    hull_index: u32,
    dynamic_attribute_flags: i64,
    corners: Vec<Vector3Dict>,
    connections: Vec<u32>,
    ladders_above: Vec<u32>,
    ladders_below: Vec<u32>,
}

/// Complete nav mesh structure from awpy
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NavMesh {
    version: u32,
    sub_version: u32,
    is_analyzed: bool,
    areas: HashMap<String, NavAreaDict>,
}

/// Load a nav mesh from JSON file and extract wall segments
pub fn load_map_from_nav(path: &std::path::Path) -> Result<MapData, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open nav file: {e}"))?;
    let reader = BufReader::new(file);

    let nav: NavMesh =
        serde_json::from_reader(reader).map_err(|e| format!("Failed to parse nav JSON: {e}"))?;

    // Extract map name from filename
    let map_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Collect edges from navigation areas
    let mut all_edges: Vec<(Vec2, Vec2)> = Vec::new();
    let mut spawn_points = Vec::new();

    // Process each navigation area
    for area in nav.areas.values() {
        // Get all corners for this area (3D)
        let corners: Vec<_> = area.corners.iter().map(|c| Vec3::from(*c)).collect();

        // Get 2D corners (XY plane)
        let corners_2d: Vec<_> = corners.iter().map(|c| Vec2::new(c.x, c.y)).collect();

        // Create edges from polygon perimeter
        let n = corners_2d.len();
        for i in 0..n {
            let j = (i + 1) % n;
            all_edges.push((corners_2d[i], corners_2d[j]));
        }

        // Add spawns from spawn areas (heuristic: areas with connection count 0 or special flags)
        if area.connections.is_empty() {
            // This might be a spawn point
            let centroid = arena_centroid(&corners);
            spawn_points.push(SpawnPoint {
                position: centroid,
                team: if area.dynamic_attribute_flags & 0x1 != 0 {
                    SpawnTeam::T
                } else {
                    SpawnTeam::CT
                },
                angle: 90.0,
            });
        }
    }

    // Convert edges to walls, removing those that are shared (connections)
    // Optimized version using HashMap for O(n) instead of O(n²) matching

    // Step 1: Index edges by their midpoint (rounded to tolerance grid)
    // This allows us to quickly find matching edges
    const TOLERANCE: f32 = 10.0; // Grid cell size for edge matching

    let mut edge_index: std::collections::HashMap<(i32, i32), Vec<(Vec2, Vec2)>> =
        std::collections::HashMap::new();

    for edge in &all_edges {
        // Calculate midpoint and round to grid
        let mid_x = ((edge.0.x + edge.1.x) / 2.0 / TOLERANCE).round() as i32;
        let mid_y = ((edge.0.y + edge.1.y) / 2.0 / TOLERANCE).round() as i32;
        edge_index.entry((mid_x, mid_y)).or_default().push(*edge);
    }

    // Step 2: Find wall edges (edges that don't have a matching reverse edge)
    let mut wall_edges: Vec<(Vec2, Vec2)> = Vec::new();

    for edge in &all_edges {
        // Calculate midpoint and look for matches
        let mid_x = ((edge.0.x + edge.1.x) / 2.0 / TOLERANCE).round() as i32;
        let mid_y = ((edge.0.y + edge.1.y) / 2.0 / TOLERANCE).round() as i32;

        if let Some(candidates) = edge_index.get(&(mid_x, mid_y)) {
            // Check if there's a matching reverse edge
            let is_connection = candidates.iter().any(|other| {
                // Same edge but reversed direction = connection between walkable areas
                edges_equal(&(edge.0, edge.1), &(other.1, other.0))
            });

            if !is_connection {
                wall_edges.push(*edge);
            }
        } else {
            // No candidates found, this is definitely a wall
            wall_edges.push(*edge);
        }
    }

    // Convert to WallSegments
    let walls: Vec<WallSegment> = wall_edges
        .iter()
        .map(|&(start, end)| WallSegment {
            start,
            end,
            material: Material::Solid,
        })
        .collect();

    // Calculate map bounds from all areas
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for area in nav.areas.values() {
        for corner in &area.corners {
            min_x = min_x.min(corner.x as f32);
            min_y = min_y.min(corner.y as f32);
            max_x = max_x.max(corner.x as f32);
            max_y = max_y.max(corner.y as f32);
        }
    }

    // Convert nav areas to navigation nodes for pathfinding
    let nav_nodes: Vec<NavNode> = nav
        .areas
        .iter()
        .map(|(id_str, area)| {
            let corners_3d: Vec<_> = area.corners.iter().map(|c| Vec3::from(*c)).collect();

            let center = arena_centroid(&corners_3d);

            // Copy connection IDs directly (already u32 in NavAreaDict)
            let connections = area.connections.clone();

            // Parse area ID from string key
            let id: u32 = id_str.parse().unwrap_or(0);

            // Build 2D bounding box from corners
            let corners_2d: Vec<Vec2> = corners_3d.iter().map(|c| Vec2::new(c.x, c.y)).collect();
            let bbox = Some(compute_bbox2d(&corners_2d));

            NavNode {
                id,
                center,
                connections,
                bbox,
            }
        })
        .collect();

    // Simplify walls
    let walls = simplify_walls(walls);

    Ok(MapData {
        name: map_name,
        bounds: (min_x, min_y, max_x, max_y),
        walls,
        spawns: spawn_points,
        bombsites: Vec::new(),
        nav_nodes,
        bvh: None,
    })
}

/// Calculate centroid of a list of 3D points
fn arena_centroid(points: &[Vec3]) -> Vec3 {
    if points.is_empty() {
        return Vec3::default();
    }

    let sum: Vec3 = points.iter().fold(Vec3::default(), |acc, p| {
        Vec3::new(acc.x + p.x, acc.y + p.y, acc.z + p.z)
    });

    Vec3::new(
        sum.x / points.len() as f32,
        sum.y / points.len() as f32,
        sum.z / points.len() as f32,
    )
}

/// Load triangles from a .tri file (CS2 physics collision data)
pub fn load_map_from_tri(path: &std::path::Path) -> Result<MapData, String> {
    MapData::load_from_tri(path)
}

/// Simplify wall segments by merging collinear segments and removing duplicates
pub fn simplify_walls(mut walls: Vec<WallSegment>) -> Vec<WallSegment> {
    if walls.is_empty() {
        return walls;
    }

    // Step 1: Remove exact duplicates
    walls.sort_by(|a, b| {
        let ord = a
            .start
            .x
            .partial_cmp(&b.start.x)
            .unwrap_or(std::cmp::Ordering::Equal);
        if ord != std::cmp::Ordering::Equal {
            ord
        } else {
            a.start
                .y
                .partial_cmp(&b.start.y)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    walls.dedup_by(|a, b| {
        a.start.x == b.start.x && a.start.y == b.start.y && a.end.x == b.end.x && a.end.y == b.end.y
    });

    if walls.len() <= 1 {
        return walls;
    }

    // Step 2: Merge collinear segments
    // Group walls by their approximate direction and position
    let mut merged: Vec<WallSegment> = Vec::new();

    // Sort by start point, then by end point
    walls.sort_by(|a, b| {
        let ord = a
            .start
            .x
            .partial_cmp(&b.start.x)
            .unwrap_or(std::cmp::Ordering::Equal);
        if ord != std::cmp::Ordering::Equal {
            ord
        } else {
            let ord2 = a
                .start
                .y
                .partial_cmp(&b.start.y)
                .unwrap_or(std::cmp::Ordering::Equal);
            if ord2 != std::cmp::Ordering::Equal {
                ord2
            } else {
                let ord3 = a
                    .end
                    .x
                    .partial_cmp(&b.end.x)
                    .unwrap_or(std::cmp::Ordering::Equal);
                if ord3 != std::cmp::Ordering::Equal {
                    ord3
                } else {
                    a.end
                        .y
                        .partial_cmp(&b.end.y)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
            }
        }
    });

    let tolerance = 0.5; // Small tolerance for coordinate comparison

    for wall in walls {
        let mut merged_this = false;

        // Try to merge with existing walls
        for existing in &mut merged {
            // Check if walls are collinear (same line, possibly extended)
            if are_collinear(&existing.start, &existing.end, &wall.start, tolerance)
                || are_collinear(&existing.start, &existing.end, &wall.end, tolerance)
            {
                // Check if they're on the same line (collinear)
                let direction = Vec2::new(
                    existing.end.x - existing.start.x,
                    existing.end.y - existing.start.y,
                );
                let len = (direction.x * direction.x + direction.y * direction.y).sqrt();

                if len > tolerance {
                    // Normalize direction
                    let norm_dir = Vec2::new(direction.x / len, direction.y / len);

                    // Check if wall.start and wall.end are approximately on the same line
                    let line_start = existing.start;
                    let to_start =
                        Vec2::new(wall.start.x - line_start.x, wall.start.y - line_start.y);
                    let to_end = Vec2::new(wall.end.x - line_start.x, wall.end.y - line_start.y);

                    // Check perpendicular distance from line
                    let perp_start = (to_start.x * (-norm_dir.y) + to_start.y * norm_dir.x).abs();
                    let perp_end = (to_end.x * (-norm_dir.y) + to_end.y * norm_dir.x).abs();

                    if perp_start < tolerance && perp_end < tolerance {
                        // Walls are collinear - merge them
                        let combined_start = Vec2::new(
                            existing
                                .start
                                .x
                                .min(wall.start.x)
                                .min(existing.end.x)
                                .min(wall.end.x),
                            existing
                                .start
                                .y
                                .min(wall.start.y)
                                .min(existing.end.y)
                                .min(wall.end.y),
                        );
                        let combined_end = Vec2::new(
                            existing
                                .start
                                .x
                                .max(wall.start.x)
                                .max(existing.end.x)
                                .max(wall.end.x),
                            existing
                                .start
                                .y
                                .max(wall.start.y)
                                .max(existing.end.y)
                                .max(wall.end.y),
                        );

                        *existing = WallSegment {
                            start: combined_start,
                            end: combined_end,
                            material: existing.material,
                        };
                        merged_this = true;
                        break;
                    }
                }
            }
        }

        if !merged_this {
            merged.push(wall);
        }
    }

    merged
}

/// Check if two points are approximately on the same line defined by p1-p2
fn are_collinear(p1: &Vec2, p2: &Vec2, point: &Vec2, tolerance: f32) -> bool {
    // Cross product to check collinearity
    let cross = (point.x - p1.x) * (p2.y - p1.y) - (point.y - p1.y) * (p2.x - p1.x);
    cross.abs() < tolerance
}

/// Load a map by name, searching common locations
pub fn load_map_by_name(map_name: &str) -> Option<MapData> {
    let name_lower = map_name.to_lowercase();

    let vphys_search_paths = [
        std::path::Path::new("vphys"),
        std::path::Path::new("data/vphys"),
        std::path::Path::new("assets/vphys"),
        std::path::Path::new("crates/sentinel-map/assets/vphys"),
    ];
    let vphys_names = if name_lower.starts_with("de_")
        || name_lower.starts_with("cs_")
        || name_lower.starts_with("ar_")
    {
        vec![format!("{name_lower}.vphys")]
    } else {
        vec![
            format!("de_{name_lower}.vphys"),
            format!("{name_lower}.vphys"),
        ]
    };
    for search_path in &vphys_search_paths {
        for name in &vphys_names {
            let vphys_path = search_path.join(name);
            if vphys_path.exists()
                && let Ok(map_data) = load_map_from_vphys(&vphys_path)
            {
                return Some(map_data);
            }
        }
    }

    // Check if it's one of our built-in maps
    if name_lower.contains("dust2") {
        return Some(MapData::dust2());
    }
    if name_lower.contains("mirage") {
        return Some(MapData::mirage());
    }
    if name_lower.contains("inferno") {
        return Some(MapData::inferno());
    }

    // Try loading .tri files for CS2 maps
    let tri_extensions = ["de_", "cs_", "ar_"];
    for prefix in &tri_extensions {
        let tri_name = format!("{prefix}{name_lower}.tri");
        let tri_search_paths = [
            std::path::Path::new("tris"),
            std::path::Path::new("data/tris"),
            std::path::Path::new("assets/tris"),
            std::path::Path::new("crates/sentinel-map/assets/tris"),
            std::path::Path::new("C:/Users/User/Desktop/sental/tris"),
        ];

        for search_path in &tri_search_paths {
            let tri_path = search_path.join(&tri_name);
            if tri_path.exists()
                && let Ok(map_data) = MapData::load_from_tri(&tri_path)
            {
                return Some(map_data);
            }
        }
    }

    // Also try the map name directly as a .tri file
    let direct_tri_name = format!("{name_lower}.tri");
    for search_path in &[
        std::path::Path::new("tris"),
        std::path::Path::new("data/tris"),
        std::path::Path::new("assets/tris"),
        std::path::Path::new("C:/Users/User/Desktop/sental/tris"),
    ] {
        let tri_path = search_path.join(&direct_tri_name);
        if tri_path.exists()
            && let Ok(map_data) = MapData::load_from_tri(&tri_path)
        {
            return Some(map_data);
        }
    }

    // Search for nav JSON files
    let search_paths = [
        std::path::Path::new("data/nav"),
        std::path::Path::new("assets/nav"),
        std::path::Path::new("crates/sentinel-map/assets/nav"),
    ];

    for search_path in &search_paths {
        let nav_path = search_path.join(format!("{name_lower}.json"));
        if nav_path.exists() {
            return load_map_from_nav(&nav_path).ok();
        }

        // Try CS2 map naming convention
        let cs2_map_name = if name_lower.starts_with("de_") {
            name_lower.clone()
        } else {
            format!("de_{name_lower}")
        };

        let cs2_nav_path = search_path.join(format!("{cs2_map_name}.json"));
        if cs2_nav_path.exists() {
            return load_map_from_nav(&cs2_nav_path).ok();
        }
    }

    None
}
/// Load a map from a .vphys file (CS2 physics collision data with accurate geometry)
pub fn load_map_from_vphys(path: &std::path::Path) -> Result<MapData, String> {
    crate::vphys_parser::VPhysData::load_vphys_as_mapdata(path)
}

#[cfg(test)]
mod tri_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Helper to create a minimal .tri file with test triangles (with height)
    fn create_test_tri_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        // Create a triangle with height (z varies): p1=(0,0,0), p2=(100,0,100), p3=(0,100,50)
        // Height difference is 100, which is >= 50 threshold
        let triangle: [f32; 9] = [
            0.0, 0.0, 0.0, // p1
            100.0, 0.0, 100.0, // p2
            0.0, 100.0, 50.0, // p3
        ];

        for val in &triangle {
            file.write_all(&val.to_le_bytes())
                .expect("Failed to write triangle");
        }

        file.flush().expect("Failed to flush file");
        file
    }

    #[test]
    fn test_load_map_from_tri_basic() {
        let tri_file = create_test_tri_file();

        let result = load_map_from_tri(tri_file.path());
        assert!(
            result.is_ok(),
            "Failed to load tri file: {:?}",
            result.err()
        );

        let map = result.unwrap();
        assert!(!map.name.is_empty());
        assert!(
            map.walls.len() >= 3,
            "Expected at least 3 walls from triangle, got {}",
            map.walls.len()
        );
    }

    #[test]
    fn test_load_map_from_tri_multiple_triangles() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        // Create two triangles with height that form a WALL
        // Triangle 1: p1=(0,0,0), p2=(100,0,100), p3=(0,100,50)
        // Triangle 2: p1=(100,0,100), p2=(100,100,150), p3=(0,100,50)
        let triangles: [[f32; 9]; 2] = [
            [0.0, 0.0, 0.0, 100.0, 0.0, 100.0, 0.0, 100.0, 50.0],
            [100.0, 0.0, 100.0, 100.0, 100.0, 150.0, 0.0, 100.0, 50.0],
        ];

        for tri in &triangles {
            for val in *tri {
                file.write_all(&val.to_le_bytes())
                    .expect("Failed to write triangle");
            }
        }

        file.flush().expect("Failed to flush file");

        let result = load_map_from_tri(file.path());
        assert!(
            result.is_ok(),
            "Failed to load tri file: {:?}",
            result.err()
        );

        let map = result.unwrap();
        assert!(
            map.walls.len() >= 3,
            "Expected at least 3 walls from triangles, got {}",
            map.walls.len()
        );
    }

    #[test]
    fn test_tri_file_not_found() {
        let result = load_map_from_tri(std::path::Path::new("/nonexistent/file.tri"));
        assert!(result.is_err(), "Expected error for nonexistent file");
    }

    #[test]
    fn test_empty_tri_file() {
        let file = NamedTempFile::new().expect("Failed to create temp file");

        let result = load_map_from_tri(file.path());
        // Empty file should return empty walls
        assert!(
            result.is_ok(),
            "Failed to load empty tri file: {:?}",
            result.err()
        );
        let map = result.unwrap();
        assert!(map.walls.is_empty(), "Empty file should produce no walls");
    }

    #[test]
    fn test_flat_triangle_no_walls() {
        // Test that flat triangles (z diff < threshold) produce no walls
        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        // Flat triangle with z=0 everywhere (height diff = 0 < 50)
        let triangle: [f32; 9] = [0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 100.0, 0.0];

        for val in &triangle {
            file.write_all(&val.to_le_bytes())
                .expect("Failed to write triangle");
        }

        file.flush().expect("Failed to flush file");

        let map = load_map_from_tri(file.path()).expect("Failed to load");
        assert!(
            map.walls.is_empty(),
            "Flat triangle should produce no walls (height < threshold)"
        );
    }

    #[test]
    fn test_wall_segment_from_triangle() {
        let tri_file = create_test_tri_file();

        let map = load_map_from_tri(tri_file.path()).expect("Failed to load");

        // Triangle with vertices at (0,0,0), (100,0,100), (0,100,50)
        // Projects to 2D walls: 3 edges from the triangle
        assert!(
            map.walls.len() >= 3,
            "Expected at least 3 walls from triangle, got {}",
            map.walls.len()
        );
    }

    #[test]
    fn test_load_real_dust2_tri() {
        // Test loading real de_dust2.tri file if it exists
        let tri_paths = [
            std::path::Path::new("C:/Users/User/Desktop/sental/tris/de_dust2.tri"),
            std::path::Path::new("../../tris/de_dust2.tri"),
            std::path::Path::new("tris/de_dust2.tri"),
        ];

        let mut loaded = false;
        for tri_path in &tri_paths {
            if tri_path.exists() {
                if let Ok(map) = load_map_from_tri(tri_path) {
                    assert!(!map.name.is_empty(), "Map should have a name");
                    assert!(!map.walls.is_empty(), "Map should have walls loaded");
                    assert!(map.bvh.is_some(), "BVH should be built from .tri file");
                    println!("Loaded {} walls from {:?}", map.walls.len(), tri_path);
                    loaded = true;
                    break;
                }
            }
        }

        if !loaded {
            println!("Skipping real .tri file test - file not found in expected locations");
        }
    }

    #[test]
    fn test_bvh_raycast() {
        // Create a test triangle file
        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        // Create a floor triangle at z=0 and a wall triangle at z=100
        // Floor: (0,0,0), (100,0,0), (0,100,0) - flat, won't produce walls
        // Wall: (0,0,0), (100,0,0), (0,0,100) - height 100, will produce walls
        let wall_triangle: [f32; 9] = [
            0.0, 0.0, 0.0, // p1
            100.0, 0.0, 0.0, // p2
            0.0, 0.0, 100.0, // p3 - 100 units up
        ];

        for val in &wall_triangle {
            file.write_all(&val.to_le_bytes())
                .expect("Failed to write triangle");
        }

        file.flush().expect("Failed to flush file");

        let map = load_map_from_tri(file.path()).expect("Failed to load");

        // Check that BVH was built
        assert!(map.bvh.is_some(), "BVH should be built");

        // Test 3D line blocking
        // Test ray that should be blocked (shot through the wall)
        let origin = Vec3::new(50.0, -50.0, 50.0); // Below the wall
        let direction = Vec3::new(0.0, 1.0, 0.0); // Ray going up

        // This should be blocked by the wall
        assert!(
            map.line_blocked_3d(origin, direction),
            "Ray should be blocked by wall BVH"
        );

        let before_wall = Vec3::new(50.0, -10.0, 50.0);
        let on_wall = Vec3::new(50.0, 0.0, 50.0);
        let beyond_wall = Vec3::new(50.0, 50.0, 50.0);
        assert!(
            map.segment_blocked_3d(origin, beyond_wall),
            "Segment crossing the wall should be blocked"
        );
        assert!(
            !map.segment_blocked_3d(origin, before_wall),
            "Finite segment ending before the wall must remain clear"
        );
        assert!(
            map.segment_blocked_3d(origin, on_wall),
            "Target on the wall surface must be treated as blocked"
        );
        assert!(
            !map.segment_blocked_3d(origin, origin),
            "Coinciding positions must remain clear without a zero-length raycast"
        );
        assert!(
            map.segment_blocked_3d(
                Vec3::new(50.0, -1_000_000.0, 50.0),
                Vec3::new(50.0, 1_000_000.0, 50.0),
            ),
            "Long segment crossing the wall should be blocked"
        );

        let empty_file = NamedTempFile::new().expect("Failed to create empty tri file");
        let empty_map =
            load_map_from_tri(empty_file.path()).expect("Failed to load empty tri file");
        assert!(
            !empty_map.segment_blocked_3d(origin, beyond_wall),
            "Empty geometry must remain clear"
        );
    }

    #[test]
    fn test_bvh_segment_ignores_degenerate_triangle() {
        let mut file = NamedTempFile::new().expect("Failed to create tri file");
        for value in [0.0_f32; 9] {
            file.write_all(&value.to_le_bytes())
                .expect("Failed to write degenerate triangle");
        }
        file.flush().expect("Failed to flush tri file");

        let map = load_map_from_tri(file.path()).expect("Failed to load degenerate triangle");
        assert!(
            !map.segment_blocked_3d(Vec3::new(0.0, -50.0, 0.0), Vec3::new(0.0, 50.0, 0.0),),
            "Degenerate triangle must not block a segment"
        );
    }
}
