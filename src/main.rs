#[macro_use]
extern crate diesel;

mod data;
mod db;
mod network;
mod physics;
mod schema;

use std::str::FromStr;

use argh::FromArgs;
use color_eyre::eyre::Result;
use diesel::{Connection, PgConnection};
use tracing_subscriber::fmt::writer::MakeWriterExt;

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

    db::POOL.set(move || PgConnection::establish(&args.db_url).unwrap());
    db::POOL.get();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .on_thread_start(move || {
            db::POOL.get();
        })
        .build()
        .expect("Failed building the Runtime")
        .block_on(async {
            color_eyre::install()?;

            let result: Result<Vec<data::TankInfo>> = (async {
                let mut res = Vec::new();
                if let Ok(mut dir) = tokio::fs::read_dir("Tanks").await {
                    while let Ok(Some(entry)) = dir.next_entry().await {
                        if entry.path().is_file()
                            && entry.path().extension().map_or(false, |v| v == "json")
                        {
                            let content = tokio::fs::read(entry.path()).await?;
                            let value: data::TankInfo = serde_json::from_slice(&content)?;
                            res.push(value);
                        }
                    }
                }
                Ok(res)
            })
            .await;

            data::TANKS.set(result?);

            let log = std::fs::File::create("debug.log")?;
            tracing::subscriber::set_global_default(
                tracing_subscriber::FmtSubscriber::builder()
                    .with_max_level(tracing::Level::DEBUG)
                    .with_writer(log)
                    .map_writer(move |f| {
                        f.with_min_level(tracing::Level::DEBUG)
                            .or_else(std::io::stdout)
                    })
                    .finish(),
            )?;

            let mut server = Server::new(args.port, args.keylog);
            server.start().await?;

            Ok(())
        })
}
