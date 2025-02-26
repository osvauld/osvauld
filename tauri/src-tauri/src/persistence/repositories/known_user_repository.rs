use crate::database::schema::users;
use crate::domains::models::user::User;
use crate::domains::repositories::{RepositoryError, UserRepository};
use crate::persistence::models::UserModel;

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
    async fn add_known_user(&self, user: User) -> Result<(), RepositoryError> {
        let user_model = UserModel::from(&user);
        let mut conn = self.connection.lock().await;
        diesel::insert_into(users::table)
            .values(user_model)
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}
