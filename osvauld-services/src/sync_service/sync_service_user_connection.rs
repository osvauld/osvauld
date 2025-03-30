use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::UserConnectionPayload;
use osvauld_core::models::sync_record::{StatusChangeSet, SyncRecord, SyncRecordSet};
use osvauld_core::models::user::User;
use osvauld_core::repositories::RepositoryError;
use tracing::info;

use super::sync_service_core::SyncService;

impl SyncService {
    pub async fn process_user_connection_payload(
        &self,
        payload: &UserConnectionPayload,
        current_user_id: &str,
        current_device_id: &str,
    ) -> Result<Option<UserConnectionPayload>, RepositoryError> {
        match payload {
            UserConnectionPayload::Request { user, devices } => {
                // Process request and get response
                let response = self
                    .process_first_user_connection(
                        user,
                        devices,
                        current_user_id,
                        current_device_id,
                    )
                    .await?;

                Ok(Some(response))
            }

            UserConnectionPayload::Response {
                user,
                devices,
                user_addition_record,
            } => {
                // Process response and get acknowledgment
                let ack = self
                    .process_first_user_connection_response(
                        user,
                        devices,
                        user_addition_record,
                        current_user_id,
                        current_device_id,
                    )
                    .await?;

                Ok(Some(ack))
            }

            UserConnectionPayload::Acknowledgment {
                user_id,
                user_addition_records,
                completion_record,
                updated_device_record_ids,
                updated_device_record_status_ids,
            } => {
                // Process acknowledgment and get complete message
                let complete = self
                    .handle_user_add_ack(
                        user_id,
                        user_addition_records,
                        completion_record,
                        current_device_id,
                        current_user_id,
                        updated_device_record_ids,
                        updated_device_record_status_ids,
                    )
                    .await?;

                Ok(Some(complete))
            }

            UserConnectionPayload::Complete {
                completion_record,
                device_record_status_id,
                updated_device_record_ids,
                updated_device_record_status_ids,
            } => {
                // Process complete message and maybe get final sync
                self.process_user_connection_complete(
                    completion_record,
                    device_record_status_id,
                    current_device_id,
                    updated_device_record_ids,
                    updated_device_record_status_ids,
                )
                .await
            }

            UserConnectionPayload::FinalSync {
                device_record_status_id,
            } => {
                // Process final sync (no response)
                self.process_user_connection_final_sync(device_record_status_id)
                    .await
            }
        }
    }
    pub async fn process_first_user_connection(
        &self,
        user: &User,
        devices: &Vec<Device>,
        current_user_id: &str,
        current_device_id: &str,
    ) -> Result<UserConnectionPayload, RepositoryError> {
        // Get current user's devices current device is already added when we add the user first
        // time.
        let user_devices = self
            .device_repository
            .get_devices_by_user_except(current_user_id, &[current_device_id.to_string()])
            .await?;

        // Create modified user with first_sync = false
        let mut new_user = user.clone();
        new_user.first_sync = false;

        // Get current user
        let current_user = self.user_repository.get_user_by_id(current_user_id).await?;

        // Combine all devices
        let all_devices: Vec<Device> = user_devices.iter().chain(devices.iter()).cloned().collect();

        // Create user addition record
        let user_addition_record = SyncRecord::create_user_sync_record(
            user.id.clone(),
            current_device_id.to_string(),
            &all_devices,
        );

        // Save to database
        self.db
            .sync_add_new_user(&new_user, devices, &user_addition_record)
            .await?;

        // Return the Response payload directly
        Ok(UserConnectionPayload::Response {
            user: current_user,
            devices: user_devices,
            user_addition_record,
        })
    }

    pub async fn process_first_user_connection_response(
        &self,
        user: &User,
        devices: &Vec<Device>,
        user_addition_record: &SyncRecordSet,
        current_user_id: &str,
        current_device_id: &str,
    ) -> Result<UserConnectionPayload, RepositoryError> {
        // Get current user's devices
        let user_devices = self
            .device_repository
            .get_devices_by_user_except(current_user_id, &[current_device_id.to_string()])
            .await?;

        // Combine all devices
        let all_devices: Vec<Device> = user_devices.iter().chain(devices.iter()).cloned().collect();

        // Create remote user addition record
        let remote_user_addition_record = SyncRecord::create_user_sync_record(
            user.id.clone(),
            current_device_id.to_string(),
            &all_devices,
        );

        let (processed_records, processed_statuses, updated_record_ids, updated_status_ids) =
            SyncRecord::process_device_records(
                &user_addition_record.device_records,
                &user_addition_record.device_record_statuses,
                current_device_id,
            );
        let updated_user_addition_record = SyncRecordSet {
            sync_record: user_addition_record.sync_record.clone(),
            device_records: processed_records,
            device_record_statuses: processed_statuses,
        };
        // Create completion record
        let completion_record = SyncRecord::create_completion_records(
            user_addition_record.sync_record.id.clone(),
            current_device_id.to_string(),
            &all_devices,
        );

        // Save to database
        self.db
            .complete_new_user_add(
                &user.id,
                devices,
                &updated_user_addition_record,
                &remote_user_addition_record,
                &completion_record,
            )
            .await?;

        // Return the Acknowledgment payload directly
        Ok(UserConnectionPayload::Acknowledgment {
            user_id: current_user_id.to_string(),
            user_addition_records: remote_user_addition_record,
            completion_record,
            updated_device_record_ids: updated_record_ids,
            updated_device_record_status_ids: updated_status_ids,
        })
    }

    pub async fn handle_user_add_ack(
        &self,
        remote_user_id: &str,
        user_addition_record: &SyncRecordSet,
        completion_record: &StatusChangeSet,
        current_device_id: &str,
        current_user_id: &str,
        updated_device_record_ids: &[String],
        updated_device_record_status_ids: &[String],
    ) -> Result<UserConnectionPayload, RepositoryError> {
        // Process completion record
        let (updated_completion_record, device_record_status_id) =
            SyncRecord::process_completion_record(completion_record, current_device_id);

        let (processed_records, processed_statuses, updated_record_ids, updated_status_ids) =
            SyncRecord::process_device_records(
                &user_addition_record.device_records,
                &user_addition_record.device_record_statuses,
                current_device_id,
            );
        let updated_user_addition_record = SyncRecordSet {
            sync_record: user_addition_record.sync_record.clone(),
            device_records: processed_records,
            device_record_statuses: processed_statuses,
        };
        // Get devices from both users
        let remote_devices = self
            .device_repository
            .get_devices_by_user_id(remote_user_id)
            .await?;
        let user_devices = self
            .device_repository
            .get_devices_by_user_except(current_user_id, &[current_device_id.to_string()])
            .await?;

        // Combine all devices
        let all_devices: Vec<Device> = user_devices
            .iter()
            .chain(remote_devices.iter())
            .cloned()
            .collect();

        // Create local completion record
        let local_completion_record = SyncRecord::create_completion_records(
            updated_user_addition_record.sync_record.id.clone(),
            current_device_id.to_string(),
            &all_devices,
        );

        // Save to database
        self.db
            .handle_user_add_ack(
                remote_user_id,
                &updated_user_addition_record,
                &updated_completion_record,
                &local_completion_record,
                updated_device_record_ids,
                updated_device_record_status_ids,
            )
            .await?;

        // Return the Complete payload directly
        Ok(UserConnectionPayload::Complete {
            completion_record: local_completion_record,
            device_record_status_id,
            updated_device_record_ids: updated_record_ids,
            updated_device_record_status_ids: updated_status_ids,
        })
    }
    pub async fn process_user_connection_complete(
        &self,
        completion_record: &StatusChangeSet,
        device_sync_record_id: &Option<String>,
        current_device_id: &str,
        updated_device_record_status_ids: &[String],
        updated_device_record_ids: &[String],
    ) -> Result<Option<UserConnectionPayload>, RepositoryError> {
        // Save the local completion record

        let (updated_completion_record, updated_device_sync_record_id) =
            SyncRecord::process_completion_record(completion_record, current_device_id);
        self.db
            .handle_user_connection_complete(
                &updated_completion_record,
                device_sync_record_id.clone(),
                updated_device_record_ids,
                updated_device_record_status_ids,
            )
            .await?;
        info!("sending final ack {:?}", updated_device_sync_record_id);
        Ok(Some(UserConnectionPayload::FinalSync {
            device_record_status_id: updated_device_sync_record_id,
        }))
    }
    pub async fn process_user_connection_final_sync(
        &self,
        device_sync_record: &Option<String>,
    ) -> Result<Option<UserConnectionPayload>, RepositoryError> {
        // Save the final device record
        if let Some(device_sync_record_status) = device_sync_record {
            self.sync_repository
                .update_device_sync_record_status(device_sync_record_status.clone())
                .await?;
        }

        // No further response needed
        Ok(None)
    }
}
