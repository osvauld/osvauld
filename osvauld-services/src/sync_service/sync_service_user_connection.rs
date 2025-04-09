// In sync_service_user_connection.rs
use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::UserConnectionPayload;
use osvauld_core::models::sync_record::{StatusChangeSet, SyncRecord, SyncRecordSet};
use osvauld_core::models::user::User;
use osvauld_core::repositories::RepositoryError;
use tracing::{Span, info, debug, error, instrument};

use super::sync_service_core::SyncService;

impl SyncService {
    #[instrument(skip(self, payload, current_span), fields(user_id = %current_user_id, device_id = %current_device_id, payload_type = ?std::mem::discriminant(payload)))]
    pub async fn process_user_connection_payload(
        &self,
        payload: &UserConnectionPayload,
        current_user_id: &str,
        current_device_id: &str,
        current_span: Span,
    ) -> Result<Option<UserConnectionPayload>, RepositoryError> {
        // Enter the parent span context
        let _enter = current_span.enter();
        
        info!("Processing user connection payload");
        
        match payload {
            UserConnectionPayload::Request { user, devices } => {
                info!(
                    remote_user_id = %user.id,
                    device_count = devices.len(),
                    "Processing user connection request"
                );
                
                // Process request and get response
                match self.process_first_user_connection(
                    user,
                    devices,
                    current_user_id,
                    current_device_id,
                ).await {
                    Ok(response) => {
                        info!("User connection request processed successfully");
                        Ok(Some(response))
                    },
                    Err(e) => {
                        error!(error = %e, "Failed to process user connection request");
                        Err(e)
                    }
                }
            }

            UserConnectionPayload::Response {
                user,
                devices,
                user_addition_record,
            } => {
                info!(
                    remote_user_id = %user.id,
                    device_count = devices.len(),
                    "Processing user connection response"
                );
                
                // Process response and get acknowledgment
                match self.process_first_user_connection_response(
                    user,
                    devices,
                    user_addition_record,
                    current_user_id,
                    current_device_id,
                ).await {
                    Ok(ack) => {
                        info!("User connection response processed successfully");
                        Ok(Some(ack))
                    },
                    Err(e) => {
                        error!(error = %e, "Failed to process user connection response");
                        Err(e)
                    }
                }
            }

            UserConnectionPayload::Acknowledgment {
                user_id,
                user_addition_records,
                completion_record,
                updated_device_record_ids,
                updated_device_record_status_ids,
            } => {
                info!(
                    remote_user_id = %user_id,
                    "Processing user connection acknowledgment"
                );
                
                // Process acknowledgment and get complete message
                match self.handle_user_add_ack(
                    user_id,
                    user_addition_records,
                    completion_record,
                    current_device_id,
                    current_user_id,
                    updated_device_record_ids,
                    updated_device_record_status_ids,
                ).await {
                    Ok(complete) => {
                        info!("User connection acknowledgment processed successfully");
                        Ok(Some(complete))
                    },
                    Err(e) => {
                        error!(error = %e, "Failed to process user connection acknowledgment");
                        Err(e)
                    }
                }
            }

            UserConnectionPayload::Complete {
                completion_record,
                device_record_status_id,
                updated_device_record_ids,
                updated_device_record_status_ids,
            } => {
                info!(
                    device_record_status = ?device_record_status_id,
                    "Processing user connection complete message"
                );
                
                // Process complete message and maybe get final sync
                match self.process_user_connection_complete(
                    completion_record,
                    device_record_status_id,
                    current_device_id,
                    updated_device_record_ids,
                    updated_device_record_status_ids,
                ).await {
                    Ok(response) => {
                        info!("User connection complete processed successfully");
                        Ok(response)
                    },
                    Err(e) => {
                        error!(error = %e, "Failed to process user connection complete message");
                        Err(e)
                    }
                }
            }

            UserConnectionPayload::FinalSync {
                device_record_status_id,
            } => {
                info!(
                    device_record_status = ?device_record_status_id,
                    "Processing user connection final sync"
                );
                
                // Process final sync (no response)
                match self.process_user_connection_final_sync(device_record_status_id).await {
                    Ok(result) => {
                        info!("User connection final sync processed successfully");
                        Ok(result)
                    },
                    Err(e) => {
                        error!(error = %e, "Failed to process user connection final sync");
                        Err(e)
                    }
                }
            }
        }
    }

    #[instrument(skip(self, user, devices), fields(user_id = %user.id, current_user_id = %current_user_id, current_device_id = %current_device_id, device_count = devices.len()))]
    pub async fn process_first_user_connection(
        &self,
        user: &User,
        devices: &Vec<Device>,
        current_user_id: &str,
        current_device_id: &str,
    ) -> Result<UserConnectionPayload, RepositoryError> {
        debug!("Getting current user's devices");
        
        // Get current user's devices current device is already added when we add the user first
        // time.
        let user_devices = match self
            .device_repository
            .get_devices_by_user_id(current_user_id)
            .await {
                Ok(devices) => {
                    debug!(device_count = devices.len(), "Retrieved user devices");
                    devices
                },
                Err(e) => {
                    error!(error = %e, "Failed to get user devices");
                    return Err(e);
                }
            };

        // Create modified user with first_sync = false
        debug!("Creating modified user with first_sync = false");
        let mut new_user = user.clone();
        new_user.first_sync = false;
        new_user.owner = false;

        // Get current user
        debug!("Getting current user details");
        let current_user = match self.user_repository.get_user_by_id(current_user_id).await {
            Ok(user) => {
                debug!("Retrieved current user");
                user
            },
            Err(e) => {
                error!(error = %e, "Failed to get current user");
                return Err(e);
            }
        };

        // Combine all devices
        debug!("Combining devices");
        let all_devices: Vec<Device> = user_devices.iter().chain(devices.iter()).cloned().collect();
        debug!(device_count = all_devices.len(), "Combined devices");

        // Create user addition record
        debug!("Creating user addition record");
        let user_addition_record = SyncRecord::create_user_sync_record(
            user.id.clone(),
            current_device_id.to_string(),
            &all_devices,
        );
        debug!("User addition record created");

        // Save to database
        debug!("Saving to database");
        match self.db
            .sync_add_new_user(&new_user, devices, &user_addition_record)
            .await {
                Ok(_) => debug!("Data saved to database"),
                Err(e) => {
                    error!(error = %e, "Failed to save data to database");
                    return Err(e);
                }
            };
        let other_user_devices: Vec<Device> = user_devices.iter().filter(|ud|ud.id != current_device_id).cloned().collect();

        info!("First user connection processed successfully");
        
        // Return the Response payload directly
        Ok(UserConnectionPayload::Response {
            user: current_user,
            devices: other_user_devices,
            user_addition_record,
        })
    }

    #[instrument(skip(self, user, devices, user_addition_record), fields(
        user_id = %user.id, 
        current_user_id = %current_user_id, 
        current_device_id = %current_device_id,
        device_count = devices.len(),
        record_id = %user_addition_record.sync_record.id
    ))]
    pub async fn process_first_user_connection_response(
        &self,
        user: &User,
        devices: &Vec<Device>,
        user_addition_record: &SyncRecordSet,
        current_user_id: &str,
        current_device_id: &str,
    ) -> Result<UserConnectionPayload, RepositoryError> {
        debug!("Getting current user's devices");
        // Get current user's devices
        let user_devices = match self
            .device_repository
            .get_devices_by_user_id(current_user_id)
            .await {
                Ok(devices) => {
                    debug!(device_count = devices.len(), "Retrieved user devices");
                    devices
                },
                Err(e) => {
                    error!(error = %e, "Failed to get user devices");
                    return Err(e);
                }
            };

        // Combine all devices
        debug!("Combining devices");
        let all_devices: Vec<Device> = user_devices.iter().chain(devices.iter()).cloned().collect();
        debug!(device_count = all_devices.len(), "Combined devices");

        // Create remote user addition record
        debug!("Creating remote user addition record");
        let remote_user_addition_record = SyncRecord::create_user_sync_record(
            user.id.clone(),
            current_device_id.to_string(),
            &all_devices,
        );
        debug!("Remote user addition record created");

        debug!("Processing device records");
        let (processed_records, processed_statuses, updated_record_ids, updated_status_ids) =
            SyncRecord::process_device_records(
                &user_addition_record.device_records,
                &user_addition_record.device_record_statuses,
                current_device_id,
            );
        debug!(
            record_count = processed_records.len(), 
            status_count = processed_statuses.len(),
            "Device records processed"
        );
        
        let updated_user_addition_record = SyncRecordSet {
            sync_record: user_addition_record.sync_record.clone(),
            device_records: processed_records,
            device_record_statuses: processed_statuses,
        };
        
        // Create completion record
        debug!("Creating completion record");
        let completion_record = SyncRecord::create_completion_records(
            user_addition_record.sync_record.id.clone(),
            current_device_id.to_string(),
            &all_devices,
        );
        debug!("Completion record created");

        // Save to database
        debug!("Saving to database");
        match self.db
            .complete_new_user_add(
                &user.id,
                devices,
                &updated_user_addition_record,
                &remote_user_addition_record,
                &completion_record,
            )
            .await {
                Ok(_) => debug!("Data saved to database"),
                Err(e) => {
                    error!(error = %e, "Failed to save data to database");
                    return Err(e);
                }
            };

        info!("User connection response processed successfully");
        
        // Return the Acknowledgment payload directly
        Ok(UserConnectionPayload::Acknowledgment {
            user_id: current_user_id.to_string(),
            user_addition_records: remote_user_addition_record,
            completion_record,
            updated_device_record_ids: updated_record_ids,
            updated_device_record_status_ids: updated_status_ids,
        })
    }

    #[instrument(skip(self, user_addition_record, completion_record, updated_device_record_ids, updated_device_record_status_ids), fields(
        remote_user_id = %remote_user_id,
        current_device_id = %current_device_id,
        current_user_id = %current_user_id,
        record_id = %user_addition_record.sync_record.id
    ))]
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
        debug!("Processing completion record");
        
        // Process completion record
        let (updated_completion_record, device_record_status_id) =
            SyncRecord::process_completion_record(completion_record, current_device_id);
        
        debug!(status_id = ?device_record_status_id, "Processed completion record");

        debug!("Processing device records");
        let (processed_records, processed_statuses, updated_record_ids, updated_status_ids) =
            SyncRecord::process_device_records(
                &user_addition_record.device_records,
                &user_addition_record.device_record_statuses,
                current_device_id,
            );
        
        debug!(
            processed_records = processed_records.len(),
            processed_statuses = processed_statuses.len(),
            "Device records processed"
        );
        
        let updated_user_addition_record = SyncRecordSet {
            sync_record: user_addition_record.sync_record.clone(),
            device_records: processed_records,
            device_record_statuses: processed_statuses,
        };
        
        // Get devices from both users
        debug!("Getting remote user devices");
        let remote_devices = match self
            .device_repository
            .get_devices_by_user_id(remote_user_id)
            .await {
                Ok(devices) => {
                    debug!(device_count = devices.len(), "Retrieved remote user devices");
                    devices
                },
                Err(e) => {
                    error!(error = %e, "Failed to get remote user devices");
                    return Err(e);
                }
            };
            
        debug!("Getting current user devices");
        let user_devices = match self
            .device_repository
            .get_devices_by_user_id(current_user_id)
            .await {
                Ok(devices) => {
                    debug!(device_count = devices.len(), "Retrieved current user devices");
                    devices
                },
                Err(e) => {
                    error!(error = %e, "Failed to get current user devices");
                    return Err(e);
                }
            };

        // Combine all devices
        debug!("Combining devices");
        let all_devices: Vec<Device> = user_devices
            .iter()
            .chain(remote_devices.iter())
            .cloned()
            .collect();
            
        debug!(device_count = all_devices.len(), "Combined devices");

        // Create local completion record
        debug!("Creating local completion record");
        let local_completion_record = SyncRecord::create_completion_records(
            updated_user_addition_record.sync_record.id.clone(),
            current_device_id.to_string(),
            &all_devices,
        );
        debug!("Local completion record created");

        // Save to database
        debug!("Saving to database");
        match self.db
            .handle_user_add_ack(
                remote_user_id,
                &updated_user_addition_record,
                &updated_completion_record,
                &local_completion_record,
                updated_device_record_ids,
                updated_device_record_status_ids,
            )
            .await {
                Ok(_) => debug!("Data saved to database"),
                Err(e) => {
                    error!(error = %e, "Failed to save data to database");
                    return Err(e);
                }
            };

        info!("User add acknowledgment processed successfully");
        
        // Return the Complete payload directly
        Ok(UserConnectionPayload::Complete {
            completion_record: local_completion_record,
            device_record_status_id,
            updated_device_record_ids: updated_record_ids,
            updated_device_record_status_ids: updated_status_ids,
        })
    }
    
    #[instrument(skip(self, completion_record, updated_device_record_status_ids, updated_device_record_ids), fields(
        current_device_id = %current_device_id,
        device_record_status = ?device_sync_record_id,
        updated_record_ids = updated_device_record_ids.len(),
        updated_status_ids = updated_device_record_status_ids.len()
    ))]
    pub async fn process_user_connection_complete(
        &self,
        completion_record: &StatusChangeSet,
        device_sync_record_id: &Option<String>,
        current_device_id: &str,
        updated_device_record_status_ids: &[String],
        updated_device_record_ids: &[String],
    ) -> Result<Option<UserConnectionPayload>, RepositoryError> {
        debug!("Processing completion record");
        
        // Process the completion record
        let (updated_completion_record, updated_device_sync_record_id) =
            SyncRecord::process_completion_record(completion_record, current_device_id);
            
        debug!(
            status_id = ?updated_device_sync_record_id,
            "Processed completion record"
        );
        
        // Save to database
        debug!("Handling user connection complete in database");
        match self.db
            .handle_user_connection_complete(
                &updated_completion_record,
                device_sync_record_id.clone(),
                updated_device_record_ids,
                updated_device_record_status_ids,
            )
            .await {
                Ok(_) => debug!("User connection complete saved to database"),
                Err(e) => {
                    error!(error = %e, "Failed to handle user connection complete");
                    return Err(e);
                }
            };
            
        info!(
            device_record_status = ?updated_device_sync_record_id,
            "Sending final acknowledgment"
        );
        
        Ok(Some(UserConnectionPayload::FinalSync {
            device_record_status_id: updated_device_sync_record_id,
        }))
    }
    
    #[instrument(skip(self), fields(device_record_status = ?device_sync_record))]
    pub async fn process_user_connection_final_sync(
        &self,
        device_sync_record: &Option<String>,
    ) -> Result<Option<UserConnectionPayload>, RepositoryError> {
        // Save the final device record
        if let Some(device_sync_record_status) = device_sync_record {
            debug!("Updating device sync record status");
            match self.sync_repository
                .update_device_sync_record_status(device_sync_record_status.clone())
                .await {
                    Ok(_) => debug!("Device sync record status updated"),
                    Err(e) => {
                        error!(error = %e, "Failed to update device sync record status");
                        return Err(e);
                    }
                };
        } else {
            debug!("No device sync record status to update");
        }

        info!("User connection final sync processed successfully");
        
        // No further response needed
        Ok(None)
    }
}
