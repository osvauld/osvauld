use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection, P2PEvent};
use osvauld_core::models::{
    Folder, FolderRecipientUpdate, FolderSyncMessage, Message, UnknownFoldersPayload,
};
use services::{
    add_missing_recipients, create_unknown_folders_payload,
    get_missing_remote_folder_share_records, process_unknown_folders_payload,
};
use tracing::{debug, info, instrument};
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
                folder_data: Vec::new(),
            };

            self.send_message(Message::FolderSync(
                FolderSyncMessage::UnknownFoldersPayload(empty_payload),
            ))
            .await?;
        } else {
            // Fetch unknown folders and their share records
            let unknown_folders_payload = create_unknown_folders_payload(
                &manifest.remote_missing.unknown_folders,
                self.repo_ctx.clone(),
            )
            .await?;

            self.send_message(Message::FolderSync(
                FolderSyncMessage::UnknownFoldersPayload(unknown_folders_payload),
            ))
            .await?;
        }

        info!("Unknown folders payload sent");
        Ok(())
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
            FolderSyncMessage::FolderRecipientSyncPayload(payload) => {
                self.process_folder_recipient_sync(payload).await
            }
        }
    }

    /// Processes received unknown folders payload
    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
    ), level = "info")]
    async fn process_unknown_folders_payload(
        &self,
        payload: &UnknownFoldersPayload,
    ) -> P2PResult<()> {
        info!("Processing unknown folders payload");
        process_unknown_folders_payload(payload, self.repo_ctx.clone()).await?;
        let new_folders = payload
            .folder_data
            .iter()
            .map(|folder_data| folder_data.folder.clone())
            .collect::<Vec<Folder>>();
        self.event_emitter.emit(P2PEvent::FoldersAdded {
            folders: new_folders,
        });
        if !self.is_initiator {
            let manifest = self.get_user_manifest_result().await?;
            let our_unknown_folders = create_unknown_folders_payload(
                &manifest.remote_missing.unknown_folders,
                self.repo_ctx.clone(),
            )
            .await?;

            self.send_message(Message::FolderSync(
                FolderSyncMessage::UnknownFoldersPayload(our_unknown_folders),
            ))
            .await?;
        } else {
            let manifest_data = self.get_user_manifest_result().await?;
            let missing_remote_fsr = get_missing_remote_folder_share_records(
                manifest_data.folders_requiring_recipient_sync,
                self.repo_ctx.clone(),
            )
            .await?;
            let message = FolderSyncMessage::FolderRecipientSyncPayload(missing_remote_fsr);
            self.send_message(Message::FolderSync(message)).await?;
        }

        info!("Unknown folders exchange completed");
        Ok(())
    }

    async fn process_folder_recipient_sync(
        &self,
        payload: &[FolderRecipientUpdate],
    ) -> P2PResult<()> {
        let manifest_data = self.get_user_manifest_result().await?;
        let current_user = self.get_local_user().await?;
        let peer_user = self.get_peer_user().await;
        add_missing_recipients(
            payload,
            &manifest_data.remote_missing.unknown_resources,
            self.repo_ctx.clone(),
            &peer_user.id,
            &current_user,
            &self.crypto_utils,
            &self.domain,
        )
        .await?;
        if !self.is_initiator {
            let missing_remote_fsr = get_missing_remote_folder_share_records(
                manifest_data.folders_requiring_recipient_sync,
                self.repo_ctx.clone(),
            )
            .await?;
            let message = FolderSyncMessage::FolderRecipientSyncPayload(missing_remote_fsr);
            self.send_message(Message::FolderSync(message)).await?;
        }
        self.send_resources().await?;
        Ok(())
    }
}
