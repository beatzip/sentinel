use crate::kv3_parser::{KV3Value, parse_kv3, bytes_to_f32_vec, bytes_to_i32_vec};
use crate::data::{Triangle3D, Vec3};

/// Collision attribute for filtering meshes/hulls
#[derive(Debug)]
pub struct CollisionAttribute {
    pub collision_group_string: String,
}

/// Mesh data from VPhys file
#[derive(Debug)]
pub struct MeshData {
    pub vertices_bytes: Vec<u8>,
    pub triangles_bytes: Vec<u8>,
    pub collision_attributes: Vec<CollisionAttribute>,
}

/// Hull data from VPhys file (for collision hulls)
#[derive(Debug)]
pub struct HullData {
    pub vertex_positions_bytes: Vec<u8>,
    pub faces: Vec<Vec<i32>>,
    pub edges: Vec<Vec<i32>>,
    pub collision_attributes: Vec<CollisionAttribute>,
}

/// Shape containing meshes and hulls
#[derive(Debug)]
pub struct VPhysShape {
    pub meshes: Vec<MeshData>,
    pub hulls: Vec<HullData>,
}

/// Part of a VPhys file
#[derive(Debug)]
pub struct VPhysPart {
    pub rn_shape: VPhysShape,
}

/// Complete VPhys file data
#[derive(Debug)]
pub struct VPhysData {
    pub version: u32,
    pub parts: Vec<VPhysPart>,
}

impl VPhysData {
    /// Parse VPhys data from KV3 value
    pub fn from_kv3(kv: &KV3Value) -> Result<Self, String> {
        let obj = kv.as_object().ok_or("KV3 root must be an object")?;
        
        let version = obj.get("version")
            .and_then(|v| v.as_i64())
            .unwrap_or(3) as u32;
        
        let mut parts = Vec::new();
        if let Some(parts_arr) = obj.get("m_parts") {
            if let Some(parts_array) = parts_arr.as_array() {
                for part_val in parts_array {
                    if let Some(part) = VPhysPart::from_kv3(part_val) {
                        parts.push(part);
                    }
                }
            }
        }
        
        Ok(VPhysData { version, parts })
    }
    
    /// Extract all triangles from VPhys data (meshes + hulls)
    pub fn extract_triangles(&self) -> Vec<Triangle3D> {
        let mut triangles = Vec::new();
        
        for part in &self.parts {
            // Extract from meshes
            for mesh in &part.rn_shape.meshes {
                triangles.extend(Self::extract_mesh_triangles(mesh));
            }
            
            // Extract from hulls
            for hull in &part.rn_shape.hulls {
                triangles.extend(Self::extract_hull_triangles(hull));
            }
        }
        
        triangles
    }
    
    /// Load VPhys from file path
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read vphys file: {}", e))?;
        
        let kv = parse_kv3(&content)
            .map_err(|e| format!("Failed to parse KV3: {:?}", e))?;
        
        Self::from_kv3(&kv)
    }
    
    /// Load VPhys from file path and extract triangles + build BVH
    pub fn load_vphys_as_mapdata(path: &std::path::Path) -> Result<crate::data::MapData, String> {
        let vphys = Self::load_from_file(path)?;
        let triangles = vphys.extract_triangles();
        
        let map_name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        // Build BVH from triangles
        let bvh = crate::data::TrianglesBVH::build(triangles.clone());
        
        // Calculate bounds from triangles
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        
        for tri in &triangles {
            min_x = min_x.min(tri.v1.x).min(tri.v2.x).min(tri.v3.x);
            min_y = min_y.min(tri.v1.y).min(tri.v2.y).min(tri.v3.y);
            max_x = max_x.max(tri.v1.x).max(tri.v2.x).max(tri.v3.x);
            max_y = max_y.max(tri.v1.y).max(tri.v2.y).max(tri.v3.y);
        }
        
        // Convert 3D triangles to 2D wall segments
        let walls: Vec<crate::data::WallSegment> = triangles.iter().filter_map(|triangle| {
            let z_min = triangle.v1.z.min(triangle.v2.z).min(triangle.v3.z);
            let z_max = triangle.v1.z.max(triangle.v2.z).max(triangle.v3.z);
            let z_diff = z_max - z_min;
            
            if z_diff < 50.0 {
                return None;
            }
            
            let v1 = crate::data::Vec2::new(triangle.v1.x, triangle.v1.y);
            let v2 = crate::data::Vec2::new(triangle.v2.x, triangle.v2.y);
            let v3 = crate::data::Vec2::new(triangle.v3.x, triangle.v3.y);
            
            Some(vec![
                crate::data::WallSegment { start: v1, end: v2, material: crate::data::Material::Solid },
                crate::data::WallSegment { start: v2, end: v3, material: crate::data::Material::Solid },
                crate::data::WallSegment { start: v3, end: v1, material: crate::data::Material::Solid },
            ])
        }).flatten().collect();
        
        let walls = crate::loader::simplify_walls(walls);
        
        Ok(crate::data::MapData {
            name: map_name,
            bounds: (min_x, min_y, max_x, max_y),
            walls,
            spawns: Vec::new(),
            bombsites: Vec::new(),
            nav_nodes: Vec::new(),
            bvh: Some(bvh),
        })
    }
}

impl VPhysData {
    /// Extract triangles from a mesh (m_Mesh.m_Triangles indices + m_Mesh.m_Vertices positions)
    fn extract_mesh_triangles(mesh: &MeshData) -> Vec<Triangle3D> {
        let mut triangles = Vec::new();
        
        let vertices = match bytes_to_f32_vec(&mesh.vertices_bytes) {
            Ok(v) => v,
            Err(_) => return triangles,
        };
        
        let indices = match bytes_to_i32_vec(&mesh.triangles_bytes) {
            Ok(i) => i,
            Err(_) => return triangles,
        };
        
        // Filter by collision attributes
        let use_default = mesh.collision_attributes.iter()
            .any(|ca| ca.collision_group_string == "default");
        
        if !use_default && !mesh.collision_attributes.is_empty() {
            return triangles;
        }
        
        // Each triangle is 3 indices
        let vertex_count = vertices.len() / 3;
        for i in (0..indices.len()).step_by(3) {
            if i + 2 >= indices.len() { break; }
            
            let idx0 = indices[i] as usize;
            let idx1 = indices[i + 1] as usize;
            let idx2 = indices[i + 2] as usize;
            
            if idx0 >= vertex_count || idx1 >= vertex_count || idx2 >= vertex_count {
                continue;
            }
            
            let v0 = Self::index_to_vertex(&vertices, idx0);
            let v1 = Self::index_to_vertex(&vertices, idx1);
            let v2 = Self::index_to_vertex(&vertices, idx2);
            
            triangles.push(Triangle3D::new(v0, v1, v2));
        }
        
        triangles
    }
    
    /// Extract triangles from a hull (m_Hull.m_VertexPositions + m_Hull.m_Faces)
    fn extract_hull_triangles(hull: &HullData) -> Vec<Triangle3D> {
        let mut triangles = Vec::new();
        
        let vertices = match bytes_to_f32_vec(&hull.vertex_positions_bytes) {
            Ok(v) => v,
            Err(_) => return triangles,
        };
        
        // Filter by collision attributes
        let use_default = hull.collision_attributes.iter()
            .any(|ca| ca.collision_group_string == "default");
        
        if !use_default && !hull.collision_attributes.is_empty() {
            return triangles;
        }
        
        // Use faces if available (each face is 3 vertex indices)
        if !hull.faces.is_empty() {
            for face in &hull.faces {
                if face.len() >= 3 {
                    let v0 = Self::index_to_vertex(&vertices, face[0] as usize);
                    let v1 = Self::index_to_vertex(&vertices, face[1] as usize);
                    let v2 = Self::index_to_vertex(&vertices, face[2] as usize);
                    triangles.push(Triangle3D::new(v0, v1, v2));
                }
            }
        } else if hull.edges.len() >= 3 {
            // Triangulate from edges (3 edges = 1 triangle)
            for chunk in hull.edges.chunks(3) {
                if chunk.len() == 3 {
                    // edges[0] = [v0, v1], edges[1] = [v1, v2], edges[2] = [v2, v0]
                    let v0 = Self::edge_vertex(&chunk[0], 0, &vertices);
                    let v1 = Self::edge_vertex(&chunk[0], 1, &vertices);
                    let v2 = Self::edge_vertex(&chunk[1], 1, &vertices);
                    triangles.push(Triangle3D::new(v0, v1, v2));
                }
            }
        }
        
        triangles
    }
    
    fn index_to_vertex(vertices: &[f32], idx: usize) -> Vec3 {
        let offset = idx * 3;
        if offset + 2 < vertices.len() {
            Vec3::new(vertices[offset], vertices[offset + 1], vertices[offset + 2])
        } else {
            Vec3::default()
        }
    }
    
    fn edge_vertex(edge: &[i32], pos: usize, vertices: &[f32]) -> Vec3 {
        let idx = edge[pos] as usize;
        Self::index_to_vertex(vertices, idx)
    }
}

impl VPhysPart {
    fn from_kv3(kv: &KV3Value) -> Option<Self> {
        let obj = kv.as_object()?;
        
        let rn_shape = VPhysShape::from_kv3(obj.get("m_rnShape")?)?;
        
        Some(VPhysPart { rn_shape })
    }
}

impl VPhysShape {
    fn from_kv3(kv: &KV3Value) -> Option<Self> {
        let obj = kv.as_object()?;
        
        let mut meshes = Vec::new();
        if let Some(meshes_val) = obj.get("m_meshes") {
            if let Some(meshes_arr) = meshes_val.as_array() {
                for mesh_val in meshes_arr {
                    if let Some(mesh) = MeshData::from_kv3(mesh_val) {
                        meshes.push(mesh);
                    }
                }
            }
        }
        
        let mut hulls = Vec::new();
        if let Some(hulls_val) = obj.get("m_hulls") {
            if let Some(hulls_arr) = hulls_val.as_array() {
                for hull_val in hulls_arr {
                    if let Some(hull) = HullData::from_kv3(hull_val) {
                        hulls.push(hull);
                    }
                }
            }
        }
        
        Some(VPhysShape { meshes, hulls })
    }
}

impl MeshData {
    fn from_kv3(kv: &KV3Value) -> Option<Self> {
        let obj = kv.as_object()?;
        
        let mut vertices_bytes = Vec::new();
        let mut triangles_bytes = Vec::new();
        let mut collision_attributes = Vec::new();
        
        if let Some(mesh_obj) = obj.get("m_Mesh") {
            if let Some(mesh_data) = mesh_obj.as_object() {
                if let Some(vertices_val) = mesh_data.get("m_Vertices") {
                    if let Some(bytes) = vertices_val.as_bytes() {
                        vertices_bytes = bytes.clone();
                    }
                }
                if let Some(triangles_val) = mesh_data.get("m_Triangles") {
                    if let Some(bytes) = triangles_val.as_bytes() {
                        triangles_bytes = bytes.clone();
                    }
                }
            }
        }
        
        if let Some(attrs_val) = obj.get("m_CollisionAttributes") {
            if let Some(attrs_arr) = attrs_val.as_array() {
                for attr_val in attrs_arr {
                    if let Some(attr_obj) = attr_val.as_object() {
                        if let Some(group_str) = attr_obj.get("m_CollisionGroupString") {
                            if let Some(group) = group_str.as_str() {
                                collision_attributes.push(CollisionAttribute {
                                    collision_group_string: group.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        
        Some(MeshData { vertices_bytes, triangles_bytes, collision_attributes })
    }
}

impl HullData {
    fn from_kv3(kv: &KV3Value) -> Option<Self> {
        let obj = kv.as_object()?;
        
        let mut vertex_positions_bytes = Vec::new();
        let mut faces = Vec::new();
        let mut edges = Vec::new();
        let mut collision_attributes = Vec::new();
        
        if let Some(hull_obj) = obj.get("m_Hull") {
            if let Some(hull_data) = hull_obj.as_object() {
                // Parse m_Hull.m_VertexPositions
                if let Some(vertices_val) = hull_data.get("m_VertexPositions") {
                    if let Some(bytes) = vertices_val.as_bytes() {
                        vertex_positions_bytes = bytes.clone();
                    }
                }
                
                // Parse m_Hull.m_Faces
                if let Some(faces_val) = hull_data.get("m_Faces") {
                    if let Some(faces_arr) = faces_val.as_array() {
                        for face_val in faces_arr {
                            if let Some(face_bytes) = face_val.as_bytes() {
                                if let Ok(indices) = bytes_to_i32_vec(face_bytes) {
                                    faces.push(indices);
                                }
                            }
                        }
                    }
                }
                
                // Parse m_Hull.m_Edges
                if let Some(edges_val) = hull_data.get("m_Edges") {
                    if let Some(edges_arr) = edges_val.as_array() {
                        for edge_val in edges_arr {
                            if let Some(edge_bytes) = edge_val.as_bytes() {
                                if let Ok(indices) = bytes_to_i32_vec(edge_bytes) {
                                    edges.push(indices);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if let Some(attrs_val) = obj.get("m_CollisionAttributes") {
            if let Some(attrs_arr) = attrs_val.as_array() {
                for attr_val in attrs_arr {
                    if let Some(attr_obj) = attr_val.as_object() {
                        if let Some(group_str) = attr_obj.get("m_CollisionGroupString") {
                            if let Some(group) = group_str.as_str() {
                                collision_attributes.push(CollisionAttribute {
                                    collision_group_string: group.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        
        Some(HullData { vertex_positions_bytes, faces, edges, collision_attributes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_real_dust2_vphys() {
        let vphys_paths = [
            std::path::Path::new("C:/Users/User/Desktop/sental/tris/de_dust2.vphys"),
            std::path::Path::new("C:/Users/User/Desktop/sental/vphys/de_dust2.vphys"),
        ];
        
        let mut loaded = false;
        for vphys_path in &vphys_paths {
            if vphys_path.exists() {
                if let Ok(vphys) = VPhysData::load_from_file(vphys_path) {
                    let triangles = vphys.extract_triangles();
                    assert!(triangles.len() > 0, "Should have triangles");
                    println!("Loaded {} triangles from {:?}", triangles.len(), vphys_path);
                    loaded = true;
                    break;
                }
            }
        }
        
        if !loaded {
            println!("Skipping real .vphys file test - file not found");
        }
    }
}
