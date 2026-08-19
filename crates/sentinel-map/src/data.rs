use serde::{Deserialize, Serialize};

/// 3D position vector (local to sentinel-map to avoid circular dependency)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn distance_to(&self, other: &Vec3) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

/// 2D line segment for wall detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallSegment {
    pub start: Vec2,
    pub end: Vec2,
    pub material: Material,
}

/// Helper to compute AABB2D from Vec2 points
pub fn compute_bbox2d(points: &[Vec2]) -> AABB2D {
    if points.is_empty() {
        return AABB2D {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 0.0,
        };
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for p in points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    AABB2D {
        min_x,
        min_y,
        max_x,
        max_y,
    }
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
    /// Bounding box of the area (for fast point-in-area tests)
    #[serde(skip, default)]
    pub bbox: Option<AABB2D>,
}

/// 2D axis-aligned bounding box for area lookups
#[derive(Debug, Clone, Copy)]
pub struct AABB2D {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

/// A triangle in 3D space for collision/visibility calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Triangle3D {
    pub v1: Vec3,
    pub v2: Vec3,
    pub v3: Vec3,
}

impl Triangle3D {
    /// Create a new triangle from three vertices
    pub fn new(v1: Vec3, v2: Vec3, v3: Vec3) -> Self {
        Self { v1, v2, v3 }
    }

    /// Calculate the centroid of the triangle
    pub fn centroid(&self) -> Vec3 {
        Vec3::new(
            (self.v1.x + self.v2.x + self.v3.x) / 3.0,
            (self.v1.y + self.v2.y + self.v3.y) / 3.0,
            (self.v1.z + self.v2.z + self.v3.z) / 3.0,
        )
    }

    /// Check if a ray starting at origin with direction intersects this triangle
    /// Returns distance to intersection if hit, None otherwise
    pub fn ray_intersect(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        // Möller–Trumbore ray-triangle intersection algorithm
        let edge1 = Vec3::new(
            self.v2.x - self.v1.x,
            self.v2.y - self.v1.y,
            self.v2.z - self.v1.z,
        );
        let edge2 = Vec3::new(
            self.v3.x - self.v1.x,
            self.v3.y - self.v1.y,
            self.v3.z - self.v1.z,
        );

        let h = cross_product(direction, edge2);
        let a = dot_product(edge1, h);

        if a > -0.0001 && a < 0.0001 {
            return None; // Ray parallel to triangle
        }

        let f = 1.0 / a;
        let s = Vec3::new(
            origin.x - self.v1.x,
            origin.y - self.v1.y,
            origin.z - self.v1.z,
        );
        let u = f * dot_product(s, h);

        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = cross_product(s, edge1);
        let v = f * dot_product(direction, q);

        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * dot_product(edge2, q);
        if t > 0.0001 {
            return Some(t);
        }

        None
    }
}

/// Axis-aligned bounding box
#[derive(Debug, Clone, Copy, Default)]
pub struct AABB3D {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB3D {
    /// Create a new AABB from min and max points
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Create an AABB from a single triangle
    pub fn from_triangle(triangle: &Triangle3D) -> Self {
        Self {
            min: Vec3::new(
                triangle.v1.x.min(triangle.v2.x).min(triangle.v3.x),
                triangle.v1.y.min(triangle.v2.y).min(triangle.v3.y),
                triangle.v1.z.min(triangle.v2.z).min(triangle.v3.z),
            ),
            max: Vec3::new(
                triangle.v1.x.max(triangle.v2.x).max(triangle.v3.x),
                triangle.v1.y.max(triangle.v2.y).max(triangle.v3.y),
                triangle.v1.z.max(triangle.v2.z).max(triangle.v3.z),
            ),
        }
    }

    /// Check if a ray intersects this AABB
    pub fn intersects_ray(&self, origin: Vec3, direction: Vec3) -> bool {
        let mut t_min = f32::NEG_INFINITY;
        let mut t_max = f32::INFINITY;

        for i in 0..3 {
            let (origin_i, dir_i, min_i, max_i) = match i {
                0 => (origin.x, direction.x, self.min.x, self.max.x),
                1 => (origin.y, direction.y, self.min.y, self.max.y),
                _ => (origin.z, direction.z, self.min.z, self.max.z),
            };

            if dir_i.abs() < 0.0001 {
                if origin_i < min_i || origin_i > max_i {
                    return false;
                }
            } else {
                let t1 = (min_i - origin_i) / dir_i;
                let t2 = (max_i - origin_i) / dir_i;
                let t_near = t1.min(t2);
                let t_far = t1.max(t2);

                if t_near > t_min {
                    t_min = t_near;
                }
                if t_far < t_max {
                    t_max = t_far;
                }

                if t_min > t_max || t_max < 0.0 {
                    return false;
                }
            }
        }

        true
    }

    /// Expand this AABB to include another point
    pub fn expand(&mut self, point: Vec3) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.min.z = self.min.z.min(point.z);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
        self.max.z = self.max.z.max(point.z);
    }

    /// Expand this AABB to include another AABB
    pub fn expand_aabb(&mut self, other: &AABB3D) {
        self.expand(other.min);
        self.expand(other.max);
    }
}

/// Helper functions for vector operations
fn dot_product(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn cross_product(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

/// Node in the BVH tree
#[derive(Debug, Clone)]
pub struct BVHNode3D {
    pub aabb: AABB3D,
    /// Triangle for leaf nodes (legacy support)
    pub triangle: Option<Triangle3D>,
    /// Triangles in leaf nodes (for better performance)
    pub triangles: Vec<Triangle3D>,
    pub left: Option<Box<BVHNode3D>>,
    pub right: Option<Box<BVHNode3D>>,
}

impl BVHNode3D {
    pub fn new(aabb: AABB3D) -> Self {
        Self {
            aabb,
            triangle: None,
            triangles: Vec::new(),
            left: None,
            right: None,
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.left.is_none() && self.right.is_none()
    }
}

/// BVH builder for triangle trees
pub struct TrianglesBVH;

impl TrianglesBVH {
    /// Build a BVH tree from a list of triangles
    pub fn build(triangles: Vec<Triangle3D>) -> BVHNode3D {
        if triangles.is_empty() {
            return BVHNode3D::new(AABB3D::default());
        }

        let aabb = Self::compute_bounding_box(&triangles);
        Self::build_node(triangles, &aabb)
    }

    fn build_node(mut triangles: Vec<Triangle3D>, aabb: &AABB3D) -> BVHNode3D {
        let mut node = BVHNode3D::new(*aabb);

        // Leaf node criteria - store up to 2 triangles directly
        if triangles.len() <= 2 {
            for triangle in triangles {
                if node.triangle.is_none() {
                    node.triangle = Some(triangle);
                } else {
                    node.triangles.push(triangle);
                }
            }
            return node;
        }

        // Find longest axis
        let axis = Self::choose_axis(aabb);

        // Sort triangles by centroid
        triangles.sort_by(|a, b| {
            let c1 = Self::triangle_centroid(a);
            let c2 = Self::triangle_centroid(b);
            match axis {
                0 => c1.x.partial_cmp(&c2.x).unwrap_or(std::cmp::Ordering::Equal),
                1 => c1.y.partial_cmp(&c2.y).unwrap_or(std::cmp::Ordering::Equal),
                _ => c1.z.partial_cmp(&c2.z).unwrap_or(std::cmp::Ordering::Equal),
            }
        });

        // Split at median
        let mid = triangles.len() / 2;
        let left_triangles: Vec<Triangle3D> = triangles.drain(..mid).collect();
        let right_triangles: Vec<Triangle3D> = triangles;

        let left_aabb = Self::compute_bounding_box(&left_triangles);
        let right_aabb = Self::compute_bounding_box(&right_triangles);

        node.left = Some(Box::new(Self::build_node(left_triangles, &left_aabb)));
        node.right = Some(Box::new(Self::build_node(right_triangles, &right_aabb)));

        node
    }

    fn compute_bounding_box(triangles: &[Triangle3D]) -> AABB3D {
        let mut min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);

        for triangle in triangles {
            min.x = min
                .x
                .min(triangle.v1.x)
                .min(triangle.v2.x)
                .min(triangle.v3.x);
            min.y = min
                .y
                .min(triangle.v1.y)
                .min(triangle.v2.y)
                .min(triangle.v3.y);
            min.z = min
                .z
                .min(triangle.v1.z)
                .min(triangle.v2.z)
                .min(triangle.v3.z);

            max.x = max
                .x
                .max(triangle.v1.x)
                .max(triangle.v2.x)
                .max(triangle.v3.x);
            max.y = max
                .y
                .max(triangle.v1.y)
                .max(triangle.v2.y)
                .max(triangle.v3.y);
            max.z = max
                .z
                .max(triangle.v1.z)
                .max(triangle.v2.z)
                .max(triangle.v3.z);
        }

        AABB3D::new(min, max)
    }

    fn choose_axis(aabb: &AABB3D) -> usize {
        let extent_x = aabb.max.x - aabb.min.x;
        let extent_y = aabb.max.y - aabb.min.y;
        let extent_z = aabb.max.z - aabb.min.z;

        if extent_x >= extent_y && extent_x >= extent_z {
            0
        } else if extent_y >= extent_z {
            1
        } else {
            2
        }
    }

    fn triangle_centroid(triangle: &Triangle3D) -> Vec3 {
        Vec3::new(
            (triangle.v1.x + triangle.v2.x + triangle.v3.x) / 3.0,
            (triangle.v1.y + triangle.v2.y + triangle.v3.y) / 3.0,
            (triangle.v1.z + triangle.v2.z + triangle.v3.z) / 3.0,
        )
    }
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
    /// BVH tree for 3D raycasting visibility (built from .tri files)
    #[serde(skip)]
    pub bvh: Option<BVHNode3D>,
}

impl MapData {
    /// Check if a line between two points intersects any wall
    pub fn line_blocked(&self, from: Vec2, to: Vec2) -> bool {
        self.walls
            .iter()
            .any(|wall| segments_intersect(from, to, wall.start, wall.end))
    }

    /// Check if a 3D ray from origin in direction is blocked by any wall
    /// Uses BVH tree for efficient ray-triangle intersection if available
    pub fn line_blocked_3d(&self, origin: Vec3, direction: Vec3) -> bool {
        if let Some(ref bvh) = self.bvh {
            // Use BVH for efficient raycasting
            if let Some(_hit_triangle) = self.ray_intersect_bvh(origin, direction, bvh) {
                return true;
            }
        }
        // Fallback to 2D wall checking
        let from = Vec2::new(origin.x, origin.y);
        let to = Vec2::new(origin.x + direction.x, origin.y + direction.y);
        self.line_blocked(from, to)
    }

    /// Check whether the finite segment from `from` to `to` intersects map geometry.
    /// Unlike [`line_blocked_3d`], an obstacle beyond the target does not block visibility.
    pub fn segment_blocked_3d(&self, from: Vec3, to: Vec3) -> bool {
        let distance = from.distance_to(&to);
        if distance <= 0.0001 {
            return false;
        }
        let direction = Vec3::new(
            (to.x - from.x) / distance,
            (to.y - from.y) / distance,
            (to.z - from.z) / distance,
        );
        if let Some(ref bvh) = self.bvh {
            return self
                .ray_intersect_bvh(from, direction, bvh)
                .is_some_and(|hit_distance| hit_distance <= distance + 0.0001);
        }
        self.line_blocked(Vec2::new(from.x, from.y), Vec2::new(to.x, to.y))
    }

    /// Ray-triangle intersection using BVH (returns hit distance if intersection)
    fn ray_intersect_bvh(&self, origin: Vec3, direction: Vec3, node: &BVHNode3D) -> Option<f32> {
        // Check if ray misses AABB
        if !node.aabb.intersects_ray(origin, direction) {
            return None;
        }

        // If it's a leaf node, check all triangles
        if node.is_leaf() {
            // Check legacy single triangle
            if let Some(ref triangle) = node.triangle
                && let Some(t) = self.ray_intersect_triangle(origin, direction, triangle)
            {
                return Some(t);
            }

            // Check multiple triangles in leaf
            for triangle in &node.triangles {
                if let Some(t) = self.ray_intersect_triangle(origin, direction, triangle) {
                    return Some(t);
                }
            }

            return None;
        }

        // Check bounding box first
        if !node.aabb.intersects_ray(origin, direction) {
            return None;
        }

        // Recursively check children
        let mut closest_hit = f32::MAX;

        if let Some(ref left) = node.left
            && let Some(t) = self.ray_intersect_bvh(origin, direction, left)
        {
            closest_hit = closest_hit.min(t);
        }

        if let Some(ref right) = node.right
            && let Some(t) = self.ray_intersect_bvh(origin, direction, right)
        {
            closest_hit = closest_hit.min(t);
        }

        if closest_hit.is_finite() {
            Some(closest_hit)
        } else {
            None
        }
    }

    /// Ray-triangle intersection using Möller–Trumbore algorithm
    fn ray_intersect_triangle(
        &self,
        origin: Vec3,
        direction: Vec3,
        triangle: &Triangle3D,
    ) -> Option<f32> {
        // Möller–Trumbore ray-triangle intersection algorithm
        let edge1 = Vec3::new(
            triangle.v2.x - triangle.v1.x,
            triangle.v2.y - triangle.v1.y,
            triangle.v2.z - triangle.v1.z,
        );
        let edge2 = Vec3::new(
            triangle.v3.x - triangle.v1.x,
            triangle.v3.y - triangle.v1.y,
            triangle.v3.z - triangle.v1.z,
        );

        let h = cross_product(direction, edge2);
        let a = dot_product(edge1, h);

        if a > -0.0001 && a < 0.0001 {
            return None; // Ray parallel to triangle
        }

        let f = 1.0 / a;
        let s = Vec3::new(
            origin.x - triangle.v1.x,
            origin.y - triangle.v1.y,
            origin.z - triangle.v1.z,
        );
        let u = f * dot_product(s, h);

        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = cross_product(s, edge1);
        let v = f * dot_product(direction, q);

        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * dot_product(edge2, q);

        if t > 0.0001 { Some(t) } else { None }
    }

    /// Build BVH from walls for 3D raycasting
    pub fn build_bvh_from_walls(&mut self) {
        if self.walls.is_empty() {
            return;
        }

        // Convert wall segments to triangles for raycasting
        let triangles: Vec<Triangle3D> = self
            .walls
            .iter()
            .flat_map(|wall| {
                let z = 100.0; // Wall height
                vec![
                    Triangle3D::new(
                        Vec3::new(wall.start.x, wall.start.y, 0.0),
                        Vec3::new(wall.end.x, wall.end.y, 0.0),
                        Vec3::new(wall.start.x, wall.start.y, z),
                    ),
                    Triangle3D::new(
                        Vec3::new(wall.end.x, wall.end.y, 0.0),
                        Vec3::new(wall.end.x, wall.end.y, z),
                        Vec3::new(wall.start.x, wall.start.y, z),
                    ),
                ]
            })
            .collect();

        self.bvh = Some(TrianglesBVH::build(triangles));
    }

    /// Load map from .tri file (CS2 physics collision data)
    pub fn load_from_tri(path: &std::path::Path) -> Result<Self, String> {
        use std::io::Read;

        let mut file =
            std::fs::File::open(path).map_err(|e| format!("Failed to open tri file: {e}"))?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read tri file: {e}"))?;

        // Parse triangles (each is 9 f32 values = 36 bytes)
        let triangle_count = buffer.len() / 36;
        let mut triangles: Vec<Triangle3D> = Vec::new();

        for i in 0..triangle_count {
            let offset = i * 36;
            if offset + 36 > buffer.len() {
                break;
            }

            let p1_x = f32::from_le_bytes([
                buffer[offset],
                buffer[offset + 1],
                buffer[offset + 2],
                buffer[offset + 3],
            ]);
            let p1_y = f32::from_le_bytes([
                buffer[offset + 4],
                buffer[offset + 5],
                buffer[offset + 6],
                buffer[offset + 7],
            ]);
            let p1_z = f32::from_le_bytes([
                buffer[offset + 8],
                buffer[offset + 9],
                buffer[offset + 10],
                buffer[offset + 11],
            ]);

            let p2_x = f32::from_le_bytes([
                buffer[offset + 12],
                buffer[offset + 13],
                buffer[offset + 14],
                buffer[offset + 15],
            ]);
            let p2_y = f32::from_le_bytes([
                buffer[offset + 16],
                buffer[offset + 17],
                buffer[offset + 18],
                buffer[offset + 19],
            ]);
            let p2_z = f32::from_le_bytes([
                buffer[offset + 20],
                buffer[offset + 21],
                buffer[offset + 22],
                buffer[offset + 23],
            ]);

            let p3_x = f32::from_le_bytes([
                buffer[offset + 24],
                buffer[offset + 25],
                buffer[offset + 26],
                buffer[offset + 27],
            ]);
            let p3_y = f32::from_le_bytes([
                buffer[offset + 28],
                buffer[offset + 29],
                buffer[offset + 30],
                buffer[offset + 31],
            ]);
            let p3_z = f32::from_le_bytes([
                buffer[offset + 32],
                buffer[offset + 33],
                buffer[offset + 34],
                buffer[offset + 35],
            ]);

            triangles.push(Triangle3D::new(
                Vec3::new(p1_x, p1_y, p1_z),
                Vec3::new(p2_x, p2_y, p2_z),
                Vec3::new(p3_x, p3_y, p3_z),
            ));
        }

        let map_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Build walls from triangles (project to 2D)
        let walls: Vec<WallSegment> = triangles
            .iter()
            .filter_map(|triangle| {
                // Check if triangle has significant height (z difference)
                let z_min = triangle.v1.z.min(triangle.v2.z).min(triangle.v3.z);
                let z_max = triangle.v1.z.max(triangle.v2.z).max(triangle.v3.z);
                let z_diff = z_max - z_min;

                // Only include walls with significant height difference
                if z_diff < 50.0 {
                    return None;
                }

                // Project 3D triangle edges to 2D walls
                let v1 = Vec2::new(triangle.v1.x, triangle.v1.y);
                let v2 = Vec2::new(triangle.v2.x, triangle.v2.y);
                let v3 = Vec2::new(triangle.v3.x, triangle.v3.y);

                Some(vec![
                    WallSegment {
                        start: v1,
                        end: v2,
                        material: Material::Solid,
                    },
                    WallSegment {
                        start: v2,
                        end: v3,
                        material: Material::Solid,
                    },
                    WallSegment {
                        start: v3,
                        end: v1,
                        material: Material::Solid,
                    },
                ])
            })
            .flatten()
            .collect();

        // Simplify walls
        let walls = crate::loader::simplify_walls(walls);

        // Build BVH
        let bvh = TrianglesBVH::build(triangles);

        Ok(MapData {
            name: map_name,
            bounds: (-5000.0, -5000.0, 5000.0, 5000.0),
            walls,
            spawns: Vec::new(),
            bombsites: Vec::new(),
            nav_nodes: Vec::new(),
            bvh: Some(bvh),
        })
    }

    /// Check if a 3D position is behind a wall (using BVH or 2D fallback)
    pub fn is_behind_wall(&self, from: Vec3, direction: Vec3) -> bool {
        self.line_blocked_3d(from, direction)
    }

    /// Get triangles in BVH for serialization
    pub fn get_triangles(&self) -> Vec<Triangle3D> {
        if let Some(ref bvh) = self.bvh {
            let mut triangles = Vec::new();
            Self::collect_triangles_bvh(bvh, &mut triangles);
            triangles
        } else {
            Vec::new()
        }
    }

    fn collect_triangles_bvh(node: &BVHNode3D, triangles: &mut Vec<Triangle3D>) {
        if let Some(ref triangle) = node.triangle {
            triangles.push(triangle.clone());
        }
        if let Some(ref left) = node.left {
            Self::collect_triangles_bvh(left, triangles);
        }
        if let Some(ref right) = node.right {
            Self::collect_triangles_bvh(right, triangles);
        }
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

    /// Find the nav area that contains a 2D position (by bounding box first, then polygon check).
    /// Returns the NavNode ID if found.
    pub fn find_area_2d(&self, pos: Vec2) -> Option<u32> {
        // Fast path: bounding box test
        let mut candidates: Vec<&NavNode> = self
            .nav_nodes
            .iter()
            .filter(|node| {
                if let Some(ref bbox) = node.bbox {
                    pos.x >= bbox.min_x
                        && pos.x <= bbox.max_x
                        && pos.y >= bbox.min_y
                        && pos.y <= bbox.max_y
                } else {
                    false
                }
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Among candidates with bounding box, prefer the one whose centroid is closest
        candidates.sort_by(|a, b| {
            let da = (pos.x - a.center.x).hypot(pos.y - a.center.y);
            let db = (pos.x - b.center.x).hypot(pos.y - b.center.y);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

        Some(candidates[0].id)
    }

    /// Check if two nav areas are walkable-connected (reachable via connections).
    /// Uses BFS over the nav graph.
    pub fn can_walk_between(&self, from_area_id: u32, to_area_id: u32) -> bool {
        if from_area_id == to_area_id {
            return true;
        }

        // BFS
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from_area_id);
        visited.insert(from_area_id);

        // Build adjacency for fast lookup
        let node_map: std::collections::HashMap<u32, &NavNode> =
            self.nav_nodes.iter().map(|n| (n.id, n)).collect();

        while let Some(current) = queue.pop_front() {
            if let Some(node) = node_map.get(&current) {
                for &conn_id in &node.connections {
                    if conn_id == to_area_id {
                        return true;
                    }
                    if visited.insert(conn_id) {
                        queue.push_back(conn_id);
                    }
                }
            }
        }

        false
    }

    /// Check if a line between two points is unblocked.
    /// Uses nav mesh connectivity as primary check — if observer and target
    /// are in walkable-connected areas, they can see each other regardless of walls.
    /// Falls back to wall raycasting if connectivity check is inconclusive.
    pub fn line_blocked_via_nav(&self, from: Vec2, to: Vec2) -> bool {
        let from_area = match self.find_area_2d(from) {
            Some(id) => id,
            None => {
                // Observer is outside nav mesh — fall back to wall checking
                return self.line_blocked(from, to);
            }
        };

        let to_area = match self.find_area_2d(to) {
            Some(id) => id,
            None => {
                // Target is outside nav mesh — fall back to wall checking
                return self.line_blocked(from, to);
            }
        };

        // Same area = definitely visible (one contiguous walkable space)
        if from_area == to_area {
            return false;
        }

        // Different areas = check if reachable via connections
        if self.can_walk_between(from_area, to_area) {
            // Connected areas typically have openings/doors
            // But do a quick wall check along the direct line
            // If the direct line is also clear, definitely visible
            if !self.line_blocked(from, to) {
                return false;
            }
            // Line is blocked by walls, but areas are connected —
            // the connection may go around (doorway around a corner)
            // This is still "blocked" for a direct line-of-sight shot
            return true;
        }

        // Not connected — fall back to wall checking
        // (they may be on different floors, or truly separated)
        self.line_blocked(from, to)
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
            bvh: None,
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
            bvh: None,
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
            bvh: None,
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
pub fn segments_intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
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

    #[test]
    fn test_load_from_nav_json() {
        use crate::loader;
        use std::path::Path;
        let nav_path = Path::new("../../assets/nav/de_dust2.json");
        if nav_path.exists() {
            let map = loader::load_map_from_nav(nav_path);
            assert!(map.is_ok(), "Failed to load nav: {:?}", map.err());
            let map = map.unwrap();
            assert!(map.name.contains("dust2") || map.name == "de_dust2");
        }
        // Skip if file doesn't exist (e.g., in CI environment)
    }
}

// ==================== Nav Visibility Unit Tests ====================

#[cfg(test)]
mod nav_tests {
    use super::*;

    /// Helper: create a minimal map with 3 nav areas
    /// Area 0 (id=0) at center, connected to Area 1 (id=1) and Area 2 (id=2)
    /// Area 1 and Area 2 are NOT connected to each other (separated by a wall)
    fn create_nav_test_map() -> MapData {
        let areas = vec![
            // Area 0: center room at (0, 0), size ~100x100
            NavNode {
                id: 0,
                center: Vec3::new(0.0, 0.0, 64.0),
                connections: vec![1, 2],
                bbox: Some(AABB2D {
                    min_x: -50.0,
                    min_y: -50.0,
                    max_x: 50.0,
                    max_y: 50.0,
                }),
            },
            // Area 1: left room at (-200, 0), connected to Area 0
            NavNode {
                id: 1,
                center: Vec3::new(-200.0, 0.0, 64.0),
                connections: vec![0],
                bbox: Some(AABB2D {
                    min_x: -250.0,
                    min_y: -50.0,
                    max_x: -150.0,
                    max_y: 50.0,
                }),
            },
            // Area 2: right room at (200, 0), connected to Area 0
            NavNode {
                id: 2,
                center: Vec3::new(200.0, 0.0, 64.0),
                connections: vec![0],
                bbox: Some(AABB2D {
                    min_x: 150.0,
                    min_y: -50.0,
                    max_x: 250.0,
                    max_y: 50.0,
                }),
            },
        ];

        MapData {
            name: "nav_test".to_string(),
            bounds: (-300.0, -100.0, 300.0, 100.0),
            walls: vec![
                // Wall separating Area 1 from Area 2 (between the rooms, but not blocking the connection through Area 0)
                WallSegment {
                    start: Vec2::new(-100.0, 0.0),
                    end: Vec2::new(100.0, 0.0),
                    material: Material::Solid,
                },
            ],
            spawns: Vec::new(),
            bombsites: Vec::new(),
            nav_nodes: areas,
            bvh: None,
        }
    }

    // ==================== find_area_2d Tests ====================

    #[test]
    fn test_find_area_2d_same_area() {
        let map = create_nav_test_map();

        // Position inside Area 0's bbox
        let pos = Vec2::new(10.0, 10.0);
        let area_id = map.find_area_2d(pos);
        assert_eq!(area_id, Some(0), "Should find Area 0");

        // Position inside Area 1's bbox
        let pos1 = Vec2::new(-200.0, 0.0);
        let area_id1 = map.find_area_2d(pos1);
        assert_eq!(area_id1, Some(1), "Should find Area 1");

        // Position inside Area 2's bbox
        let pos2 = Vec2::new(200.0, 0.0);
        let area_id2 = map.find_area_2d(pos2);
        assert_eq!(area_id2, Some(2), "Should find Area 2");
    }

    #[test]
    fn test_find_area_2d_outside_all_areas() {
        let map = create_nav_test_map();

        // Position outside all nav area bboxes
        let pos = Vec2::new(500.0, 500.0);
        let area_id = map.find_area_2d(pos);
        assert_eq!(
            area_id, None,
            "Should return None for position outside all areas"
        );
    }

    // ==================== can_walk_between Tests ====================

    #[test]
    fn test_can_walk_between_same_area() {
        let map = create_nav_test_map();

        // Same area should always return true
        assert!(map.can_walk_between(0, 0));
        assert!(map.can_walk_between(1, 1));
        assert!(map.can_walk_between(2, 2));
    }

    #[test]
    fn test_can_walk_between_directly_connected() {
        let map = create_nav_test_map();

        // Area 0 <-> Area 1 (direct connection)
        assert!(map.can_walk_between(0, 1));
        assert!(map.can_walk_between(1, 0));

        // Area 0 <-> Area 2 (direct connection)
        assert!(map.can_walk_between(0, 2));
        assert!(map.can_walk_between(2, 0));
    }

    #[test]
    fn test_can_walk_between_transitive() {
        let map = create_nav_test_map();

        // Area 1 -> Area 0 -> Area 2 (transitive connection through Area 0)
        assert!(
            map.can_walk_between(1, 2),
            "Area 1 should reach Area 2 via Area 0"
        );
        assert!(
            map.can_walk_between(2, 1),
            "Area 2 should reach Area 1 via Area 0"
        );
    }

    #[test]
    fn test_can_walk_between_unreachable() {
        // Test with a map where some areas are truly disconnected
        let areas = vec![
            // Area A: isolated at (0, 0)
            NavNode {
                id: 10,
                center: Vec3::new(0.0, 0.0, 64.0),
                connections: Vec::new(), // No connections
                bbox: Some(AABB2D {
                    min_x: -50.0,
                    min_y: -50.0,
                    max_x: 50.0,
                    max_y: 50.0,
                }),
            },
            // Area B: isolated at (500, 500)
            NavNode {
                id: 11,
                center: Vec3::new(500.0, 500.0, 64.0),
                connections: Vec::new(), // No connections
                bbox: Some(AABB2D {
                    min_x: 450.0,
                    min_y: 450.0,
                    max_x: 550.0,
                    max_y: 550.0,
                }),
            },
        ];

        let map = MapData {
            name: "disconnected".to_string(),
            bounds: (0.0, 0.0, 600.0, 600.0),
            walls: Vec::new(),
            spawns: Vec::new(),
            bombsites: Vec::new(),
            nav_nodes: areas,
            bvh: None,
        };

        // Isolated areas should not reach each other
        assert!(!map.can_walk_between(10, 11));
        assert!(!map.can_walk_between(11, 10));
    }

    // ==================== line_blocked_via_nav Tests ====================

    #[test]
    fn test_line_blocked_via_nav_same_area() {
        let map = create_nav_test_map();

        // Both positions inside Area 0 — should NOT be blocked (same nav area)
        let from = Vec2::new(10.0, 10.0);
        let to = Vec2::new(-10.0, -10.0);
        assert!(
            !map.line_blocked_via_nav(from, to),
            "Same nav area should always be visible"
        );
    }

    #[test]
    fn test_line_blocked_via_nav_connected_clear_line() {
        let map = create_nav_test_map();

        // Area 1 to Area 0: connected, clear line (through the doorway)
        // Wall is at y=0 from x=-100 to x=100 — use positions above wall
        let from2 = Vec2::new(-180.0, 10.0);
        let to2 = Vec2::new(-10.0, 10.0);
        // This line does NOT cross the wall at y=0
        assert!(
            !map.line_blocked_via_nav(from2, to2),
            "Connected areas with clear line should be visible"
        );
    }

    #[test]
    fn test_line_blocked_via_nav_connected_blocked_line() {
        let map = create_nav_test_map();

        // From Area 1 to Area 2: connected via Area 0, but direct line crosses wall
        // Wall is at y=0 from x=-100 to x=100
        // Line from (-180, -10) to (180, 10) crosses the wall at an angle
        let from = Vec2::new(-180.0, -10.0); // Inside Area 1, below wall line
        let to = Vec2::new(180.0, 10.0); // Inside Area 2, above wall line
        // The wall at y=0 from x=-100 to x=100 blocks the direct diagonal line
        // Since areas are connected but direct line is blocked -> falls back to wall check
        assert!(
            map.line_blocked_via_nav(from, to),
            "Connected areas with blocked direct line should be blocked"
        );
    }

    #[test]
    fn test_line_blocked_via_nav_outside_nav() {
        let map = create_nav_test_map();

        // One position outside nav mesh — falls back to wall checking
        let from = Vec2::new(500.0, 500.0); // Outside nav
        let to = Vec2::new(10.0, 10.0); // Inside Area 0

        // No wall between these positions (wall is at y=0 from -100 to 100)
        assert!(
            !map.line_blocked_via_nav(from, to),
            "Out-of-nav position should fall back to wall checking"
        );
    }

    #[test]
    fn test_line_blocked_via_nav_empty_nav() {
        let map = MapData {
            name: "empty".to_string(),
            bounds: (0.0, 0.0, 100.0, 100.0),
            walls: vec![WallSegment {
                start: Vec2::new(50.0, 0.0),
                end: Vec2::new(50.0, 100.0),
                material: Material::Solid,
            }],
            spawns: Vec::new(),
            bombsites: Vec::new(),
            nav_nodes: Vec::new(), // No nav areas
            bvh: None,
        };

        // With no nav areas, should fall back to wall checking
        let from = Vec2::new(10.0, 50.0);
        let to = Vec2::new(90.0, 50.0);
        assert!(
            map.line_blocked_via_nav(from, to),
            "No nav areas should fall back to wall checking"
        );
    }
}
