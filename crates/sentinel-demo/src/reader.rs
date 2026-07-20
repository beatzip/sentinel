use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Read;

use sentinel_common::error::Result;

/// Demo frame types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    /// Signon/frame data
    Signon = 1,
    /// Packet data
    Packet = 2,
    /// Sync tick (client should stop rendering)
    SyncTick = 3,
    /// Console command
    ConsoleCommand = 4,
    /// User input
    UserInput = 5,
    /// Data tables
    DataTables = 6,
    /// Stop recording
    Stop = 7,
    /// Custom data
    CustomData = 8,
    /// String tables
    StringTables = 9,
}

impl FrameType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Signon),
            2 => Some(Self::Packet),
            3 => Some(Self::SyncTick),
            4 => Some(Self::ConsoleCommand),
            5 => Some(Self::UserInput),
            6 => Some(Self::DataTables),
            7 => Some(Self::Stop),
            8 => Some(Self::CustomData),
            9 => Some(Self::StringTables),
            _ => None,
        }
    }
}

/// A single frame from the demo file
#[derive(Debug, Clone)]
pub struct DemFrame {
    /// Frame type
    pub frame_type: FrameType,
    /// Tick number when this frame was recorded
    pub tick: u32,
    /// Raw frame data (varies by type)
    pub data: Vec<u8>,
}

/// Reader for streaming through demo file frames
pub struct DemReader {
    reader: Box<dyn Read>,
    frame_count: u32,
}

impl DemReader {
    pub fn new(reader: impl Read + 'static) -> Self {
        Self {
            reader: Box::new(reader),
            frame_count: 0,
        }
    }

    /// Read the next frame from the demo file
    pub fn read_frame(&mut self) -> Result<Option<DemFrame>> {
        // Read frame type
        let frame_type_byte = match self.reader.read_u8() {
            Ok(b) => b,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    return Ok(None);
                }
                return Err(e.into());
            }
        };

        let frame_type = match FrameType::from_u8(frame_type_byte) {
            Some(ft) => ft,
            None => {
                return Err(sentinel_common::error::SentinelError::Parse(format!(
                    "Unknown frame type: {}",
                    frame_type_byte
                )));
            }
        };

        // Stop frame has no additional data
        if frame_type == FrameType::Stop {
            return Ok(Some(DemFrame {
                frame_type,
                tick: 0,
                data: Vec::new(),
            }));
        }

        // Read tick number
        let tick = self.reader.read_u32::<LittleEndian>()?;

        // Read frame size
        let frame_size = self.reader.read_u32::<LittleEndian>()? as usize;

        // Read frame data
        let mut data = vec![0u8; frame_size];
        self.reader.read_exact(&mut data)?;

        self.frame_count += 1;

        Ok(Some(DemFrame {
            frame_type,
            tick,
            data,
        }))
    }

    /// Get the number of frames read so far
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }
}

/// Iterator over demo frames
pub struct DemFrameIterator {
    reader: DemReader,
}

impl DemFrameIterator {
    pub fn new(reader: impl Read + 'static) -> Self {
        Self {
            reader: DemReader::new(reader),
        }
    }
}

impl Iterator for DemFrameIterator {
    type Item = Result<DemFrame>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.read_frame() {
            Ok(Some(frame)) => {
                if frame.frame_type == FrameType::Stop {
                    None
                } else {
                    Some(Ok(frame))
                }
            }
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}
