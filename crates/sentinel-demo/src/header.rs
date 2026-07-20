use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Read;

use sentinel_common::error::{Result, SentinelError};

/// Source 2 demo file magic number: "HL2DEMO\0"
const DEMO_MAGIC: &[u8; 8] = b"HL2DEMO\0";

/// Demo file header containing metadata about the recording
#[derive(Debug, Clone)]
pub struct DemHeader {
    /// Demo protocol version
    pub protocol: u32,
    /// Network protocol version
    pub network_protocol: u32,
    /// Server name
    pub server_name: String,
    /// Client name (who recorded)
    pub client_name: String,
    /// Map name
    pub map_name: String,
    /// Game directory
    pub game_dir: String,
    /// Total time in seconds
    pub total_ticks: u32,
    /// Frame rate (tick rate)
    pub tick_rate: u32,
}

impl DemHeader {
    /// Parse a demo file header from a reader
    pub fn read(reader: &mut impl Read) -> Result<Self> {
        // Read and verify magic number
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        if magic != *DEMO_MAGIC {
            return Err(SentinelError::InvalidDemoFormat(
                "Invalid magic number".to_string(),
            ));
        }

        // Read header fields
        let protocol = reader.read_u32::<LittleEndian>()?;
        let network_protocol = reader.read_u32::<LittleEndian>()?;

        // Skip server name placeholder (we'll read the actual string)
        let server_name = read_c_string(reader, 64)?;
        let client_name = read_c_string(reader, 64)?;
        let map_name = read_c_string(reader, 64)?;
        let game_dir = read_c_string(reader, 260)?;

        let total_ticks = reader.read_u32::<LittleEndian>()?;
        let tick_rate = reader.read_u32::<LittleEndian>()?;

        // Skip remaining header bytes (there are more fields we don't need)
        let mut remaining = [0u8; 8];
        let _ = reader.read_exact(&mut remaining);

        Ok(Self {
            protocol,
            network_protocol,
            server_name,
            client_name,
            map_name,
            game_dir,
            total_ticks,
            tick_rate,
        })
    }

    /// Get the duration in seconds
    pub fn duration_seconds(&self) -> f64 {
        self.total_ticks as f64 / self.tick_rate as f64
    }
}

/// Read a null-terminated C string with a maximum length
fn read_c_string(reader: &mut impl Read, max_len: usize) -> Result<String> {
    let mut buf = vec![0u8; max_len];
    reader.read_exact(&mut buf)?;

    // Find the null terminator
    let end = buf.iter().position(|&b| b == 0).unwrap_or(max_len);
    let s = String::from_utf8_lossy(&buf[..end]).to_string();
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_invalid_magic() {
        let data = [0u8; 100];
        let mut cursor = Cursor::new(data);
        let result = DemHeader::read(&mut cursor);
        assert!(result.is_err());
    }
}
