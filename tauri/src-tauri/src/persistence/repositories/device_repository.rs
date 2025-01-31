use crate::database::schema::devices;
use crate::database::DbConnection;
use crate::domains::models::device::Device;
use crate::domains::repositories::{DeviceRepository, RepositoryError};
use crate::persistence::models::DeviceModel;
use async_trait::async_trait;
use chrono::Local;
use diesel::prelude::*;

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
    async fn get_all_devices(&self) -> Result<Vec<Device>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let device_models = devices::table
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
    async fn get_devices_except(
        &self,
        exclude_ids: &[String],
    ) -> Result<Vec<Device>, RepositoryError> {
        use crate::database::schema::devices::dsl::*;

        let conn = &mut self.connection.lock().await;

        devices
            .filter(id.ne_all(exclude_ids))
            .load::<DeviceModel>(conn)
            .map(|device_models| {
                device_models
                    .into_iter()
                    .map(|model| Device {
                        id: model.id,
                        device_key: model.device_key,
                        created_at: model.created_at,
                        updated_at: model.updated_at,
                        last_synced_at: model.last_synced_at,
                    })
                    .collect()
            })
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }
}
