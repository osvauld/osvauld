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

    async fn save_many(&self, share_records: &[ShareRecord]) -> Result<(), RepositoryError> {
        if share_records.is_empty() {
            return Ok(());
        }
        let mut conn = self.connection.lock().await;
        let share_record_models: Vec<ShareRecordModel> =
            share_records.iter().map(ShareRecordModel::from).collect();

        // SQLite doesn't support batch insert with on_conflict, so use transaction with individual inserts
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for share_record in &share_record_models {
                diesel::insert_into(share_records::table)
                    .values(share_record)
                    .on_conflict(share_records::id)
                    .do_nothing()
                    .execute(conn)?;
            }
            Ok(())
        })
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

    async fn find_by_resource(
        &self,
        resource_id: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let share_record_models = share_records::table
            .filter(share_records::resource_id.eq(resource_id))
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
    async fn get_user_share_records(
        &self,
        user_id: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Get all share records involving this user (either as sharer or shared_with)
        let share_record_models = share_records::table
            .filter(share_records::recipient_user_id.eq(user_id))
            .load::<ShareRecordModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        let share_records = share_record_models
            .into_iter()
            .map(|model| model.to_domain())
            .collect();

        Ok(share_records)
    }
}
