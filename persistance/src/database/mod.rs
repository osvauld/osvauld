use crate::repositories::{
    SqliteDeviceRepository, SqliteFolderRepository, SqliteFolderShareRecordRepository,
    SqliteResourceKeyRepository, SqliteResourceRepository, SqliteShareRepository,
    SqliteStoreRepository, SqliteUserRepository, SqliteVectorClockRepository,
};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use log::info;
use osvauld_core::repositories::{
    DeviceRepository, FolderRepository, FolderShareRecordRepository, ResourceKeyRepository,
    ResourceRepository, ShareRepository, StoreRepository, UserRepository, VectorClockRepository,
};
use std::sync::Arc;
pub mod schema;

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
pub type DbConnection = Arc<DbPool>;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
#[derive(Clone)]
pub struct RepositoryContext {
    pub folder_repo: Arc<dyn FolderRepository>,
    pub resource_repo: Arc<dyn ResourceRepository>,
    pub device_repo: Arc<dyn DeviceRepository>,
    pub share_repo: Arc<dyn ShareRepository>,
    pub resource_key_repo: Arc<dyn ResourceKeyRepository>,
    pub store_repo: Arc<dyn StoreRepository>,
    pub user_repo: Arc<dyn UserRepository>,
    pub vector_clock_repo: Arc<dyn VectorClockRepository>,
    pub folder_share_repo: Arc<dyn FolderShareRecordRepository>,
}
pub async fn connect_database(db_path: &str) -> Result<DbConnection, Box<dyn std::error::Error + Send + Sync>> {
    let manager = ConnectionManager::<SqliteConnection>::new(db_path);
    
    // Create pool with configuration
    // Set max_size to at least your MAX_CONCURRENT_TASKS + some buffer
    let pool = Pool::builder()
        .max_size(20) // Allow 20 concurrent connections
        .connection_timeout(std::time::Duration::from_secs(30))
        .build(manager)?;
    
    Ok(Arc::new(pool))
}
pub fn initialize_repositories(connection: DbConnection) -> RepositoryContext {
    let folder_repo = Arc::new(SqliteFolderRepository::new(connection.clone()));
    let resource_repo = Arc::new(SqliteResourceRepository::new(connection.clone()));
    let device_repo = Arc::new(SqliteDeviceRepository::new(connection.clone()));
    let share_repo = Arc::new(SqliteShareRepository::new(connection.clone()));
    let resource_key_repo = Arc::new(SqliteResourceKeyRepository::new(connection.clone()));
    let store_repo = Arc::new(SqliteStoreRepository::new(connection.clone()));
    let user_repo = Arc::new(SqliteUserRepository::new(connection.clone()));
    let vector_clock_repo = Arc::new(SqliteVectorClockRepository::new(connection.clone()));
    let folder_share_repo = Arc::new(SqliteFolderShareRecordRepository::new(connection.clone()));

    RepositoryContext {
        folder_repo,
        resource_repo,
        device_repo,
        share_repo,
        resource_key_repo,
        store_repo,
        user_repo,
        vector_clock_repo,
        folder_share_repo,
    }
}

pub async fn run_migrations(
    pool: &DbConnection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut conn = pool.get()?;
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
