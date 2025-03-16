use crate::database::DbConnection;
use crate::database::schema::devices;
use crate::models::DeviceModel;
use async_trait::async_trait;
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
    async fn save(&self, device: Device) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        let device_model = DeviceModel {
            id: device.id,
            device_key: device.device_key,
            user_id: device.user_id,
            created_at: device.created_at,
            updated_at: device.updated_at,
            last_synced_at: device.last_synced_at,
        };

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

    async fn udpate_last_synced_at(
        &self,
        device_id: &str,
        timestamp: i64,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
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
}
