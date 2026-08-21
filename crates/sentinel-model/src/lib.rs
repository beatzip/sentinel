//! Source 2 model resource discovery.
//!
//! This crate intentionally stops at resource container and dependency discovery.
//! It does not parse VMDL geometry, decode AG2, infer bones, or create spatial evidence.

pub mod kv3;
pub mod resource_descriptor;

pub use kv3::{
    decode_binary_kv3_v5, inspect_vmdl_semantics, BinaryKv3Compression, BinaryKv3Header,
    Kv3DecodeError, Kv3Document, Kv3Field, Kv3Value, VmdlSemanticFinding, VmdlSemanticInspection,
};
pub use resource_descriptor::{
    describe_vmdl_file, resolve_dependencies, DependencyResolution, DependencyStatus,
    ExternalDependency, ModelDependencyKind, ResourceBlock, ResourceDescriptor,
    ResourceDescriptorError, ResourceHeader,
};
