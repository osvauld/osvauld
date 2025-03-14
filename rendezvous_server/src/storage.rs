use serde::{Deserialize, Serialize};
use sled::{Db, Result as SledResult};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::spawn_blocking;

#[derive(Serialize, Deserialize, Debug)]
pub enum ConnectionStatus {
    Online,
    Offline,
}

impl ConnectionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConnectionStatus::Online => "online",
            ConnectionStatus::Offline => "offline",
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ClientInfo {
    pub ws_connection_id: String,
    pub connection_string: Option<String>,
    pub connection_status: ConnectionStatus,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DB_Clients {
    pub user_id: String,
    pub ws_connection_id: String,
    pub connection_string: Option<String>,
    pub connection_status: ConnectionStatus,
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
            connection_status: ConnectionStatus::Online,
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

    pub async fn get_all_clients(&self) -> sled::Result<Vec<DB_Clients>> {
        let db = self.db.clone();
        let clients = spawn_blocking(move || -> sled::Result<Vec<DB_Clients>> {
            let mut result = Vec::new();
            for item in db.iter() {
                let (_key, value) = item?;
                let client_info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                    sled::Error::ReportableBug(format!("Deserialization error: {}", e))
                })?;
                let key = String::from_utf8(_key.to_vec())
                    .expect("The IVec does not contain valid UTF-8 data");
                let db_client: DB_Clients = DB_Clients {
                    user_id: key,
                    connection_status: client_info.connection_status,
                    connection_string: client_info.connection_string,
                    ws_connection_id: client_info.ws_connection_id,
                };
                result.push(db_client);
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

    pub async fn get_client_connection_status(
        &self,
        user_id: &str,
    ) -> sled::Result<Option<ConnectionStatus>> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();

        let result = spawn_blocking(move || -> sled::Result<Option<ConnectionStatus>> {
            if let Some(value) = db.get(user_id_string.as_bytes())? {
                let client_info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                    sled::Error::ReportableBug(format!("Deserialization error: {}", e))
                })?;
                Ok(Some(client_info.connection_status))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| sled::Error::ReportableBug(format!("JoinError: {}", e)))??;
        Ok(result)
    }

    pub async fn update_connection_status(
        &self,
        user_id: &str,
        status: ConnectionStatus,
    ) -> sled::Result<()> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();

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
            info.connection_status = status;
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
}
