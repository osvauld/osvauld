use crate::database::schema::users;
use crate::models::UserModel;
use chrono::Local;
use osvauld_core::models::user::User;
use osvauld_core::repositories::{RepositoryError, UserRepository};

use crate::DbConnection;
use async_trait::async_trait;
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
}
