use diesel::PgConnection;

use crate::{data::Player, schema::players::dsl::*};
use diesel::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;

lazy_static::lazy_static! {
    pub static ref POOL: thread_local::ThreadLocal<PgConnection> = {
        let tl = thread_local::ThreadLocal::with_capacity(num_cpus::get());
        tl
    };
}
lazy_static::lazy_static! {
    pub static ref ID_GEN: Arc<Mutex<snowflake::SnowflakeIdGenerator>> = {
        let gen = snowflake::SnowflakeIdGenerator::new(1, 1);
        Arc::new(Mutex::new(gen))
    };
}

pub fn os_id_matches(client_id: i64, os_id: String) -> color_eyre::Result<bool> {
    let conn = POOL.get().unwrap();
    let res = players
        .find(client_id)
        .select(machine_id)
        .load::<String>(conn)?;
    Ok(res.len() == 1 && os_id == res[0])
}

pub fn save(player: Player) -> color_eyre::Result<()> {
    let conn = POOL.get().unwrap();

    assert_eq!(
        diesel::insert_into(players)
            .values(&player)
            .execute(conn)
            .expect("Error saving player"),
        1
    );
    Ok(())
}
