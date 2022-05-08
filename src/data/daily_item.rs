use diesel::Queryable;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Queryable)]
pub struct DailyItem {
    pub price: u32,
    pub tank_id: u32,
    pub count: u32,
    pub bought: bool,
}