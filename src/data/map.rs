use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MapObject {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub rotation: f32,
}
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Map {
    pub name: String,
    pub width: i32,
    pub height: i32,
    #[serde(rename = "player1Y")]
    pub player1_y: i32,
    #[serde(rename = "player2Y")]
    pub player2_y: i32,
    pub objects: Vec<MapObject>,
}
