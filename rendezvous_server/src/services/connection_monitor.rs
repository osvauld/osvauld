use crate::error::AppError;
use crate::models::{Clients, UserClientMappings};
use crate::services::connection_service::ConnectionStatus;
use crate::storage::Storage;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;
use tracing::{error, info};

pub async fn start_connection_monitor(
    clients: Clients,
    storage: Arc<Storage>,
    user_mappings: UserClientMappings,
    check_interval: Duration,
    timeout: Duration,
) {
    let mut interval = time::interval(check_interval);

    info!("Starting websocket connection monitor");

    loop {
        interval.tick().await;
        if let Err(e) = check_connections(
            clients.clone(),
            storage.clone(),
            user_mappings.clone(),
            timeout,
        )
        .await
        {
            error!("Error checking connections: {}", e);
        }
    }
}

async fn check_connections(
    clients: Clients,
    storage: Arc<Storage>,
    user_mappings: UserClientMappings,
    timeout: Duration,
) -> Result<(), AppError> {
    info!("Checking connection status for all clients");

    let now = Instant::now();
    let mut disconnected_clients = Vec::new();

    {
        let clients_lock = clients.lock().await;
        for (client_id, client_info) in clients_lock.iter() {
            if now.duration_since(client_info.last_activity) > timeout {
                disconnected_clients.push(client_id.clone());
            }
        }
    }

    if !disconnected_clients.is_empty() {
        info!("Found {} inactive connections", disconnected_clients.len());

        let mappings_lock = user_mappings.lock().await;

        for client_id in disconnected_clients {
            if let Some(mapping) = mappings_lock.iter().find(|m| m.client_id == client_id) {
                match storage
                    .update_connection_status(&mapping.user_id, ConnectionStatus::Offline)
                    .await
                {
                    Ok(_) => info!("Marked user {} as offline", mapping.user_id),
                    Err(e) => error!("Error updating connection status: {}", e),
                }
            }
        }
    }

    Ok(())
}
