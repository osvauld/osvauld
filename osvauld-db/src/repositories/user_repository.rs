use crate::DbConnection;
use crate::database::schema::{devices, store_items, users};
use crate::models::{DeviceModel, UserModel};
use async_trait::async_trait;
use chrono::Local;
use diesel::prelude::*;
use osvauld_core::models::{Certificate, Device, User, UserWithDeviceIds};
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

    async fn complete_user_addtion(&self, user_id: &str) -> Result<(), RepositoryError> {
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

            // 5. Save device
            let device_model = DeviceModel::from(device);
            diesel::insert_into(devices::table)
                .values(&device_model)
                .execute(conn)?;

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
}
