use crate::database::schema::{device_record_status, device_records, sync_records};
use crate::database::DbConnection;
use crate::domains::models::sync_record::{
    DeviceRecord, DeviceRecordSet, DeviceRecordStatus, StatusChangeSet, SyncRecord, SyncRecordSet,
};
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

    async fn update_device_record(
        &self,
        device_id: String,
        sync_id: String,
    ) -> Result<(), RepositoryError> {
        //TODO: update the device_sync_record of other device_record_id
        let mut conn = self.connection.lock().await;
        let device_record_id = device_records::table
            .filter(device_records::sync_record_id.eq(&sync_id))
            .filter(device_records::device_id.eq(&device_id))
            .select(device_records::id)
            .first::<String>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        diesel::update(device_records::table)
            .filter(device_records::sync_record_id.eq(&sync_id))
            .filter(device_records::device_id.eq(&device_id))
            .filter(device_records::status.eq("pending"))
            .set((
                device_records::synced.eq(true),
                device_records::updated_at.eq(chrono::Local::now().timestamp_millis()),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        diesel::update(device_record_status::table)
            .filter(device_record_status::device_record_id.eq(&device_record_id))
            .filter(device_record_status::aware_device_id.eq(&device_id))
            .set((
                device_record_status::synced.eq(true),
                device_record_status::updated_at.eq(chrono::Local::now().timestamp_millis()),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }
    async fn update_device_sync_record_status(
        &self,
        device_sync_record_id: String,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        diesel::update(device_record_status::table)
            .filter(device_record_status::id.eq(device_sync_record_id))
            .set(device_record_status::synced.eq(true))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }
    async fn add_status_change_set(
        &self,
        status_set: StatusChangeSet,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let device_model: DeviceRecordModel =
                DeviceRecordModel::from(&status_set.device_record);

            let status_models: Vec<DeviceRecordStatusModel> = status_set
                .device_record_statuses
                .iter()
                .map(DeviceRecordStatusModel::from)
                .collect();

            diesel::insert_into(device_records::table)
                .values(&device_model)
                .execute(conn)?;

            diesel::insert_into(device_record_status::table)
                .values(&status_models)
                .execute(conn)?;

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }
    async fn update_device_record_set(
        &self,
        record_set: DeviceRecordSet,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // Iterate over each device record and update it

            let status_models: Vec<DeviceRecordStatusModel> = record_set
                .device_record_statuses
                .iter()
                .map(DeviceRecordStatusModel::from)
                .collect();
            let device_models: Vec<DeviceRecordModel> = record_set
                .device_records
                .iter()
                .map(DeviceRecordModel::from)
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

    async fn get_pending_sync_by_type(
        &self,
        device_id: &str,
        resource_type: &str,
    ) -> Result<Option<(SyncRecord, Vec<DeviceRecord>, Vec<DeviceRecordStatus>)>, RepositoryError>
    {
        let mut conn = self.connection.lock().await;

        // Start by finding unsynced device record status
        let unsynced_status = device_record_status::table
            .inner_join(device_records::table.inner_join(sync_records::table))
            .filter(device_record_status::aware_device_id.eq(device_id))
            .filter(device_record_status::synced.eq(false))
            .filter(device_records::synced.eq(false))
            .filter(sync_records::resource_type.eq(resource_type))
            .order_by(sync_records::created_at.asc())
            .select(DeviceRecordStatusModel::as_select())
            .first::<DeviceRecordStatusModel>(&mut *conn)
            .optional()
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        match unsynced_status {
            Some(status) => {
                // Get the device record
                let device_record = device_records::table
                    .find(&status.device_record_id)
                    .first::<DeviceRecordModel>(&mut *conn)
                    .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

                // Get the sync record
                let sync_record = sync_records::table
                    .find(&device_record.sync_record_id)
                    .first::<SyncRecordModel>(&mut *conn)
                    .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

                // Get all device records for this sync
                let device_records = device_records::table
                    .filter(device_records::sync_record_id.eq(&sync_record.id))
                    .select(DeviceRecordModel::as_select())
                    .load::<DeviceRecordModel>(&mut *conn)
                    .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

                // Get all device record statuses
                let device_record_ids: Vec<String> =
                    device_records.iter().map(|dr| dr.id.clone()).collect();
                let status_records = device_record_status::table
                    .filter(device_record_status::device_record_id.eq_any(device_record_ids))
                    .select(DeviceRecordStatusModel::as_select())
                    .load::<DeviceRecordStatusModel>(&mut *conn)
                    .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

                Ok(Some((
                    sync_record.to_domain(),
                    device_records.iter().map(|dr| dr.to_domain()).collect(),
                    status_records.iter().map(|sr| sr.to_domain()).collect(),
                )))
            }
            None => Ok(None),
        }
    }
    async fn get_unsynced_device_sync_records(
        &self,
        device_id: &str,
    ) -> Result<Vec<(DeviceRecord, Vec<DeviceRecordStatus>)>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Get device records that have unsynced status for the target device
        let unsynced_records = device_records::table
            .inner_join(device_record_status::table)
            .filter(device_record_status::aware_device_id.eq(device_id))
            .filter(device_record_status::synced.eq(false))
            .order_by(device_records::created_at.asc())
            .select(DeviceRecordModel::as_select())
            .distinct() // Add distinct to avoid duplicate device records
            .load::<DeviceRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();

        // For each unsynced record, get all its statuses
        for device_record in unsynced_records {
            let statuses = device_record_status::table
                .filter(device_record_status::device_record_id.eq(&device_record.id))
                .select(DeviceRecordStatusModel::as_select())
                .load::<DeviceRecordStatusModel>(&mut *conn)
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

            result.push((
                device_record.to_domain(),
                statuses.iter().map(|s| s.to_domain()).collect(),
            ));
        }

        Ok(result)
    }

    async fn update_sync_status(
        &self,
        device_record_ids: Vec<String>,
        status_record_ids: Vec<String>,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // Update device records
            if !device_record_ids.is_empty() {
                diesel::update(device_records::table)
                    .filter(device_records::id.eq_any(device_record_ids))
                    .set((
                        device_records::synced.eq(true),
                        device_records::updated_at.eq(chrono::Local::now().timestamp_millis()),
                    ))
                    .execute(conn)?;
            }

            // Update status records
            if !status_record_ids.is_empty() {
                diesel::update(device_record_status::table)
                    .filter(device_record_status::id.eq_any(status_record_ids))
                    .set((
                        device_record_status::synced.eq(true),
                        device_record_status::updated_at
                            .eq(chrono::Local::now().timestamp_millis()),
                    ))
                    .execute(conn)?;
            }

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }
    async fn update_device_sync_record_by_device_id(
        &self,
        device_record_ids: Vec<String>,
        synced_device_id: String,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            diesel::update(device_record_status::table)
                .filter(device_record_status::aware_device_id.eq(synced_device_id))
                .filter(device_record_status::device_record_id.eq_any(device_record_ids))
                .set(device_record_status::synced.eq(true))
                .execute(conn)?;
            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }
}
