use crate::DbConnection;
use crate::database::schema::{devices, store_items, users};
use crate::models::{DeviceModel, UserModel};
use async_trait::async_trait;
use chrono::Local;
use diesel::BelongingToDsl;
use diesel::prelude::*;
use osvauld_core::models::{Certificate, Device, User, UserWithDeviceIds, UserWithDevices};
use osvauld_core::repositories::{RepositoryError, UserRepository};
use std::collections::HashMap;
pub struct SqliteUserRepository {
    connection: DbConnection,
}

impl SqliteUserRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl UserRepository for SqliteUserRepository {
    async fn add_known_user(&self, user: &User) -> Result<(), RepositoryError> {
        let user_model = UserModel::from(user);
        let mut conn = self.connection.lock().await;
        diesel::insert_into(users::table)
            .values(user_model)
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }
    async fn get_known_users(&self) -> Result<Vec<User>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let user_models = users::table
            .filter(users::owner.eq(false))
            .load::<UserModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(UserModel::to_domain_users(user_models))
    }

    async fn get_users_by_ids(&self, user_ids: &[String]) -> Result<Vec<User>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let user_models = users::table
            .filter(users::owner.eq(false))
            .filter(users::id.eq_any(user_ids))
            .load::<UserModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(UserModel::to_domain_users(user_models))
    }

    async fn get_user_by_id(&self, user_id: &str) -> Result<User, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let user_model = users::table
            .filter(users::id.eq(user_id))
            .first::<UserModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;
        let user: User = user_model.into();
        Ok(user)
    }

    async fn complete_user_addition(&self, user_id: &str) -> Result<(), RepositoryError> {
        let now = Local::now().timestamp_millis();
        let mut conn = self.connection.lock().await;
        diesel::update(users::table)
            .filter(users::id.eq(user_id))
            .set((users::first_sync.eq(true), users::updated_at.eq(now)))
            .execute(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;
        Ok(())
    }

    async fn add_known_users_bulk(&self, users: &[User]) -> Result<(), RepositoryError> {
        if users.is_empty() {
            return Ok(());
        }

        let user_models: Vec<UserModel> = users.iter().map(UserModel::from).collect();
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for user_model in &user_models {
                diesel::insert_into(users::table)
                    .values(user_model)
                    .on_conflict(users::id) // Assuming id is the primary key
                    .do_nothing() // Skip if the user already exists
                    .execute(conn)?;
            }
            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }
    async fn commit_signup_transaction(
        &self,
        user: &User,
        primary_certificate: &Certificate,
        device: &Device,
        device_certificate: &Certificate,
        peer_device: Option<&Device>,
        ucan_certificate: &Certificate,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let now = Local::now().timestamp_millis();

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // 1. Add the user
            let user_model = UserModel::from(user);
            diesel::insert_into(users::table)
                .values(user_model)
                .execute(conn)?;

            // 2. Store primary certificate
            diesel::insert_into(store_items::table)
                .values((
                    store_items::key.eq("primary_key"),
                    store_items::value.eq(&primary_certificate.private_key),
                    store_items::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(store_items::table)
                .values((
                    store_items::key.eq("primary_key_salt"),
                    store_items::value.eq(&primary_certificate.salt),
                    store_items::updated_at.eq(now),
                ))
                .execute(conn)?;

            // 3. Store device certificate
            diesel::insert_into(store_items::table)
                .values((
                    store_items::key.eq("device_key"),
                    store_items::value.eq(&device_certificate.private_key),
                    store_items::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(store_items::table)
                .values((
                    store_items::key.eq("device_key_salt"),
                    store_items::value.eq(&device_certificate.salt),
                    store_items::updated_at.eq(now),
                ))
                .execute(conn)?;

            // 4. Store device key ID
            diesel::insert_into(store_items::table)
                .values((
                    store_items::key.eq("device_id"),
                    store_items::value.eq(&device.id),
                    store_items::updated_at.eq(now),
                ))
                .execute(conn)?;

            // 3. Store ucan certificate
            diesel::insert_into(store_items::table)
                .values((
                    store_items::key.eq("ucan_key"),
                    store_items::value.eq(&ucan_certificate.private_key),
                    store_items::updated_at.eq(now),
                ))
                .execute(conn)?;
            // 5. Save device
            let device_model = DeviceModel::from(device);
            diesel::insert_into(devices::table)
                .values(&device_model)
                .execute(conn)?;
            if let Some(peer_device) = peer_device {
                let device_model = DeviceModel::from(peer_device);
                diesel::insert_into(devices::table)
                    .values(&device_model)
                    .execute(conn)?;
            }

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }
    async fn get_other_users_with_device_ids(
        &self,
    ) -> Result<Vec<UserWithDeviceIds>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Get all devices for other users (non-owner users only)
        let user_devices: Vec<(String, String)> = devices::table
            .inner_join(users::table.on(devices::user_id.eq(users::id)))
            .filter(users::owner.eq(false))
            .select((devices::user_id, devices::id))
            .load::<(String, String)>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Group device IDs by user ID
        let mut user_device_map: HashMap<String, Vec<String>> = HashMap::new();
        for (user_id, device_id) in user_devices {
            user_device_map
                .entry(user_id)
                .or_insert_with(Vec::new)
                .push(device_id);
        }

        // Convert to Vec<UserWithDeviceIds>
        let result: Vec<UserWithDeviceIds> = user_device_map
            .into_iter()
            .map(|(user_id, device_ids)| UserWithDeviceIds {
                user_id,
                device_ids,
            })
            .collect();

        Ok(result)
    }
    async fn get_users_with_devices_by_user_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<UserWithDevices>, RepositoryError> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self.connection.lock().await;

        let user_models = users::table
            .filter(users::id.eq_any(user_ids))
            .load::<UserModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        let device_models = DeviceModel::belonging_to(&user_models)
            .load::<DeviceModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        let grouped_devices = device_models.grouped_by(&user_models);

        let result: Vec<UserWithDevices> = user_models
            .into_iter()
            .zip(grouped_devices)
            .map(|(user_model, device_models)| {
                let user: User = user_model.into();
                let devices: Vec<Device> = device_models.into_iter().map(Into::into).collect();
                UserWithDevices { user, devices }
            })
            .collect();

        Ok(result)
    }
    async fn add_users_with_devices_bulk(
        &self,
        users_with_devices: &[UserWithDevices],
    ) -> Result<(), RepositoryError> {
        if users_with_devices.is_empty() {
            return Ok(());
        }

        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for user_with_devices in users_with_devices {
                // 1. Insert the user
                let user_model = UserModel::from(&user_with_devices.user);
                diesel::insert_into(users::table)
                    .values(&user_model)
                    .on_conflict(users::id)
                    .do_update()
                    .set((
                        users::ucan_pub_key.eq(&user_with_devices.user.ucan_pub_key),
                        users::ucan_token.eq(&user_with_devices.user.ucan_token),
                        users::first_sync.eq(&user_with_devices.user.first_sync),
                        users::ucan_cid.eq(&user_with_devices.user.ucan_cid),
                    ))
                    .execute(conn)?;

                // 2. Insert associated devices
                for device in &user_with_devices.devices {
                    let device_model = DeviceModel::from(device);
                    diesel::insert_into(devices::table)
                        .values(&device_model)
                        .on_conflict(devices::id)
                        .do_nothing()
                        .execute(conn)?;
                }
            }
            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }
    async fn get_user_device_mapping(
        &self,
    ) -> Result<HashMap<String, Vec<String>>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let user_devices: Vec<(String, String)> = devices::table
            .inner_join(users::table.on(devices::user_id.eq(users::id)))
            .select((devices::user_id, devices::id))
            .load::<(String, String)>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Group device IDs by user ID
        let mut user_device_map: HashMap<String, Vec<String>> = HashMap::new();
        for (user_id, device_id) in user_devices {
            user_device_map
                .entry(user_id)
                .or_insert_with(Vec::new)
                .push(device_id);
        }

        Ok(user_device_map)
    }

    async fn get_users_with_device_ids_by_user_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<UserWithDeviceIds>, RepositoryError> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self.connection.lock().await;

        // Get all devices for the specified user IDs
        let user_devices: Vec<(String, String)> = devices::table
            .inner_join(users::table.on(devices::user_id.eq(users::id)))
            .filter(users::id.eq_any(user_ids))
            .select((devices::user_id, devices::id))
            .load::<(String, String)>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Group device IDs by user ID
        let mut user_device_map: HashMap<String, Vec<String>> = HashMap::new();
        for (user_id, device_id) in user_devices {
            user_device_map
                .entry(user_id)
                .or_insert_with(Vec::new)
                .push(device_id);
        }

        // Convert to Vec<UserWithDeviceIds>
        let result: Vec<UserWithDeviceIds> = user_device_map
            .into_iter()
            .map(|(user_id, device_ids)| UserWithDeviceIds {
                user_id,
                device_ids,
            })
            .collect();

        Ok(result)
    }

    async fn get_user_by_device_id(&self, device_id: &str) -> Result<User, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let user_model = devices::table
            .inner_join(users::table.on(devices::user_id.eq(users::id)))
            .filter(devices::id.eq(device_id))
            .select(users::all_columns)
            .first::<UserModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        let user: User = user_model.into();
        Ok(user)
    }
    async fn get_ucan_by_cid(&self, cid: &str) -> Result<String, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let user_record_model = users::table
            .filter(users::ucan_cid.eq(cid))
            .first::<UserModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;
        Ok(user_record_model.ucan_token)
    }
}
