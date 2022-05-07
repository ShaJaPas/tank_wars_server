#[macro_use]
extern crate diesel;

mod network;
mod data;
//mod db;

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
        
    });
    println!("{}", migr.make::<Pg>());

    let mut server = Server::new(args.port, args.keylog);
    server.start().await?;

    Ok(())
}
