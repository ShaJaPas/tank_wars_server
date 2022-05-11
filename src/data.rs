mod daily_item;
mod player;
mod tank;

pub use daily_item::*;
pub use tank::*;
pub use player::*;

use serde::{Deserialize, Serialize};

lazy_static::lazy_static! {
    pub static ref CLIENTS : dashmap::DashMap<usize, i32> = dashmap::DashMap::new();
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Packet {
    SignInRequest {
        os_id: String,
        client_id: Option<i64>,
    },
    SignInResponse {
        client_id: Option<i64>,
    },
}

#[allow(dead_code)]
pub const LOGIN_STREAM_ID : u64 = 0;