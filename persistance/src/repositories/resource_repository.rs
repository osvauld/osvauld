use crate::DbConnection;
use crate::database::schema::{resources, share_records};
use crate::models::{ResourceModel, ShareRecordModel};
use async_trait::async_trait;
use chrono::Local;
use diesel::QueryDsl;
use diesel::prelude::*;
use log::error;
use osvauld_core::models::resource::EncryptedResource;
use osvauld_core::models::ShareRecord;
use osvauld_core::repositories::{RepositoryError, ResourceRepository};
use std::collections::HashMap;

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
    async fn save_encrypted(&self, resource: &EncryptedResource) -> Result<(), RepositoryError> {
        let resource_model = ResourceModel::from(resource);
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        diesel::insert_into(resources::table)
            .values(resource_model)
            .execute(&mut conn)
            .map_err(|e| {
                error!("Error saving resource: {}", e);
                RepositoryError::DatabaseError(format!("Failed to save resource: {}", e))
            })?;

        Ok(())
    }

    async fn find_by_folder(
        &self,
        folder_id: &str,
        _user_id: &str,
    ) -> Result<Vec<EncryptedResource>, RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let models = resources::table
            .filter(resources::folder_id.eq(folder_id))
            .load::<ResourceModel>(&mut conn)
            .map_err(|e| {
                error!("Error finding resources by folder: {}", e);
                RepositoryError::DatabaseError(format!("Failed to find resources: {}", e))
            })?;

        Ok(ResourceModel::to_domain_resources(models))
    }

    async fn find_all_by_folder(
        &self,
        folder_id: &str,
        user_id: &str,
    ) -> Result<Vec<EncryptedResource>, RepositoryError> {
        // Same as find_by_folder for now
        self.find_by_folder(folder_id, user_id).await
    }

    async fn find_by_id(&self, id: &str) -> Result<EncryptedResource, RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let model = resources::table
            .find(id)
            .first::<ResourceModel>(&mut conn)
            .map_err(|e| {
                error!("Error finding resource by id: {}", e);
                RepositoryError::NotFound
            })?;

        Ok(EncryptedResource::from(model))
    }

    async fn delete_resource(&self, id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        diesel::delete(resources::table.find(id))
            .execute(&mut conn)
            .map_err(|e| {
                error!("Error deleting resource: {}", e);
                RepositoryError::DatabaseError(format!("Failed to delete resource: {}", e))
            })?;

        Ok(())
    }

    async fn get_all_resources(
        &self,
        _user_id: &str,
    ) -> Result<Vec<EncryptedResource>, RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let models = resources::table
            .load::<ResourceModel>(&mut conn)
            .map_err(|e| {
                error!("Error loading all resources: {}", e);
                RepositoryError::DatabaseError(format!("Failed to load resources: {}", e))
            })?;

        Ok(ResourceModel::to_domain_resources(models))
    }

    async fn update_resource(&self, data: &str, encrypted_key: &str, resource_id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let now = Local::now().timestamp();

        diesel::update(resources::table.find(resource_id))
            .set((
                resources::encrypted_data.eq(data),
                resources::encrypted_key.eq(encrypted_key),
                resources::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(|e| {
                error!("Error updating resource: {}", e);
                RepositoryError::DatabaseError(format!("Failed to update resource: {}", e))
            })?;

        Ok(())
    }

    async fn get_all_resource_ids(&self) -> Result<Vec<String>, RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let ids = resources::table
            .select(resources::id)
            .load::<String>(&mut conn)
            .map_err(|e| {
                error!("Error loading resource IDs: {}", e);
                RepositoryError::DatabaseError(format!("Failed to load resource IDs: {}", e))
            })?;

        Ok(ids)
    }

    async fn get_folder_ids_for_resources(
        &self,
        resource_ids: &[String],
    ) -> Result<HashMap<String, Vec<String>>, RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let results = resources::table
            .filter(resources::id.eq_any(resource_ids))
            .select((resources::id, resources::folder_id))
            .load::<(String, String)>(&mut conn)
            .map_err(|e| {
                error!("Error loading folder IDs for resources: {}", e);
                RepositoryError::DatabaseError(format!("Failed to load folder IDs: {}", e))
            })?;

        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for (resource_id, folder_id) in results {
            map.entry(folder_id).or_insert_with(Vec::new).push(resource_id);
        }

        Ok(map)
    }

    async fn get_resource_ids_by_folder_id(
        &self,
        folder_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let ids = resources::table
            .filter(resources::folder_id.eq(folder_id))
            .select(resources::id)
            .load::<String>(&mut conn)
            .map_err(|e| {
                error!("Error loading resource IDs by folder: {}", e);
                RepositoryError::DatabaseError(format!("Failed to load resource IDs: {}", e))
            })?;

        Ok(ids)
    }

    async fn save_resource_with_share_records(
        &self,
        resource: &EncryptedResource,
        share_records: &[ShareRecord],
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.get().map_err(|e| {
            RepositoryError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let resource_model = ResourceModel::from(resource);

        // Use transaction to ensure atomicity
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // Insert resource (replace if exists)
            diesel::insert_or_ignore_into(resources::table)
                .values(&resource_model)
                .execute(conn)?;

            // Insert all share records (replace if exists)
            for share_record in share_records {
                let share_record_model = ShareRecordModel::from(share_record);
                diesel::insert_or_ignore_into(share_records::table)
                    .values(&share_record_model)
                    .execute(conn)?;
            }

            Ok(())
        })
        .map_err(|e| {
            error!(
                "Error saving resource {} with {} share records: {}",
                resource.id,
                share_records.len(),
                e
            );
            RepositoryError::DatabaseError(format!(
                "Failed to save resource with share records: {}",
                e
            ))
        })?;

        Ok(())
    }
}
