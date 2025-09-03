use crate::DbConnection;
use crate::database::schema::{folder_share_records, folders, resources};
use crate::models::{FolderModel, FolderShareRecordModel};
use async_trait::async_trait;
use chrono::Local;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use osvauld_core::models::folder::Folder;
use osvauld_core::models::{FolderManifestData, FolderShareRecord};
use osvauld_core::repositories::{FolderRepository, RepositoryError};
pub struct SqliteFolderRepository {
    connection: DbConnection,
}

impl SqliteFolderRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl FolderRepository for SqliteFolderRepository {
    async fn save_folder_with_share_record(
        &self,
        folder: &Folder,
        folder_share_record: &FolderShareRecord,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let folder_model = FolderModel::from(folder);
        let folder_share_model = FolderShareRecordModel::from(folder_share_record);

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // Save the folder first
            diesel::insert_into(folders::table)
                .values(&folder_model)
                .execute(conn)?;

            // Save the folder share record
            diesel::insert_into(folder_share_records::table)
                .values(&folder_share_model)
                .execute(conn)?;

            Ok(())
        })
        .map_err(|e| {
            RepositoryError::DatabaseError(format!(
                "Failed to save folder '{}' with share record in transaction: {}",
                folder.name, e
            ))
        })?;

        Ok(())
    }

    async fn find_all(&self) -> Result<Vec<Folder>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let folder_models = folders::table
            .filter(folders::deleted.eq(false))
            .load::<FolderModel>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!("Failed to retrieve all folders: {}", e))
            })?;
        Ok(folder_models.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(&self, id: &str) -> Result<Folder, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let folder_model = folders::table
            .find(id)
            .first::<FolderModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(format!(
                    "Failed to find folder with id '{}': {}",
                    id, e
                )),
            })?;

        Ok(folder_model.into())
    }

    async fn soft_delete(&self, folder_id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let now = Local::now().timestamp_millis();

        // Start a transaction
        conn.transaction(|conn| -> Result<(), DieselError> {
            // First soft delete all resources in the folder
            diesel::update(resources::table)
                .filter(resources::folder_id.eq(folder_id))
                .set((
                    resources::deleted.eq(true),
                    resources::deleted_at.eq(Some(now)),
                    resources::updated_at.eq(now),
                ))
                .execute(conn)?;

            // Then soft delete the folder
            diesel::update(folders::table)
                .filter(folders::id.eq(folder_id))
                .set((
                    folders::deleted.eq(true),
                    folders::deleted_at.eq(Some(now)),
                    folders::updated_at.eq(now),
                ))
                .execute(conn)?;

            Ok(())
        })
        .map_err(|e| {
            RepositoryError::DatabaseError(format!(
                "Failed to soft delete resources in folder '{}': {}",
                folder_id, e
            ))
        })?;
        Ok(())
    }
    async fn get_default_folder(&self) -> Result<Folder, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let folder_model = folders::table
            .filter(folders::deleted.eq(false))
            .filter(folders::default_folder.eq(true))
            .order_by(folders::created_at.asc())
            .first::<FolderModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(format!("Failed to get default folder: {}", e)),
            })?;

        Ok(folder_model.into())
    }
    async fn get_folders_by_ids(
        &self,
        folder_ids: &[String],
    ) -> Result<Vec<Folder>, RepositoryError> {
        if folder_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self.connection.lock().await;

        let folder_models = folders::table
            .filter(folders::id.eq_any(folder_ids))
            .filter(folders::deleted.eq(false))
            .load::<FolderModel>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get {} folders by IDs: {}",
                    folder_ids.len(),
                    e
                ))
            })?;
        Ok(folder_models.into_iter().map(Into::into).collect())
    }
    async fn add_folders_bulk(&self, folders: &[Folder]) -> Result<(), RepositoryError> {
        if folders.is_empty() {
            return Ok(());
        }

        let folder_models: Vec<FolderModel> = folders.iter().map(FolderModel::from).collect();
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for folder_model in &folder_models {
                diesel::insert_into(folders::table)
                    .values(folder_model)
                    .on_conflict(folders::id) // Assuming id is the primary key
                    .do_nothing() // Skip if the folder already exists
                    .execute(conn)?;
            }
            Ok(())
        })
        .map_err(|e| {
            RepositoryError::DatabaseError(format!(
                "Failed to bulk add {} folders: {}",
                folders.len(),
                e
            ))
        })
    }
    async fn get_folder_manifest_for_user(
        &self,
        peer_user_id: &str,
    ) -> Result<Vec<FolderManifestData>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // First, get all folder IDs where peer_user is a recipient
        let folder_ids: Vec<String> = folder_share_records::table
            .filter(folder_share_records::recipient_user_id.eq(peer_user_id))
            .select(folder_share_records::folder_id)
            .distinct()
            .load::<String>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get folders for peer user '{}': {}",
                    peer_user_id, e
                ))
            })?;

        if folder_ids.is_empty() {
            return Ok(Vec::new());
        }

        // For each folder, get ALL recipient user IDs
        let mut folder_manifest_data = Vec::new();

        for folder_id in folder_ids {
            // Get all recipients for this folder
            let recipient_ids: Vec<String> = folder_share_records::table
                .filter(folder_share_records::folder_id.eq(&folder_id))
                .select(folder_share_records::recipient_user_id)
                .distinct()
                .load::<String>(&mut *conn)
                .map_err(|e| {
                    RepositoryError::DatabaseError(format!(
                        "Failed to get recipients for folder '{}': {}",
                        folder_id, e
                    ))
                })?;

            folder_manifest_data.push(FolderManifestData {
                folder_id,
                recipient_user_ids: recipient_ids,
            });
        }

        Ok(folder_manifest_data)
    }
}
