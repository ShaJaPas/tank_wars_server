#[macro_use]
extern crate diesel;

mod data;
mod db;
mod network;
mod schema;

use std::str::FromStr;

use argh::FromArgs;
use color_eyre::eyre::Result;
use diesel::{PgConnection, Connection};

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
    #[argh(
        option,
        default = "String::from_str(\"postgres://konstantin:123@localhost:5432/test\").unwrap()"
    )]
    db_url: String,
}

fn main() -> Result<()> {

    let args: Cli = argh::from_env();
    db::POOL.get_or(||{
        PgConnection::establish(&args.db_url).unwrap()
    });
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .on_thread_start(move || {
            db::POOL.get_or(||{
                PgConnection::establish(&args.db_url).unwrap()
            });
        })
        .build()
        .expect("Failed building the Runtime")
        .block_on(async {
            color_eyre::install()?;

            tracing::subscriber::set_global_default(
                tracing_subscriber::FmtSubscriber::builder()
                    .with_max_level(tracing::Level::INFO)
                    .finish(),
            )?;
            //let db = db::get_connection().await?.get_database_backend();
            let mut server = Server::new(args.port, args.keylog);
            server.start().await?;

            Ok(())
        })
}
