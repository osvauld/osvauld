use crate::database::schema::{folders, resources};
use crate::domains::models::folder::Folder;
use crate::domains::repositories::{FolderRepository, RepositoryError};
use crate::persistence::models::FolderModel;
use crate::DbConnection;
use async_trait::async_trait;
use chrono::Local;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
pub struct SqliteFolderRepository {
    connection: DbConnection,
}

impl SqliteFolderRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl FolderRepository for SqliteFolderRepository {
    async fn save(&self, folder: &Folder) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let folder_model = FolderModel::from(folder);

        diesel::insert_into(folders::table)
            .values(&folder_model)
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn find_all(&self) -> Result<Vec<Folder>, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let folder_models = folders::table
            .filter(folders::deleted.eq(false))
            .load::<FolderModel>(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(folder_models.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(&self, id: &str) -> Result<Folder, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let folder_model = folders::table
            .find(id)
            .first::<FolderModel>(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        Ok(folder_model.into())
    }

    async fn soft_delete(&self, folder_id: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let now = Local::now().timestamp_millis();

        // Start a transaction
        conn.transaction(|conn| -> Result<(), DieselError> {
            // First soft delete all resources in the folder
            diesel::update(resources::table)
                .filter(resources::folder_id.eq(folder_id))
                .set((
                    resources::deleted.eq(true),
                    resources::deleted_at.eq(Some(now)),
                    resources::updated_at.eq(now),
                ))
                .execute(conn)?;

            // Then soft delete the folder
            diesel::update(folders::table)
                .filter(folders::id.eq(folder_id))
                .set((
                    folders::deleted.eq(true),
                    folders::deleted_at.eq(Some(now)),
                    folders::updated_at.eq(now),
                ))
                .execute(conn)?;

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
