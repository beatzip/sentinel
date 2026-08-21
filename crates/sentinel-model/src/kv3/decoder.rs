use lz4_flex::block::decompress_into;
use thiserror::Error;

use super::header::{
    BinaryKv3Compression, BinaryKv3Header, PoolCounts, BINARY_KV3_TRAILER, MAX_KV3_DEPTH,
    MAX_KV3_NODES,
};
use super::value::{Kv3Document, Kv3Field, Kv3Value};

const TYPE_NULL: u8 = 1;
const TYPE_BOOLEAN: u8 = 2;
const TYPE_INT64: u8 = 3;
const TYPE_UINT64: u8 = 4;
const TYPE_DOUBLE: u8 = 5;
const TYPE_STRING: u8 = 6;
const TYPE_BINARY_BLOB: u8 = 7;
const TYPE_ARRAY: u8 = 8;
const TYPE_OBJECT: u8 = 9;
const TYPE_ARRAY_TYPED: u8 = 10;
const TYPE_INT32: u8 = 11;
const TYPE_UINT32: u8 = 12;
const TYPE_BOOLEAN_TRUE: u8 = 13;
const TYPE_BOOLEAN_FALSE: u8 = 14;
const TYPE_INT64_ZERO: u8 = 15;
const TYPE_INT64_ONE: u8 = 16;
const TYPE_DOUBLE_ZERO: u8 = 17;
const TYPE_DOUBLE_ONE: u8 = 18;
const TYPE_FLOAT: u8 = 19;
const TYPE_INT16: u8 = 20;
const TYPE_UINT16: u8 = 21;
const TYPE_INT32_AS_BYTE: u8 = 23;
const TYPE_ARRAY_TYPE_BYTE_LENGTH: u8 = 24;
const TYPE_ARRAY_TYPE_AUXILIARY_BUFFER: u8 = 25;

#[derive(Debug, Error)]
pub enum Kv3DecodeError {
    #[error("Binary KV3 v5 input ended while reading {0}")]
    UnexpectedEof(&'static str),
    #[error("Binary KV3 v5 magic is not the little-endian 0x4B563305 marker")]
    InvalidMagic,
    #[error("unsupported Binary KV3 compression method {0}")]
    UnsupportedCompression(u32),
    #[error(
        "Zstandard Binary KV3 v5 compression is deliberately unsupported by this LZ4-only decoder"
    )]
    UnsupportedZstd,
    #[error("Binary KV3 binary blobs are deliberately unsupported by this bounded first decoder")]
    UnsupportedBinaryBlobs,
    #[error("invalid Binary KV3 v5 header: {0}")]
    InvalidHeader(&'static str),
    #[error("Binary KV3 {label} exceeds decoder limit of {limit} bytes")]
    LimitExceeded { label: &'static str, limit: usize },
    #[error("Binary KV3 LZ4 decompression failed: {0}")]
    Lz4(String),
    #[error("Binary KV3 v5 string id {0} is outside the declared string table")]
    InvalidStringId(i32),
    #[error("Binary KV3 v5 string table has an unterminated UTF-8 string")]
    UnterminatedString,
    #[error("Binary KV3 v5 string table is not valid UTF-8")]
    InvalidUtf8,
    #[error("Binary KV3 node type {0} is unsupported")]
    UnsupportedNodeType(u8),
    #[error("Binary KV3 node count exceeds decoder limit of {0}")]
    NodeLimitExceeded(usize),
    #[error("Binary KV3 nesting exceeds decoder limit of {0}")]
    DepthLimitExceeded(usize),
    #[error("Binary KV3 declared a negative {label}")]
    NegativeCount { label: &'static str },
    #[error("Binary KV3 trailer is missing or invalid")]
    InvalidTrailer,
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(crate) fn position(&self) -> usize {
        self.position
    }

    fn take(&mut self, count: usize, label: &'static str) -> Result<&'a [u8], Kv3DecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(Kv3DecodeError::UnexpectedEof(label))?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(Kv3DecodeError::UnexpectedEof(label))?;
        self.position = end;
        Ok(value)
    }

    pub(crate) fn read_array<const N: usize>(
        &mut self,
        label: &'static str,
    ) -> Result<[u8; N], Kv3DecodeError> {
        self.take(N, label)?
            .try_into()
            .map_err(|_| Kv3DecodeError::UnexpectedEof(label))
    }

    pub(crate) fn read_u16(&mut self, label: &'static str) -> Result<u16, Kv3DecodeError> {
        Ok(u16::from_le_bytes(self.read_array(label)?))
    }

    pub(crate) fn read_u32(&mut self, label: &'static str) -> Result<u32, Kv3DecodeError> {
        Ok(u32::from_le_bytes(self.read_array(label)?))
    }

    pub(crate) fn read_count(&mut self, label: &'static str) -> Result<usize, Kv3DecodeError> {
        let value = i32::from_le_bytes(self.read_array(label)?);
        usize::try_from(value).map_err(|_| Kv3DecodeError::NegativeCount { label })
    }
}

#[derive(Clone, Debug, Default)]
struct Pools {
    bytes1: Vec<u8>,
    bytes2: Vec<u8>,
    bytes4: Vec<u8>,
    bytes8: Vec<u8>,
    bytes1_position: usize,
    bytes2_position: usize,
    bytes4_position: usize,
    bytes8_position: usize,
}

impl Pools {
    fn from_buffer(bytes: &[u8], counts: PoolCounts, start: usize) -> Result<Self, Kv3DecodeError> {
        let mut offset = start;
        let bytes1 = take_pool(bytes, &mut offset, counts.bytes1, "byte pool")?;
        let bytes2 = if counts.bytes2 == 0 {
            Vec::new()
        } else {
            align(&mut offset, 2);
            take_pool(
                bytes,
                &mut offset,
                counts
                    .bytes2
                    .checked_mul(2)
                    .ok_or(Kv3DecodeError::InvalidHeader("word pool overflow"))?,
                "word pool",
            )?
        };
        let bytes4 = if counts.bytes4 == 0 {
            Vec::new()
        } else {
            align(&mut offset, 4);
            take_pool(
                bytes,
                &mut offset,
                counts
                    .bytes4
                    .checked_mul(4)
                    .ok_or(Kv3DecodeError::InvalidHeader("dword pool overflow"))?,
                "dword pool",
            )?
        };
        let bytes8 = if counts.bytes8 == 0 {
            Vec::new()
        } else {
            align(&mut offset, 8);
            take_pool(
                bytes,
                &mut offset,
                counts
                    .bytes8
                    .checked_mul(8)
                    .ok_or(Kv3DecodeError::InvalidHeader("qword pool overflow"))?,
                "qword pool",
            )?
        };
        Ok(Self {
            bytes1,
            bytes2,
            bytes4,
            bytes8,
            ..Self::default()
        })
    }

    fn take<const N: usize>(
        &mut self,
        pool: Pool,
        label: &'static str,
    ) -> Result<[u8; N], Kv3DecodeError> {
        let (bytes, position) = match pool {
            Pool::Bytes1 => (&self.bytes1, &mut self.bytes1_position),
            Pool::Bytes2 => (&self.bytes2, &mut self.bytes2_position),
            Pool::Bytes4 => (&self.bytes4, &mut self.bytes4_position),
            Pool::Bytes8 => (&self.bytes8, &mut self.bytes8_position),
        };
        let end = position
            .checked_add(N)
            .ok_or(Kv3DecodeError::UnexpectedEof(label))?;
        let value = bytes
            .get(*position..end)
            .ok_or(Kv3DecodeError::UnexpectedEof(label))?;
        *position = end;
        value
            .try_into()
            .map_err(|_| Kv3DecodeError::UnexpectedEof(label))
    }

    fn take_u8(&mut self, label: &'static str) -> Result<u8, Kv3DecodeError> {
        Ok(self.take::<1>(Pool::Bytes1, label)?[0])
    }

    fn take_i16(&mut self, label: &'static str) -> Result<i16, Kv3DecodeError> {
        Ok(i16::from_le_bytes(self.take(Pool::Bytes2, label)?))
    }

    fn take_u16(&mut self, label: &'static str) -> Result<u16, Kv3DecodeError> {
        Ok(u16::from_le_bytes(self.take(Pool::Bytes2, label)?))
    }

    fn take_i32(&mut self, label: &'static str) -> Result<i32, Kv3DecodeError> {
        Ok(i32::from_le_bytes(self.take(Pool::Bytes4, label)?))
    }

    fn take_u32(&mut self, label: &'static str) -> Result<u32, Kv3DecodeError> {
        Ok(u32::from_le_bytes(self.take(Pool::Bytes4, label)?))
    }

    fn take_f32(&mut self, label: &'static str) -> Result<f32, Kv3DecodeError> {
        Ok(f32::from_le_bytes(self.take(Pool::Bytes4, label)?))
    }

    fn take_i64(&mut self, label: &'static str) -> Result<i64, Kv3DecodeError> {
        Ok(i64::from_le_bytes(self.take(Pool::Bytes8, label)?))
    }

    fn take_u64(&mut self, label: &'static str) -> Result<u64, Kv3DecodeError> {
        Ok(u64::from_le_bytes(self.take(Pool::Bytes8, label)?))
    }

    fn take_f64(&mut self, label: &'static str) -> Result<f64, Kv3DecodeError> {
        Ok(f64::from_le_bytes(self.take(Pool::Bytes8, label)?))
    }
}

#[derive(Clone, Copy)]
enum Pool {
    Bytes1,
    Bytes2,
    Bytes4,
    Bytes8,
}

struct Context {
    strings: Vec<String>,
    auxiliary: Pools,
    main: Pools,
    object_lengths: Vec<i32>,
    object_length_position: usize,
    types: Vec<u8>,
    type_position: usize,
    nodes: usize,
}

/// Decodes one self-contained Binary KV3 v5 block into a generic tree.
///
/// The initial bounded implementation supports uncompressed and Valve's 16 KiB
/// LZ4 buffers and rejects Zstd and binary-blob payloads explicitly. It does
/// not attach any VMDL semantics to the returned tree.
pub fn decode_binary_kv3_v5(bytes: &[u8]) -> Result<Kv3Document, Kv3DecodeError> {
    let mut reader = Reader::new(bytes);
    let header = BinaryKv3Header::parse(&mut reader)?;
    let auxiliary_buffer = read_buffer(
        &mut reader,
        header.compression,
        header.auxiliary_compressed_size,
        header.auxiliary_uncompressed_size,
    )?;
    let main_buffer = read_buffer(
        &mut reader,
        header.compression,
        header.main_compressed_size,
        header.main_uncompressed_size,
    )?;
    if reader.position() != bytes.len() {
        return Err(Kv3DecodeError::InvalidHeader(
            "unexpected trailing block bytes",
        ));
    }

    let mut auxiliary = Pools::from_buffer(&auxiliary_buffer, header.auxiliary_counts, 0)?;
    let string_count = usize::try_from(auxiliary.take_i32("string count")?).map_err(|_| {
        Kv3DecodeError::NegativeCount {
            label: "string count",
        }
    })?;
    let mut strings = Vec::with_capacity(string_count);
    for _ in 0..string_count {
        strings.push(take_null_terminated_utf8(&mut auxiliary)?);
    }

    let object_lengths_size = header
        .main_object_count
        .checked_mul(4)
        .ok_or(Kv3DecodeError::InvalidHeader("object-length pool overflow"))?;
    let raw_object_lengths = main_buffer
        .get(..object_lengths_size)
        .ok_or(Kv3DecodeError::UnexpectedEof("object lengths"))?;
    let object_lengths = raw_object_lengths
        .chunks_exact(4)
        .map(|chunk| i32::from_le_bytes(chunk.try_into().expect("four-byte chunks")))
        .collect();
    let main = Pools::from_buffer(&main_buffer, header.main_counts, object_lengths_size)?;
    let type_start = main_pool_end(&main_buffer, header.main_counts, object_lengths_size)?;
    let type_end = type_start
        .checked_add(header.type_count)
        .ok_or(Kv3DecodeError::InvalidHeader("type pool overflow"))?;
    let types = main_buffer
        .get(type_start..type_end)
        .ok_or(Kv3DecodeError::UnexpectedEof("type pool"))?
        .to_vec();
    let trailer = main_buffer
        .get(type_end..type_end + 4)
        .ok_or(Kv3DecodeError::UnexpectedEof("trailer"))?;
    if u32::from_le_bytes(trailer.try_into().expect("four-byte trailer")) != BINARY_KV3_TRAILER {
        return Err(Kv3DecodeError::InvalidTrailer);
    }
    if type_end + 4 != main_buffer.len() {
        return Err(Kv3DecodeError::InvalidHeader(
            "unexpected bytes after trailer",
        ));
    }

    let mut context = Context {
        strings,
        auxiliary,
        main,
        object_lengths,
        object_length_position: 0,
        types,
        type_position: 0,
        nodes: 0,
    };
    let node_type = context.read_type()?;
    let root = context.read_value(node_type, 0)?;
    if context.type_position != context.types.len() {
        return Err(Kv3DecodeError::InvalidHeader("unused type entries"));
    }
    Ok(Kv3Document {
        format_guid: header.format_guid,
        root,
    })
}

impl Context {
    fn read_type(&mut self) -> Result<u8, Kv3DecodeError> {
        let raw = *self
            .types
            .get(self.type_position)
            .ok_or(Kv3DecodeError::UnexpectedEof("node type"))?;
        self.type_position += 1;
        if raw & 0x80 != 0 {
            self.type_position = self
                .type_position
                .checked_add(1)
                .ok_or(Kv3DecodeError::UnexpectedEof("node flag"))?;
            if self.type_position > self.types.len() {
                return Err(Kv3DecodeError::UnexpectedEof("node flag"));
            }
            Ok(raw & 0x3F)
        } else {
            Ok(raw)
        }
    }

    fn read_object_length(&mut self) -> Result<usize, Kv3DecodeError> {
        let value = *self
            .object_lengths
            .get(self.object_length_position)
            .ok_or(Kv3DecodeError::UnexpectedEof("object length"))?;
        self.object_length_position += 1;
        usize::try_from(value).map_err(|_| Kv3DecodeError::NegativeCount {
            label: "object length",
        })
    }

    fn string(&self, id: i32) -> Result<String, Kv3DecodeError> {
        if id == -1 {
            return Ok(String::new());
        }
        self.strings
            .get(usize::try_from(id).map_err(|_| Kv3DecodeError::InvalidStringId(id))?)
            .cloned()
            .ok_or(Kv3DecodeError::InvalidStringId(id))
    }

    fn read_value(&mut self, node_type: u8, depth: usize) -> Result<Kv3Value, Kv3DecodeError> {
        self.nodes += 1;
        if self.nodes > MAX_KV3_NODES {
            return Err(Kv3DecodeError::NodeLimitExceeded(MAX_KV3_NODES));
        }
        if depth > MAX_KV3_DEPTH {
            return Err(Kv3DecodeError::DepthLimitExceeded(MAX_KV3_DEPTH));
        }
        match node_type {
            TYPE_NULL => Ok(Kv3Value::Null),
            TYPE_BOOLEAN => Ok(Kv3Value::Bool(self.main.take_u8("boolean")? == 1)),
            TYPE_INT64 => Ok(Kv3Value::Int64(self.main.take_i64("int64")?)),
            TYPE_UINT64 => Ok(Kv3Value::UInt64(self.main.take_u64("uint64")?)),
            TYPE_DOUBLE => Ok(Kv3Value::Double(self.main.take_f64("double")?)),
            TYPE_STRING => {
                let string_id = self.main.take_i32("string id")?;
                Ok(Kv3Value::String(self.string(string_id)?))
            }
            TYPE_BINARY_BLOB => Err(Kv3DecodeError::UnsupportedBinaryBlobs),
            TYPE_ARRAY => {
                let length = count_from_i32(self.main.take_i32("array length")?, "array length")?;
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    let child_type = self.read_type()?;
                    values.push(self.read_value(child_type, depth + 1)?);
                }
                Ok(Kv3Value::Array(values))
            }
            TYPE_OBJECT => {
                let length = self.read_object_length()?;
                let mut fields = Vec::with_capacity(length);
                for _ in 0..length {
                    let child_type = self.read_type()?;
                    let key_id = self.main.take_i32("object key string id")?;
                    let key = self.string(key_id)?;
                    let value = self.read_value(child_type, depth + 1)?;
                    fields.push(Kv3Field { key, value });
                }
                Ok(Kv3Value::Object(fields))
            }
            TYPE_ARRAY_TYPED | TYPE_ARRAY_TYPE_BYTE_LENGTH => {
                let length = if node_type == TYPE_ARRAY_TYPED {
                    count_from_i32(
                        self.main.take_i32("typed array length")?,
                        "typed array length",
                    )?
                } else {
                    self.main.take_u8("typed byte-length array")? as usize
                };
                let subtype = self.read_type()?;
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    values.push(self.read_value(subtype, depth + 1)?);
                }
                Ok(Kv3Value::Array(values))
            }
            TYPE_INT32 => Ok(Kv3Value::Int32(self.main.take_i32("int32")?)),
            TYPE_UINT32 => Ok(Kv3Value::UInt32(self.main.take_u32("uint32")?)),
            TYPE_BOOLEAN_TRUE => Ok(Kv3Value::Bool(true)),
            TYPE_BOOLEAN_FALSE => Ok(Kv3Value::Bool(false)),
            TYPE_INT64_ZERO => Ok(Kv3Value::Int64(0)),
            TYPE_INT64_ONE => Ok(Kv3Value::Int64(1)),
            TYPE_DOUBLE_ZERO => Ok(Kv3Value::Double(0.0)),
            TYPE_DOUBLE_ONE => Ok(Kv3Value::Double(1.0)),
            TYPE_FLOAT => Ok(Kv3Value::Float(self.main.take_f32("float")?)),
            TYPE_INT16 => Ok(Kv3Value::Int32(self.main.take_i16("int16")? as i32)),
            TYPE_UINT16 => Ok(Kv3Value::UInt32(self.main.take_u16("uint16")? as u32)),
            TYPE_INT32_AS_BYTE => Ok(Kv3Value::Int32(self.main.take_u8("int32-as-byte")? as i32)),
            TYPE_ARRAY_TYPE_AUXILIARY_BUFFER => {
                let length = self.main.take_u8("auxiliary typed array length")? as usize;
                let subtype = self.read_type()?;
                std::mem::swap(&mut self.main, &mut self.auxiliary);
                let result = (|| {
                    let mut values = Vec::with_capacity(length);
                    for _ in 0..length {
                        values.push(self.read_value(subtype, depth + 1)?);
                    }
                    Ok(Kv3Value::Array(values))
                })();
                std::mem::swap(&mut self.main, &mut self.auxiliary);
                result
            }
            value => Err(Kv3DecodeError::UnsupportedNodeType(value)),
        }
    }
}

fn read_buffer(
    reader: &mut Reader<'_>,
    compression: BinaryKv3Compression,
    compressed_size: usize,
    uncompressed_size: usize,
) -> Result<Vec<u8>, Kv3DecodeError> {
    match compression {
        BinaryKv3Compression::Uncompressed => Ok(reader
            .take(uncompressed_size, "uncompressed buffer")?
            .to_vec()),
        BinaryKv3Compression::Lz4 => {
            let input = reader.take(compressed_size, "LZ4 buffer")?;
            let mut output = vec![0; uncompressed_size];
            let decoded = decompress_into(input, &mut output)
                .map_err(|error| Kv3DecodeError::Lz4(error.to_string()))?;
            if decoded != uncompressed_size {
                return Err(Kv3DecodeError::Lz4(format!(
                    "expected {uncompressed_size} bytes, got {decoded}"
                )));
            }
            Ok(output)
        }
        BinaryKv3Compression::Zstd => Err(Kv3DecodeError::UnsupportedZstd),
    }
}

fn take_pool(
    bytes: &[u8],
    offset: &mut usize,
    length: usize,
    label: &'static str,
) -> Result<Vec<u8>, Kv3DecodeError> {
    let end = offset
        .checked_add(length)
        .ok_or(Kv3DecodeError::UnexpectedEof(label))?;
    let value = bytes
        .get(*offset..end)
        .ok_or(Kv3DecodeError::UnexpectedEof(label))?
        .to_vec();
    *offset = end;
    Ok(value)
}

fn main_pool_end(bytes: &[u8], counts: PoolCounts, start: usize) -> Result<usize, Kv3DecodeError> {
    let pools = Pools::from_buffer(bytes, counts, start)?;
    let mut offset = start;
    offset += pools.bytes1.len();
    if !pools.bytes2.is_empty() {
        align(&mut offset, 2);
        offset += pools.bytes2.len();
    }
    if !pools.bytes4.is_empty() {
        align(&mut offset, 4);
        offset += pools.bytes4.len();
    }
    if !pools.bytes8.is_empty() {
        align(&mut offset, 8);
        offset += pools.bytes8.len();
    }
    Ok(offset)
}

fn take_null_terminated_utf8(pools: &mut Pools) -> Result<String, Kv3DecodeError> {
    let start = pools.bytes1_position;
    let remainder = pools
        .bytes1
        .get(start..)
        .ok_or(Kv3DecodeError::UnterminatedString)?;
    let nul = remainder
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Kv3DecodeError::UnterminatedString)?;
    let value = std::str::from_utf8(&remainder[..nul])
        .map_err(|_| Kv3DecodeError::InvalidUtf8)?
        .to_owned();
    pools.bytes1_position += nul + 1;
    Ok(value)
}

fn count_from_i32(value: i32, label: &'static str) -> Result<usize, Kv3DecodeError> {
    usize::try_from(value).map_err(|_| Kv3DecodeError::NegativeCount { label })
}

fn align(value: &mut usize, alignment: usize) {
    *value = (*value + alignment - 1) & !(alignment - 1);
}

#[cfg(test)]
mod tests {
    use super::{decode_binary_kv3_v5, Kv3DecodeError};
    use crate::kv3::Kv3Value;

    #[test]
    fn decodes_uncompressed_object_tree() {
        let document = decode_binary_kv3_v5(&fixture(false)).expect("fixture must decode");
        assert_eq!(
            document
                .root
                .object_field("_class")
                .and_then(Kv3Value::as_str),
            Some("CRenderMesh")
        );
    }

    #[test]
    fn decodes_lz4_object_tree() {
        let document = decode_binary_kv3_v5(&fixture(true)).expect("fixture must decode");
        assert_eq!(
            document
                .root
                .object_field("_class")
                .and_then(Kv3Value::as_str),
            Some("CRenderMesh")
        );
    }

    #[test]
    fn rejects_invalid_magic() {
        let mut input = fixture(false);
        input[0] = 0;
        assert!(matches!(
            decode_binary_kv3_v5(&input),
            Err(Kv3DecodeError::InvalidMagic)
        ));
    }

    #[test]
    #[ignore = "requires SENTINEL_GATE1C_VMDL to point to the local oracle resource"]
    fn local_oracle_vmdl_blocks_decode_to_expected_generic_facts() {
        let path = std::env::var("SENTINEL_GATE1C_VMDL")
            .expect("SENTINEL_GATE1C_VMDL must point to local ctm_diver_varianta.vmdl_c");
        let resource = std::fs::read(&path).expect("local VMDL resource must be readable");
        let descriptor = crate::describe_vmdl_file(std::path::Path::new(&path))
            .expect("local VMDL resource must describe");
        let mut documents = std::collections::BTreeMap::new();
        for tag in ["MDAT", "CTRL", "RED2", "DATA"] {
            let block = descriptor
                .blocks
                .iter()
                .find(|block| block.tag == tag)
                .expect("oracle block must exist");
            let start = block.offset as usize;
            let end = start + block.stored_size as usize;
            documents.insert(
                tag,
                decode_binary_kv3_v5(&resource[start..end]).expect("oracle block must decode"),
            );
        }

        let mdat = documents.get("MDAT").expect("MDAT must decode");
        assert_eq!(
            mdat.root.object_field("_class").and_then(Kv3Value::as_str),
            Some("CRenderMesh")
        );
        assert_eq!(
            mdat.root
                .object_field("m_hitboxsets")
                .and_then(Kv3Value::as_array),
            Some([].as_slice())
        );

        let data = documents.get("DATA").expect("DATA must decode");
        let skeleton = data
            .root
            .object_field("m_modelSkeleton")
            .expect("DATA exposes model skeleton tree");
        assert_eq!(
            skeleton
                .object_field("m_boneName")
                .and_then(Kv3Value::as_array),
            Some([Kv3Value::String("dummy".to_owned())].as_slice())
        );
        assert_eq!(
            skeleton
                .object_field("m_nParent")
                .and_then(Kv3Value::as_array),
            Some([Kv3Value::Int32(-1)].as_slice())
        );
    }

    fn fixture(lz4: bool) -> Vec<u8> {
        let mut auxiliary = b"_class\0CRenderMesh\0".to_vec();
        while auxiliary.len() % 4 != 0 {
            auxiliary.push(0);
        }
        auxiliary.extend_from_slice(&2_i32.to_le_bytes());

        let mut main = Vec::new();
        main.extend_from_slice(&1_i32.to_le_bytes()); // object length
        main.extend_from_slice(&0_i32.to_le_bytes()); // key string id
        main.extend_from_slice(&1_i32.to_le_bytes()); // value string id
        main.extend_from_slice(&[9, 6]); // object, string
        main.extend_from_slice(&0xFFEEDD00_u32.to_le_bytes());

        let (auxiliary_payload, main_payload, compression, compressed_total) = if lz4 {
            let auxiliary_payload = literal_lz4(&auxiliary);
            let main_payload = literal_lz4(&main);
            let compressed_total = auxiliary_payload.len() + main_payload.len();
            (auxiliary_payload, main_payload, 1_u32, compressed_total)
        } else {
            (auxiliary.clone(), main.clone(), 0_u32, 0)
        };
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[5, b'3', b'V', b'K']);
        bytes.extend_from_slice(&[0; 16]);
        bytes.extend_from_slice(&compression.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(if lz4 { 16 * 1024_u16 } else { 0 }).to_le_bytes());
        push_i32(&mut bytes, 19); // auxiliary bytes1, excluding alignment bytes
        push_i32(&mut bytes, 1);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 2); // types
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        push_i32(&mut bytes, (auxiliary.len() + main.len()) as i32);
        push_i32(&mut bytes, compressed_total as i32);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, auxiliary.len() as i32);
        push_i32(
            &mut bytes,
            if lz4 {
                auxiliary_payload.len() as i32
            } else {
                0
            },
        );
        push_i32(&mut bytes, main.len() as i32);
        push_i32(&mut bytes, if lz4 { main_payload.len() as i32 } else { 0 });
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 2);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 1);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        bytes.extend_from_slice(&auxiliary_payload);
        bytes.extend_from_slice(&main_payload);
        bytes
    }

    fn literal_lz4(input: &[u8]) -> Vec<u8> {
        let mut output = vec![0xF0];
        let mut remainder = input.len() - 15;
        while remainder >= 255 {
            output.push(255);
            remainder -= 255;
        }
        output.push(remainder as u8);
        output.extend_from_slice(input);
        output
    }

    fn push_i32(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
