use sled::{Db, Result as SledResult};
use std::sync::Arc;
use tokio::task::spawn_blocking;

pub struct Storage {
    db: Arc<Db>,
}

impl Storage {
    pub fn new(db_path: &str) -> SledResult<Self> {
        let db = sled::open(db_path)?;
        Ok(Self { db: Arc::new(db) })
    }

    pub async fn save_client(&self, id: &str, value: &str) -> sled::Result<()> {
        let db = self.db.clone();
        let id = id.to_string();
        let value = value.to_string();
        spawn_blocking(move || -> sled::Result<()> {
            db.insert(id.as_bytes(), value.as_bytes())?;
            Ok(())
        })
        .await
        .map_err(|e| sled::Error::ReportableBug(format!("JoinError: {}", e)))??;
        Ok(())
    }

    pub async fn get_all_clients(&self) -> sled::Result<Vec<(String, String)>> {
        let db = self.db.clone();
        let clients = spawn_blocking(move || -> sled::Result<Vec<(String, String)>> {
            let mut result = Vec::new();
            for item in db.iter() {
                let (key, value) = item?;
                // Convert from bytes to UTF-8 strings. If conversion fails, create an error.
                let key_str = String::from_utf8(key.to_vec())
                    .map_err(|_| sled::Error::Unsupported("Invalid UTF-8 in key".into()))?;
                let value_str = String::from_utf8(value.to_vec())
                    .map_err(|_| sled::Error::Unsupported("Invalid UTF-8 in value".into()))?;
                result.push((key_str, value_str));
            }
            Ok(result)
        })
        .await
        .map_err(|e| sled::Error::ReportableBug(format!("JoinError: {}", e)))??;
        Ok(clients)
    }
}
