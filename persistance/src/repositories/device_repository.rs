use crate::database::DbConnection;
use crate::database::schema::devices;
use crate::models::DeviceModel;
use async_trait::async_trait;
use chrono::Local;
use diesel::ExpressionMethods;
use diesel::prelude::*;
use osvauld_core::models::device::Device;
use osvauld_core::repositories::{DeviceRepository, RepositoryError};

pub struct SqliteDeviceRepository {
    connection: DbConnection,
}

impl SqliteDeviceRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl DeviceRepository for SqliteDeviceRepository {
    async fn save(&self, device: &Device) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        let device_model = DeviceModel::from(device);

        diesel::insert_into(devices::table)
            .values(&device_model)
            .execute(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to save device '{}' for user '{}': {}",
                    device.id, device.user_id, e
                ))
            })?;

        Ok(())
    }

    async fn find_by_id(&self, device_id: &str) -> Result<Device, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let device = devices::table
            .find(device_id)
            .first::<DeviceModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(format!(
                    "Failed to find device '{}': {}",
                    device_id, e
                )),
            })?;

        Ok(device.into())
    }

    async fn get_devices_by_user_id(&self, user_id: &str) -> Result<Vec<Device>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let device_models = devices::table
            .filter(devices::user_id.eq(user_id))
            .order_by(devices::created_at.desc())
            .load::<DeviceModel>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get devices for user '{}': {}",
                    user_id, e
                ))
            })?;

        Ok(DeviceModel::to_domain_devices(device_models))
    }

    async fn update_last_synced_at(&self, device_id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let timestamp = Local::now().timestamp_millis();

        diesel::update(devices::table)
            .filter(devices::id.eq(device_id))
            .set(devices::last_synced_at.eq(timestamp))
            .execute(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to update last_synced_at for device '{}' to timestamp {}: {}",
                    device_id, timestamp, e
                ))
            })?;
        Ok(())
    }

    async fn get_devices_by_user_except(
        &self,
        user_id: &str,
        exclude_ids: &[String],
    ) -> Result<Vec<Device>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let device_models = devices::table
            .filter(devices::id.ne_all(exclude_ids))
            .filter(devices::user_id.eq(user_id))
            .order_by(devices::created_at.desc())
            .load::<DeviceModel>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get devices for user '{}' excluding {} devices: {}",
                    user_id,
                    exclude_ids.len(),
                    e
                ))
            })?;
        Ok(DeviceModel::to_domain_devices(device_models))
    }

    async fn get_all_devices_except(
        &self,
        except_devices: &[String],
    ) -> Result<Vec<Device>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let device_models = devices::table
            .filter(devices::id.ne_all(except_devices))
            .load::<DeviceModel>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get all devices excluding {} devices: {}",
                    except_devices.len(),
                    e
                ))
            })?;
        Ok(DeviceModel::to_domain_devices(device_models))
    }

    async fn save_many(&self, devices: &[Device]) -> Result<(), RepositoryError> {
        if devices.is_empty() {
            return Ok(());
        }

        let mut conn = self.connection.lock().await;
        let device_models: Vec<DeviceModel> = devices.iter().map(DeviceModel::from).collect();
        let device_count = device_models.len();

        // SQLite doesn't support batch insert with on_conflict, so use transaction with individual inserts
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for (_index, device_model) in device_models.iter().enumerate() {
                diesel::insert_into(devices::table)
                    .values(device_model)
                    .on_conflict(devices::id)
                    .do_nothing()
                    .execute(conn)
                    .map_err(|_e| diesel::result::Error::RollbackTransaction)?;
            }
            Ok(())
        })
        .map_err(|e| {
            RepositoryError::DatabaseError(format!(
                "Failed to save {} devices in bulk operation: {}",
                device_count, e
            ))
        })?;

        Ok(())
    }

    async fn get_devices_by_user_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<Device>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // If the user_ids array is empty, return an empty vector
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Query devices that belong to any of the specified user IDs
        let device_models = devices::table
            .filter(devices::user_id.eq_any(user_ids))
            .order_by(devices::created_at.desc())
            .load::<DeviceModel>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get devices for {} users: {}",
                    user_ids.len(),
                    e
                ))
            })?;

        // Convert models to domain objects
        Ok(DeviceModel::to_domain_devices(device_models))
    }

    async fn get_device_ids_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let device_ids = devices::table
            .filter(devices::user_id.eq(user_id))
            .select(devices::id)
            .load::<String>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get device IDs for user '{}': {}",
                    user_id, e
                ))
            })?;

        Ok(device_ids)
    }

    async fn get_devices_by_ids(
        &self,
        device_ids: &[String],
    ) -> Result<Vec<Device>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // If the device_ids array is empty, return an empty vector
        if device_ids.is_empty() {
            return Ok(Vec::new());
        }

        let device_models = devices::table
            .filter(devices::id.eq_any(device_ids))
            .load::<DeviceModel>(&mut *conn)
            .map_err(|e| {
                RepositoryError::DatabaseError(format!(
                    "Failed to get {} devices by IDs: {}",
                    device_ids.len(),
                    e
                ))
            })?;

        // Convert models to domain objects
        Ok(DeviceModel::to_domain_devices(device_models))
    }
}
