use std::{time::Duration, fmt::Debug};

use diesel::{PgConnection, r2d2::{PooledConnection, ConnectionManager, Pool}};
use tracing::info;

pub fn get_pooled_connection(database_url: &str) -> color_eyre::Result<PooledConnection<ConnectionManager<PgConnection>>> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = Pool::builder().build(manager)?;
    let conn = pool.clone().get_timeout(Duration::from_secs(8))?;
    info!("created connection pool for PostgreSQL {:?}", Pool::<ConnectionManager<PgConnection>>::builder());
    Ok(conn)
}