use crate::database::schema::resource_shares;
use crate::database::DbConnection;
use crate::domains::models::resource_share::{PermissionLevel, ResourceShare, ShareStatus};
use crate::domains::repositories::{RepositoryError, ResourceShareRepository};
use crate::persistence::models::ResourceShareModel;
use async_trait::async_trait;
use diesel::prelude::*;

pub struct SqliteResourceShareRepository {
    connection: DbConnection,
}

impl SqliteResourceShareRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl ResourceShareRepository for SqliteResourceShareRepository {
    async fn save(&self, share: &ResourceShare) -> Result<(), RepositoryError> {
        let share_model = ResourceShareModel::from(share);
        let mut conn = self.connection.lock().await;
        diesel::insert_into(resource_shares::table)
            .values(share_model)
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    // Implement the rest of the methods
}
