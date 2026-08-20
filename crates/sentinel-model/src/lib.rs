//! Source 2 model resource discovery.
//!
//! This crate intentionally stops at resource container and dependency discovery.
//! It does not parse VMDL geometry, decode AG2, infer bones, or create spatial evidence.

pub mod resource_descriptor;

pub use resource_descriptor::{
    DependencyResolution, DependencyStatus, ExternalDependency, ModelDependencyKind, ResourceBlock,
    ResourceDescriptor, ResourceDescriptorError, ResourceHeader, describe_vmdl_file,
    resolve_dependencies,
};
