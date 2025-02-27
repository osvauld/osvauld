use crate::database::schema::resources;
use crate::domains::models::resource::Resource;
use crate::domains::repositories::{RepositoryError, ResourceRepository};
use crate::persistence::models::ResourceModel;
use chrono::Local;

use crate::DbConnection;
use async_trait::async_trait;
use diesel::prelude::*;

pub struct SqliteResourceRepository {
    connection: DbConnection,
}

impl SqliteResourceRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
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

    async fn find_by_folder(&self, folder_id: &str) -> Result<Vec<Resource>, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let resource_models = resources::table
            .filter(resources::folder_id.eq(folder_id))
            .filter(resources::deleted.eq(false))
            .load::<ResourceModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(ResourceModel::to_domain_resources(resource_models))
    }

    async fn find_all_by_folder(&self, folder_id: &str) -> Result<Vec<Resource>, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let resource_models = resources::table
            .filter(resources::folder_id.eq(folder_id))
            .load::<ResourceModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(ResourceModel::to_domain_resources(resource_models))
    }
    async fn find_by_id(&self, id: &str) -> Result<Resource, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let resource_model = resources::table
            .find(id)
            .first::<ResourceModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            });
        Ok(resource_model?.into())
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

    async fn get_all_resources(&self) -> Result<Vec<Resource>, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let resource_models = resources::table
            .filter(resources::deleted.eq(false))
            .load::<ResourceModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(ResourceModel::to_domain_resources(resource_models))
    }

    async fn get_favourites(&self) -> Result<Vec<Resource>, RepositoryError> {
        let mut conn = self.connection.lock().await;
        let resource_models = resources::table
            .filter(resources::deleted.eq(false))
            .filter(resources::favourite.eq(true))
            .load::<ResourceModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(ResourceModel::to_domain_resources(resource_models))
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
}
