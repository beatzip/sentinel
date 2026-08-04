use serde::{Deserialize, Serialize};

/// Unique player identifier (SteamID)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerId(pub u64);

impl PlayerId {
    pub fn new(steam_id: u64) -> Self {
        Self(steam_id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "STEAM_0:1:{}", self.0)
    }
}

/// Player team
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Team {
    Terrorist,
    CounterTerrorist,
    Unassigned,
}

/// Complete player state at a tick
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: PlayerId,
    pub name: String,
    pub team: Team,
    pub position: Vec3,
    pub velocity: Vec3,
    pub view_angles: Angles,
    pub weapon: Weapon,
    pub health: i32,
    pub armor: i32,
    pub money: i32,
    pub flash_duration: f32,
    pub scoped: bool,
    pub reloading: bool,
    pub alive: bool,
}

/// 3D position vector
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn distance_to(&self, other: &Vec3) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(&self) -> Self {
        let len = self.length();
        if len > 0.0 {
            Self {
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
            }
        } else {
            *self
        }
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Vec3;

    fn sub(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl From<Vec3> for sentinel_map::Vec3 {
    fn from(v: Vec3) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

impl From<sentinel_map::Vec3> for Vec3 {
    fn from(v: sentinel_map::Vec3) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

/// View angles (pitch, yaw, roll)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Angles {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

/// Weapon type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weapon {
    None,
    Knife,
    Pistol,
    SMG,
    Rifle,
    Sniper,
    Shotgun,
    MG,
    Grenade,
    C4,
    DefuseKit,
}

impl Weapon {
    pub fn is_gun(&self) -> bool {
        matches!(
            self,
            Weapon::Pistol
                | Weapon::SMG
                | Weapon::Rifle
                | Weapon::Sniper
                | Weapon::Shotgun
                | Weapon::MG
        )
    }

    pub fn is_sniper(&self) -> bool {
        matches!(self, Weapon::Sniper)
    }
}
