use crate::DbConnection;
use crate::database::schema::{
    devices, resource_keys, resource_vector_clocks, resources, share_records,
};
use crate::models::{
    DeviceModel, ResourceKeyModel, ResourceModel, ResourceVectorClockModel, ShareRecordModel,
};
use async_trait::async_trait;
use chrono::Local;
use diesel::QueryDsl;
use diesel::prelude::*;
use osvauld_core::models::{
    Device, Resource, ResourceKey, ResourceKeyPair, ResourceManifestData, ResourceSyncData,
    ResourceVectorClock, ResourceWithKey, ShareRecord,
};
use osvauld_core::repositories::{RepositoryError, ResourceRepository};
use std::collections::HashMap;

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

    async fn update_resource(&self, data: &str, resource_id: &str) -> Result<(), RepositoryError> {
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
    async fn save_resource_with_dependencies(
        &self,
        resource: &Resource,
        resource_key: &ResourceKey,
        share_record: &ShareRecord,
        vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Use a transaction to ensure all operations succeed or fail together
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // 1. Insert the resource
            let resource_model = ResourceModel::from(resource);
            diesel::insert_into(resources::table)
                .values(&resource_model)
                .execute(conn)?;

            // 2. Insert the resource key
            let resource_key_model = ResourceKeyModel::from(resource_key);
            diesel::insert_into(resource_keys::table)
                .values(&resource_key_model)
                .execute(conn)?;

            // 3. Insert the share record
            let share_record_model = ShareRecordModel::from(share_record);
            diesel::insert_into(share_records::table)
                .values(&share_record_model)
                .execute(conn)?;

            // 4. Insert vector clocks if any
            if !vector_clocks.is_empty() {
                let vector_clock_models =
                    ResourceVectorClockModel::from_domain_vector_clocks(vector_clocks);
                diesel::insert_into(resource_vector_clocks::table)
                    .values(&vector_clock_models)
                    .execute(conn)?;
            }

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn share_resource_transaction(
        &self,
        resource_key: &ResourceKey,
        share_record: &ShareRecord,
        recipient_vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Use a transaction to ensure all operations succeed or fail together
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // 1. Save the resource key for the recipient
            let resource_key_model = ResourceKeyModel::from(resource_key);
            diesel::insert_into(resource_keys::table)
                .values(&resource_key_model)
                .execute(conn)?;

            // 2. Save the share record
            let share_record_model = ShareRecordModel::from(share_record);
            diesel::insert_into(share_records::table)
                .values(&share_record_model)
                .execute(conn)?;

            // 3. Save the vector clocks for recipient devices (if any)
            if !recipient_vector_clocks.is_empty() {
                let vector_clock_models =
                    ResourceVectorClockModel::from_domain_vector_clocks(recipient_vector_clocks);
                diesel::insert_into(resource_vector_clocks::table)
                    .values(&vector_clock_models)
                    .execute(conn)?;
            }

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }
    async fn add_device_with_vector_clocks(
        &self,
        device: &Device,
        vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Use a transaction to ensure all operations succeed or fail together
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // 1. Insert the device
            let device_model = DeviceModel::from(device);
            diesel::insert_into(devices::table)
                .values(&device_model)
                .execute(conn)?;

            // 2. Insert vector clocks if any
            if !vector_clocks.is_empty() {
                let vector_clock_models =
                    ResourceVectorClockModel::from_domain_vector_clocks(vector_clocks);
                diesel::insert_into(resource_vector_clocks::table)
                    .values(&vector_clock_models)
                    .execute(conn)?;
            }

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }
    async fn get_resource_manifest_data(
        &self,
        resource_ids: Option<&[String]>,
    ) -> Result<Vec<ResourceManifestData>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Get resource IDs based on parameter
        let target_resource_ids: Vec<String> = match resource_ids {
            Some(ids) => {
                if ids.is_empty() {
                    return Ok(Vec::new());
                }
                ids.to_vec()
            }
            None => {
                // Get all resource IDs (non-deleted) if no specific IDs provided
                resources::table
                    .filter(resources::deleted.eq(false))
                    .select(resources::id)
                    .load::<String>(&mut *conn)
                    .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
            }
        };

        if target_resource_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Get all share records for these resources in one query
        let share_records: Vec<(String, String)> = share_records::table
            .filter(share_records::resource_id.eq_any(&target_resource_ids))
            .select((share_records::resource_id, share_records::id))
            .load::<(String, String)>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Get all vector clocks for these resources in one query
        let vector_clock_models: Vec<ResourceVectorClockModel> = resource_vector_clocks::table
            .filter(resource_vector_clocks::resource_id.eq_any(&target_resource_ids))
            .select(ResourceVectorClockModel::as_select())
            .load::<ResourceVectorClockModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Group share records by resource_id
        let mut share_records_map: HashMap<String, Vec<String>> = HashMap::new();
        for (resource_id, share_record_id) in share_records {
            share_records_map
                .entry(resource_id)
                .or_insert_with(Vec::new)
                .push(share_record_id);
        }

        // Group vector clocks by resource_id
        let mut vector_clocks_map: HashMap<String, Vec<ResourceVectorClock>> = HashMap::new();
        for vector_clock_model in vector_clock_models {
            let resource_id = vector_clock_model.resource_id.clone();
            vector_clocks_map
                .entry(resource_id)
                .or_insert_with(Vec::new)
                .push(vector_clock_model.into());
        }

        // Build the final result
        let result: Vec<ResourceManifestData> = target_resource_ids
            .into_iter()
            .map(|resource_id| {
                let share_record_ids = share_records_map.remove(&resource_id).unwrap_or_default();
                let vector_clocks = vector_clocks_map.remove(&resource_id).unwrap_or_default();
                ResourceManifestData {
                    resource_id,
                    share_record_ids,
                    vector_clocks,
                }
            })
            .collect();

        Ok(result)
    }

    async fn get_resource_sync_data(
        &self,
        resource_id: &str,
    ) -> Result<ResourceSyncData, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // 1. Get the resource
        let resource_model = resources::table
            .filter(resources::id.eq(resource_id))
            .first::<ResourceModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        // 2. Get resource keys belonging to this resource
        let resource_key_models = ResourceKeyModel::belonging_to(&resource_model)
            .load::<ResourceKeyModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // 3. Get share records belonging to this resource
        let share_record_models = ShareRecordModel::belonging_to(&resource_model)
            .load::<ShareRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // 4. Get vector clocks belonging to this resource
        let vector_clock_models = ResourceVectorClockModel::belonging_to(&resource_model)
            .load::<ResourceVectorClockModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // 5. Convert to domain objects
        let resource: Resource = resource_model.into();
        let resource_keys: Vec<ResourceKey> =
            resource_key_models.into_iter().map(Into::into).collect();
        let share_records: Vec<ShareRecord> = share_record_models
            .into_iter()
            .map(|m| m.to_domain())
            .collect();
        let vector_clocks: Vec<ResourceVectorClock> =
            vector_clock_models.into_iter().map(Into::into).collect();

        Ok(ResourceSyncData {
            resource,
            resource_keys,
            share_records,
            vector_clocks,
        })
    }
    async fn save_resource_sync_data(
        &self,
        sync_data: &ResourceSyncData,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // 1. Insert the resource
            let resource_model = ResourceModel::from(&sync_data.resource);
            diesel::insert_into(resources::table)
                .values(&resource_model)
                .on_conflict(resources::id)
                .do_nothing()
                .execute(conn)?;

            // 2. Insert resource keys
            for resource_key in &sync_data.resource_keys {
                let resource_key_model = ResourceKeyModel::from(resource_key);
                diesel::insert_into(resource_keys::table)
                    .values(&resource_key_model)
                    .on_conflict(resource_keys::id)
                    .do_nothing()
                    .execute(conn)?;
            }

            // 3. Insert share records
            for share_record in &sync_data.share_records {
                let share_record_model = ShareRecordModel::from(share_record);
                diesel::insert_into(share_records::table)
                    .values(&share_record_model)
                    .on_conflict(share_records::id)
                    .do_nothing()
                    .execute(conn)?;
            }

            // 4. Insert vector clocks
            for vector_clock in &sync_data.vector_clocks {
                let vector_clock_model = ResourceVectorClockModel::from(vector_clock);
                diesel::insert_into(resource_vector_clocks::table)
                    .values(&vector_clock_model)
                    .on_conflict(resource_vector_clocks::id)
                    .do_nothing()
                    .execute(conn)?;
            }

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
