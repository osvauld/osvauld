use crate::DbConnection;
use crate::database::schema::{resource_keys, resources};
use crate::models::{ResourceKeyModel, ResourceModel};
use async_trait::async_trait;
use chrono::Local;
use diesel::QueryDsl;
use diesel::prelude::*;
use osvauld_core::models::resource::{Resource, ResourceKeyPair, ResourceWithKey};
use osvauld_core::models::resource_key::ResourceKey;
use osvauld_core::models::vectorClock::VectorClock;
use osvauld_core::repositories::{RepositoryError, ResourceRepository};

pub struct SqliteResourceRepository {
    connection: DbConnection,
}

impl SqliteResourceRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
    async fn query_resources_with_keys(
        &self,
        user_id: &str,
        folder_id: Option<&str>,
        favorites_only: bool,
        include_deleted: bool,
    ) -> Result<Vec<ResourceWithKey>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Build base query with join to resource_keys
        let mut query = resources::table
            .inner_join(
                resource_keys::table.on(resources::id
                    .eq(resource_keys::resource_id)
                    .and(resource_keys::user_id.eq(user_id))),
            )
            .into_boxed();

        // Apply optional filters
        if !include_deleted {
            query = query.filter(resources::deleted.eq(false));
        }

        if favorites_only {
            query = query.filter(resources::favourite.eq(true));
        }

        if let Some(folder) = folder_id {
            query = query.filter(resources::folder_id.eq(folder));
        }

        // Execute query
        let results = query
            .select((ResourceModel::as_select(), resource_keys::encrypted_key))
            .order_by(resources::last_accessed.desc())
            .load::<(ResourceModel, String)>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Convert to domain objects
        let resources_with_keys = results
            .into_iter()
            .map(|(resource_model, encrypted_key)| ResourceWithKey {
                resource: resource_model.into(),
                encrypted_key,
            })
            .collect();

        Ok(resources_with_keys)
    }
}

#[async_trait]
impl ResourceRepository for SqliteResourceRepository {
    async fn save(&self, resource: &Resource) -> Result<(), RepositoryError> {
        let resource_model = ResourceModel::from(resource);
        let mut conn = self.connection.lock().await;
        diesel::insert_into(resources::table)
            .values(resource_model)
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn delete_resource(&self, id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        diesel::delete(resources::table)
            .filter(resources::id.eq(id))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn soft_delete_resource(&self, id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let now = Local::now().timestamp_millis();
        diesel::update(resources::table)
            .filter(resources::id.eq(id))
            .set((
                // Set the deleted flag to true
                resources::deleted.eq(true),
                // Record when the deletion happened
                resources::deleted_at.eq(Some(now)),
                // Update the updated_at timestamp to track the change
                resources::updated_at.eq(now),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn toggle_fav(&self, resource_id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let now = Local::now().timestamp_millis();

        // First get current favorite status
        let current_favourite: bool = resources::table
            .select(resources::favourite)
            .filter(resources::id.eq(resource_id))
            .first(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        // Toggle favorite status
        diesel::update(resources::table)
            .filter(resources::id.eq(resource_id))
            .set((
                resources::favourite.eq(!current_favourite),
                resources::updated_at.eq(now),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn update_last_accessed(&self, resource_id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let now = Local::now().timestamp_millis();

        diesel::update(resources::table)
            .filter(resources::id.eq(resource_id))
            .set(resources::last_accessed.eq(now))
            .execute(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        Ok(())
    }

    async fn update_resource(
        &self,
        data: String,
        resource_id: String,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let now = Local::now().timestamp_millis();

        diesel::update(resources::table)
            .filter(resources::id.eq(resource_id))
            .set((resources::data.eq(data), resources::updated_at.eq(now)))
            .execute(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        Ok(())
    }
    async fn find_by_folder(
        &self,
        folder_id: &str,
        user_id: &str,
    ) -> Result<Vec<ResourceWithKey>, RepositoryError> {
        self.query_resources_with_keys(user_id, Some(folder_id), false, false)
            .await
    }

    async fn find_all_by_folder(
        &self,
        folder_id: &str,
        user_id: &str,
    ) -> Result<Vec<ResourceWithKey>, RepositoryError> {
        // This includes deleted resources
        self.query_resources_with_keys(user_id, Some(folder_id), false, true)
            .await
    }

    async fn find_by_id(
        &self,
        id: &str,
        user_id: &str,
    ) -> Result<ResourceWithKey, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Query for the specific resource with join to user's key
        let result = resources::table
            .inner_join(
                resource_keys::table.on(resources::id
                    .eq(resource_keys::resource_id)
                    .and(resource_keys::user_id.eq(user_id))),
            )
            .filter(resources::id.eq(id))
            .select((ResourceModel::as_select(), resource_keys::encrypted_key))
            .first::<(ResourceModel, String)>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        // Convert to domain object
        Ok(ResourceWithKey {
            resource: result.0.into(),
            encrypted_key: result.1,
        })
    }

    async fn get_all_resources(
        &self,
        user_id: &str,
    ) -> Result<Vec<ResourceWithKey>, RepositoryError> {
        self.query_resources_with_keys(user_id, None, false, false)
            .await
    }

    async fn get_favourites(&self, user_id: &str) -> Result<Vec<ResourceWithKey>, RepositoryError> {
        self.query_resources_with_keys(user_id, None, true, false)
            .await
    }
    async fn find_resource_with_key(
        &self,
        resource_id: &str,
        user_id: &str,
    ) -> Result<ResourceKeyPair, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Get the resource (use first() to get a single result)
        let resource_model = resources::table
            .filter(resources::id.eq(resource_id))
            .first::<ResourceModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        // Get the key (use first() to get a single result)
        let key_model = resource_keys::table
            .filter(resource_keys::resource_id.eq(resource_id))
            .filter(resource_keys::user_id.eq(user_id))
            .first::<ResourceKeyModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        // Convert models to domain objects
        let resource: Resource = resource_model.into();
        let key: ResourceKey = key_model.into();

        // Return as ResourceKeyPair
        Ok(ResourceKeyPair { resource, key })
    }
    async fn save_resource_with_key(
        &self,
        resource: &Resource,
        key: &ResourceKey,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Use a transaction to ensure both operations succeed or fail together
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // Convert to models
            let resource_model = ResourceModel::from(resource);
            let resource_key_model = ResourceKeyModel::from(key);

            // Insert resource
            diesel::insert_into(resources::table)
                .values(&resource_model)
                .execute(conn)?;

            // Insert resource key
            diesel::insert_into(resource_keys::table)
                .values(&resource_key_model)
                .execute(conn)?;

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn update_resource_vector_clock(
        &self,
        resource_id: &str,
        vector_clock: &VectorClock,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let now = Local::now().timestamp_millis();

        let vector_clock_json = serde_json::to_string(vector_clock)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        diesel::update(resources::table)
            .filter(resources::id.eq(resource_id))
            .set((
                resources::vector_clock.eq(vector_clock_json),
                resources::updated_at.eq(now),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn find_by_id_raw(&self, id: &str) -> Result<Resource, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let resource_model = resources::table
            .filter(resources::id.eq(id))
            .first::<ResourceModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        Ok(resource_model.into())
    }
}
