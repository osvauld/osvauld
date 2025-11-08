use crate::DbConnection;
use crate::database::schema::{folder_share_records, users};
use crate::models::{FolderShareRecordModel, UserModel};
use async_trait::async_trait;
use diesel::prelude::*;
use osvauld_core::models::folder_share_record::FolderShareRecord;
use osvauld_core::models::share_record::ShareOperation;
use osvauld_core::models::User;
use osvauld_core::repositories::{FolderShareRecordRepository, RepositoryError};

pub struct SqliteFolderShareRecordRepository {
    connection: DbConnection,
}

impl SqliteFolderShareRecordRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl FolderShareRecordRepository for SqliteFolderShareRecordRepository {
    async fn save(&self, folder_share_record: &FolderShareRecord) -> Result<(), RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;
        let model = FolderShareRecordModel::from(folder_share_record);

        diesel::insert_into(folder_share_records::table)
            .values(&model)
            .execute(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to save folder share record for folder '{}': {}",
                    folder_share_record.folder_id, e
                ))
            })?;

        Ok(())
    }

    async fn get_records_by_folder_id(
        &self,
        folder_id: &str,
    ) -> Result<Vec<FolderShareRecord>, RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let models: Vec<FolderShareRecordModel> = folder_share_records::table
            .filter(folder_share_records::folder_id.eq(folder_id))
            .filter(folder_share_records::operation_type.eq(ShareOperation::Share.to_string()))
            .load(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get folder share records for folder '{}': {}",
                    folder_id, e
                ))
            })?;

        Ok(FolderShareRecordModel::to_domain_records(models))
    }

    async fn get_records_by_recipient_user_id(
        &self,
        user_id: &str,
    ) -> Result<Vec<FolderShareRecord>, RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let models: Vec<FolderShareRecordModel> = folder_share_records::table
            .filter(folder_share_records::recipient_user_id.eq(user_id))
            .filter(folder_share_records::operation_type.eq(ShareOperation::Share.to_string()))
            .load(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get folder share records for user '{}': {}",
                    user_id, e
                ))
            })?;

        Ok(FolderShareRecordModel::to_domain_records(models))
    }

    async fn save_bulk_with_conflict_ignore(
        &self,
        folder_share_records: &[FolderShareRecord],
    ) -> Result<(), RepositoryError> {
        if folder_share_records.is_empty() {
            return Ok(());
        }

        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;
        let models: Vec<FolderShareRecordModel> = folder_share_records
            .iter()
            .map(FolderShareRecordModel::from)
            .collect();

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for model in &models {
                diesel::insert_into(folder_share_records::table)
                    .values(model)
                    .on_conflict(folder_share_records::id)
                    .do_nothing()
                    .execute(conn)?;
            }
            Ok(())
        })
        .map_err(|e| {
            RepositoryError::DatabaseError(format!(
                "Failed to bulk save {} folder share records: {}",
                folder_share_records.len(),
                e
            ))
        })?;

        Ok(())
    }

    async fn get_ucan_by_cid(&self, cid: &str) -> Result<String, RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let ucan_token = folder_share_records::table
            .filter(folder_share_records::ucan_cid.eq(cid))
            .select(folder_share_records::ucan_token)
            .first::<String>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(format!(
                    "Failed to get UCAN token by CID '{}' from folder share records: {}",
                    cid, e
                )),
            })?;

        Ok(ucan_token)
    }
    // Removed: share_folder_transaction - no longer needed with new architecture
    // Resource keys and vector clocks are managed differently now
    async fn get_shared_users(&self, folder_id: &str) -> Result<Vec<User>, RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let user_models: Vec<UserModel> = folder_share_records::table
            .inner_join(users::table.on(folder_share_records::recipient_user_id.eq(users::id)))
            .filter(folder_share_records::folder_id.eq(folder_id))
            .filter(folder_share_records::operation_type.eq(ShareOperation::Share.to_string()))
            .select(users::all_columns)
            .load::<UserModel>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get shared users for folder '{}': {}",
                    folder_id, e
                ))
            })?;

        Ok(UserModel::to_domain_users(user_models))
    }

    async fn find_by_folder_and_user(
        &self,
        folder_id: &str,
        user_id: &str,
    ) -> Result<Option<FolderShareRecord>, RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let result = folder_share_records::table
            .filter(folder_share_records::folder_id.eq(folder_id))
            .filter(folder_share_records::recipient_user_id.eq(user_id))
            .filter(folder_share_records::operation_type.eq(ShareOperation::Share.to_string()))
            .first::<FolderShareRecordModel>(&mut *conn)
            .optional()
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to find folder share record for folder '{}' and user '{}': {}",
                    folder_id, user_id, e
                ))
            })?;

        Ok(result.map(|model| model.to_domain()))
    }
}
