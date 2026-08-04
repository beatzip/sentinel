use serde::{Deserialize, Serialize};

use sentinel_core::Tick;

use crate::kinds::EventValue;

/// A parsed shot event from weapon_fire
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShotEvent {
    /// Tick when the shot was fired
    pub tick: Tick,
    /// SteamID of the shooter
    pub shooter_id: u64,
    /// Weapon used
    pub weapon: String,
    /// Number of penetrations (0 = no penetration)
    pub penetrated: i64,
    /// Whether the weapon was fired in alt-fire mode
    pub is_alt_fire: bool,
}

/// A hitgroup enum matching CS2 hitgroup values
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitGroup {
    Generic,
    Head,
    Chest,
    Stomach,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
    Neck,
    Gear,
    Unknown(i64),
}

impl From<i64> for HitGroup {
    fn from(value: i64) -> Self {
        match value {
            0 => HitGroup::Generic,
            1 => HitGroup::Head,
            2 => HitGroup::Chest,
            3 => HitGroup::Stomach,
            4 => HitGroup::LeftArm,
            5 => HitGroup::RightArm,
            6 => HitGroup::LeftLeg,
            7 => HitGroup::RightLeg,
            8 => HitGroup::Neck,
            10 => HitGroup::Gear,
            other => HitGroup::Unknown(other),
        }
    }
}

/// A parsed damage event from player_hurt
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageEvent {
    /// Tick when the damage occurred
    pub tick: Tick,
    /// SteamID of the victim
    pub victim_id: u64,
    /// SteamID of the attacker (None if self-damage or environment)
    pub attacker_id: Option<u64>,
    /// Weapon used by the attacker
    pub weapon: String,
    /// Damage dealt to health
    pub dmg_health: i64,
    /// Damage dealt to armor
    pub dmg_armor: i64,
    /// Hit group that was hit
    pub hitgroup: HitGroup,
    /// Victim's health before taking damage
    pub victim_health: i64,
    /// Victim's armor before taking damage
    pub victim_armor: i64,
    /// Real damage applied (cannot exceed remaining health)
    pub dmg_health_real: i64,
}

impl DamageEvent {
    /// Calculate real damage: min(dmg_health, remaining health)
    pub fn calculate_real_damage(&self) -> i64 {
        let remaining_health = self.victim_health.saturating_sub(self.dmg_health);
        if remaining_health < 0 {
            self.dmg_health + remaining_health // remaining_health is negative, so we subtract the overkill
        } else {
            self.dmg_health
        }
    }
}

/// Helper to create a ShotEvent from raw GameEvent data
pub fn shot_from_event(tick: Tick, data: &[(String, EventValue)]) -> ShotEvent {
    let shooter_id = data
        .iter()
        .find(|(k, _)| k == "userid")
        .and_then(|(_, v)| match v {
            EventValue::PlayerId(id) => Some(*id),
            EventValue::Integer(i) => Some(*i as u64),
            _ => None,
        })
        .unwrap_or(0);

    let weapon = data
        .iter()
        .find(|(k, _)| k == "weapon")
        .and_then(|(_, v)| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let penetrated = data
        .iter()
        .find(|(k, _)| k == "penetrated")
        .and_then(|(_, v)| v.as_i64())
        .unwrap_or(0);

    let is_alt_fire = data
        .iter()
        .find(|(k, _)| k == "is_alt_fire")
        .and_then(|(_, v)| v.as_bool())
        .unwrap_or(false);

    ShotEvent {
        tick,
        shooter_id,
        weapon,
        penetrated,
        is_alt_fire,
    }
}

/// Helper to create a DamageEvent from raw GameEvent data
pub fn damage_from_event(
    tick: Tick,
    data: &[(String, EventValue)],
) -> DamageEvent {
    let victim_id = data
        .iter()
        .find(|(k, _)| k == "userid")
        .and_then(|(_, v)| match v {
            EventValue::PlayerId(id) => Some(*id),
            EventValue::Integer(i) => Some(*i as u64),
            _ => None,
        })
        .unwrap_or(0);

    let attacker_id = data
        .iter()
        .find(|(k, _)| k == "attacker")
        .and_then(|(_, v)| match v {
            EventValue::PlayerId(id) => Some(*id),
            EventValue::Integer(i) => Some(*i as u64),
            _ => None,
        });

    let weapon = data
        .iter()
        .find(|(k, _)| k == "weapon")
        .and_then(|(_, v)| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let dmg_health = data
        .iter()
        .find(|(k, _)| k == "dmg_health")
        .and_then(|(_, v)| v.as_i64())
        .unwrap_or(0);

    let dmg_armor = data
        .iter()
        .find(|(k, _)| k == "dmg_armor")
        .and_then(|(_, v)| v.as_i64())
        .unwrap_or(0);

    let hitgroup = data
        .iter()
        .find(|(k, _)| k == "hitgroup")
        .and_then(|(_, v)| v.as_i64())
        .unwrap_or(0);

    let victim_health = data
        .iter()
        .find(|(k, _)| k == "victim_health")
        .and_then(|(_, v)| v.as_i64())
        .unwrap_or(100);

    let victim_armor = data
        .iter()
        .find(|(k, _)| k == "victim_armor")
        .and_then(|(_, v)| v.as_i64())
        .unwrap_or(0);

    let dmg_health_real = if dmg_health > victim_health {
        victim_health
    } else {
        dmg_health
    };

    DamageEvent {
        tick,
        victim_id,
        attacker_id,
        weapon,
        dmg_health,
        dmg_armor,
        hitgroup: HitGroup::from(hitgroup),
        victim_health,
        victim_armor,
        dmg_health_real,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{make_event, EventKind};

    #[test]
    fn test_hitgroup_conversion() {
        assert_eq!(HitGroup::from(0), HitGroup::Generic);
        assert_eq!(HitGroup::from(1), HitGroup::Head);
        assert_eq!(HitGroup::from(2), HitGroup::Chest);
        assert_eq!(HitGroup::from(3), HitGroup::Stomach);
        assert_eq!(HitGroup::from(4), HitGroup::LeftArm);
        assert_eq!(HitGroup::from(5), HitGroup::RightArm);
        assert_eq!(HitGroup::from(6), HitGroup::LeftLeg);
        assert_eq!(HitGroup::from(7), HitGroup::RightLeg);
        assert_eq!(HitGroup::from(8), HitGroup::Neck);
        assert_eq!(HitGroup::from(10), HitGroup::Gear);
        assert_eq!(HitGroup::from(99), HitGroup::Unknown(99));
    }

    #[test]
    fn test_shot_event_creation() {
        let event = make_event(
            EventKind::WeaponFire,
            Tick(100),
            vec![
                ("userid", EventValue::PlayerId(12345)),
                ("weapon", EventValue::String("ak47".to_string())),
                ("penetrated", EventValue::Integer(2)),
                ("is_alt_fire", EventValue::Boolean(false)),
            ],
        );

        let shot = shot_from_event(event.tick, &event.data.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>());
        assert_eq!(shot.shooter_id, 12345);
        assert_eq!(shot.weapon, "ak47");
        assert_eq!(shot.penetrated, 2);
        assert_eq!(shot.is_alt_fire, false);
    }

    #[test]
    fn test_damage_event_creation() {
        let event = make_event(
            EventKind::PlayerHurt,
            Tick(200),
            vec![
                ("userid", EventValue::PlayerId(67890)),
                ("attacker", EventValue::PlayerId(11111)),
                ("weapon", EventValue::String("m4a1".to_string())),
                ("dmg_health", EventValue::Integer(25)),
                ("hitgroup", EventValue::Integer(1)), // head
                ("victim_health", EventValue::Integer(75)),
                ("victim_armor", EventValue::Integer(50)),
            ],
        );

        let damage = damage_from_event(event.tick, &event.data.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>());
        assert_eq!(damage.victim_id, 67890);
        assert_eq!(damage.attacker_id, Some(11111));
        assert_eq!(damage.dmg_health, 25);
        assert_eq!(damage.hitgroup, HitGroup::Head);
        assert_eq!(damage.victim_health, 75);
        assert_eq!(damage.dmg_health_real, 25);
    }

    #[test]
    fn test_damage_event_real_damage_cap() {
        // Player has 30 health, takes 50 damage -> real damage is 30
        let event = make_event(
            EventKind::PlayerHurt,
            Tick(300),
            vec![
                ("userid", EventValue::PlayerId(999)),
                ("attacker", EventValue::PlayerId(888)),
                ("weapon", EventValue::String("awp".to_string())),
                ("dmg_health", EventValue::Integer(50)),
                ("hitgroup", EventValue::Integer(0)),
                ("victim_health", EventValue::Integer(30)),
                ("victim_armor", EventValue::Integer(0)),
            ],
        );

        let damage = damage_from_event(event.tick, &event.data.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>());
        assert_eq!(damage.dmg_health_real, 30);
    }
}
