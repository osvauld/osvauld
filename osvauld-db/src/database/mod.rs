use diesel::Connection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use log::info;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod schema;

pub type DbConnection = Arc<Mutex<SqliteConnection>>;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub async fn connect_database(db_path: &str) -> Result<DbConnection, diesel::result::Error> {
    let path = Path::new(db_path);
    let conn = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
    Ok(Arc::new(Mutex::new(conn)))
}

pub async fn run_migrations(
    conn: &DbConnection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut conn = conn.lock().await;
    conn.run_pending_migrations(MIGRATIONS)?;
    info!("Migrations completed successfully");
    Ok(())
}

pub async fn initialize_database(
    db_path: &str,
) -> Result<DbConnection, Box<dyn std::error::Error + Send + Sync>> {
    let conn = connect_database(db_path).await?;
    run_migrations(&conn).await?;
    Ok(conn)
}
