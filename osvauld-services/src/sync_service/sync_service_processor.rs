use log::warn;
use osvauld_core::models::device::Device;
use osvauld_core::models::folder::Folder;
use osvauld_core::models::p2p::{SyncAckType, SyncPayload};
use osvauld_core::models::resource::ResourceKeyPair;
use osvauld_core::models::share_record::ShareRecord;
use osvauld_core::models::sync_record::{
     DeviceRecord, DeviceRecordStatus, SyncRecord, SyncRecordSet
};
use osvauld_core::models::sync_types::SyncMergeResult;
use osvauld_core::models::sync_types::SyncOperations;
use osvauld_core::models::user::User;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::RepositoryError;

use tracing::{Span, info, debug, error, instrument, trace};

use super::sync_service_core::SyncService;

impl SyncService {
    #[instrument(
        skip(self, payload,  current_span), 
        fields(
            payload_type = ?std::mem::discriminant(payload),
            remote_user_id= %remote_user_id,
            current_user_id = %current_user_id,
            device_id = %device_id,
            current_device_id = %current_device_id
        ),
        level = "info"
    )]
    pub async fn process_sync_payload(
        &self,
        payload: &SyncPayload,
        remote_user_id: &str,
        device_id: &str,
        current_device_id: &str,
        current_user_id: &str,
        current_span: Span,
    ) -> Result<SyncAckType, RepositoryError>
    {
        // Enter the parent span
        let _guard = current_span.enter();
        
        info!("Processing sync payload");
        
        let result = match payload {
            SyncPayload::DeviceSync {
                sync_record,
                device_records,
                device_record_statuses,
                device,
            } => {
                debug!(
                    sync_record_id = %sync_record.id, 
                    device_records = device_records.len(),
                    "Processing device sync payload"
                );
                
                self.process_device_sync(
                    sync_record,
                    device_records,
                    device_record_statuses,
                    device,
                    current_device_id,
                    current_user_id,
                )
                .await
            }

            SyncPayload::UserSync {
                sync_data,
                user_data,
            } => {
                debug!(
                    sync_data_count = sync_data.len(),
                    user_data_count = user_data.len(),
                    "Processing user sync payload"
                );
                
                self.process_user_sync(sync_data, user_data, current_device_id, current_user_id)
                    .await
            }

            SyncPayload::ResourceSync {
                sync_record,
                device_records,
                device_record_statuses,
                resource,
                vector_clocks,
            } => {
                debug!(
                    sync_record_id = %sync_record.id,
                    resource_id = %resource.resource.id,
                    device_records = device_records.len(),
                    "Processing resource sync payload"
                );
                
                self.process_resource_sync(
                    sync_record,
                    device_records,
                    device_record_statuses,
                    resource,
                    vector_clocks,
                    current_device_id,
                    current_user_id,
                    remote_user_id,
                )
                .await
            }

            SyncPayload::FolderSync {
                sync_record,
                device_records,
                device_record_statuses,
                folder,
            } => {
                debug!(
                    sync_record_id = %sync_record.id,
                    folder_id = %folder.id,
                    device_records = device_records.len(),
                    "Processing folder sync payload"
                );
                
                self.process_folder_sync(
                    sync_record,
                    device_records,
                    device_record_statuses,
                    folder,
                    current_device_id,
                    current_user_id,
                )
                .await
            }

            SyncPayload::ShareSync { sync_record, device_records, device_record_statuses, share_record } => {
                debug!(
                share_record = %share_record.id,
                    sync_record = %sync_record.id,
                    "processing share record"
            );
                    self.process_share_sync(sync_record, device_records, device_record_statuses,share_record, current_device_id, current_user_id).await
            }



            SyncPayload::StatusUpdate(payload) => {
                debug!(
                    status_records = payload.len(),
                    "Processing status update"
                );
                
                self.process_status_update(payload, current_device_id).await
            }
            SyncPayload::ResourceMerge(payload) => {
                info!("not used");
                warn!("warning this shouldnt be called");
                Ok(
                    SyncAckType::UpdateReceived
                )
            }
        };
        
        match &result {
            Ok(ack) => {
                info!(
                    ack_type = ?std::mem::discriminant(ack),
                    "Sync payload processed successfully"
                );
            },
            Err(e) => {
                error!(
                    error = %e,
                    "Failed to process sync payload"
                );
            }
        }
        
        result
    }

    // Process device sync payload
    #[instrument(
        skip(self, sync_record, device_records, device_record_statuses, device), 
        fields(
            sync_record_id = %sync_record.id,
            device_id = %device.id,
            current_device_id = %current_device_id,
            user_id = %user_id,
            record_count = device_records.len()
        ),
        level = "debug"
    )]
    async fn process_device_sync(
        &self,
        sync_record: &SyncRecord,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        device: &Device,
        current_device_id: &str,
        user_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        debug!("Processing device sync");
        
        let user_devices = self.device_repository.get_devices_by_user_id(user_id).await?;
        
        // Prepare common sync data
        debug!("Preparing common sync data");
        let (merge_result, record_exists) = match self.prepare_common_sync_data(
            sync_record,
            device_records,
            device_record_statuses,
            current_device_id,
            user_devices,
        ).await {
            Ok(result) => {
                debug!(
                    record_exists = result.1,
                    local_records = result.0.local_operations.records_to_add.len(),
                    remote_records = result.0.remote_operations.records_to_add.len(),
                    "Common sync data prepared"
                );
                result
            },
            Err(e) => {
                error!(error = %e, "Failed to prepare common sync data");
                return Err(e);
            }
        };

        if !record_exists {
            debug!("Record doesn't exist, creating new sync record set");
            let record_set = SyncRecordSet {
                sync_record: sync_record.clone(),
                device_records: merge_result.local_operations.records_to_add.clone(),
                device_record_statuses: merge_result.local_operations.status_records_to_add.clone(),
            };

            // Use db transaction method for saving device sync
            debug!("Saving device sync");
            match self.db.save_device_sync(device, &record_set, &[]).await {
                Ok(_) => debug!("Device sync saved successfully"),
                Err(e) => {
                    error!(error = %e, "Failed to save device sync");
                    return Err(e);
                }
            }
        } else {
            debug!("Record exists, applying sync operations");
            match self.db.apply_sync_operations(&merge_result.local_operations).await {
                Ok(_) => debug!("Sync operations applied successfully"),
                Err(e) => {
                    error!(error = %e, "Failed to apply sync operations");
                    return Err(e);
                }
            }
        }
        
        info!("Device sync processed successfully");
        Ok(SyncAckType::FullSync(merge_result.remote_operations))
    }

    // Process resource sync payload
    #[instrument(
        skip(self, sync_record, device_records, device_record_statuses, resource, vector_clocks), 
        fields(
            sync_record_id = %sync_record.id,
            resource_id = %resource.resource.id,
            current_device_id = %current_device_id,
            current_user_id = %current_user_id,
            remote_user_id = %remote_user_id,
            record_count = device_records.len(),
            vector_clock_count = vector_clocks.len()
        ),
        level = "debug"
    )]
    async fn process_resource_sync(
        &self,
        sync_record: &SyncRecord,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        resource: &ResourceKeyPair,
        vector_clocks: &[ResourceVectorClock],
        current_device_id: &str,
        current_user_id: &str,
        remote_user_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        debug!("Processing resource sync");
        
        // Get user's other devices
        let user_devices = self.device_repository.get_devices_by_user_id(current_user_id).await?;
        
        // Prepare common sync data
        debug!("Preparing common sync data");
        let (merge_result, record_exist) = match self.prepare_common_sync_data(
            sync_record,
            device_records,
            device_record_statuses,
            current_device_id,
            user_devices,
        ).await {
            Ok(result) => {
                debug!(
                    record_exists = result.1,
                    local_records = result.0.local_operations.records_to_add.len(),
                    remote_records = result.0.remote_operations.records_to_add.len(),
                    "Common sync data prepared"
                );
                result
            },
            Err(e) => {
                error!(error = %e, "Failed to prepare common sync data");
                return Err(e);
            }
        };

        if !record_exist {
            debug!("Record doesn't exist, creating new sync record set");
            // Create sync record set
            let record_set = SyncRecordSet {
                sync_record: sync_record.clone(),
                device_records: merge_result.local_operations.records_to_add.clone(),
                device_record_statuses: merge_result.local_operations.status_records_to_add.clone(),
            };

            let mut resource_pair = resource.clone();
            if current_user_id != remote_user_id {
                let default_folder = self.folder_repository.get_default_folder().await?;
                resource_pair.resource.folder_id = default_folder.id;
            } 
            // Use db transaction method for saving resource sync
            debug!("Saving resource sync");
            match self.db.save_resource_sync(&resource_pair, vector_clocks, &record_set).await {
                Ok(_) => debug!("Resource sync saved successfully"),
                Err(e) => {
                    error!(error = %e, "Failed to save resource sync");
                    return Err(e);
                }
            }
        } else {
            debug!("Record exists, applying sync operations");
            match self.db.apply_sync_operations(&merge_result.local_operations).await {
                Ok(_) => debug!("Sync operations applied successfully"),
                Err(e) => {
                    error!(error = %e, "Failed to apply sync operations");
                    return Err(e);
                }
            }
        }

        info!("Resource sync processed successfully");
        Ok(SyncAckType::FullSync(merge_result.remote_operations))
    }

    #[instrument(
        skip(self, sync_record, device_records, device_record_statuses, share_record), 
        fields(
            sync_record_id = %sync_record.id,
            share_record_id = %share_record.id,
            current_device_id = %current_device_id,
            current_user_id = %current_user_id
        ),
        level = "debug"
    )]
    async fn process_share_sync(
        &self,
        sync_record: &SyncRecord,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        share_record: &ShareRecord,
        current_device_id: &str,
        current_user_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        debug!("Processing share sync");
        
        let user_devices = self.device_repository.get_devices_by_user_id(current_user_id).await?;
        
        // Prepare common sync data
        debug!("Preparing common sync data");
        let (merge_result, record_exist) = match self.prepare_common_sync_data(
            sync_record,
            device_records,
            device_record_statuses,
            current_device_id,
            user_devices,
        ).await {
            Ok(result) => {
                debug!(
                    record_exists = result.1,
                    local_records = result.0.local_operations.records_to_add.len(),
                    remote_records = result.0.remote_operations.records_to_add.len(),
                    "Common sync data prepared"
                );
                result
            },
            Err(e) => {
                error!(error = %e, "Failed to prepare common sync data");
                return Err(e);
            }
        };

        if !record_exist {
            debug!("Record doesn't exist, creating new sync record set");
            // Create sync record set
            let record_set = SyncRecordSet {
                sync_record: sync_record.clone(),
                device_records: merge_result.local_operations.records_to_add.clone(),
                device_record_statuses: merge_result.local_operations.status_records_to_add.clone(),
            };

            // Use db transaction method for saving share sync
            debug!("Saving share sync");
            match self.db.save_share_sync(share_record, &record_set).await {
                Ok(_) => debug!("Share sync saved successfully"),
                Err(e) => {
                    error!(error = %e, "Failed to save share sync");
                    return Err(e);
                }
            }
        } else {
            debug!("Record exists, applying sync operations");
            match self.db.apply_sync_operations(&merge_result.local_operations).await {
                Ok(_) => debug!("Sync operations applied successfully"),
                Err(e) => {
                    error!(error = %e, "Failed to apply sync operations");
                    return Err(e);
                }
            }
        }

        info!("Share sync processed successfully");
        Ok(SyncAckType::FullSync(merge_result.remote_operations))
    }

    // Process folder sync payload
    #[instrument(
        skip(self, sync_record, device_records, device_record_statuses, folder), 
        fields(
            sync_record_id = %sync_record.id,
            folder_id = %folder.id,
            current_device_id = %current_device_id,
            current_user_id = %current_user_id,
            record_count = device_records.len()
        ),
        level = "debug"
    )]
    async fn process_folder_sync(
        &self,
        sync_record: &SyncRecord,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        folder: &Folder,
        current_device_id: &str,
        current_user_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        debug!("Processing folder sync");
        let user_devices = self.device_repository.get_devices_by_user_id(current_user_id).await?;

        // Prepare common sync data
        debug!("Preparing common sync data");
        let (merge_result, record_exist) = match self.prepare_common_sync_data(
            sync_record,
            device_records,
            device_record_statuses,
            current_device_id,
            user_devices,
        ).await {
            Ok(result) => {
                debug!(
                    record_exists = result.1,
                    local_records = result.0.local_operations.records_to_add.len(),
                    remote_records = result.0.remote_operations.records_to_add.len(),
                    "Common sync data prepared"
                );
                result
            },
            Err(e) => {
                error!(error = %e, "Failed to prepare common sync data");
                return Err(e);
            }
        };

        if !record_exist {
            debug!("Record doesn't exist, creating new sync record set");
            // Create sync record set
            let record_set = SyncRecordSet {
                sync_record: sync_record.clone(),
                device_records: merge_result.local_operations.records_to_add.clone(),
                device_record_statuses: merge_result.local_operations.status_records_to_add.clone(),
            };

            // Use db transaction method for saving folder sync
            debug!("Saving folder sync");
            match self.db.save_folder_sync(folder, &record_set).await {
                Ok(_) => debug!("Folder sync saved successfully"),
                Err(e) => {
                    error!(error = %e, "Failed to save folder sync");
                    return Err(e);
                }
            }
        } else {
            debug!("Record exists, applying sync operations");
            match self.db.apply_sync_operations(&merge_result.local_operations).await {
                Ok(_) => debug!("Sync operations applied successfully"),
                Err(e) => {
                    error!(error = %e, "Failed to apply sync operations");
                    return Err(e);
                }
            }
        }

        info!("Folder sync processed successfully");
        Ok(SyncAckType::FullSync(merge_result.remote_operations))
    }

    // Process status update payload
    #[instrument(
        skip(self, payload), 
        fields(
            payload_count = payload.len(),
            current_device_id = %current_device_id
        ),
        level = "debug"
    )]
    async fn process_status_update(
        &self,
        payload: &Vec<(DeviceRecord, Vec<DeviceRecordStatus>)>,
        current_device_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        debug!("Processing status update payload");
        
        // Process device records
        let mut updated_device_sync_records = Vec::new();
        let mut updated_device_record_ids = Vec::new();
        let mut to_add_records = Vec::new();

        for (i, (device_record, device_record_statuses)) in payload.iter().enumerate() {
            trace!(
                index = i,
                device_record_id = %device_record.id,
                status_count = device_record_statuses.len(),
                "Processing device record"
            );
            
            // Get local device record
            let local_device_record = match self
                .sync_repository
                .get_device_record_by_id(&device_record.id)
                .await {
                    Ok(record) => {
                        trace!(record_found = record.is_some(), "Device record lookup result");
                        record
                    },
                    Err(e) => {
                        error!(
                            error = %e,
                            device_record_id = %device_record.id,
                            "Failed to get device record by ID"
                        );
                        return Err(e);
                    }
                };

            // Check if we need to add this record's status for the current device
            if let Some(status) = device_record_statuses
                .iter()
                .find(|status| status.aware_device_id == current_device_id)
            {
                trace!(
                    status_id = %status.id,
                    "Adding status record for current device"
                );
                updated_device_sync_records.push(status.id.clone());
            }

            if local_device_record.is_some() {
                // Record exists locally
                if device_record.synced {
                    // If synced is true, add to updated IDs
                    trace!(
                        device_record_id = %device_record.id,
                        "Adding device record ID to updated list (exists and synced)"
                    );
                    updated_device_record_ids.push(device_record.id.clone());
                }
            } else {
                // Record doesn't exist locally, add to to_add_records
                trace!(
                    device_record_id = %device_record.id,
                    "Adding device record to to_add_records (doesn't exist locally)"
                );
                to_add_records.push((device_record.clone(), device_record_statuses.clone()));
            }
        }
        
        debug!(
            to_add_records = to_add_records.len(),
            updated_records = updated_device_sync_records.len(),
            updated_ids = updated_device_record_ids.len(),
            "Applying device sync updates"
        );
        
        match self.db
            .apply_device_sync_updates(
                &to_add_records,
                &updated_device_sync_records,
                &updated_device_record_ids,
            )
            .await {
                Ok(_) => debug!("Device sync updates applied successfully"),
                Err(e) => {
                    error!(error = %e, "Failed to apply device sync updates");
                    return Err(e);
                }
            };

        info!(
            updated_records = updated_device_sync_records.len(),
            "Status update processed successfully"
        );
        Ok(SyncAckType::DeviceSyncRecords(updated_device_sync_records))
    }

    #[instrument(
        skip(self, sync_data, user_data), 
        fields(
            sync_data_count = sync_data.len(),
            user_data_count = user_data.len(),
            current_device_id = %current_device_id,
            current_user_id = %current_user_id
        ),
        level = "debug"
    )]
    async fn process_user_sync(
        &self,
        sync_data: &Vec<SyncRecordSet>,
        user_data: &Vec<(User, Vec<Device>)>,
        current_device_id: &str,
        current_user_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        debug!("Processing user sync");
        
        // Get current user's other devices
        let user_devices = self.device_repository.get_devices_by_user_id(current_user_id).await?;

        // Collect all devices from other users (not current user)
        let mut all_devices = Vec::new();
        let mut users_to_add = Vec::new();
        let mut devices_to_add = Vec::new();

        for (i, (user, devices)) in user_data.iter().enumerate() {
            trace!(
                index = i,
                user_id = %user.id,
                device_count = devices.len(),
                "Processing user data entry"
            );
            
            // Check if user exists
            let user_exists = match self.user_repository.get_user_by_id(&user.id).await {
                Ok(_) => {
                    trace!(user_id = %user.id, "User exists");
                    true
                },
                Err(RepositoryError::NotFound) => {
                    trace!(user_id = %user.id, "User does not exist");
                    false
                },
                Err(e) => {
                    error!(error = %e, user_id = %user.id, "Error checking if user exists");
                    return Err(e);
                }
            };

            // Only add the user if they don't already exist
            if !user_exists {
                trace!(user_id = %user.id, "Adding user to users_to_add");
                let mut user_clone = user.clone();
                user_clone.owner = false;
                users_to_add.push(user_clone);
                devices_to_add.extend(devices.clone());
            }
            
            // Only add other users' data to be saved
            if user.id != current_user_id {
                trace!(
                    user_id = %user.id,
                    device_count = devices.len(),
                    "Adding devices to all_devices (other user)"
                );
                all_devices.extend(devices.clone());
            }
        }

        // Add current user's other devices to the full device list for sync record processing
        all_devices.extend(user_devices.clone());

        debug!(
            users_to_add = users_to_add.len(),
            devices_to_add = devices_to_add.len(),
            all_devices = all_devices.len(),
            "Collected devices and users"
        );

        // Create a combined SyncOperations for the final result
        let mut combined_remote_operations = SyncOperations::new();

        // List to collect all record sets that need to be added
        let mut record_sets_to_add = Vec::new();
        let mut operations_to_apply = Vec::new();

        // Process all sync records
        debug!(sync_data_count = sync_data.len(), "Processing sync records");
        for (i, record_set) in sync_data.iter().enumerate() {
            trace!(
                index = i,
                sync_record_id = %record_set.sync_record.id,
                device_records = record_set.device_records.len(),
                status_records = record_set.device_record_statuses.len(),
                "Processing sync record"
            );
            
            let (merge_result, record_exists) = match self
                .prepare_common_sync_data(
                    &record_set.sync_record,
                    &record_set.device_records,
                    &record_set.device_record_statuses,
                    current_device_id,
                    all_devices.clone(),
                )
                .await {
                    Ok(result) => {
                        trace!(
                            record_exists = result.1,
                            local_records = result.0.local_operations.records_to_add.len(),
                            remote_records = result.0.remote_operations.records_to_add.len(),
                            "Prepared common sync data for record"
                        );
                        result
                    },
                    Err(e) => {
                        error!(
                            error = %e,
                            sync_record_id = %record_set.sync_record.id,
                            "Failed to prepare common sync data"
                        );
                        return Err(e);
                    }
                };

            if !record_exists {
                trace!(
                    sync_record_id = %record_set.sync_record.id,
                    "Record doesn't exist, creating sync record set"
                );
                // Create sync record set for new records
                let record_set = SyncRecordSet {
                    sync_record: record_set.sync_record.clone(),
                    device_records: merge_result.local_operations.records_to_add.clone(),
                    device_record_statuses: merge_result
                        .local_operations
                        .status_records_to_add
                        .clone(),
                };

                record_sets_to_add.push(record_set);
            } else {
                trace!(
                    sync_record_id = %record_set.sync_record.id,
                    "Record exists, storing operations to apply"
                );
                // Store operations to apply for existing records
                operations_to_apply.push(merge_result.local_operations.clone());
            }

            // Combine remote operations
            trace!(
                sync_record_id = %record_set.sync_record.id,
                "Combining remote operations"
            );
            combined_remote_operations
                .records_to_add
                .extend(merge_result.remote_operations.records_to_add);
            combined_remote_operations
                .status_records_to_add
                .extend(merge_result.remote_operations.status_records_to_add);
            combined_remote_operations
                .record_ids_to_update
                .extend(merge_result.remote_operations.record_ids_to_update);
            combined_remote_operations
                .status_ids_to_update
                .extend(merge_result.remote_operations.status_ids_to_update);
        }

        // Now use the transaction service to add everything in one go
        debug!(
            users_to_add = users_to_add.len(),
            devices_to_add = devices_to_add.len(),
            record_sets = record_sets_to_add.len(),
            "Adding users, devices, and record sets"
        );
        match self.db
            .add_users_and_devices_batch(&users_to_add, &devices_to_add, &record_sets_to_add)
            .await {
                Ok(_) => debug!("Added users, devices, and record sets successfully"),
                Err(e) => {
                    error!(error = %e, "Failed to add users, devices, and record sets");
                    return Err(e);
                }
            };

        // Apply operations for existing records
        debug!(
            operations_count = operations_to_apply.len(),
            "Applying operations for existing records"
        );
        for (i, ops) in operations_to_apply.iter().enumerate() {
            trace!(
                index = i,
                records_to_add = ops.records_to_add.len(),
                status_records_to_add = ops.status_records_to_add.len(),
                "Applying operation"
            );
            
            match self.db.apply_sync_operations(ops).await {
                Ok(_) => trace!(index = i, "Applied operation successfully"),
                Err(e) => {
                    error!(
                        error = %e,
                        index = i,
                        "Failed to apply operation"
                    );
                    return Err(e);
                }
            }
        }

        // Log final operation counts
        info!(
            records_to_add = combined_remote_operations.records_to_add.len(),
            status_records_to_add = combined_remote_operations.status_records_to_add.len(),
            record_ids_to_update = combined_remote_operations.record_ids_to_update.len(),
            status_ids_to_update = combined_remote_operations.status_ids_to_update.len(),
            "User sync processed successfully"
        );
        
        // Return the combined remote operations as the acknowledgment
        Ok(SyncAckType::FullSync(combined_remote_operations))
    }


    #[instrument(
        skip(self, sync_record, device_records, device_record_statuses, devices), 
        fields(
            sync_record_id = %sync_record.id,
            device_records = device_records.len(),
            device_record_statuses = device_record_statuses.len(),
            current_device_id = %current_device_id,
            device_count = devices.len()
        ),
        level = "debug"
    )]
    pub async fn prepare_common_sync_data(
        &self,
        sync_record: &SyncRecord,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        current_device_id: &str,
        devices: Vec<Device>,
    ) -> Result<(SyncMergeResult, bool), RepositoryError> {
        debug!("Preparing common sync data");
        
        let mut record_exists= false;
        
        // Check if this sync record already exists by ID directly
        debug!("Checking if sync record exists");
        let existing_record = match self
            .sync_repository
            .get_sync_record_by_id(&sync_record.id)
            .await {
                Ok(record) => {
                    debug!(record_exists = record.is_some(), "Sync record lookup result");
                    record
                },
                Err(e) => {
                    error!(error = %e, "Failed to get sync record by ID");
                    return Err(e);
                }
            };

        // If the sync record exists, fetch existing device records and statuses
        if existing_record.is_some() {
            debug!("Sync record exists, fetching device records and statuses");
            record_exists = true;
            
            // Get existing device records and statuses in one call
            let (local_device_records, local_device_statuses) = match self
                .sync_repository
                .get_device_records_and_statuses_by_sync_record(&sync_record.id)
                .await {
                    Ok((records, statuses)) => {
                        debug!(
                            record_count = records.len(),
                            status_count = statuses.len(),
                            "Retrieved device records and statuses"
                        );
                        (records, statuses)
                    },
                    Err(e) => {
                        error!(
                            error = %e,
                            sync_record_id = %sync_record.id,
                            "Failed to get device records and statuses"
                        );
                        return Err(e);
                    }
                };

            // Perform the merge operation
            debug!("Performing merge operation for existing record");
            let merge_result = SyncRecord::merge_sync_records(
                &local_device_records,
                &local_device_statuses,
                device_records,
                device_record_statuses,
                current_device_id,
            );
            
            debug!(
                local_records_to_add = merge_result.local_operations.records_to_add.len(),
                local_statuses_to_add = merge_result.local_operations.status_records_to_add.len(),
                remote_records_to_add = merge_result.remote_operations.records_to_add.len(),
                remote_statuses_to_add = merge_result.remote_operations.status_records_to_add.len(),
                "Merge operation completed for existing record"
            );

            // Return the merge result directly
            return Ok((merge_result, record_exists));
        }

        // If no existing record was found, this is a new sync record
        debug!("Sync record doesn't exist, processing device records for new record");
        
        // Process device records using the existing logic - this updates sync flags for records associated with current_device_id
        let (processed_records, processed_statuses, updated_record_ids, updated_status_ids) =
            SyncRecord::process_device_records(
                device_records,
                device_record_statuses,
                current_device_id,
            );
            
        debug!(
            processed_records = processed_records.len(),
            processed_statuses = processed_statuses.len(),
            updated_record_ids = updated_record_ids.len(),
            updated_status_ids = updated_status_ids.len(),
            "Processed device records for new record"
        );

        // Create completion records for the current device
        debug!("Creating completion records");
        let completion_records = SyncRecord::create_completion_records(
            sync_record.id.clone(),
            current_device_id.to_string(),
            &devices,
        );
        
        debug!(
            completion_record_id = %completion_records.device_record.id,
            status_record_count = completion_records.device_record_statuses.len(),
            "Completion records created"
        );

        // Construct a new merge result for the new record
        debug!("Constructing merge result for new record");
        let mut merge_result = SyncMergeResult::new();

        // For a new record, all processed records and statuses go into local_operations
        merge_result.local_operations.records_to_add = processed_records;
        merge_result
            .local_operations
            .records_to_add
            .push(completion_records.device_record.clone());
        merge_result.local_operations.status_records_to_add = processed_statuses;
        merge_result
            .local_operations
            .status_records_to_add
            .extend(completion_records.device_record_statuses.clone());

        // For the remote operations, we need to:
        // 1. Send back our completion record so the sender knows we processed their sync
        merge_result
            .remote_operations
            .records_to_add
            .push(completion_records.device_record);
        merge_result.remote_operations.status_records_to_add =
            completion_records.device_record_statuses;

        // 2. Tell the remote device which records and statuses we've updated locally
        // These are the IDs returned by process_device_records - they indicate which records
        // were marked as synced=true on our side and need to be updated on the remote side
        merge_result.remote_operations.record_ids_to_update = updated_record_ids;
        merge_result.remote_operations.status_ids_to_update = updated_status_ids;

        debug!(
            local_records_to_add = merge_result.local_operations.records_to_add.len(),
            local_statuses_to_add = merge_result.local_operations.status_records_to_add.len(),
            remote_records_to_add = merge_result.remote_operations.records_to_add.len(),
            remote_statuses_to_add = merge_result.remote_operations.status_records_to_add.len(),
            "Merge result constructed for new record"
        );
        
        info!("Common sync data prepared successfully");
        Ok((merge_result, record_exists))
    }


    #[instrument(
        skip(self,doc ), 
        fields(
            add_vector= %add_vector.len(),
            update_vector= update_vector.len(),
            resource_id= %resource_id
        ),
        level = "debug"
    )]
    pub async fn merge_updated_doc(
        &self,
        doc: &str,
        add_vector: &[ResourceVectorClock],
        update_vector: &[ResourceVectorClock],
        resource_id: &str,
        current_span: Span,
    ) -> Result<(), RepositoryError> {
        // Use db transaction method for merging updated document
        let _guard = current_span.enter();
        self.db
            .merge_document(doc, add_vector, update_vector, resource_id)
            .await
    }
}
