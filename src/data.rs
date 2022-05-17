mod daily_item;
mod player;
mod tank;
mod tank_info;

use std::collections::HashMap;

pub use daily_item::*;
pub use player::*;
pub use tank::*;
pub use tank_info::*;

use serde::{Deserialize, Serialize};

lazy_static::lazy_static! {
    pub static ref CLIENTS : dashmap::DashMap<usize, Client> = dashmap::DashMap::new();
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

    FilesSyncRequest {
        file_names: HashMap<String, Vec<u8>>,
    },

    FilesSyncResponse {
        file_names: Vec<(String, Vec<u8>)>,
    },
}

#[derive(Default, Debug)]
pub struct Client {
    pub id: i64,
}

pub const LOGIN_STREAM_ID: u64 = 0;
pub const DATA_SYNC_STREAM_ID: u64 = 1;
