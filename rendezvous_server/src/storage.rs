use crate::error::AppError;
use crate::models::ClientStatus;
use crate::services::connection_service::ConnectionStatus;
use serde::{Deserialize, Serialize};
use sled::{Db, Result as SledResult};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::spawn_blocking;
use tracing::info;

#[derive(Serialize, Deserialize, Debug)]
struct ClientInfo {
    ws_connection_id: String,
    connection_string: Option<String>,
    connection_status: ConnectionStatus,
    connection_requested: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DbClients {
    pub user_id: String,
    pub ws_connection_id: String,
    pub connection_string: Option<String>,
    pub connection_status: ConnectionStatus,
    pub connection_requested: Vec<String>,
}

pub struct Storage {
    db: Arc<Db>,
    client_id: Arc<Mutex<Option<String>>>,
}

impl Storage {
    pub fn new(db_path: &str) -> SledResult<Self> {
        let db = sled::open(db_path)?;
        info!("Storage initialized with database path: {}", db_path);
        Ok(Self {
            db: Arc::new(db),
            client_id: Arc::new(Mutex::new(None)),
        })
    }
    pub async fn save_client(&self, user_id: &str, client_id: &str) -> Result<(), AppError> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();
        let ws_connection_id_string = client_id.to_string();

        let client_info = ClientInfo {
            ws_connection_id: ws_connection_id_string.clone(),
            connection_string: None,
            connection_status: ConnectionStatus::Online,
            connection_requested: Vec::new(),
        };

        let serialized = serde_json::to_vec(&client_info).map_err(|e| {
            AppError::SerializationError(format!("Failed to serialize client info: {}", e))
        })?;

        {
            let mut lock = self.client_id.lock().await;
            *lock = Some(user_id_string.clone());
        }

        // Perform the database insert operation
        spawn_blocking(move || -> Result<(), AppError> {
            db.insert(user_id_string.as_bytes(), serialized)
                .map_err(|e| AppError::StorageError(format!("Failed to insert client: {}", e)))?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))??;

        info!(
            "Successfully saved client {} with connection ID {}",
            user_id, client_id
        );
        Ok(())
    }

    pub async fn get_ws_connection_id(&self, user_id: &str) -> Result<Option<String>, AppError> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();

        let result = spawn_blocking(move || -> Result<Option<String>, AppError> {
            match db.get(user_id_string.as_bytes()) {
                Ok(Some(value)) => {
                    let client_info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                        AppError::SerializationError(format!(
                            "Failed to deserialize client info: {}",
                            e
                        ))
                    })?;
                    Ok(Some(client_info.ws_connection_id))
                }
                Ok(None) => Ok(None),
                Err(e) => Err(AppError::StorageError(format!("Database error: {}", e))),
            }
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))??;

        Ok(result)
    }

    pub async fn get_client_details(&self, user_id: &str) -> Result<Option<DbClients>, AppError> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();

        let result = spawn_blocking(move || -> Result<Option<DbClients>, AppError> {
            match db.get(user_id_string.as_bytes()) {
                Ok(Some(value)) => {
                    let client_info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                        AppError::SerializationError(format!(
                            "Failed to deserialize client info: {}",
                            e
                        ))
                    })?;
                    Ok(Some(DbClients {
                        user_id: user_id_string,
                        ws_connection_id: client_info.ws_connection_id,
                        connection_string: client_info.connection_string,
                        connection_status: client_info.connection_status,
                        connection_requested: client_info.connection_requested,
                    }))
                }
                Ok(None) => Ok(None),
                Err(e) => Err(AppError::StorageError(format!("Database error: {}", e))),
            }
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))??;

        Ok(result)
    }

    pub async fn save_connection_string(
        &self,
        user_id: &str,
        connection_string: &str,
    ) -> Result<(), AppError> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();
        let new_connection_string = connection_string.to_string();

        let current = spawn_blocking({
            let db = db.clone();
            let user_id_string = user_id_string.clone();
            move || db.get(user_id_string.as_bytes())
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))?
        .map_err(|e| AppError::StorageError(format!("Database error: {}", e)))?;

        let client_info = if let Some(value) = current {
            let mut info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                AppError::SerializationError(format!("Failed to deserialize client info: {}", e))
            })?;
            info.connection_string = Some(new_connection_string);
            info
        } else {
            return Err(AppError::StorageError(format!(
                "Client record not found for user ID: {}",
                user_id
            )));
        };

        let serialized = serde_json::to_vec(&client_info).map_err(|e| {
            AppError::SerializationError(format!("Failed to serialize client info: {}", e))
        })?;

        spawn_blocking(move || -> Result<(), AppError> {
            db.insert(user_id_string.as_bytes(), serialized)
                .map_err(|e| {
                    AppError::StorageError(format!("Failed to update connection string: {}", e))
                })?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))??;

        info!("Successfully saved connection string for user {}", user_id);
        Ok(())
    }

    pub async fn update_connection_status(
        &self,
        user_id: &str,
        status: ConnectionStatus,
    ) -> Result<(), AppError> {
        let status = &status;
        let db = self.db.clone();
        let user_id_string = user_id.to_string();

        let current = spawn_blocking({
            let db = db.clone();
            let user_id_string = user_id_string.clone();
            move || db.get(user_id_string.as_bytes())
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))?
        .map_err(|e| AppError::StorageError(format!("Database error: {}", e)))?;

        let client_info = if let Some(value) = current {
            let mut info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                AppError::SerializationError(format!("Failed to deserialize client info: {}", e))
            })?;
            info.connection_status = status.clone();
            info
        } else {
            return Err(AppError::StorageError(format!(
                "Client record not found for user ID: {}",
                user_id
            )));
        };

        let serialized = serde_json::to_vec(&client_info).map_err(|e| {
            AppError::SerializationError(format!("Failed to serialize client info: {}", e))
        })?;

        spawn_blocking(move || -> Result<(), AppError> {
            db.insert(user_id_string.as_bytes(), serialized)
                .map_err(|e| {
                    AppError::StorageError(format!("Failed to update connection status: {}", e))
                })?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))??;

        info!(
            "Successfully updated connection status to {:?} for user {}",
            status.as_str(),
            user_id
        );
        Ok(())
    }

    pub async fn get_all_clients(&self) -> Result<Vec<ClientStatus>, AppError> {
        let db = self.db.clone();

        let clients = spawn_blocking(move || -> Result<Vec<ClientStatus>, AppError> {
            let mut result = Vec::new();
            for item in db.iter() {
                match item {
                    Ok((key, value)) => {
                        let client_info: ClientInfo =
                            serde_json::from_slice(&value).map_err(|e| {
                                AppError::SerializationError(format!(
                                    "Failed to deserialize client info: {}",
                                    e
                                ))
                            })?;

                        let user_id = String::from_utf8(key.to_vec()).map_err(|_| {
                            AppError::StorageError("Invalid UTF-8 in user ID".to_string())
                        })?;

                        result.push(ClientStatus {
                            user_id,
                            connection_status: client_info.connection_status.as_str().to_string(),
                        });
                    }
                    Err(e) => {
                        return Err(AppError::StorageError(format!(
                            "Error iterating over database: {}",
                            e
                        )));
                    }
                }
            }
            Ok(result)
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))??;

        Ok(clients)
    }

    pub async fn update_connection_requested_by(
        &self,
        user_id: &str,
        requested_by: &str,
    ) -> Result<(), AppError> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();

        let current = spawn_blocking({
            let db = db.clone();
            let user_id_string = user_id_string.clone();
            move || db.get(user_id_string.as_bytes())
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))?
        .map_err(|e| AppError::StorageError(format!("Database error: {}", e)))?;

        let client_info = if let Some(value) = current {
            let mut info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                AppError::SerializationError(format!("Failed to deserialize client info: {}", e))
            })?;
            let mut connection_requested_by = info.connection_requested;
            connection_requested_by.push(requested_by.to_string());
            info.connection_requested = connection_requested_by;
            info
        } else {
            return Err(AppError::StorageError(format!(
                "Client record not found for user ID: {}",
                user_id
            )));
        };

        let serialized = serde_json::to_vec(&client_info).map_err(|e| {
            AppError::SerializationError(format!("Failed to serialize client info: {}", e))
        })?;

        spawn_blocking(move || -> Result<(), AppError> {
            db.insert(user_id_string.as_bytes(), serialized)
                .map_err(|e| {
                    AppError::StorageError(format!("Failed to update connection string: {}", e))
                })?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    pub async fn create_new_user_for_connection_request(
        &self,
        user_id: &str,
        requested_by: &str,
    ) -> Result<(), AppError> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();
        let ws_connection_id_string = "";
        let mut connection_requested = Vec::new();
        connection_requested.push(requested_by.into());
        let client_info = ClientInfo {
            ws_connection_id: ws_connection_id_string.into(),
            connection_string: None,
            connection_status: ConnectionStatus::Offline,
            connection_requested: connection_requested,
        };

        let serialized = serde_json::to_vec(&client_info).map_err(|e| {
            AppError::SerializationError(format!("Failed to serialize client info: {}", e))
        })?;

        spawn_blocking(move || -> Result<(), AppError> {
            db.insert(user_id_string.as_bytes(), serialized)
                .map_err(|e| AppError::StorageError(format!("Failed to insert client: {}", e)))?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))??;

        info!(
            "Successfully saved client {} with connection ID {}",
            user_id, ws_connection_id_string
        );
        Ok(())
    }

    pub async fn update_client_info(&self, client_info: &DbClients) -> Result<(), AppError> {
        let db = self.db.clone();
        let user_id_string = client_info.user_id.clone();

        let current = spawn_blocking({
            let db = db.clone();
            let user_id_string = user_id_string.clone();
            move || db.get(user_id_string.as_bytes())
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))?
        .map_err(|e| AppError::StorageError(format!("Database error: {}", e)))?;

        let client_info = if let Some(value) = current {
            let mut info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                AppError::SerializationError(format!("Failed to deserialize client info: {}", e))
            })?;
            info.connection_requested = client_info.connection_requested.clone();
            info.connection_status = client_info.connection_status.clone();
            info.ws_connection_id = client_info.ws_connection_id.clone();
            info
        } else {
            return Err(AppError::StorageError(format!(
                "Client record not found for user ID: {:?}",
                client_info.clone()
            )));
        };

        let serialized = serde_json::to_vec(&client_info).map_err(|e| {
            AppError::SerializationError(format!("Failed to serialize client info: {}", e))
        })?;

        spawn_blocking(move || -> Result<(), AppError> {
            db.insert(user_id_string.as_bytes(), serialized)
                .map_err(|e| {
                    AppError::StorageError(format!("Failed to update connection string: {}", e))
                })?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    pub async fn get_client_connection_status(
        &self,
        user_id: &str,
    ) -> Result<Option<ConnectionStatus>, AppError> {
        let db = self.db.clone();
        let user_id_string = user_id.to_string();

        let result = spawn_blocking(move || -> Result<Option<ConnectionStatus>, AppError> {
            match db.get(user_id_string.as_bytes()) {
                Ok(Some(value)) => {
                    let client_info: ClientInfo = serde_json::from_slice(&value).map_err(|e| {
                        AppError::SerializationError(format!(
                            "Failed to deserialize client info: {}",
                            e
                        ))
                    })?;
                    Ok(Some(client_info.connection_status))
                }
                Ok(None) => Ok(None),
                Err(e) => Err(AppError::StorageError(format!("Database error: {}", e))),
            }
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))??;

        Ok(result)
    }

    pub async fn get_all_clients_detailes(&self) -> Result<Vec<DbClients>, AppError> {
        let db = self.db.clone();

        let clients = spawn_blocking(move || -> Result<Vec<DbClients>, AppError> {
            let mut result = Vec::new();
            for item in db.iter() {
                match item {
                    Ok((key, value)) => {
                        let client_info: ClientInfo =
                            serde_json::from_slice(&value).map_err(|e| {
                                AppError::SerializationError(format!(
                                    "Failed to deserialize client info: {}",
                                    e
                                ))
                            })?;

                        let user_id = String::from_utf8(key.to_vec()).map_err(|_| {
                            AppError::StorageError("Invalid UTF-8 in user ID".to_string())
                        })?;

                        result.push(DbClients {
                            user_id,
                            ws_connection_id: client_info.ws_connection_id,
                            connection_string: client_info.connection_string,
                            connection_status: client_info.connection_status,
                            connection_requested: client_info.connection_requested,
                        });
                    }
                    Err(e) => {
                        return Err(AppError::StorageError(format!(
                            "Error iterating over database: {}",
                            e
                        )));
                    }
                }
            }
            Ok(result)
        })
        .await
        .map_err(|e| AppError::StorageError(format!("Task join error: {}", e)))??;

        Ok(clients)
    }
}
