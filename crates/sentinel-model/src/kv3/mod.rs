//! Bounded Binary KV3 v5 decoding for read-only model-resource inspection.
//!
//! This module decodes a generic KV3 tree only. It intentionally does not
//! interpret VMDL, skeleton, hitbox, AG2, transform, or spatial semantics.

mod decoder;
mod header;
mod inspection;
mod value;

pub use decoder::{decode_binary_kv3_v5, Kv3DecodeError};
pub use header::{BinaryKv3Compression, BinaryKv3Header};
pub use inspection::{inspect_vmdl_semantics, VmdlSemanticFinding, VmdlSemanticInspection};
pub use value::{Kv3Document, Kv3Field, Kv3Value};
