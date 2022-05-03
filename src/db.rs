use std::time::Duration;

use sea_orm::{DatabaseConnection, Database, ConnectOptions};

pub async fn get_connection() -> color_eyre::Result<DatabaseConnection>{
    let mut opt = ConnectOptions::new("postgres://konstantin:123@localhost/test".to_owned());

    opt.connect_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true);

    let db: DatabaseConnection = Database::connect(opt).await?;
    Ok(db)
}