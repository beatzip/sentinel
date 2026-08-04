use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use sentinel_core::Tick;

/// All game event types we track
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKind {
    // Player events
    PlayerSpawn,
    PlayerDeath,
    PlayerHurt,
    PlayerDisconnect,
    PlayerConnect,

    // Weapon events
    WeaponFire,
    ReloadStart,
    ReloadEnd,
    WeaponSwitch,

    // Movement events
    Jump,
    Land,
    CrouchToggle,
    Duck,
    PlayerSound,

    // Grenade events
    SmokeGrenadeDetonate,
    SmokeGrenadeExpired,
    FlashGrenadeDetonate,
    HEGrenadeDetonate,
    MolotovDetonate,
    InfernoStart,
    InfernoExpire,
    DecoyStart,
    DecoyExpire,

    // Bomb events
    BombPlant,
    BombDefuse,
    BombExplode,
    BombDropped,

    // Round events
    RoundStart,
    RoundEnd,
    RoundFreezeEnd,
    RoundMVP,

    // Kill events
    KillAssist,
    KillConfirmed,
}

/// A raw game event with associated data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEvent {
    /// The type of event
    pub kind: EventKind,
    /// Tick when this event occurred
    pub tick: Tick,
    /// Event-specific data as key-value pairs
    pub data: BTreeMap<String, EventValue>,
}

/// Event data value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventValue {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    PlayerId(u64),
    Vector(f32, f32, f32),
}

impl EventValue {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            EventValue::Integer(v) => Some(*v),
            EventValue::Float(v) => Some(*v as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            EventValue::Float(v) => Some(*v),
            EventValue::Integer(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            EventValue::String(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            EventValue::Boolean(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_player_id(&self) -> Option<u64> {
        match self {
            EventValue::PlayerId(v) => Some(*v),
            EventValue::Integer(v) => Some(*v as u64),
            _ => None,
        }
    }
}

/// Helper to create events quickly
pub fn make_event(kind: EventKind, tick: Tick, data: Vec<(&str, EventValue)>) -> GameEvent {
    let data = data.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    GameEvent { kind, tick, data }
}

/// Convenience constructors for common events
pub fn player_death(tick: Tick, attacker: u64, victim: u64, weapon: &str) -> GameEvent {
    make_event(
        EventKind::PlayerDeath,
        tick,
        vec![
            ("attacker", EventValue::PlayerId(attacker)),
            ("victim", EventValue::PlayerId(victim)),
            ("weapon", EventValue::String(weapon.to_string())),
        ],
    )
}

pub fn weapon_fire(tick: Tick, shooter: u64, weapon: &str) -> GameEvent {
    make_event(
        EventKind::WeaponFire,
        tick,
        vec![
            ("userid", EventValue::PlayerId(shooter)),
            ("weapon", EventValue::String(weapon.to_string())),
        ],
    )
}

pub fn player_hurt(
    tick: Tick,
    victim: u64,
    attacker: u64,
    weapon: &str,
    dmg_health: i64,
    hitgroup: i64,
    victim_health: i64,
) -> GameEvent {
    make_event(
        EventKind::PlayerHurt,
        tick,
        vec![
            ("userid", EventValue::PlayerId(victim)),
            ("attacker", EventValue::PlayerId(attacker)),
            ("weapon", EventValue::String(weapon.to_string())),
            ("dmg_health", EventValue::Integer(dmg_health)),
            ("hitgroup", EventValue::Integer(hitgroup)),
            ("victim_health", EventValue::Integer(victim_health)),
        ],
    )
}

pub fn smoke_detonate(tick: Tick, thrower: u64, position: (f32, f32, f32)) -> GameEvent {
    make_event(
        EventKind::SmokeGrenadeDetonate,
        tick,
        vec![
            ("userid", EventValue::PlayerId(thrower)),
            (
                "position",
                EventValue::Vector(position.0, position.1, position.2),
            ),
        ],
    )
}

pub fn smoke_expired(tick: Tick, entity_id: u64, position: (f32, f32, f32)) -> GameEvent {
    make_event(
        EventKind::SmokeGrenadeExpired,
        tick,
        vec![
            ("entityid", EventValue::Integer(entity_id as i64)),
            (
                "position",
                EventValue::Vector(position.0, position.1, position.2),
            ),
        ],
    )
}

pub fn inferno_start(tick: Tick, owner: u64, position: (f32, f32, f32)) -> GameEvent {
    make_event(
        EventKind::InfernoStart,
        tick,
        vec![
            ("userid", EventValue::PlayerId(owner)),
            (
                "position",
                EventValue::Vector(position.0, position.1, position.2),
            ),
        ],
    )
}

pub fn inferno_expire(tick: Tick, entity_id: u64) -> GameEvent {
    make_event(
        EventKind::InfernoExpire,
        tick,
        vec![("entityid", EventValue::Integer(entity_id as i64))],
    )
}

pub fn bomb_plant(tick: Tick, planter: u64, site: char) -> GameEvent {
    make_event(
        EventKind::BombPlant,
        tick,
        vec![
            ("userid", EventValue::PlayerId(planter)),
            ("site", EventValue::String(site.to_string())),
        ],
    )
}

pub fn round_start(tick: Tick, round: u32) -> GameEvent {
    make_event(
        EventKind::RoundStart,
        tick,
        vec![("round", EventValue::Integer(round as i64))],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_value_conversions() {
        let int_val = EventValue::Integer(42);
        assert_eq!(int_val.as_i64(), Some(42));
        assert_eq!(int_val.as_f64(), Some(42.0));

        let float_val = EventValue::Float(42.42);
        assert_eq!(float_val.as_f64(), Some(42.42));

        let str_val = EventValue::String("test".to_string());
        assert_eq!(str_val.as_str(), Some("test"));

        let bool_val = EventValue::Boolean(true);
        assert_eq!(bool_val.as_bool(), Some(true));
    }

    #[test]
    fn test_event_creation() {
        let event = player_death(Tick(100), 12345, 67890, "ak47");
        assert_eq!(event.kind, EventKind::PlayerDeath);
        assert_eq!(event.tick, Tick(100));
        assert_eq!(
            event.data.get("attacker").and_then(|v| v.as_player_id()),
            Some(12345)
        );
        assert_eq!(
            event.data.get("victim").and_then(|v| v.as_player_id()),
            Some(67890)
        );
    }
}
