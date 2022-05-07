#[macro_use]
extern crate diesel;

mod network;
mod data;
mod db;

use std::str::FromStr;

use argh::FromArgs;
use barrel::{Migration, backend::Pg};
use color_eyre::eyre::Result;

use crate::network::Server;

/// TankWars
#[derive(Debug, FromArgs)]
struct Cli {
    /// port to run
    #[argh(option, default = "51875")]
    port: u16,

    /// file to log TLS keys to for debugging
    #[argh(option, default = "false")]
    keylog: bool,

    /// DB url to connect to
    #[argh(option, default = "String::from_str(\"postgres://konstantin:123@localhost:1234/test\").unwrap()")]
    db_url: String
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Cli = argh::from_env();
    color_eyre::install()?;

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::INFO)
            .finish(),
    )?;
    //let db = db::get_connection().await?.get_database_backend();

    let mut migr = Migration::new();
    migr.create_table("players", |t| {
        t.add_column("id", barrel::types::custom("BIGINT").primary(true));
        t.add_column("machine_id", barrel::types::varchar(20).nullable(false));
        t.add_column("reg_date", barrel::types::datetime().nullable(false));
        t.add_column("last_online", barrel::types::datetime().nullable(false));
        t.add_column("nickname", barrel::types::varchar(20).nullable(true));
        t.add_column("battles_count", barrel::types::integer().nullable(false));
        t.add_column("victories_count", barrel::types::integer().nullable(false));
        t.add_column("xp", barrel::types::integer().nullable(false));
        t.add_column("coins", barrel::types::integer().nullable(false));
        t.add_column("diamonds", barrel::types::integer().nullable(false));
        t.add_column("daily_items_time", barrel::types::datetime().nullable(false));
        t.add_column("friends_nicks", barrel::types::array(&barrel::types::varchar(20)).nullable(false));
        t.add_column("accuracy", barrel::types::float().nullable(false));
        t.add_column("damage_dealt", barrel::types::integer().nullable(false));
        t.add_column("damage_taken", barrel::types::integer().nullable(false));
        t.add_column("trophies", barrel::types::integer().nullable(false));

    });
    println!("{}", migr.make::<Pg>());
    use diesel::prelude::*;
    println!("{}", diesel::PgConnection::establish(&args.db_url).err().unwrap());

    let mut server = Server::new(args.port, args.keylog);
    server.start().await?;

    Ok(())
}
