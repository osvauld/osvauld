use crate::database::schema::sync_records;
use crate::database::DbConnection;
use crate::domains::models::sync_record::SyncRecord;
use crate::domains::repositories::{RepositoryError, SyncRepository};
use crate::persistence::models::SyncRecordModel;
use async_trait::async_trait;
use diesel::prelude::*;

pub struct SqliteSyncRepository {
    connection: DbConnection,
}

impl SqliteSyncRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl SyncRepository for SqliteSyncRepository {
    async fn save_sync_records(&self, records: &[SyncRecord]) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let sync_models: Vec<SyncRecordModel> =
            records.iter().map(|r| SyncRecordModel::from(r)).collect();

        diesel::insert_into(sync_records::table)
            .values(&sync_models)
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn get_pending_records(
        &self,
        target_device_id: &str,
    ) -> Result<Vec<SyncRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let sync_models = sync_records::table
            .filter(sync_records::status.eq("pending"))
            .filter(sync_records::target_device_id.eq(target_device_id))
            .order_by(sync_records::created_at.asc())
            .select(SyncRecordModel::as_select())
            .load::<SyncRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(SyncRecordModel::to_domain_records(sync_models))
    }

    async fn update_status(&self, sync_id: &str, status: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        diesel::update(sync_records::table)
            .filter(sync_records::id.eq(sync_id))
            .set(sync_records::status.eq(status))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn get_sync_records_by_device_id(
        &self,
        target_device_id: &str,
    ) -> Result<Vec<SyncRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let sync_models = sync_records::table
            .filter(sync_records::target_device_id.eq(target_device_id))
            .filter(sync_records::operation_type.ne("status_change"))
            .order_by(sync_records::created_at.asc())
            .select(SyncRecordModel::as_select())
            .load::<SyncRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(SyncRecordModel::to_domain_records(sync_models))
    }

    async fn find_by_id(&self, sync_id: &str) -> Result<SyncRecord, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let sync_rec = sync_records::table
            .filter(sync_records::id.eq(sync_id))
            .select(SyncRecordModel::as_select())
            .first::<SyncRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(sync_rec.into())
    }
}
