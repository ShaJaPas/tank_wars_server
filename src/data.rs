mod chest;
mod daily_item;
mod map;
mod player;
mod tank;
mod tank_info;

use quinn::Connection;
use rand::Rng;
use std::collections::HashMap;
use strum::Display;
use tokio::sync::mpsc::UnboundedSender;

pub use chest::*;
pub use daily_item::*;
pub use map::*;
pub use player::*;
pub use tank::*;
pub use tank_info::*;

use serde::{Deserialize, Serialize};

use crate::physics::PhysicsCommand;

pub static CLIENTS: state::Storage<dashmap::DashMap<usize, Client>> = state::Storage::new();

pub static TANKS: state::Storage<Vec<TankInfo>> = state::Storage::new();

pub static NICKNAME_REGEX: state::Storage<regex::Regex> = state::Storage::new();

pub static MATCHMAKER: state::LocalStorage<UnboundedSender<BalancerCommand>> =
    state::LocalStorage::new();

pub static PHYSICS: state::LocalStorage<UnboundedSender<PhysicsCommand>> =
    state::LocalStorage::new();

#[derive(Debug, Serialize, Deserialize, Display)]
pub enum Packet {
    SignInRequest {
        os_id: String,
        client_id: Option<i64>,
    },
    SignInResponse {
        client_id: Option<i64>,
        profile: Option<Player>,
    },

    FilesSyncRequest {
        file_names: HashMap<String, Vec<u8>>,
    },

    FilesSyncResponse {
        file_names: Vec<(String, Vec<u8>)>,
    },

    PlayerProfileRequest {
        nickname: String,
    },

    PlayerProfileResponse {
        profile: Option<Player>,
        nickname: String,
    },

    SetNicknameRequest {
        nickname: String,
    },

    SetNicknameResponse {
        error: Option<String>,
    },

    GetChestRequest {
        name: ChestName,
    },

    GetChestResponse {
        chest: Chest,
    },

    UpgradeTankRequest {
        id: i32,
    },

    UpgradeTankResponse {
        id: Option<i32>,
    },

    GetDailyItemsRequest,

    GetDailyItemsResponse {
        items: Vec<DailyItem>,
        updated: bool,
    },

    JoinMatchMakerRequest {
        id: i32,
    },

    MapFoundResponse {
        wait_time: f32,
        map: Map,
        opponent_nick: String,
        opponent_tank: Tank,
        initial_packet: GamePacket,
    },

    //Without responses
    LeaveMatchMakerRequest,
    Shoot,
    Explosion {
        x: f32,
        y: f32,
        hit: bool,
    },
}

//This packet server sends to client
#[derive(Debug, Serialize, Deserialize)]
pub struct GamePacket {
    pub time_left: u16,
    pub my_data: GamePlayerData,
    pub opponent_data: GamePlayerData,
}

//This packet client sends to server
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerPosition {
    pub body_rotation: f32,
    pub gun_rotation: f32,
    pub moving: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GamePlayerData {
    pub x: f32,
    pub y: f32,
    pub body_rotation: f32,
    pub gun_rotation: f32,
    pub hp: u16,
    pub cool_down: f32,
    pub bullets: Vec<BulletData>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BulletData {
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
}

#[derive(Debug)]
pub enum BalancerCommand {
    AddPlayer {
        player: Box<Player>,
        tank_id: i32,
        conn: Connection,
    },
    RemovePlayer(i64),
}

#[derive(Debug)]
pub struct Client {
    pub id: i64,
}
pub struct WeightedRandomList<T>
where
    T: Copy + PartialEq,
{
    entries: Vec<(T, f32)>,
    acc_weight: f32,
}

impl<T: Copy + PartialEq> WeightedRandomList<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            acc_weight: 0f32,
        }
    }

    pub fn add_entry(&mut self, element: T, weight: f32) {
        self.acc_weight += weight;
        self.entries.push((element, weight));
    }

    pub fn remove_enty(&mut self, element: T) -> Result<f32, String> {
        let chance = self
            .entries
            .iter()
            .filter(|f| f.0 == element)
            .last()
            .ok_or("Zero elements!")?
            .1;
        self.acc_weight -= chance;
        self.entries.retain(|f| f.0 != element);
        Ok(chance)
    }

    pub fn get_random(&self) -> color_eyre::Result<T> {
        if self.entries.is_empty() {
            return Err(color_eyre::eyre::eyre!("Length must be greater than 0"));
        }
        let mut sum = 0f32;
        let r: f32 = rand::thread_rng().gen_range(0.0..self.acc_weight);
        for entry in &self.entries {
            sum += entry.1;
            if sum >= r {
                return Ok(entry.0);
            }
        }
        Ok(self.entries.last().unwrap().0)
    }
}
