use crate::database::schema::{share_records, user_record_status, user_records};
use crate::database::DbConnection;
use crate::domains::models::share_record::{
    ShareRecord, ShareRecordSet, ShareStatusChangeSet, UserRecord, UserRecordSet, UserRecordStatus,
};
use crate::domains::models::share_types::ShareOperation;
use crate::domains::repositories::{RepositoryError, ShareRepository};
use crate::persistence::models::{ShareRecordModel, UserRecordModel, UserRecordStatusModel};
use async_trait::async_trait;
use diesel::prelude::*;

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
    async fn add_share_record_set(
        &self,
        record_set: ShareRecordSet,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let share_model = ShareRecordModel::from(&record_set.share_record);
            let user_models: Vec<UserRecordModel> = record_set
                .user_records
                .iter()
                .map(UserRecordModel::from)
                .collect();
            let status_models: Vec<UserRecordStatusModel> = record_set
                .user_record_statuses
                .iter()
                .map(UserRecordStatusModel::from)
                .collect();

            diesel::insert_into(share_records::table)
                .values(&share_model)
                .execute(conn)?;

            diesel::insert_into(user_records::table)
                .values(&user_models)
                .execute(conn)?;

            diesel::insert_into(user_record_status::table)
                .values(&status_models)
                .execute(conn)?;

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }

    async fn add_status_change_set(
        &self,
        status_set: ShareStatusChangeSet,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let user_model: UserRecordModel = UserRecordModel::from(&status_set.user_record);

            let status_models: Vec<UserRecordStatusModel> = status_set
                .user_record_statuses
                .iter()
                .map(UserRecordStatusModel::from)
                .collect();

            diesel::insert_into(user_records::table)
                .values(&user_model)
                .execute(conn)?;

            diesel::insert_into(user_record_status::table)
                .values(&status_models)
                .execute(conn)?;

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }

    async fn update_user_record(
        &self,
        user_id: String,
        share_id: String,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let user_record_id = user_records::table
            .filter(user_records::share_record_id.eq(&share_id))
            .filter(user_records::user_id.eq(&user_id))
            .select(user_records::id)
            .first::<String>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        diesel::update(user_records::table)
            .filter(user_records::share_record_id.eq(&share_id))
            .filter(user_records::user_id.eq(&user_id))
            .filter(user_records::status.eq("pending"))
            .set((
                user_records::synced.eq(true),
                user_records::updated_at.eq(chrono::Local::now().timestamp_millis()),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        diesel::update(user_record_status::table)
            .filter(user_record_status::user_record_id.eq(&user_record_id))
            .filter(user_record_status::aware_user_id.eq(&user_id))
            .set((
                user_record_status::synced.eq(true),
                user_record_status::updated_at.eq(chrono::Local::now().timestamp_millis()),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn update_user_record_set(
        &self,
        record_set: UserRecordSet,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let status_models: Vec<UserRecordStatusModel> = record_set
                .user_record_statuses
                .iter()
                .map(UserRecordStatusModel::from)
                .collect();

            let user_models: Vec<UserRecordModel> = record_set
                .user_records
                .iter()
                .map(UserRecordModel::from)
                .collect();

            diesel::insert_into(user_records::table)
                .values(&user_models)
                .execute(conn)?;

            diesel::insert_into(user_record_status::table)
                .values(&status_models)
                .execute(conn)?;

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }

    async fn get_pending_share_by_user(
        &self,
        user_id: &str,
    ) -> Result<Option<(ShareRecord, Vec<UserRecord>, Vec<UserRecordStatus>)>, RepositoryError>
    {
        let mut conn = self.connection.lock().await;

        // Start by finding unsynced user record status
        let unsynced_status = user_record_status::table
            .inner_join(user_records::table.inner_join(share_records::table))
            .filter(user_record_status::aware_user_id.eq(user_id))
            .filter(user_record_status::synced.eq(false))
            .filter(user_records::synced.eq(false))
            .order_by(share_records::created_at.asc())
            .select(UserRecordStatusModel::as_select())
            .first::<UserRecordStatusModel>(&mut *conn)
            .optional()
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        match unsynced_status {
            Some(status) => {
                // Get the user record
                let user_record = user_records::table
                    .find(&status.user_record_id)
                    .first::<UserRecordModel>(&mut *conn)
                    .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

                // Get the share record
                let share_record = share_records::table
                    .find(&user_record.share_record_id)
                    .first::<ShareRecordModel>(&mut *conn)
                    .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

                // Get all user records for this share
                let user_records = user_records::table
                    .filter(user_records::share_record_id.eq(&share_record.id))
                    .select(UserRecordModel::as_select())
                    .load::<UserRecordModel>(&mut *conn)
                    .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

                // Get all user record statuses
                let user_record_ids: Vec<String> =
                    user_records.iter().map(|ur| ur.id.clone()).collect();
                let status_records = user_record_status::table
                    .filter(user_record_status::user_record_id.eq_any(user_record_ids))
                    .select(UserRecordStatusModel::as_select())
                    .load::<UserRecordStatusModel>(&mut *conn)
                    .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

                Ok(Some((
                    share_record.to_domain(),
                    user_records.iter().map(|ur| ur.to_domain()).collect(),
                    status_records.iter().map(|sr| sr.to_domain()).collect(),
                )))
            }
            None => Ok(None),
        }
    }

    async fn get_unsynced_user_share_records(
        &self,
        user_id: &str,
    ) -> Result<Vec<(UserRecord, Vec<UserRecordStatus>)>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Get user records that have unsynced status for the target user
        let unsynced_records = user_records::table
            .inner_join(user_record_status::table)
            .filter(user_record_status::aware_user_id.eq(user_id))
            .filter(user_record_status::synced.eq(false))
            .order_by(user_records::created_at.asc())
            .select(UserRecordModel::as_select())
            .distinct()
            .load::<UserRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();

        // For each unsynced record, get all its statuses
        for user_record in unsynced_records {
            let statuses = user_record_status::table
                .filter(user_record_status::user_record_id.eq(&user_record.id))
                .select(UserRecordStatusModel::as_select())
                .load::<UserRecordStatusModel>(&mut *conn)
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

            result.push((
                user_record.to_domain(),
                statuses.iter().map(|s| s.to_domain()).collect(),
            ));
        }

        Ok(result)
    }

    async fn update_user_sync_record_status(
        &self,
        user_sync_record_id: String,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        diesel::update(user_record_status::table)
            .filter(user_record_status::id.eq(user_sync_record_id))
            .set(user_record_status::synced.eq(true))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn update_share_status(
        &self,
        user_record_ids: Vec<String>,
        status_record_ids: Vec<String>,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // Update user records
            if !user_record_ids.is_empty() {
                diesel::update(user_records::table)
                    .filter(user_records::id.eq_any(user_record_ids))
                    .set((
                        user_records::synced.eq(true),
                        user_records::updated_at.eq(chrono::Local::now().timestamp_millis()),
                    ))
                    .execute(conn)?;
            }

            // Update status records
            if !status_record_ids.is_empty() {
                diesel::update(user_record_status::table)
                    .filter(user_record_status::id.eq_any(status_record_ids))
                    .set((
                        user_record_status::synced.eq(true),
                        user_record_status::updated_at.eq(chrono::Local::now().timestamp_millis()),
                    ))
                    .execute(conn)?;
            }

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }

    async fn update_user_sync_record_by_user_id(
        &self,
        user_record_ids: Vec<String>,
        synced_user_id: String,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            diesel::update(user_record_status::table)
                .filter(user_record_status::aware_user_id.eq(synced_user_id))
                .filter(user_record_status::user_record_id.eq_any(user_record_ids))
                .set(user_record_status::synced.eq(true))
                .execute(conn)?;
            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }

    async fn get_all_share_records(&self) -> Result<Vec<ShareRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let models = share_records::table
            .order_by(share_records::created_at.asc())
            .select(ShareRecordModel::as_select())
            .load::<ShareRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(models.iter().map(|m| m.to_domain()).collect())
    }
    async fn get_effective_share_records(
        &self,
        resource_id: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let models = share_records::table
            .filter(share_records::resource_id.eq(resource_id))
            .filter(share_records::operation_type.eq(ShareOperation::Share.to_string()))
            .order_by(share_records::created_at.desc())
            .limit(1) // Start with just one record for our optimization
            .select(ShareRecordModel::as_select())
            .load::<ShareRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(models.iter().map(|m| m.to_domain()).collect())
    }

    async fn get_user_records_by_share_id(
        &self,
        share_id: &str,
    ) -> Result<Vec<UserRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Get all user records for this share record
        let models = user_records::table
            .filter(user_records::share_record_id.eq(share_id))
            .select(UserRecordModel::as_select())
            .load::<UserRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(models.iter().map(|ur| ur.to_domain()).collect())
    }

    async fn get_share_record_by_id(&self, share_id: &str) -> Result<ShareRecord, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Query for a specific share record by its ID
        let share_model = share_records::table
            .find(share_id)
            .select(ShareRecordModel::as_select())
            .first::<ShareRecordModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        // Convert the model to domain object
        Ok(share_model.to_domain())
    }
}
