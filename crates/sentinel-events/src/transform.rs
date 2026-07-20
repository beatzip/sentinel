use byteorder::ReadBytesExt;
use sentinel_core::Tick;
use std::io::Read;

use crate::kinds::{EventKind, EventValue, GameEvent};

/// Raw packet data extracted from a demo frame
#[derive(Debug, Clone)]
pub struct RawPacketData {
    pub tick: u32,
    pub data: Vec<u8>,
}

/// Transform raw packet data into typed game events
pub struct EventTransformer;

/// Well-known CS2 game event type IDs (Source 2)
const EVENT_PLAYER_DEATH: u16 = 1;
const EVENT_PLAYER_SPAWN: u16 = 2;
const EVENT_WEAPON_FIRE: u16 = 3;
const EVENT_PLAYER_HURT: u16 = 4;
const EVENT_ROUND_START: u16 = 5;
const EVENT_ROUND_END: u16 = 6;
const EVENT_BOMB_PLANT: u16 = 7;
const EVENT_BOMB_DEFUSE: u16 = 8;
const EVENT_SMOKE_DETONATE: u16 = 9;
const EVENT_FLASH_DETONATE: u16 = 10;
const EVENT_HE_DETONATE: u16 = 11;
const EVENT_MOLOTOV_DETONATE: u16 = 12;

impl EventTransformer {
    /// Transform a raw packet into game events.
    /// This reads the binary packet data and produces typed GameEvents.
    pub fn transform_packet(packet: &RawPacketData) -> Vec<GameEvent> {
        let tick = Tick::new(packet.tick);
        let mut events = Vec::new();

        // Parse binary commands from the packet
        let mut cursor = std::io::Cursor::new(packet.data.as_slice());

        while let Ok(cmd_type) = cursor.read_u8() {
            match cmd_type {
                // Game event command
                1 => {
                    if let Some(event) = Self::decode_game_event(tick, &mut cursor) {
                        events.push(event);
                    }
                }
                // Entity update — skip
                2 => {
                    let _ = Self::skip_entity_update(&mut cursor);
                }
                // Skip/padding
                3 => {
                    let _ = cursor.read_u8();
                }
                // Unknown — skip 4 bytes
                _ => {
                    let mut buf = [0u8; 4];
                    let _ = cursor.read_exact(&mut buf);
                }
            }
        }

        events
    }

    /// Decode a single game event from binary
    fn decode_game_event(tick: Tick, cursor: &mut std::io::Cursor<&[u8]>) -> Option<GameEvent> {
        use byteorder::{LittleEndian, ReadBytesExt};

        let event_type = cursor.read_u16::<LittleEndian>().ok()?;
        let field_count = cursor.read_u8().ok()? as usize;

        let mut fields = std::collections::BTreeMap::new();
        for i in 0..field_count {
            let type_tag = cursor.read_u8().ok()?;
            let key = format!("field_{}", i);

            match type_tag {
                0 => {
                    let val = cursor.read_i64::<LittleEndian>().ok()?;
                    fields.insert(key, EventValue::Integer(val));
                }
                1 => {
                    let val = cursor.read_f64::<LittleEndian>().ok()?;
                    fields.insert(key, EventValue::Float(val));
                }
                2 => {
                    let len = cursor.read_u16::<LittleEndian>().ok()? as usize;
                    let mut buf = vec![0u8; len];
                    cursor.read_exact(&mut buf).ok()?;
                    fields.insert(
                        key,
                        EventValue::String(String::from_utf8_lossy(&buf).to_string()),
                    );
                }
                3 => {
                    let val = cursor.read_u8().ok()? != 0;
                    fields.insert(key, EventValue::Boolean(val));
                }
                4 => {
                    let val = cursor.read_u32::<LittleEndian>().ok()?;
                    fields.insert(key, EventValue::PlayerId(val as u64));
                }
                _ => continue,
            }
        }

        // Map event type ID to EventKind and extract known fields
        let (kind, field_map) = Self::map_event(event_type, fields)?;
        Some(GameEvent {
            kind,
            tick,
            data: field_map,
        })
    }

    /// Map raw event type ID and fields to a typed GameEvent
    fn map_event(
        event_type: u16,
        fields: std::collections::BTreeMap<String, EventValue>,
    ) -> Option<(EventKind, std::collections::BTreeMap<String, EventValue>)> {
        match event_type {
            EVENT_PLAYER_DEATH => {
                let mut data = std::collections::BTreeMap::new();
                // Extract attacker, victim, weapon from fields
                if let Some(v) = fields.get("field_0") {
                    data.insert("attacker".to_string(), v.clone());
                }
                if let Some(v) = fields.get("field_1") {
                    data.insert("victim".to_string(), v.clone());
                }
                if let Some(v) = fields.get("field_2") {
                    data.insert("weapon".to_string(), v.clone());
                }
                Some((EventKind::PlayerDeath, data))
            }
            EVENT_PLAYER_SPAWN => {
                let mut data = std::collections::BTreeMap::new();
                if let Some(v) = fields.get("field_0") {
                    data.insert("userid".to_string(), v.clone());
                }
                if let Some(v) = fields.get("field_1") {
                    data.insert("team".to_string(), v.clone());
                }
                Some((EventKind::PlayerSpawn, data))
            }
            EVENT_WEAPON_FIRE => {
                let mut data = std::collections::BTreeMap::new();
                if let Some(v) = fields.get("field_0") {
                    data.insert("userid".to_string(), v.clone());
                }
                if let Some(v) = fields.get("field_1") {
                    data.insert("weapon".to_string(), v.clone());
                }
                Some((EventKind::WeaponFire, data))
            }
            EVENT_PLAYER_HURT => {
                let mut data = std::collections::BTreeMap::new();
                if let Some(v) = fields.get("field_0") {
                    data.insert("userid".to_string(), v.clone());
                }
                if let Some(v) = fields.get("field_1") {
                    data.insert("dmg_health".to_string(), v.clone());
                }
                Some((EventKind::PlayerHurt, data))
            }
            EVENT_ROUND_START => {
                let mut data = std::collections::BTreeMap::new();
                if let Some(v) = fields.get("field_0") {
                    data.insert("round".to_string(), v.clone());
                }
                Some((EventKind::RoundStart, data))
            }
            EVENT_ROUND_END => {
                let mut data = std::collections::BTreeMap::new();
                if let Some(v) = fields.get("field_0") {
                    data.insert("winner".to_string(), v.clone());
                }
                Some((EventKind::RoundEnd, data))
            }
            EVENT_BOMB_PLANT => {
                let mut data = std::collections::BTreeMap::new();
                if let Some(v) = fields.get("field_0") {
                    data.insert("userid".to_string(), v.clone());
                }
                if let Some(v) = fields.get("field_1") {
                    data.insert("site".to_string(), v.clone());
                }
                Some((EventKind::BombPlant, data))
            }
            EVENT_BOMB_DEFUSE => Some((EventKind::BombDefuse, fields)),
            EVENT_SMOKE_DETONATE => {
                let mut data = std::collections::BTreeMap::new();
                if let Some(v) = fields.get("field_0") {
                    data.insert("userid".to_string(), v.clone());
                }
                Some((EventKind::SmokeGrenadeDetonate, data))
            }
            EVENT_FLASH_DETONATE => Some((EventKind::FlashGrenadeDetonate, fields)),
            EVENT_HE_DETONATE => Some((EventKind::HEGrenadeDetonate, fields)),
            EVENT_MOLOTOV_DETONATE => Some((EventKind::MolotovDetonate, fields)),
            _ => None, // Unknown event type — skip
        }
    }

    /// Skip an entity update block in the binary stream
    fn skip_entity_update(cursor: &mut std::io::Cursor<&[u8]>) -> Option<()> {
        use byteorder::{LittleEndian, ReadBytesExt};

        let entity_count = cursor.read_u16::<LittleEndian>().ok()?;
        for _ in 0..entity_count {
            let _ = cursor.read_u32::<LittleEndian>().ok()?; // entity_id
            let field_count = cursor.read_u8().ok()? as usize;
            for _ in 0..field_count {
                let type_tag = cursor.read_u8().ok()?;
                match type_tag {
                    0 => {
                        let _ = cursor.read_i64::<LittleEndian>().ok()?;
                    }
                    1 => {
                        let _ = cursor.read_f64::<LittleEndian>().ok()?;
                    }
                    2 => {
                        let len = cursor.read_u16::<LittleEndian>().ok()? as usize;
                        let mut buf = vec![0u8; len];
                        cursor.read_exact(&mut buf).ok()?;
                    }
                    _ => {
                        let _ = cursor.read_u8().ok()?;
                    }
                }
            }
        }
        Some(())
    }

    /// Transform multiple packets into events
    pub fn transform_packets(packets: &[RawPacketData]) -> Vec<GameEvent> {
        packets.iter().flat_map(Self::transform_packet).collect()
    }
}

/// Event stream that yields events from a demo file
pub struct EventStream {
    events: Vec<GameEvent>,
    position: usize,
}

impl EventStream {
    pub fn new(events: Vec<GameEvent>) -> Self {
        Self {
            events,
            position: 0,
        }
    }

    pub fn advance(&mut self) -> Option<&GameEvent> {
        if self.position < self.events.len() {
            let event = &self.events[self.position];
            self.position += 1;
            Some(event)
        } else {
            None
        }
    }

    pub fn peek(&self) -> Option<&GameEvent> {
        self.events.get(self.position)
    }

    pub fn len(&self) -> usize {
        self.events.len() - self.position
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn filter_by_kind(&self, kind: &EventKind) -> Vec<&GameEvent> {
        self.events[self.position..]
            .iter()
            .filter(|e| e.kind == *kind)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_empty_packet() {
        let packet = RawPacketData {
            tick: 100,
            data: vec![],
        };
        let events = EventTransformer::transform_packet(&packet);
        assert!(events.is_empty());
    }

    #[test]
    fn test_event_stream() {
        let events = vec![
            crate::kinds::player_death(Tick(100), 1, 2, "ak47"),
            crate::kinds::weapon_fire(Tick(101), 1, "ak47"),
            crate::kinds::player_death(Tick(102), 3, 4, "awp"),
        ];

        let mut stream = EventStream::new(events);
        assert_eq!(stream.len(), 3);

        let first = stream.advance().unwrap();
        assert_eq!(first.kind, EventKind::PlayerDeath);
        assert_eq!(stream.len(), 2);
    }
}
