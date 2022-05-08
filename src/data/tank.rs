use diesel::Queryable;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Queryable)]
pub struct Tank {
    pub id: u32,

    pub level: u32,

    pub count: u32,
}
