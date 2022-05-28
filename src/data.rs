mod daily_item;
mod player;
mod tank;
mod tank_info;

use rand::Rng;
use std::collections::HashMap;

pub use daily_item::*;
pub use player::*;
pub use tank::*;
pub use tank_info::*;

use serde::{Deserialize, Serialize};

pub static CLIENTS: state::Storage<dashmap::DashMap<usize, Client>> = state::Storage::new();

pub static TANKS: state::Storage<Vec<TankInfo>> = state::Storage::new();

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

pub struct WeightedRandomList<T>
where
    T: Copy,
{
    entries: Vec<(T, f32)>,
    acc_weight: f32,
}

impl<T: Copy> WeightedRandomList<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            acc_weight: 0f32,
        }
    }

    pub fn add_entry(&mut self, element: T, weight: f32) {
        self.acc_weight += weight;
        self.entries.push((element, self.acc_weight));
    }

    pub fn get_random(&self) -> color_eyre::Result<T> {
        if self.entries.len() == 0 {
            return Err(color_eyre::eyre::eyre!("Length must be greater than 0"));
        }
        let r: f32 = rand::thread_rng().gen_range(0.0..1.0);
        for entry in &self.entries {
            if entry.1 >= r {
                return Ok(entry.0);
            }
        }
        Ok(self.entries.last().unwrap().0)
    }
}
