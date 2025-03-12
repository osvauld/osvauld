use crate::p2p::{service::P2PService, P2PEvent};
use iroh::endpoint::Connection;
use log::{error, info};
use osvauld_core::models::p2p::Message;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

impl P2PService {
    pub async fn send_chat_message(&self, message: String) -> Result<(), String> {
        let msg = Message::Chat(message);
        let serialized = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        self.send_message(serialized).await?;
        Ok(())
    }

    pub async fn send_message(&self, message: String) -> Result<(), String> {
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or("P2P not initialized")?;
        let connection: Arc<Connection> = {
            let active_conn = state.active_connection.lock().await;
            match &*active_conn {
                Some(conn) => Arc::clone(conn),
                None => return Err("No active connection".to_string()),
            }
        };

        let (mut send, _) = connection
            .open_bi()
            .await
            .map_err(|e| format!("Failed to open bi-directional stream: {}", e))?;

        // Write the message in chunks to handle large payloads
        const CHUNK_SIZE: usize = 8192;
        let bytes = message.as_bytes();

        for chunk in bytes.chunks(CHUNK_SIZE) {
            send.write_all(chunk)
                .await
                .map_err(|e| format!("Failed to write chunk: {}", e))?;
        }

        send.finish()
            .map_err(|e| format!("Failed to finish sending: {}", e))?;

        Ok(())
    }

    pub async fn handle_messages(&self) {
        info!("Starting message listener");
        loop {
            match self.get_active_connection().await {
                Ok(connection) => {
                    match connection.accept_bi().await {
                        Ok((_send, mut recv)) => {
                            // Use a dynamic buffer that can grow as needed
                            let mut buffer = Vec::new();
                            let mut temp_buffer = vec![0u8; 8192]; // Larger temp buffer for reading chunks

                            // Read the entire message
                            loop {
                                match recv.read(&mut temp_buffer).await {
                                    Ok(Some(n)) if n > 0 => {
                                        buffer.extend_from_slice(&temp_buffer[..n]);

                                        // Try to parse what we have so far
                                        if let Ok(message_str) = String::from_utf8(buffer.clone()) {
                                            match serde_json::from_str::<Message>(&message_str) {
                                                Ok(message) => {
                                                    info!("Successfully deserialized message");
                                                    let result = match &message {
                                                        Message::SyncRequest => {
                                                            self.handle_sync_request().await
                                                        }
                                                        Message::SyncAck(updated_data) => {
                                                            self.handle_sync_ack(
                                                                updated_data.clone(),
                                                            )
                                                            .await
                                                        }
                                                        Message::AddDevice(records) => {
                                                            self.handle_add_device_request(
                                                                records.clone(),
                                                            )
                                                            .await
                                                        }
                                                        Message::AddDeviceAck => {
                                                            let _ = self.start_device_sync().await;
                                                            Ok(())
                                                        }
                                                        Message::SyncResponse(payload) => {
                                                            info!(
                                                                "Received sync payload: {:?}",
                                                                payload
                                                            );
                                                            self.handle_sync_response(
                                                                payload.clone(),
                                                            )
                                                            .await
                                                        }
                                                        Message::SyncComplete => {
                                                            self.handle_sync_complete().await
                                                        }
                                                        Message::Chat(_)
                                                        | Message::Ping
                                                        | Message::Pong => {
                                                            // Message was previously emitted
                                                            Ok(())
                                                        }
                                                        Message::AckComplete(
                                                            device_sync_record_id,
                                                        ) => {
                                                            self.ack_complete(
                                                                device_sync_record_id.clone(),
                                                            )
                                                            .await
                                                        }
                                                        Message::SyncEvent { event, payload } => {
                                                            self.handle_sync_event(
                                                                event,
                                                                payload.clone(),
                                                            )
                                                            .await
                                                        }
                                                        Message::FirstUserConnection(user) => {
                                                            self.handle_first_user_connection(user)
                                                                .await
                                                        }
                                                        Message::UserAddAck(user_id) => {
                                                            self.handle_user_add_ack(user_id).await;
                                                            Ok(())
                                                        }
                                                        Message::SharePayload(payload) => {
                                                            self.handle_share_payload(payload).await
                                                        }
                                                        _ => Ok(()),
                                                    };

                                                    if let Err(e) = result {
                                                        error!("Error handling message: {}", e);
                                                    }
                                                    break;
                                                }
                                                Err(e) if e.is_eof() => {
                                                    // Need more data, continue reading
                                                    continue;
                                                }
                                                Err(e) => {
                                                    error!("Failed to deserialize message: {}", e);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    Ok(Some(_)) => continue, // Got some data, but need more
                                    Ok(None) => {
                                        info!("Connection closed by peer");
                                        break;
                                    }
                                    Err(e) => {
                                        error!("Error reading from connection: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to accept bi-directional stream: {}", e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get active connection: {}", e);
                    break;
                }
            }
        }
        info!("Message listener stopped");
    }

    pub async fn send_snapshot(&self, snapshot: String) -> Result<(), String> {
        let msg = Message::SyncEvent {
            event: "sync-snapshot".to_string(),
            payload: snapshot,
        };
        let serialized = serde_json::to_string(&msg)
            .map_err(|e| format!("Failed to serialize AddDevice message: {}", e))?;
        self.send_message(serialized).await?;
        Ok(())
    }

    // This method would need to be implemented by consumers of the library
    pub async fn handle_sync_event(&self, event_name: &str, payload: String) -> Result<(), String> {
        log::info!(
            "Handling sync event '{}' with payload size: {}",
            event_name,
            payload.len()
        );

        // No event emitting here, just log it
        match event_name {
            "sync-update" | "sync-snapshot" => {
                log::info!("Received sync event: {}", event_name);
                self.event_emitter.emit(P2PEvent::EditingEvent { payload });
                Ok(())
            }
            _ => {
                let err = format!("Unknown sync event type: {}", event_name);
                log::error!("{}", err);
                Err(err)
            }
        }
    }
}
