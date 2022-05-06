mod player;
mod tank;

pub use player::Entity as Player;
pub use tank::Tank;

use serde::{Serialize, Deserialize};

lazy_static::lazy_static! {
    pub static ref CLIENTS : dashmap::DashMap<usize, i32> = dashmap::DashMap::new();
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Packet {
    SignInRequest {
        os_id: String,
        client_id: Option<i64>
    },
    SignInResponse {
        client_id: i64
    },
}