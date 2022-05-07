use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

use super::Tank;
use super::DailyItem;

#[derive(Serialize, Deserialize, Queryable)]
pub struct Player{
    #[serde(skip)]
    pub id: i64,

    #[serde(skip)]
    pub machine_id: String,

    pub reg_date: DateTime<Utc>,

    pub last_online: DateTime<Utc>,

    pub nickname: String,

    pub battles_count: u32,

    pub victories_count: u32,

    pub xp: u32,

    pub rank_level: u32,

    #[serde(skip)]
    pub coins: u32,

    #[serde(skip)]
    pub diamonds: u32,

    #[serde(skip, default = "default_daily_items_time")]
    pub daily_items_time: DateTime<Utc>,

    pub friends_ids: String,

    pub accuracy: f32,

    pub damage_dealt: u32,

    pub damage_taken: u32,

    pub trophies: u32,

    pub tanks: Tank,

    pub daily_items: DailyItem,
}

fn default_daily_items_time() -> DateTime<Utc>{
    Utc::now()
}