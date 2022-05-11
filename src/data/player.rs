use chrono::{NaiveDate, NaiveTime, NaiveDateTime};
use serde::{Deserialize, Serialize};

use super::DailyItem;
use super::Tank;
use crate::schema::players;

#[derive(Serialize, Deserialize, Queryable, Insertable, Debug)]
#[table_name = "players"]
pub struct Player {
    #[serde(skip)]
    pub id: i64,

    #[serde(skip)]
    pub machine_id: String,

    pub reg_date: NaiveDateTime,

    pub last_online: NaiveDateTime,

    pub nickname: Option<String>,

    pub battles_count: i32,

    pub victories_count: i32,

    pub xp: i32,

    pub rank_level: i32,

    #[serde(skip)]
    pub coins: i32,

    #[serde(skip)]
    pub diamonds: i32,

    #[serde(skip, default = "default_naive_date_time")]
    pub daily_items_time: NaiveDateTime,

    pub friends_nicks: Vec<String>,

    pub accuracy: f32,

    pub damage_dealt: i32,

    pub damage_taken: i32,

    pub trophies: i32,

    pub tanks: Vec<Tank>,

    pub daily_items: Vec<DailyItem>,
}

fn default_naive_date_time() -> NaiveDateTime{
    NaiveDateTime::new(NaiveDate::from_ymd(1970, 1, 1), NaiveTime::from_hms(0, 0, 0))
}

impl Default for Player{
    fn default() -> Self {
        Player {
            id: 0,
            machine_id: String::new(),
            reg_date: default_naive_date_time(),
            last_online: default_naive_date_time(),
            nickname: None,
            battles_count: 0,
            victories_count: 0,
            xp: 0,
            rank_level: 0,
            coins: 0,
            diamonds: 0,
            daily_items_time: default_naive_date_time(),
            friends_nicks: Vec::new(),
            accuracy: 0.,
            damage_dealt: 0,
            damage_taken: 0,
            trophies: 0,
            tanks: [Tank::default()].to_vec(),
            daily_items: [DailyItem::default()].to_vec(),
        }
    }
}
