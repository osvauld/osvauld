use crate::database::DbConnection;
use crate::database::schema::resource_keys;
use crate::models::ResourceKeyModel;
use async_trait::async_trait;
use diesel::prelude::*;
use osvauld_core::models::resource_key::ResourceKey;
use osvauld_core::repositories::{RepositoryError, ResourceKeyRepository};

pub struct SqliteResourceKeyRepository {
    connection: DbConnection,
}

impl SqliteResourceKeyRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl ResourceKeyRepository for SqliteResourceKeyRepository {
    async fn save(&self, key: &ResourceKey) -> Result<(), RepositoryError> {
        let key_model = ResourceKeyModel::from(key);
        let mut conn = self.connection.lock().await;
        diesel::insert_into(resource_keys::table)
            .values(key_model)
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn add_resource_keys(&self, keys: &[ResourceKey]) -> Result<(), RepositoryError> {
        if keys.is_empty() {
            return Ok(());
        }
        let key_models: Vec<ResourceKeyModel> =
            keys.iter().map(|key| ResourceKeyModel::from(key)).collect();
        let mut conn = self.connection.lock().await;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for key_model in &key_models {
                diesel::insert_into(resource_keys::table)
                    .values(key_model)
                    .on_conflict(resource_keys::id)
                    .do_nothing()
                    .execute(conn)?;
            }
            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }
    async fn find_by_resource_id(
        &self,
        resource_id: &str,
    ) -> Result<Vec<ResourceKey>, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let key_models = resource_keys::table
            .filter(resource_keys::resource_id.eq(resource_id))
            .load::<ResourceKeyModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(ResourceKeyModel::to_domain_resource(key_models))
    }

    async fn find_by_resource_and_user(
        &self,
        resource_id: &str,
        user_id: &str,
    ) -> Result<ResourceKey, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let key_model = resource_keys::table
            .filter(resource_keys::resource_id.eq(resource_id))
            .filter(resource_keys::user_id.eq(user_id))
            .first::<ResourceKeyModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        Ok(key_model.into())
    }

    async fn delete_by_resource_id(&self, resource_id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        diesel::delete(resource_keys::table)
            .filter(resource_keys::resource_id.eq(resource_id))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn delete_by_resource_and_user(
        &self,
        resource_id: &str,
        user_id: &str,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        diesel::delete(resource_keys::table)
            .filter(resource_keys::resource_id.eq(resource_id))
            .filter(resource_keys::user_id.eq(user_id))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}
