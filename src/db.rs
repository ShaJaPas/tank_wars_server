use diesel::PgConnection;

use crate::{data::Player, schema::players::dsl::*};
use diesel::prelude::*;
use tokio::sync::Mutex;

pub static POOL: state::LocalStorage<PgConnection> = state::LocalStorage::new();

pub static ID_GEN: state::Storage<Mutex<snowflake::SnowflakeIdGenerator>> = state::Storage::new();

pub fn os_id_matches(client_id: i64, os_id: String) -> color_eyre::Result<bool> {
    let conn = POOL.try_get().unwrap();
    let res = players
        .find(client_id)
        .select(machine_id)
        .load::<String>(conn)?;

    Ok(res.len() == 1 && os_id == res[0])
}

pub fn get_player_by_nickname(nick: &str) -> Option<Player> {
    let conn = POOL.try_get().unwrap();
    let res = players.filter(nickname.eq(nick)).first(conn);
    res.ok()
}

pub fn get_player_by_id(client_id: i64) -> Option<Player> {
    let conn = POOL.try_get().unwrap();
    let res = players.find(client_id).first(conn);
    res.ok()
}

pub fn save(player: &Player) -> color_eyre::Result<()> {
    let conn = POOL.try_get().unwrap();

    assert_eq!(
        diesel::insert_into(players)
            .values(player)
            .execute(conn)
            .expect("Error saving player"),
        1
    );
    Ok(())
}

pub fn update_player(player: &Player) -> color_eyre::Result<()> {
    let conn = POOL.try_get().unwrap();

    player.save_changes::<Player>(conn)?;

    Ok(())
}
