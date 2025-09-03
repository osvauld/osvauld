use crate::p2p::{
    errors::{P2PError, P2PResult, SyncError},
    peer_connection::PeerConnection,
};
use osvauld_core::models::{
    Folder, FolderRecipientDiff, FolderShareRecord, FolderSyncMessage, Message,
    UnknownFoldersPayload,
};
use tracing::{debug, error, info, instrument};
impl PeerConnection {
    /// Initiates folder synchronization process after user network sync
    #[instrument(skip(self), fields(
        connection_id = %self.get_id(),
        is_initiator = self.is_initiator
    ), level = "info")]
    pub async fn start_folder_sync(&self) -> P2PResult<()> {
        info!("Starting folder sync process");

        let manifest = self.get_user_manifest_result().await?;

        // Check if we have any unknown folders to send
        if manifest.remote_missing.unknown_folders.is_empty() {
            debug!("No unknown folders to send to remote peer");

            // Still need to send empty payload to maintain protocol flow
            let empty_payload = UnknownFoldersPayload {
                folders: Vec::new(),
                folder_share_records: Vec::new(),
            };

            self.send_message(Message::FolderSync(
                FolderSyncMessage::UnknownFoldersPayload(empty_payload),
            ))
            .await?;
        } else {
            // Fetch unknown folders and their share records
            let unknown_folders_payload = self
                .create_unknown_folders_payload(&manifest.remote_missing.unknown_folders)
                .await?;

            debug!(
                "Sending {} unknown folders with {} share records",
                unknown_folders_payload.folders.len(),
                unknown_folders_payload.folder_share_records.len()
            );

            self.send_message(Message::FolderSync(
                FolderSyncMessage::UnknownFoldersPayload(unknown_folders_payload),
            ))
            .await?;
        }

        info!("Unknown folders payload sent");
        Ok(())
    }

    /// Creates payload containing unknown folders and their share records
    #[instrument(skip(self, folder_ids), fields(
        folder_count = folder_ids.len()
    ), level = "debug")]
    async fn create_unknown_folders_payload(
        &self,
        folder_ids: &[String],
    ) -> P2PResult<UnknownFoldersPayload> {
        // Fetch folders by IDs
        let folders = self
            .repo_ctx
            .folder_repo
            .get_folders_by_ids(folder_ids)
            .await
            .map_err(|e| {
                error!("Failed to fetch folders: {}", e);
                P2PError::Sync(SyncError::FolderSyncFailed {
                    reason: format!("Failed to fetch folders: {}", e),
                })
            })?;

        // Fetch share records for these folders
        let mut all_share_records = Vec::new();
        for folder_id in folder_ids {
            let share_records = self
                .repo_ctx
                .folder_share_repo
                .get_records_by_folder_id(folder_id)
                .await
                .map_err(|e| {
                    error!(
                        "Failed to fetch share records for folder {}: {}",
                        folder_id, e
                    );
                    P2PError::Sync(SyncError::FolderSyncFailed {
                        reason: format!("Failed to fetch share records: {}", e),
                    })
                })?;

            all_share_records.extend(share_records);
        }

        Ok(UnknownFoldersPayload {
            folders,
            folder_share_records: all_share_records,
        })
    }

    /// Processes folder sync messages
    #[instrument(skip(self, message), fields(
        connection_id = %self.get_id(),
        message_type = ?std::mem::discriminant(message)
    ), level = "info")]
    pub async fn process_folder_sync_message(&self, message: &FolderSyncMessage) -> P2PResult<()> {
        match message {
            FolderSyncMessage::UnknownFoldersPayload(payload) => {
                self.process_unknown_folders_payload(payload).await
            }
            FolderSyncMessage::UnknownFoldersAck => self.process_unknown_folders_ack().await,
            FolderSyncMessage::FolderRecipientSyncPayload(payload) => {
                self.process_folder_recipient_sync(payload).await
            }
            FolderSyncMessage::FolderRecipientSyncAck => {
                self.process_folder_recipient_sync_ack().await
            }
            FolderSyncMessage::FolderSyncComplete => self.process_folder_sync_complete().await,
        }
    }

    /// Processes received unknown folders payload
    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
        folder_count = payload.folders.len(),
        share_record_count = payload.folder_share_records.len()
    ), level = "info")]
    async fn process_unknown_folders_payload(
        &self,
        payload: &UnknownFoldersPayload,
    ) -> P2PResult<()> {
        info!("Processing unknown folders payload");

        // Save received folders
        if !payload.folders.is_empty() {
            self.repo_ctx
                .folder_repo
                .add_folders_bulk(&payload.folders)
                .await
                .map_err(|e| {
                    error!("Failed to save folders: {}", e);
                    P2PError::Sync(SyncError::FolderSyncFailed {
                        reason: format!("Failed to save folders: {}", e),
                    })
                })?;

            debug!("Saved {} folders", payload.folders.len());
        }

        // Save folder share records
        if !payload.folder_share_records.is_empty() {
            self.repo_ctx
                .folder_share_repo
                .add_folder_share_records_bulk(&payload.folder_share_records)
                .await
                .map_err(|e| {
                    error!("Failed to save folder share records: {}", e);
                    P2PError::Sync(SyncError::FolderSyncFailed {
                        reason: format!("Failed to save folder share records: {}", e),
                    })
                })?;

            debug!(
                "Saved {} folder share records",
                payload.folder_share_records.len()
            );
        }

        // Now send our own unknown folders
        let manifest = self.get_user_manifest_result().await?;

        if manifest.remote_missing.unknown_folders.is_empty() {
            debug!("No unknown folders to send back");

            let empty_payload = UnknownFoldersPayload {
                folders: Vec::new(),
                folder_share_records: Vec::new(),
            };

            self.send_message(Message::FolderSync(
                FolderSyncMessage::UnknownFoldersPayload(empty_payload),
            ))
            .await?;
        } else {
            let our_unknown_folders = self
                .create_unknown_folders_payload(&manifest.remote_missing.unknown_folders)
                .await?;

            debug!(
                "Sending our {} unknown folders with {} share records",
                our_unknown_folders.folders.len(),
                our_unknown_folders.folder_share_records.len()
            );

            self.send_message(Message::FolderSync(
                FolderSyncMessage::UnknownFoldersPayload(our_unknown_folders),
            ))
            .await?;
        }

        info!("Unknown folders exchange completed");
        Ok(())
    }
}
