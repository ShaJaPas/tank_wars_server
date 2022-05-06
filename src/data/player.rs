
pub use sea_orm;

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Serialize, Deserialize};

use super::Tank;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "player")]
pub struct Model{
    #[sea_orm(primary_key, indexed, auto_increment = false)]
    #[serde(skip)]
    pub id: String,

    #[sea_orm(column_type = "DateTime")]
    pub reg_date: DateTime<Utc>,

    #[sea_orm(column_type = "DateTime")]
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

    #[sea_orm(column_type = "DateTime")]
    #[serde(skip, default = "default_daily_items_time")]
    pub daily_items_time: DateTime<Utc>,

    pub friends_ids: String,

    pub accuracy: f32,

    pub damage_dealt: u32,

    pub damage_taken: u32,

    pub trophies: u32,

    //pub tanks: Tank
    //tanks, daily_items,
}

fn default_daily_items_time() -> DateTime<Utc>{
    Utc::now()
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}