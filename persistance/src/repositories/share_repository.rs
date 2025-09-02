use crate::DbConnection;
use crate::database::schema::share_records;
use crate::models::ShareRecordModel;
use async_trait::async_trait;
use diesel::prelude::*;
use osvauld_core::models::share_record::ShareRecord;
use osvauld_core::repositories::{RepositoryError, ShareRepository};

pub struct SqliteShareRepository {
    connection: DbConnection,
}

impl SqliteShareRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl ShareRepository for SqliteShareRepository {
    async fn save(&self, share_record: &ShareRecord) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let share_record_model = ShareRecordModel::from(share_record);

        diesel::insert_into(share_records::table)
            .values(&share_record_model)
            .execute(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to save share record for resource '{}' to user '{}': {}",
                    share_record.resource_id, share_record.recipient_user_id, e
                ))
            })?;

        Ok(())
    }

    async fn save_many(&self, share_records: &[ShareRecord]) -> Result<(), RepositoryError> {
        if share_records.is_empty() {
            return Ok(());
        }
        let mut conn = self.connection.lock().await;
        let share_record_models: Vec<ShareRecordModel> =
            share_records.iter().map(ShareRecordModel::from).collect();

        // SQLite doesn't support batch insert with on_conflict, so use transaction with individual inserts
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for share_record in &share_record_models {
                diesel::insert_into(share_records::table)
                    .values(share_record)
                    .on_conflict(share_records::id)
                    .do_nothing()
                    .execute(conn)?;
            }
            Ok(())
        })
        .map_err(|e| {
            RepositoryError::DatabaseError(format!(
                "Failed to bulk save {} share records: {}",
                share_records.len(),
                e
            ))
        })?;

        Ok(())
    }
    async fn find_by_resource_and_operation(
        &self,
        resource_id: &str,
        operation_type: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let share_record_models = share_records::table
            .filter(share_records::resource_id.eq(resource_id))
            .filter(share_records::operation_type.eq(operation_type))
            .load::<ShareRecordModel>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to find share records for resource '{}' with operation '{}': {}",
                    resource_id, operation_type, e
                ))
            })?;
        // Convert database models to domain models
        let share_records = share_record_models
            .into_iter()
            .map(|model| model.to_domain())
            .collect();

        Ok(share_records)
    }

    async fn get_ucan_token_by_resource(
        &self,
        resource_id: &str,
        user_id: &str,
    ) -> Result<String, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let share_record_model = share_records::table
            .filter(share_records::resource_id.eq(resource_id))
            .filter(share_records::recipient_user_id.eq(user_id))
            .filter(share_records::operation_type.eq("share"))
            .first::<ShareRecordModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(format!(
                    "Failed to get UCAN token for resource '{}' and user '{}': {}",
                    resource_id, user_id, e
                )),
            })?;
        Ok(share_record_model.ucan_token)
    }

    async fn find_by_resource(
        &self,
        resource_id: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let share_record_models = share_records::table
            .filter(share_records::resource_id.eq(resource_id))
            .load::<ShareRecordModel>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to find share records for resource '{}': {}",
                    resource_id, e
                ))
            })?;
        // Convert database models to domain models
        let share_records = share_record_models
            .into_iter()
            .map(|model| model.to_domain())
            .collect();

        Ok(share_records)
    }
    async fn find_by_id(&self, id: &str) -> Result<ShareRecord, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let share_record_model = share_records::table
            .filter(share_records::id.eq(id))
            .first::<ShareRecordModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(format!(
                    "Failed to find share record with id '{}': {}",
                    id, e
                )),
            })?;

        // Convert database model to domain model
        Ok(share_record_model.to_domain())
    }
    async fn get_user_share_records(
        &self,
        user_id: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Step 1: Get all share records where this user is the recipient
        let user_share_records = share_records::table
            .filter(share_records::recipient_user_id.eq(user_id))
            .load::<ShareRecordModel>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get share records for user '{}': {}",
                    user_id, e
                ))
            })?;

        // Step 2: Extract all resource_ids from those share records
        let resource_ids: Vec<String> = user_share_records
            .iter()
            .map(|record| record.resource_id.clone())
            .collect();

        // Step 3: Get all share records for those resources
        let all_share_records = if resource_ids.is_empty() {
            // If user has no shared resources, return empty vector
            Vec::new()
        } else {
            share_records::table
                .filter(share_records::resource_id.eq_any(&resource_ids))
                .load::<ShareRecordModel>(&mut *conn)
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
        };

        // Step 4: Convert to domain objects
        let share_records = ShareRecordModel::to_domain_records(all_share_records);

        Ok(share_records)
    }
    async fn find_by_resource_and_operation_and_user(
        &self,
        resource_id: &str,
        operation_type: &str,
        user_id: &str,
    ) -> Result<ShareRecord, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let share_record_model = share_records::table
            .filter(share_records::resource_id.eq(resource_id))
            .filter(share_records::recipient_user_id.eq(user_id))
            .filter(share_records::operation_type.eq(operation_type))
            .first::<ShareRecordModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(format!(
                    "Failed to find share record for resource '{}', operation '{}', user '{}': {}",
                    resource_id, operation_type, user_id, e
                )),
            })?;

        Ok(share_record_model.to_domain())
    }
    async fn get_ucan_by_cid(&self, cid: &str) -> Result<String, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let share_record_model = share_records::table
            .filter(share_records::ucan_cid.eq(cid))
            .first::<ShareRecordModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(format!(
                    "Failed to get UCAN token by CID '{}': {}",
                    cid, e
                )),
            })?;
        Ok(share_record_model.ucan_token)
    }
}
