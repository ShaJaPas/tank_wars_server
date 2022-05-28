use chrono::Utc;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use rand::Rng;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use super::Tank;
use super::{DailyItem, TankRarity, WeightedRandomList};
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

fn default_naive_date_time() -> NaiveDateTime {
    NaiveDateTime::new(
        NaiveDate::from_ymd(1970, 1, 1),
        NaiveTime::from_hms(0, 0, 0),
    )
}

impl Player {
    pub fn new(id: i64, machine_id: String) -> Self {
        let mut res = Self {
            id,
            machine_id,
            reg_date: Utc::now().naive_utc(),
            last_online: Utc::now().naive_utc(),
            nickname: None,
            battles_count: 0,
            victories_count: 0,
            xp: 0,
            rank_level: 1,
            coins: 0,
            diamonds: 0,
            daily_items_time: Utc::now().naive_utc(),
            friends_nicks: Vec::new(),
            accuracy: 0.,
            damage_dealt: 0,
            damage_taken: 0,
            trophies: 0,
            tanks: Vec::new(),
            daily_items: Vec::new(),
        };
        res.daily_items = res.get_daily_items();
        res
    }

    pub fn get_efficiency(&self) -> f32 {
        let res = (self.victories_count as f32) / (self.battles_count as f32)
            * (self.accuracy + 0.5)
            * (self.damage_dealt as f32)
            / (self.damage_taken as f32);
        if res.is_normal() {
            res
        } else {
            0f32
        }
    }

    pub fn get_daily_items(&self) -> Vec<DailyItem> {
        let mut gen = rand::thread_rng();
        let mut result = Vec::with_capacity(4);
        unsafe {
            result.set_len(4);
        }
        let tanks = super::TANKS.get();
        for (i, rarity) in TankRarity::iter().take(4).enumerate() {
            let mut list = WeightedRandomList::new();
            for x in tanks.iter().filter(|x| x.characteristics.rarity == rarity) {
                list.add_entry(x, rarity.value());
            }
            let id = list.get_random().unwrap().id as i32;
            if self.tanks.iter().any(|x| x.id == id) {
                result[i] = DailyItem {
                    price: gen.gen_range(40..=50),
                    tank_id: id,
                    count: gen.gen_range(40..=50),
                    bought: false,
                };
            } else {
                let price = 60 * (TankRarity::COMMON.value() / rarity.value()).sqrt() as i32;
                result[i] = DailyItem {
                    price: gen.gen_range(price..=(price as f32 * 1.1) as i32),
                    tank_id: id,
                    count: 0,
                    bought: false,
                };
            }
        }

        result
    }
}
