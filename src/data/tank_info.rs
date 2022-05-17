use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TankInfo {
    pub id: u32,
    pub graphics_info: TankGraphicsInfo,
    pub characteristics: TankCharacteristics,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TankGraphicsInfo {
    pub bullet_name: String,
    pub tank_body_name: String,
    pub tank_gun_name: String,
    pub gun_y: u32,
    pub gun_x: u32,
    pub gun_origin_x: u32,
    pub gun_origin_y: u32,
    pub bullet_x: u32,
    pub bullet_y: u32,
    pub tank_width: u32,
    pub tank_height: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TankCharacteristics {
    pub name: String,
    pub rarity: TankRarity,
    pub hp: f32,
    pub gun_rotate_degrees: f32,
    pub body_rotate_degrees: f32,
    pub velocity: f32,
    pub reloading: f32,
    pub bullet_speed: f32,
    pub damage: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum TankRarity {
    COMMON,
    RARE,
    EPIC,
    MYTHICAL,
    LEGENDARY,
}

impl TankRarity {
    fn value(&self) -> f32 {
        match *self {
            Self::COMMON => 60.0,
            Self::RARE => 15.0,
            Self::EPIC => 2.0,
            Self::MYTHICAL => 0.15,
            Self::LEGENDARY => 0.015,
        }
    }
}
