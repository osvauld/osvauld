use crate::database::schema::{device_record_status, device_records, sync_records};
use crate::database::DbConnection;
use crate::domains::models::sync_record::{InitialDeviceSyncSet, SyncRecord, SyncRecordSet};
use crate::domains::repositories::{RepositoryError, SyncRepository};
use crate::persistence::models::{DeviceRecordModel, DeviceRecordStatusModel, SyncRecordModel};
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
    async fn add_sync_record_set(&self, record_set: SyncRecordSet) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let sync_model = SyncRecordModel::from(&record_set.sync_record);
            let device_models: Vec<DeviceRecordModel> = record_set
                .device_records
                .iter()
                .map(DeviceRecordModel::from)
                .collect();
            let status_models: Vec<DeviceRecordStatusModel> = record_set
                .device_record_statuses
                .iter()
                .map(DeviceRecordStatusModel::from)
                .collect();

            diesel::insert_into(sync_records::table)
                .values(&sync_model)
                .execute(conn)?;

            diesel::insert_into(device_records::table)
                .values(&device_models)
                .execute(conn)?;

            diesel::insert_into(device_record_status::table)
                .values(&status_models)
                .execute(conn)?;

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }
    async fn get_all_sync_records(&self) -> Result<Vec<SyncRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let models = sync_records::table
            .order_by(sync_records::created_at.asc())
            .select(SyncRecordModel::as_select())
            .load::<SyncRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(models.iter().map(|m| m.to_domain()).collect())
    }

    async fn add_initial_device_sync_set(
        &self,
        sync_set: InitialDeviceSyncSet,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let device_models: Vec<DeviceRecordModel> = sync_set
                .device_records
                .iter()
                .map(DeviceRecordModel::from)
                .collect();

            let status_models: Vec<DeviceRecordStatusModel> = sync_set
                .device_record_statuses
                .iter()
                .map(DeviceRecordStatusModel::from)
                .collect();

            diesel::insert_into(device_records::table)
                .values(&device_models)
                .execute(conn)?;

            diesel::insert_into(device_record_status::table)
                .values(&status_models)
                .execute(conn)?;

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }
}
