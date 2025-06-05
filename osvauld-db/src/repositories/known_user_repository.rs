use crate::database::schema::users;
use crate::models::DeviceModel;
use crate::models::UserModel;
use chrono::Local;
use osvauld_core::models::device::Device;
use osvauld_core::models::user::User;
use osvauld_core::repositories::{RepositoryError, UserRepository};

use crate::DbConnection;
use async_trait::async_trait;
use diesel::associations::GroupedBy;
use diesel::prelude::*;

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
    async fn get_users_and_devices_by_user_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<(User, Vec<Device>)>, RepositoryError> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self.connection.lock().await;

        // Get users
        let user_models = users::table
            .filter(users::id.eq_any(user_ids))
            .load::<UserModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Get associated devices using the relationship
        let device_models = DeviceModel::belonging_to(&user_models)
            .load::<DeviceModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Group devices by user
        let devices_per_user = device_models.grouped_by(&user_models);

        // Convert to domain objects
        let result = user_models
            .into_iter()
            .zip(devices_per_user)
            .map(|(user_model, device_models)| {
                let user = User::from(user_model);
                let devices = DeviceModel::to_domain_devices(device_models);
                (user, devices)
            })
            .collect();

        Ok(result)
    }
}
