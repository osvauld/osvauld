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
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn find_by_id(&self, device_id: &str) -> Result<Device, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let device = devices::table
            .find(device_id)
            .first::<DeviceModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        Ok(device.into())
    }
    async fn get_devices_by_user_id(&self, user_id: &str) -> Result<Vec<Device>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let device_models = devices::table
            .filter(devices::user_id.eq(user_id))
            .order_by(devices::created_at.desc())
            .load::<DeviceModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(DeviceModel::to_domain_devices(device_models))
    }

    async fn update_last_synced_at(&self, device_id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let timestamp = Local::now().timestamp_millis();
        diesel::update(devices::table)
            .filter(devices::id.eq(device_id))
            .set(devices::last_synced_at.eq(timestamp))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
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
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
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
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(DeviceModel::to_domain_devices(device_models))
    }
}
