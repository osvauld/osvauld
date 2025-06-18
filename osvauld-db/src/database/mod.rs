use crate::repositories::{
    SqliteDeviceRepository, SqliteFolderRepository, SqliteResourceKeyRepository,
    SqliteResourceRepository, SqliteShareRepository, SqliteStoreRepository, SqliteSyncRepository,
    SqliteUserRepository, SqliteVectorClockRepository,
};
use diesel::Connection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use log::info;
use osvauld_core::repositories::{
    DeviceRepository, FolderRepository, ResourceKeyRepository, ResourceRepository, ShareRepository,
    StoreRepository, SyncRepository, UserRepository, VectorClockRepository,
};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
pub mod schema;

pub type DbConnection = Arc<Mutex<SqliteConnection>>;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
pub struct RepositoryContext {
    pub folder_repo: Arc<dyn FolderRepository>,
    pub sync_repo: Arc<dyn SyncRepository>,
    pub resource_repo: Arc<dyn ResourceRepository>,
    pub device_repo: Arc<dyn DeviceRepository>,
    pub share_repo: Arc<dyn ShareRepository>,
    pub resource_key_repo: Arc<dyn ResourceKeyRepository>,
    pub store_repository: Arc<dyn StoreRepository>,
    pub user_repository: Arc<dyn UserRepository>,
    pub vector_clock_repo: Arc<dyn VectorClockRepository>,
}
pub async fn connect_database(db_path: &str) -> Result<DbConnection, diesel::result::Error> {
    let path = Path::new(db_path);
    let conn = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
    Ok(Arc::new(Mutex::new(conn)))
}
pub fn initialize_repositories(connection: DbConnection) -> RepositoryContext {
    let folder_repo = Arc::new(SqliteFolderRepository::new(connection.clone()));
    let sync_repo = Arc::new(SqliteSyncRepository::new(connection.clone()));
    let resource_repo = Arc::new(SqliteResourceRepository::new(connection.clone()));
    let device_repo = Arc::new(SqliteDeviceRepository::new(connection.clone()));
    let share_repo = Arc::new(SqliteShareRepository::new(connection.clone()));
    let resource_key_repo = Arc::new(SqliteResourceKeyRepository::new(connection.clone()));
    let store_repository = Arc::new(SqliteStoreRepository::new(connection.clone()));
    let user_repository = Arc::new(SqliteUserRepository::new(connection.clone()));
    let vector_clock_repo = Arc::new(SqliteVectorClockRepository::new(connection.clone()));

    RepositoryContext {
        folder_repo,
        sync_repo,
        resource_repo,
        device_repo,
        share_repo,
        resource_key_repo,
        store_repository,
        user_repository,
        vector_clock_repo,
    }
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
