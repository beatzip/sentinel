use super::decoder::{Kv3DecodeError, Reader};

pub const BINARY_KV3_V5_MAGIC: [u8; 4] = [5, b'3', b'V', b'K'];
pub const BINARY_KV3_V5_HEADER_BYTES: usize = 120;
pub const BINARY_KV3_TRAILER: u32 = 0xFFEEDD00;
pub const MAX_KV3_BUFFER_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_KV3_NODES: usize = 1_000_000;
pub const MAX_KV3_DEPTH: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryKv3Compression {
    Uncompressed,
    Lz4,
    Zstd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolCounts {
    pub bytes1: usize,
    pub bytes2: usize,
    pub bytes4: usize,
    pub bytes8: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinaryKv3Header {
    pub format_guid: [u8; 16],
    pub compression: BinaryKv3Compression,
    pub compression_dictionary_id: u16,
    pub compression_frame_size: u16,
    pub auxiliary_counts: PoolCounts,
    pub type_count: usize,
    pub object_count: usize,
    pub array_count: usize,
    pub uncompressed_size: usize,
    pub compressed_size: usize,
    pub binary_blob_count: usize,
    pub binary_blob_bytes: usize,
    pub auxiliary_uncompressed_size: usize,
    pub auxiliary_compressed_size: usize,
    pub main_uncompressed_size: usize,
    pub main_compressed_size: usize,
    pub main_counts: PoolCounts,
    pub main_object_count: usize,
    pub main_array_count: usize,
}

impl BinaryKv3Header {
    pub(crate) fn parse(reader: &mut Reader<'_>) -> Result<Self, Kv3DecodeError> {
        if reader.read_array::<4>("magic")? != BINARY_KV3_V5_MAGIC {
            return Err(Kv3DecodeError::InvalidMagic);
        }
        let format_guid = reader.read_array::<16>("format GUID")?;
        let compression = match reader.read_u32("compression method")? {
            0 => BinaryKv3Compression::Uncompressed,
            1 => BinaryKv3Compression::Lz4,
            2 => BinaryKv3Compression::Zstd,
            value => return Err(Kv3DecodeError::UnsupportedCompression(value)),
        };

        let compression_dictionary_id = reader.read_u16("compression dictionary ID")?;
        let compression_frame_size = reader.read_u16("compression frame size")?;
        let auxiliary_bytes1 = reader.read_count("auxiliary byte count")?;
        let auxiliary_bytes4 = reader.read_count("auxiliary dword count")?;
        let auxiliary_bytes8 = reader.read_count("auxiliary qword count")?;
        let type_count = reader.read_count("type count")?;
        let object_count = reader.read_u16("object count")? as usize;
        let array_count = reader.read_u16("array count")? as usize;
        let uncompressed_size = reader.read_count("total uncompressed size")?;
        let compressed_size = reader.read_count("total compressed size")?;
        let binary_blob_count = reader.read_count("binary blob count")?;
        let binary_blob_bytes = reader.read_count("binary blob byte size")?;
        let auxiliary_bytes2 = reader.read_count("auxiliary word count v4")?;
        let _block_compressed_sizes_bytes = reader.read_count("block compressed size table")?;
        let auxiliary_uncompressed_size = reader.read_count("auxiliary uncompressed size")?;
        let auxiliary_compressed_size = reader.read_count("auxiliary compressed size")?;
        let main_uncompressed_size = reader.read_count("main uncompressed size")?;
        let main_compressed_size = reader.read_count("main compressed size")?;
        let main_counts = PoolCounts {
            bytes1: reader.read_count("main byte count")?,
            bytes2: reader.read_count("main word count")?,
            bytes4: reader.read_count("main dword count")?,
            bytes8: reader.read_count("main qword count")?,
        };
        let _unknown_13 = reader.read_count("unknown header field 13")?;
        let main_object_count = reader.read_count("main object count")?;
        let main_array_count = reader.read_count("main array count")?;
        let _unknown_16 = reader.read_count("unknown header field 16")?;
        let auxiliary_counts = PoolCounts {
            bytes1: auxiliary_bytes1,
            bytes2: auxiliary_bytes2,
            bytes4: auxiliary_bytes4,
            bytes8: auxiliary_bytes8,
        };

        if reader.position() != BINARY_KV3_V5_HEADER_BYTES {
            return Err(Kv3DecodeError::InvalidHeader("unexpected v5 header length"));
        }
        if uncompressed_size != auxiliary_uncompressed_size + main_uncompressed_size {
            return Err(Kv3DecodeError::InvalidHeader(
                "total uncompressed size does not match both buffers",
            ));
        }
        for (label, size) in [
            ("auxiliary uncompressed buffer", auxiliary_uncompressed_size),
            ("main uncompressed buffer", main_uncompressed_size),
            ("binary blob buffer", binary_blob_bytes),
        ] {
            if size > MAX_KV3_BUFFER_BYTES {
                return Err(Kv3DecodeError::LimitExceeded {
                    label,
                    limit: MAX_KV3_BUFFER_BYTES,
                });
            }
        }
        if binary_blob_count != 0 || binary_blob_bytes != 0 {
            return Err(Kv3DecodeError::UnsupportedBinaryBlobs);
        }
        if compression == BinaryKv3Compression::Zstd {
            return Err(Kv3DecodeError::UnsupportedZstd);
        }
        if compression == BinaryKv3Compression::Lz4
            && (compression_dictionary_id != 0 || compression_frame_size != 16 * 1024)
        {
            return Err(Kv3DecodeError::InvalidHeader(
                "unsupported LZ4 dictionary or frame size",
            ));
        }
        if compression == BinaryKv3Compression::Uncompressed
            && (auxiliary_compressed_size != 0 || main_compressed_size != 0)
        {
            return Err(Kv3DecodeError::InvalidHeader(
                "uncompressed data declares compressed buffer bytes",
            ));
        }

        Ok(Self {
            format_guid,
            compression,
            compression_dictionary_id,
            compression_frame_size,
            auxiliary_counts,
            type_count,
            object_count,
            array_count,
            uncompressed_size,
            compressed_size,
            binary_blob_count,
            binary_blob_bytes,
            auxiliary_uncompressed_size,
            auxiliary_compressed_size,
            main_uncompressed_size,
            main_compressed_size,
            main_counts,
            main_object_count,
            main_array_count,
        })
    }
}
