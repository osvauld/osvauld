use crate::database::DbConnection;
use crate::database::schema::store_items;
use async_trait::async_trait;
use chrono::Local;
use diesel::prelude::*;
use osvauld_core::models::auth::Certificate;
use osvauld_core::repositories::{RepositoryError, StoreRepository};

pub struct SqliteStoreRepository {
    connection: DbConnection,
}

impl SqliteStoreRepository {
    pub fn new(connection: DbConnection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl StoreRepository for SqliteStoreRepository {
    async fn store_certificate(
        &self,
        certificate: &Certificate,
        certificate_key: String,
        salt_key: String,
    ) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let now = Local::now().timestamp_millis();

        // Use a transaction to ensure both operations succeed or fail together
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // Store the private key
            diesel::insert_into(store_items::table)
                .values((
                    store_items::key.eq(&certificate_key),
                    store_items::value.eq(&certificate.private_key),
                    store_items::updated_at.eq(now),
                ))
                .on_conflict(store_items::key)
                .do_update()
                .set((
                    store_items::value.eq(&certificate.private_key),
                    store_items::updated_at.eq(now),
                ))
                .execute(conn)?;

            // Store the salt
            diesel::insert_into(store_items::table)
                .values((
                    store_items::key.eq(&salt_key),
                    store_items::value.eq(&certificate.salt),
                    store_items::updated_at.eq(now),
                ))
                .on_conflict(store_items::key)
                .do_update()
                .set((
                    store_items::value.eq(&certificate.salt),
                    store_items::updated_at.eq(now),
                ))
                .execute(conn)?;

            Ok(())
        })
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn get_certificate(
        &self,
        certificate_key: String,
        salt_key: String,
    ) -> Result<Certificate, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Get the private key
        let private_key: String = store_items::table
            .filter(store_items::key.eq(&certificate_key))
            .select(store_items::value)
            .first(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        // Get the salt
        let salt: String = store_items::table
            .filter(store_items::key.eq(&salt_key))
            .select(store_items::value)
            .first(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        Ok(Certificate {
            private_key,
            public_key: String::new(), // This will be generated when needed
            salt,
        })
    }

    async fn is_signed_up(&self) -> Result<bool, RepositoryError> {
        let mut conn = self.connection.lock().await;

        // Check if the primary_key exists
        let count: i64 = store_items::table
            .filter(store_items::key.eq("primary_key"))
            .count()
            .get_result(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(count > 0)
    }

    async fn store_device_key(&self, device_key: &str) -> Result<(), RepositoryError> {
        let mut conn = self.connection.lock().await;
        let now = Local::now().timestamp_millis();

        diesel::insert_into(store_items::table)
            .values((
                store_items::key.eq("device_id"),
                store_items::value.eq(device_key),
                store_items::updated_at.eq(now),
            ))
            .on_conflict(store_items::key)
            .do_update()
            .set((
                store_items::value.eq(device_key),
                store_items::updated_at.eq(now),
            ))
            .execute(&mut *conn)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn get_device_key(&self) -> Result<String, RepositoryError> {
        let mut conn = self.connection.lock().await;

        let device_key: String = store_items::table
            .filter(store_items::key.eq("device_id"))
            .select(store_items::value)
            .first(&mut *conn)
            .map_err(|e| match e {
                diesel::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::DatabaseError(e.to_string()),
            })?;

        Ok(device_key)
    }
}

