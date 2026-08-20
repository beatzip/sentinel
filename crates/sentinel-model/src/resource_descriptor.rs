use std::{
    fs,
    path::{Component, Path},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const HEADER_SIZE: usize = 16;
const BLOCK_INDEX_ENTRY_SIZE: usize = 12;
const RERL_HEADER_SIZE: usize = 8;
const RERL_ENTRY_SIZE: usize = 16;
const EXPECTED_HEADER_VERSION: u16 = 12;

#[derive(Debug, Error)]
pub enum ResourceDescriptorError {
    #[error("unable to read model resource {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("model resource is too short for a Source 2 header")]
    TruncatedHeader,
    #[error("unsupported Source 2 resource header version {0}")]
    UnsupportedHeaderVersion(u16),
    #[error("declared resource size {declared} exceeds available bytes {available}")]
    DeclaredSizeOutOfBounds { declared: usize, available: usize },
    #[error("resource block index is outside the declared resource data")]
    BlockIndexOutOfBounds,
    #[error("resource block {tag} is outside the declared resource data")]
    BlockOutOfBounds { tag: String },
    #[error("RERL block is malformed: {0}")]
    InvalidExternalReference(String),
    #[error("model resource path must use the .vmdl_c extension")]
    NotCompiledVmdl,
    #[error("dependency root {path} cannot be resolved: {source}")]
    DependencyRoot {
        path: String,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceHeader {
    pub file_size: u32,
    pub header_version: u16,
    pub version: u16,
    pub block_offset: u32,
    pub block_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceBlock {
    pub tag: String,
    pub offset: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelDependencyKind {
    Mesh,
    Skeleton,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalDependency {
    pub id: u64,
    pub path: String,
    pub kind: ModelDependencyKind,
}

/// Deterministic metadata from a compiled `.vmdl_c` container.
///
/// `external_dependencies` originate solely from the binary RERL block. REDI/RED2
/// blocks are reported in `blocks` but their binary-KV3 contents are deliberately
/// not interpreted in this Gate 1B slice.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceDescriptor {
    pub schema_version: u8,
    pub resource_type: String,
    pub asset_sha256: String,
    pub header: ResourceHeader,
    pub blocks: Vec<ResourceBlock>,
    pub external_dependencies: Vec<ExternalDependency>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyStatus {
    Resolved,
    Missing,
    UnsafePath,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyResolution {
    pub id: u64,
    pub path: String,
    pub kind: ModelDependencyKind,
    pub status: DependencyStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ResourceDescriptorError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(ResourceDescriptorError::TruncatedHeader)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ResourceDescriptorError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(ResourceDescriptorError::TruncatedHeader)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, ResourceDescriptorError> {
    let value = bytes.get(offset..offset + 8).ok_or_else(|| {
        ResourceDescriptorError::InvalidExternalReference("reference ID is truncated".to_string())
    })?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32, ResourceDescriptorError> {
    Ok(read_u32(bytes, offset)? as i32)
}

fn checked_end(offset: usize, size: usize, limit: usize) -> Result<usize, ResourceDescriptorError> {
    let end = offset
        .checked_add(size)
        .filter(|end| *end <= limit)
        .ok_or(ResourceDescriptorError::BlockIndexOutOfBounds)?;
    Ok(end)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn dependency_kind(path: &str) -> ModelDependencyKind {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".vmesh_c") {
        ModelDependencyKind::Mesh
    } else if lower.ends_with(".vnmskel_c") || lower.ends_with(".vskel_c") {
        ModelDependencyKind::Skeleton
    } else {
        ModelDependencyKind::Other
    }
}

fn parse_rerl(
    bytes: &[u8],
    block: &ResourceBlock,
    file_limit: usize,
) -> Result<Vec<ExternalDependency>, ResourceDescriptorError> {
    let block_start = block.offset as usize;
    let block_end = checked_end(block_start, block.size as usize, file_limit).map_err(|_| {
        ResourceDescriptorError::InvalidExternalReference("RERL block exceeds resource".to_string())
    })?;
    if (block.size as usize) < RERL_HEADER_SIZE {
        return Err(ResourceDescriptorError::InvalidExternalReference(
            "RERL header is truncated".to_string(),
        ));
    }
    let entries_offset = read_u32(bytes, block_start).map_err(|_| {
        ResourceDescriptorError::InvalidExternalReference(
            "RERL list offset is truncated".to_string(),
        )
    })? as usize;
    let count = read_u32(bytes, block_start + 4).map_err(|_| {
        ResourceDescriptorError::InvalidExternalReference(
            "RERL list count is truncated".to_string(),
        )
    })? as usize;
    let entries_start = block_start.checked_add(entries_offset).ok_or_else(|| {
        ResourceDescriptorError::InvalidExternalReference("RERL list overflows".to_string())
    })?;
    let entries_size = count.checked_mul(RERL_ENTRY_SIZE).ok_or_else(|| {
        ResourceDescriptorError::InvalidExternalReference("RERL entry count overflows".to_string())
    })?;
    if checked_end(entries_start, entries_size, block_end).is_err() {
        return Err(ResourceDescriptorError::InvalidExternalReference(
            "RERL entries exceed block bounds".to_string(),
        ));
    }

    let mut dependencies = Vec::with_capacity(count);
    for index in 0..count {
        let entry_start = entries_start + index * RERL_ENTRY_SIZE;
        let id = read_u64(bytes, entry_start)?;
        let offset_position = entry_start + 8;
        let string_offset = read_i32(bytes, offset_position)?;
        if string_offset <= 0 {
            return Err(ResourceDescriptorError::InvalidExternalReference(format!(
                "RERL entry {index} has a non-positive resource-name offset"
            )));
        }
        let string_start = offset_position
            .checked_add(string_offset as usize)
            .filter(|start| *start < block_end)
            .ok_or_else(|| {
                ResourceDescriptorError::InvalidExternalReference(format!(
                    "RERL entry {index} resource-name offset escapes block"
                ))
            })?;
        let nul_offset = bytes[string_start..block_end]
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| {
                ResourceDescriptorError::InvalidExternalReference(format!(
                    "RERL entry {index} resource name is not NUL-terminated"
                ))
            })?;
        let path = std::str::from_utf8(&bytes[string_start..string_start + nul_offset])
            .map_err(|_| {
                ResourceDescriptorError::InvalidExternalReference(format!(
                    "RERL entry {index} resource name is not UTF-8"
                ))
            })?
            .to_string();
        dependencies.push(ExternalDependency {
            id,
            kind: dependency_kind(&path),
            path,
        });
    }
    Ok(dependencies)
}

/// Reads only the documented Source 2 container directory and RERL reference list.
pub fn describe_vmdl_bytes(bytes: &[u8]) -> Result<ResourceDescriptor, ResourceDescriptorError> {
    if bytes.len() < HEADER_SIZE {
        return Err(ResourceDescriptorError::TruncatedHeader);
    }
    let file_size = read_u32(bytes, 0)? as usize;
    let header_version = read_u16(bytes, 4)?;
    if header_version != EXPECTED_HEADER_VERSION {
        return Err(ResourceDescriptorError::UnsupportedHeaderVersion(
            header_version,
        ));
    }
    if file_size > bytes.len() || file_size < HEADER_SIZE {
        return Err(ResourceDescriptorError::DeclaredSizeOutOfBounds {
            declared: file_size,
            available: bytes.len(),
        });
    }
    let header = ResourceHeader {
        file_size: file_size as u32,
        header_version,
        version: read_u16(bytes, 6)?,
        block_offset: read_u32(bytes, 8)?,
        block_count: read_u32(bytes, 12)?,
    };
    let table_start = 8_usize
        .checked_add(header.block_offset as usize)
        .ok_or(ResourceDescriptorError::BlockIndexOutOfBounds)?;
    let table_size = (header.block_count as usize)
        .checked_mul(BLOCK_INDEX_ENTRY_SIZE)
        .ok_or(ResourceDescriptorError::BlockIndexOutOfBounds)?;
    checked_end(table_start, table_size, file_size)?;

    let mut blocks = Vec::with_capacity(header.block_count as usize);
    for index in 0..header.block_count as usize {
        let entry_start = table_start + index * BLOCK_INDEX_ENTRY_SIZE;
        let tag = String::from_utf8_lossy(&bytes[entry_start..entry_start + 4]).into_owned();
        let offset_field = entry_start + 4;
        let relative_offset = read_u32(bytes, offset_field)? as usize;
        let offset = offset_field
            .checked_add(relative_offset)
            .ok_or(ResourceDescriptorError::BlockIndexOutOfBounds)?;
        let size = read_u32(bytes, entry_start + 8)? as usize;
        if checked_end(offset, size, file_size).is_err() {
            return Err(ResourceDescriptorError::BlockOutOfBounds { tag });
        }
        blocks.push(ResourceBlock {
            tag,
            offset: offset as u32,
            size: size as u32,
        });
    }

    let external_dependencies = blocks
        .iter()
        .find(|block| block.tag == "RERL")
        .map(|block| parse_rerl(bytes, block, file_size))
        .transpose()?
        .unwrap_or_default();
    Ok(ResourceDescriptor {
        schema_version: 1,
        resource_type: "vmdl".to_string(),
        asset_sha256: sha256_bytes(&bytes[..file_size]),
        header,
        blocks,
        external_dependencies,
    })
}

pub fn describe_vmdl_file(path: &Path) -> Result<ResourceDescriptor, ResourceDescriptorError> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("vmdl_c") {
        return Err(ResourceDescriptorError::NotCompiledVmdl);
    }
    let bytes = fs::read(path).map_err(|source| ResourceDescriptorError::Read {
        path: path.display().to_string(),
        source,
    })?;
    describe_vmdl_bytes(&bytes)
}

fn safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// Resolves only already-declared RERL dependencies. Missing or unsafe paths are
/// explicit statuses; this function never creates a skeleton or geometry fallback.
pub fn resolve_dependencies(
    descriptor: &ResourceDescriptor,
    asset_root: &Path,
) -> Result<Vec<DependencyResolution>, ResourceDescriptorError> {
    let root =
        asset_root
            .canonicalize()
            .map_err(|source| ResourceDescriptorError::DependencyRoot {
                path: asset_root.display().to_string(),
                source,
            })?;
    Ok(descriptor
        .external_dependencies
        .iter()
        .map(|dependency| {
            let relative = Path::new(&dependency.path);
            if !safe_relative_path(relative) {
                return DependencyResolution {
                    id: dependency.id,
                    path: dependency.path.clone(),
                    kind: dependency.kind,
                    status: DependencyStatus::UnsafePath,
                    sha256: None,
                };
            }
            let candidate = root.join(relative);
            let Ok(candidate) = candidate.canonicalize() else {
                return DependencyResolution {
                    id: dependency.id,
                    path: dependency.path.clone(),
                    kind: dependency.kind,
                    status: DependencyStatus::Missing,
                    sha256: None,
                };
            };
            if !candidate.starts_with(&root) {
                return DependencyResolution {
                    id: dependency.id,
                    path: dependency.path.clone(),
                    kind: dependency.kind,
                    status: DependencyStatus::UnsafePath,
                    sha256: None,
                };
            }
            match fs::read(&candidate) {
                Ok(bytes) => DependencyResolution {
                    id: dependency.id,
                    path: dependency.path.clone(),
                    kind: dependency.kind,
                    status: DependencyStatus::Resolved,
                    sha256: Some(sha256_bytes(&bytes)),
                },
                Err(_) => DependencyResolution {
                    id: dependency.id,
                    path: dependency.path.clone(),
                    kind: dependency.kind,
                    status: DependencyStatus::Missing,
                    sha256: None,
                },
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn sample_vmdl() -> Vec<u8> {
        let rer_start = 48;
        let entries_start = rer_start + RERL_HEADER_SIZE;
        let strings_start = entries_start + 2 * RERL_ENTRY_SIZE;
        let mesh = b"models/player/test.vmesh_c\0";
        let skeleton = b"models/player/test.vnmskel_c\0";
        let red2_start = strings_start + mesh.len() + skeleton.len();
        let file_size = red2_start + RERL_HEADER_SIZE;
        let mut bytes = vec![0; file_size];
        put_u32(&mut bytes, 0, file_size as u32);
        put_u16(&mut bytes, 4, EXPECTED_HEADER_VERSION);
        put_u16(&mut bytes, 6, 1);
        put_u32(&mut bytes, 8, 8);
        put_u32(&mut bytes, 12, 2);

        bytes[16..20].copy_from_slice(b"RERL");
        put_u32(&mut bytes, 20, (rer_start - 20) as u32);
        put_u32(&mut bytes, 24, (red2_start - rer_start) as u32);
        bytes[28..32].copy_from_slice(b"RED2");
        put_u32(&mut bytes, 32, (red2_start - 32) as u32);
        put_u32(&mut bytes, 36, RERL_HEADER_SIZE as u32);

        put_u32(&mut bytes, rer_start, RERL_HEADER_SIZE as u32);
        put_u32(&mut bytes, rer_start + 4, 2);
        for (index, (id, string_start)) in [(7_u64, strings_start), (9, strings_start + mesh.len())]
            .into_iter()
            .enumerate()
        {
            let entry = entries_start + index * RERL_ENTRY_SIZE;
            put_u64(&mut bytes, entry, id);
            put_u32(&mut bytes, entry + 8, (string_start - (entry + 8)) as u32);
        }
        bytes[strings_start..strings_start + mesh.len()].copy_from_slice(mesh);
        bytes[strings_start + mesh.len()..red2_start].copy_from_slice(skeleton);
        bytes
    }

    #[test]
    fn describes_vmdl_blocks_and_rerl_dependencies_deterministically() {
        let descriptor = describe_vmdl_bytes(&sample_vmdl()).unwrap();
        assert_eq!(descriptor.resource_type, "vmdl");
        assert_eq!(descriptor.blocks.len(), 2);
        assert_eq!(descriptor.blocks[0].tag, "RERL");
        assert_eq!(descriptor.blocks[1].tag, "RED2");
        assert_eq!(descriptor.external_dependencies.len(), 2);
        assert_eq!(
            descriptor.external_dependencies[0].kind,
            ModelDependencyKind::Mesh
        );
        assert_eq!(
            descriptor.external_dependencies[1].kind,
            ModelDependencyKind::Skeleton
        );
        assert_eq!(
            describe_vmdl_bytes(&sample_vmdl()).unwrap().asset_sha256,
            descriptor.asset_sha256
        );
    }

    #[test]
    fn missing_and_unsafe_dependencies_do_not_fall_back() {
        let mut descriptor = describe_vmdl_bytes(&sample_vmdl()).unwrap();
        descriptor.external_dependencies.push(ExternalDependency {
            id: 11,
            path: "../escape.vmesh_c".to_string(),
            kind: ModelDependencyKind::Mesh,
        });
        let root = std::env::temp_dir().join(format!("sentinel-model-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let resolution = resolve_dependencies(&descriptor, &root).unwrap();
        assert_eq!(resolution[0].status, DependencyStatus::Missing);
        assert_eq!(resolution[1].status, DependencyStatus::Missing);
        assert_eq!(resolution[2].status, DependencyStatus::UnsafePath);
        assert!(resolution.iter().all(|item| item.sha256.is_none()));
        fs::remove_dir_all(root).unwrap();
    }
}
