use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Read;

use sentinel_common::error::{Result, SentinelError};

use crate::header::DemHeader;
use crate::reader::{DemFrame, FrameType};

/// Parsed demo file with header and frames
pub struct ParsedDemo {
    pub header: DemHeader,
    pub frames: Vec<DemFrame>,
}

impl ParsedDemo {
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        let header = DemHeader::read(&mut reader)?;
        let mut frames = Vec::new();
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        let mut cursor = std::io::Cursor::new(buffer);
        while let Ok(frame_type_byte) = cursor.read_u8() {
            let frame_type = match FrameType::from_u8(frame_type_byte) {
                Some(ft) => ft,
                None => continue,
            };
            if frame_type == FrameType::Stop {
                break;
            }
            let tick = cursor.read_u32::<LittleEndian>()?;
            let frame_size = cursor.read_u32::<LittleEndian>()? as usize;
            let mut data = vec![0u8; frame_size];
            cursor.read_exact(&mut data)?;
            frames.push(DemFrame {
                frame_type,
                tick,
                data,
            });
        }
        Ok(Self { header, frames })
    }

    pub fn from_reader(mut reader: impl Read) -> Result<Self> {
        let header = DemHeader::read(&mut reader)?;
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        let mut frames = Vec::new();
        let mut cursor = std::io::Cursor::new(buffer);
        while let Ok(frame_type_byte) = cursor.read_u8() {
            let frame_type = match FrameType::from_u8(frame_type_byte) {
                Some(ft) => ft,
                None => continue,
            };
            if frame_type == FrameType::Stop {
                break;
            }
            let tick = cursor.read_u32::<LittleEndian>()?;
            let frame_size = cursor.read_u32::<LittleEndian>()? as usize;
            let mut data = vec![0u8; frame_size];
            cursor.read_exact(&mut data)?;
            frames.push(DemFrame {
                frame_type,
                tick,
                data,
            });
        }
        Ok(Self { header, frames })
    }

    pub fn packet_frames(&self) -> impl Iterator<Item = &DemFrame> {
        self.frames
            .iter()
            .filter(|f| f.frame_type == FrameType::Packet)
    }

    pub fn tick_count(&self) -> u32 {
        self.header.total_ticks
    }

    pub fn tick_rate(&self) -> u32 {
        self.header.tick_rate
    }
}

/// Decode binary packet data into a list of raw game events.
///
/// Source 2 demo packets contain a sequence of commands. Each command is:
///   command_type: u8
///   command_data: variable length
///
/// We handle known command types and skip unknown ones.
pub fn decode_packet(tick: u32, data: &[u8]) -> Vec<RawGameEvent> {
    let mut events = Vec::new();
    let mut cursor = std::io::Cursor::new(data);

    while let Ok(cmd_type) = cursor.read_u8() {
        match cmd_type {
            // Game event command (type 1)
            1 => {
                if let Ok(event) = decode_game_event(tick, &mut cursor) {
                    events.push(event);
                }
            }
            // Entity update command (type 2)
            2 => {
                if let Ok(updates) = decode_entity_update(tick, &mut cursor) {
                    events.extend(updates);
                }
            }
            // Skip command (type 3) — just a padding byte
            3 => {
                let _ = cursor.read_u8();
            }
            // Unknown command — skip 4 bytes and continue
            _ => {
                let mut skip = [0u8; 4];
                let _ = cursor.read_exact(&mut skip);
            }
        }
    }

    events
}

/// A raw game event decoded from packet data
#[derive(Debug, Clone)]
pub struct RawGameEvent {
    pub tick: u32,
    pub event_type: u16,
    pub fields: Vec<RawEventField>,
}

#[derive(Debug, Clone)]
pub enum RawEventField {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    EntityId(u32),
}

/// Decode a single game event from the cursor
fn decode_game_event(tick: u32, cursor: &mut std::io::Cursor<&[u8]>) -> Result<RawGameEvent> {
    let event_type = cursor.read_u16::<LittleEndian>()?;
    let field_count = cursor.read_u8()? as usize;

    let mut fields = Vec::new();
    for _ in 0..field_count {
        let type_tag = cursor.read_u8()?;
        let field = match type_tag {
            0 => RawEventField::Int(cursor.read_i64::<LittleEndian>()?),
            1 => RawEventField::Float(cursor.read_f64::<LittleEndian>()?),
            2 => {
                let len = cursor.read_u16::<LittleEndian>()? as usize;
                let mut buf = vec![0u8; len];
                cursor.read_exact(&mut buf)?;
                RawEventField::String(String::from_utf8_lossy(&buf).to_string())
            }
            3 => RawEventField::Bool(cursor.read_u8()? != 0),
            4 => RawEventField::EntityId(cursor.read_u32::<LittleEndian>()?),
            _ => {
                return Err(SentinelError::Parse(format!(
                    "Unknown event field type: {type_tag}"
                )));
            }
        };
        fields.push(field);
    }

    Ok(RawGameEvent {
        tick,
        event_type,
        fields,
    })
}

/// Decode entity update commands
fn decode_entity_update(
    tick: u32,
    cursor: &mut std::io::Cursor<&[u8]>,
) -> Result<Vec<RawGameEvent>> {
    let mut events = Vec::new();
    let entity_count = cursor.read_u16::<LittleEndian>()?;

    for _ in 0..entity_count {
        let entity_id = cursor.read_u32::<LittleEndian>()?;
        let field_count = cursor.read_u8()? as usize;

        let mut fields = vec![RawEventField::EntityId(entity_id)];
        for _ in 0..field_count {
            let type_tag = cursor.read_u8()?;
            let field = match type_tag {
                0 => RawEventField::Int(cursor.read_i64::<LittleEndian>()?),
                1 => RawEventField::Float(cursor.read_f64::<LittleEndian>()?),
                2 => {
                    let len = cursor.read_u16::<LittleEndian>()? as usize;
                    let mut buf = vec![0u8; len];
                    cursor.read_exact(&mut buf)?;
                    RawEventField::String(String::from_utf8_lossy(&buf).to_string())
                }
                _ => RawEventField::Int(0),
            };
            fields.push(field);
        }

        events.push(RawGameEvent {
            tick,
            event_type: 1000, // Entity update marker
            fields,
        });
    }

    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parsed_demo_creation() {
        let mut data = Vec::new();
        data.extend_from_slice(b"HL2DEMO\0");
        data.extend_from_slice(&24u32.to_le_bytes());
        data.extend_from_slice(&24u32.to_le_bytes());
        let mut server_name = vec![0u8; 64];
        server_name[..11].copy_from_slice(b"Test Server");
        data.extend_from_slice(&server_name);
        let mut client_name = vec![0u8; 64];
        client_name[..4].copy_from_slice(b"Test");
        data.extend_from_slice(&client_name);
        let mut map_name = vec![0u8; 64];
        map_name[..8].copy_from_slice(b"de_dust2");
        data.extend_from_slice(&map_name);
        let mut game_dir = vec![0u8; 260];
        game_dir[..4].copy_from_slice(b"csgo");
        data.extend_from_slice(&game_dir);
        data.extend_from_slice(&1000u32.to_le_bytes());
        data.extend_from_slice(&64u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 8]);
        data.push(7); // Stop frame

        let mut cursor = Cursor::new(data);
        let result = ParsedDemo::from_reader(&mut cursor);
        assert!(result.is_ok());
        let demo = result.unwrap();
        assert_eq!(demo.header.server_name, "Test Server");
        assert_eq!(demo.header.map_name, "de_dust2");
    }

    #[test]
    fn test_decode_empty_packet() {
        let events = decode_packet(100, &[]);
        assert!(events.is_empty());
    }
}
