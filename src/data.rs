mod player;
mod tank;
mod daily_item;

pub use tank::Tank;
pub use daily_item::DailyItem;

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