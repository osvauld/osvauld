use crate::database::DbConnection;
use crate::database::schema::resource_vector_clocks;
use crate::models::ResourceVectorClockModel;
use async_trait::async_trait;
use chrono::Local;
use diesel::prelude::*;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::{RepositoryError, VectorClockRepository};

pub struct SqliteVectorClockRepository {
    connection: DbConnection,
}

impl SqliteVectorClockRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl VectorClockRepository for SqliteVectorClockRepository {
    async fn save_vector_clocks(
        &self,
        vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError> {
        if vector_clocks.is_empty() {
            return Ok(());
        }

        let vector_clock_models =
            ResourceVectorClockModel::from_domain_vector_clocks(vector_clocks);
        let mut conn = self.connection.lock().await;

        diesel::insert_into(resource_vector_clocks::table)
            .values(&vector_clock_models)
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn save_vector_clock(
        &self,
        vector_clock: &ResourceVectorClock,
    ) -> Result<(), RepositoryError> {
        let vector_clock_model = ResourceVectorClockModel::from(vector_clock);
        let mut conn = self.connection.lock().await;

        // Check if a record with the same device_id and resource_id already exists
        let existing_record = resource_vector_clocks::table
            .filter(resource_vector_clocks::device_id.eq(&vector_clock.device_id))
            .filter(resource_vector_clocks::resource_id.eq(&vector_clock.resource_id))
            .first::<ResourceVectorClockModel>(&mut *conn)
            .optional()
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        match existing_record {
            Some(_) => {
                // Record already exists, do nothing
                Ok(())
            }
            None => {
                // Record doesn't exist, insert new one
                diesel::insert_into(resource_vector_clocks::table)
                    .values(&vector_clock_model)
                    .execute(&mut *conn)
                    .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

                Ok(())
            }
        }
    }

    async fn increment_vector_clock(
        &self,
        resource_id: &str,
        device_id: &str,
    ) -> Result<ResourceVectorClock, RepositoryError> {
        let now = Local::now().timestamp_millis();
        let mut conn = self.connection.lock().await;

        // Start a transaction
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // Get the current vector clock
            let result = resource_vector_clocks::table
                .filter(resource_vector_clocks::resource_id.eq(resource_id))
                .filter(resource_vector_clocks::device_id.eq(device_id))
                .first::<ResourceVectorClockModel>(conn)
                .optional()?;

            match result {
                Some(mut model) => {
                    // Increment existing clock
                    model.clock_value += 1;
                    model.updated_at = now;

                    diesel::update(resource_vector_clocks::table)
                        .filter(resource_vector_clocks::id.eq(&model.id))
                        .set((
                            resource_vector_clocks::clock_value.eq(model.clock_value),
                            resource_vector_clocks::updated_at.eq(now),
                        ))
                        .execute(conn)?;

                    Ok(model)
                }
                None => {
                    // Create new clock entry with value 1
                    let new_clock = ResourceVectorClock::initialize(resource_id, device_id);
                    let model = ResourceVectorClockModel::from(&new_clock);

                    diesel::insert_into(resource_vector_clocks::table)
                        .values(&model)
                        .execute(conn)?;

                    Ok(model)
                }
            }
        })
        .map_err(|e| match e {
            diesel::result::Error::NotFound => RepositoryError::NotFound,
            _ => RepositoryError::DatabaseError(e.to_string()),
        })
        .map(ResourceVectorClockModel::into)
    }

    async fn get_vector_clocks_for_resource(
        &self,
        resource_id: &str,
    ) -> Result<Vec<ResourceVectorClock>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let models = resource_vector_clocks::table
            .filter(resource_vector_clocks::resource_id.eq(resource_id))
            .select(ResourceVectorClockModel::as_select())
            .load::<ResourceVectorClockModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(ResourceVectorClockModel::to_domain_vector_clocks(models))
    }

    async fn get_vector_clock(
        &self,
        resource_id: &str,
        device_id: &str,
    ) -> Result<ResourceVectorClock, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let model = resource_vector_clocks::table
            .filter(resource_vector_clocks::resource_id.eq(resource_id))
            .filter(resource_vector_clocks::device_id.eq(device_id))
            .select(ResourceVectorClockModel::as_select())
            .first::<ResourceVectorClockModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        Ok(model.into())
    }
    async fn check_if_device_needs_update(
        &self,
        resource_ids: &[String],
        last_synced_at: i64,
        device_id: &str,
    ) -> Result<bool, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Check if there are any vector clocks for the given resources
        // that were updated after last_synced_at by devices other than the current one
        let count: i64 = resource_vector_clocks::table
            .filter(resource_vector_clocks::resource_id.eq_any(resource_ids))
            .filter(resource_vector_clocks::updated_at.gt(last_synced_at))
            .filter(resource_vector_clocks::device_id.ne(device_id))
            .count()
            .get_result(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(count > 0)
    }

    async fn get_resource_ids_needing_updates(
        &self,
        resource_ids: &[String],
        last_synced_at: i64,
        device_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Find resources that were updated after last_synced_at by devices other than current one
        let updated_resources: Vec<String> = resource_vector_clocks::table
            .filter(resource_vector_clocks::resource_id.eq_any(resource_ids))
            .filter(resource_vector_clocks::updated_at.gt(last_synced_at))
            .filter(resource_vector_clocks::device_id.ne(device_id))
            .select(resource_vector_clocks::resource_id)
            .distinct()
            .load::<String>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(updated_resources)
    }

    async fn update_vector_clocks(
        &self,
        update_vector_clocks: &[ResourceVectorClock],
        add_vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Start a transaction to ensure atomicity
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // First, handle updates to existing vector clocks
            for clock in update_vector_clocks {
                diesel::update(resource_vector_clocks::table)
                    .filter(resource_vector_clocks::resource_id.eq(&clock.resource_id))
                    .filter(resource_vector_clocks::device_id.eq(&clock.device_id))
                    .set((
                        resource_vector_clocks::clock_value.eq(clock.clock_value as i32),
                        resource_vector_clocks::updated_at.eq(clock.updated_at),
                    ))
                    .execute(conn)?;
            }

            // Then, insert new vector clocks
            if !add_vector_clocks.is_empty() {
                let new_clock_models =
                    ResourceVectorClockModel::from_domain_vector_clocks(add_vector_clocks);

                diesel::insert_into(resource_vector_clocks::table)
                    .values(&new_clock_models)
                    .execute(conn)?;
            }

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
