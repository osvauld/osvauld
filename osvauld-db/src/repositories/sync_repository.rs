use crate::database::DbConnection;
use crate::database::schema::{device_record_status, device_records, devices, sync_records};
use crate::models::{DeviceModel, DeviceRecordModel, DeviceRecordStatusModel, SyncRecordModel};
use async_trait::async_trait;
use diesel::prelude::*;
use osvauld_core::models::device::Device;
use osvauld_core::models::sync_record::{
    DeviceRecord, DeviceRecordSet, DeviceRecordStatus, StatusChangeSet, SyncRecord, SyncRecordSet,
};
use osvauld_core::repositories::{RepositoryError, SyncRepository};

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
    async fn add_sync_record_set(&self, record_set: &SyncRecordSet) -> Result<(), RepositoryError> {
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

        // Find a device record where:
        // 1. It's for the specified device
        // 2. It's not synced yet
        // 3. It's associated with a sync record of the specified resource type
        // Order by creation time to get the oldest first
        let pending_device_record = device_records::table
            .inner_join(sync_records::table)
            .filter(device_records::device_id.eq(device_id))
            .filter(device_records::synced.eq(false))
            .filter(sync_records::resource_type.eq(resource_type))
            .order_by(sync_records::created_at.asc())
            .select(DeviceRecordModel::as_select())
            .first::<DeviceRecordModel>(&mut *conn)
            .optional()
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        if let Some(device_record) = pending_device_record {
            // Get the associated sync record
            let sync_record = sync_records::table
                .find(&device_record.sync_record_id)
                .first::<SyncRecordModel>(&mut *conn)
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

            // Get all device records for this sync
            let all_device_records = device_records::table
                .filter(device_records::sync_record_id.eq(&sync_record.id))
                .select(DeviceRecordModel::as_select())
                .load::<DeviceRecordModel>(&mut *conn)
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

            // Get all device record statuses
            let device_record_ids: Vec<String> =
                all_device_records.iter().map(|dr| dr.id.clone()).collect();

            let all_statuses = device_record_status::table
                .filter(device_record_status::device_record_id.eq_any(device_record_ids))
                .select(DeviceRecordStatusModel::as_select())
                .load::<DeviceRecordStatusModel>(&mut *conn)
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

            return Ok(Some((
                sync_record.to_domain(),
                all_device_records.iter().map(|dr| dr.to_domain()).collect(),
                all_statuses.iter().map(|sr| sr.to_domain()).collect(),
            )));
        }

        Ok(None)
    }

    async fn get_unsynced_device_sync_records(
        &self,
        device_id: &str,
    ) -> Result<Vec<(DeviceRecord, Vec<DeviceRecordStatus>)>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Find device record statuses that:
        // 1. Are for this device to be aware of
        // 2. Are not synced yet
        let unsynced_statuses = device_record_status::table
            .filter(device_record_status::aware_device_id.eq(device_id))
            .filter(device_record_status::synced.eq(false))
            .order_by(device_record_status::created_at.asc())
            .select(DeviceRecordStatusModel::as_select())
            .load::<DeviceRecordStatusModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Group by device_record_id to avoid duplicates
        let device_record_ids: Vec<String> = unsynced_statuses
            .iter()
            .map(|status| status.device_record_id.clone())
            .collect::<std::collections::HashSet<String>>()
            .into_iter()
            .collect();

        let mut result = Vec::new();

        for device_record_id in device_record_ids {
            // Get the device record
            let device_record = device_records::table
                .find(&device_record_id)
                .first::<DeviceRecordModel>(&mut *conn)
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

            // Get all statuses for this device record
            let statuses = device_record_status::table
                .filter(device_record_status::device_record_id.eq(&device_record_id))
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

    async fn get_sync_records_by_resource_and_operation(
        &self,
        resource_id: &str,
        operation_type: &str,
    ) -> Result<Vec<SyncRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let models = sync_records::table
            .filter(sync_records::resource_id.eq(resource_id))
            .filter(sync_records::operation_type.eq(operation_type))
            .order_by(sync_records::created_at.desc())
            .select(SyncRecordModel::as_select())
            .load::<SyncRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(models.iter().map(|m| m.to_domain()).collect())
    }

    async fn get_device_records_by_sync_id(
        &self,
        sync_id: &str,
    ) -> Result<Vec<DeviceRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let models = device_records::table
            .filter(device_records::sync_record_id.eq(sync_id))
            .select(DeviceRecordModel::as_select())
            .load::<DeviceRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(models.iter().map(|dr| dr.to_domain()).collect())
    }

    async fn get_users_with_unsynced_devices(&self) -> Result<Vec<Device>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // This query gets all device IDs that have unsynced records
        let device_ids: Vec<String> = device_record_status::table
            .filter(device_record_status::synced.eq(false))
            .inner_join(devices::table.on(device_record_status::aware_device_id.eq(devices::id)))
            .select(devices::id)
            .distinct()
            .load::<String>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Get the full device objects for these IDs
        let device_models = devices::table
            .filter(devices::id.eq_any(device_ids))
            .load::<DeviceModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(DeviceModel::to_domain_devices(device_models))
    }

    async fn update_device_record_statuses_for_sync(
        &self,
        sync_record_id: String,
        device_id: String,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        // First, get all device records for this sync_record
        let device_record_ids: Vec<String> = device_records::table
            .filter(device_records::sync_record_id.eq(&sync_record_id))
            .select(device_records::id)
            .load::<String>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Then update all device record statuses where aware_device_id matches
        // and device_record_id is in the list of device_record_ids for this sync
        diesel::update(device_record_status::table)
            .filter(device_record_status::device_record_id.eq_any(device_record_ids))
            .filter(device_record_status::aware_device_id.eq(&device_id))
            .set((
                device_record_status::synced.eq(true),
                device_record_status::updated_at.eq(chrono::Local::now().timestamp_millis()),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }
    async fn get_resource_ids_for_device(
        &self,
        device_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Query for resource IDs through the device_records -> sync_records path
        // Filter for resource_type = "resource"
        let resource_ids: Vec<String> = device_records::table
            .inner_join(sync_records::table)
            .filter(device_records::device_id.eq(device_id))
            .filter(sync_records::resource_type.eq("resource"))
            .select(sync_records::resource_id)
            .distinct()
            .load::<String>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(resource_ids)
    }

    async fn update_device_status_record_for_device(
        &self,
        device_id: &str,
        device_record_id: &str,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        diesel::update(device_record_status::table)
            .filter(device_record_status::device_record_id.eq(device_record_id))
            .filter(device_record_status::aware_device_id.eq(device_id))
            .set((
                device_record_status::synced.eq(true),
                device_record_status::updated_at.eq(chrono::Local::now().timestamp_millis()),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn update_device_sync_status_by_ids(
        &self,
        device_record_ids: Vec<String>,
        device_record_status_ids: Vec<String>,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // Update device records if there are any IDs provided
            if !device_record_ids.is_empty() {
                diesel::update(device_records::table)
                    .filter(device_records::id.eq_any(device_record_ids))
                    .set((
                        device_records::synced.eq(true),
                        device_records::updated_at.eq(chrono::Local::now().timestamp_millis()),
                    ))
                    .execute(conn)?;
            }

            // Update device record statuses if there are any IDs provided
            if !device_record_status_ids.is_empty() {
                diesel::update(device_record_status::table)
                    .filter(device_record_status::id.eq_any(device_record_status_ids))
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

    async fn get_device_records_and_statuses_by_sync_record(
        &self,
        sync_record_id: &str,
    ) -> Result<(Vec<DeviceRecord>, Vec<DeviceRecordStatus>), RepositoryError> {
        let mut conn = self.connection.lock().await;

        // First, get all device records for this sync_record
        let device_records = device_records::table
            .filter(device_records::sync_record_id.eq(sync_record_id))
            .select(DeviceRecordModel::as_select())
            .load::<DeviceRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        if device_records.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }

        // Extract device record IDs
        let device_record_ids: Vec<String> =
            device_records.iter().map(|dr| dr.id.clone()).collect();

        // Get all statuses for these device records
        let status_models = device_record_status::table
            .filter(device_record_status::device_record_id.eq_any(device_record_ids))
            .select(DeviceRecordStatusModel::as_select())
            .load::<DeviceRecordStatusModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Convert models to domain objects
        let domain_records = device_records.iter().map(|m| m.to_domain()).collect();
        let domain_statuses = status_models.iter().map(|m| m.to_domain()).collect();

        Ok((domain_records, domain_statuses))
    }

    async fn get_sync_record_by_id(
        &self,
        sync_id: &str,
    ) -> Result<Option<SyncRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let result = sync_records::table
            .find(sync_id)
            .select(SyncRecordModel::as_select())
            .first::<SyncRecordModel>(&mut *conn)
            .optional()
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(result.map(|model| model.to_domain()))
    }

    async fn add_device_records_bulk(
        &self,
        records: &[DeviceRecord],
    ) -> Result<(), RepositoryError> {
        if records.is_empty() {
            return Ok(());
        }

        let mut conn = self.connection.lock().await;

        let device_models: Vec<DeviceRecordModel> =
            records.iter().map(DeviceRecordModel::from).collect();

        diesel::insert_into(device_records::table)
            .values(&device_models)
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn add_device_record_statuses_bulk(
        &self,
        statuses: &[DeviceRecordStatus],
    ) -> Result<(), RepositoryError> {
        if statuses.is_empty() {
            return Ok(());
        }

        let mut conn = self.connection.lock().await;

        let status_models: Vec<DeviceRecordStatusModel> =
            statuses.iter().map(DeviceRecordStatusModel::from).collect();

        diesel::insert_into(device_record_status::table)
            .values(&status_models)
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn update_device_records_synced_bulk(
        &self,
        record_ids: &[String],
    ) -> Result<(), RepositoryError> {
        if record_ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.connection.lock().await;

        diesel::update(device_records::table)
            .filter(device_records::id.eq_any(record_ids))
            .set((
                device_records::synced.eq(true),
                device_records::updated_at.eq(chrono::Local::now().timestamp_millis()),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn update_device_record_statuses_synced_bulk(
        &self,
        status_ids: &[String],
    ) -> Result<(), RepositoryError> {
        if status_ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.connection.lock().await;

        diesel::update(device_record_status::table)
            .filter(device_record_status::id.eq_any(status_ids))
            .set((
                device_record_status::synced.eq(true),
                device_record_status::updated_at.eq(chrono::Local::now().timestamp_millis()),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
