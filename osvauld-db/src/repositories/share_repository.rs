use crate::DbConnection;
use crate::database::schema::share_records;
use crate::models::ShareRecordModel;
use async_trait::async_trait;
use diesel::prelude::*;
use osvauld_core::models::share_record::ShareRecord;
use osvauld_core::repositories::{RepositoryError, ShareRepository};

pub struct SqliteShareRepository {
    connection: DbConnection,
}

impl SqliteShareRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl ShareRepository for SqliteShareRepository {
    async fn save(&self, share_record: &ShareRecord) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let share_record_model = ShareRecordModel::from(share_record);

        diesel::insert_into(share_records::table)
            .values(&share_record_model)
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn find_by_resource_and_operation(
        &self,
        resource_id: &str,
        operation_type: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let share_record_models = share_records::table
            .filter(share_records::resource_id.eq(resource_id))
            .filter(share_records::operation_type.eq(operation_type))
            .load::<ShareRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        // Convert database models to domain models
        let share_records = share_record_models
            .into_iter()
            .map(|model| model.to_domain())
            .collect();

        Ok(share_records)
    }

    async fn find_by_id(&self, id: &str) -> Result<ShareRecord, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let share_record_model = share_records::table
            .filter(share_records::id.eq(id))
            .first::<ShareRecordModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        // Convert database model to domain model
        Ok(share_record_model.to_domain())
    }
}
