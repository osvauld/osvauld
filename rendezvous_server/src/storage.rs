use serde::{Deserialize, Serialize};
use sled::{Db, Result as SledResult};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::spawn_blocking;

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientInfo {
    pub ws_connection_id: String,
    pub connection_string: Option<String>,
}

pub struct Storage {
    db: Arc<Db>,
    client_id: Arc<Mutex<Option<String>>>,
}

impl Storage {
    pub fn new(db_path: &str) -> SledResult<Self> {
        let db = sled::open(db_path)?;
        Ok(Self {
            db: Arc::new(db),
            client_id: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn save_client(&self, user_id: &str, ws_connection_id: &str) -> sled::Result<()> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();
        let ws_connection_id_string = ws_connection_id.to_string();
        let client_info = ClientInfo {
            ws_connection_id: ws_connection_id_string.clone(),
            connection_string: None,
        };

        let serialized = serde_json::to_vec(&client_info)
            .map_err(|e| sled::Error::ReportableBug(format!("Serialization error: {}", e)))?;

        {
            let mut lock = self.client_id.lock().await;
            *lock = Some(user_id_string.clone());
        }

        spawn_blocking(move || -> sled::Result<()> {
            db.insert(user_id_string.as_bytes(), serialized)?;
            Ok(())
        })
        .await
        .map_err(|e| sled::Error::ReportableBug(format!("JoinError: {}", e)))??;
        print!("successfully saved");
        Ok(())
    }

    pub async fn get_all_clients(&self) -> sled::Result<Vec<ClientInfo>> {
        let db = self.db.clone();
        let clients = spawn_blocking(move || -> sled::Result<Vec<ClientInfo>> {
            let mut result = Vec::new();
            for item in db.iter() {
                let (_key, value) = item?;
                let client_info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                    sled::Error::ReportableBug(format!("Deserialization error: {}", e))
                })?;
                result.push(client_info);
            }
            Ok(result)
        })
        .await
        .map_err(|e| sled::Error::ReportableBug(format!("JoinError: {}", e)))??;

        Ok(clients)
    }

    pub async fn save_connection_string(
        &self,
        user_id: &str,
        connection_string: &str,
    ) -> sled::Result<()> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();
        let new_connection_string = connection_string.to_string();

        let current = spawn_blocking({
            let db = db.clone();
            let user_id_string = user_id_string.clone();
            move || db.get(user_id_string.as_bytes())
        })
        .await
        .map_err(|e| sled::Error::ReportableBug(format!("JoinError: {}", e)))??;

        let client_info = if let Some(value) = current {
            let mut info: ClientInfo = serde_json::from_slice(&value)
                .map_err(|e| sled::Error::ReportableBug(format!("Deserialization error: {}", e)))?;
            info.connection_string = Some(new_connection_string);
            info
        } else {
            return Err(sled::Error::ReportableBug("Client record not found".into()));
        };

        let serialized = serde_json::to_vec(&client_info)
            .map_err(|e| sled::Error::ReportableBug(format!("Serialization error: {}", e)))?;

        spawn_blocking(move || -> sled::Result<()> {
            db.insert(user_id_string.as_bytes(), serialized)?;
            Ok(())
        })
        .await
        .map_err(|e| sled::Error::ReportableBug(format!("JoinError: {}", e)))??;

        Ok(())
    }

    pub async fn get_connection_string(&self, user_id: &str) -> sled::Result<Option<String>> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();

        let result = spawn_blocking(move || -> sled::Result<Option<String>> {
            if let Some(value) = db.get(user_id_string.as_bytes())? {
                let client_info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                    sled::Error::ReportableBug(format!("Deserialization error: {}", e))
                })?;
                Ok(client_info.connection_string)
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| sled::Error::ReportableBug(format!("JoinError: {}", e)))??;
        Ok(result)
    }

    pub async fn get_ws_connection_id(&self, user_id: &str) -> sled::Result<Option<String>> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();

        let result = spawn_blocking(move || -> sled::Result<Option<String>> {
            if let Some(value) = db.get(user_id_string.as_bytes())? {
                let client_info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                    sled::Error::ReportableBug(format!("Deserialization error: {}", e))
                })?;
                Ok(Some(client_info.ws_connection_id))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| sled::Error::ReportableBug(format!("JoinError: {}", e)))??;
        Ok(result)
    }
}
