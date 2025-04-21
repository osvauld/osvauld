use osvauld_core::models::sync_types::OperationType;
use osvauld_core::models::{device::Device, sync_types::ResourceType};
use osvauld_core::models::p2p::{SyncPayload, PhaseType};
use osvauld_core::models::sync_record::SyncRecordSet;
use osvauld_core::models::user::User;
use osvauld_core::repositories::RepositoryError;
use tracing::{Span, info, debug, error, instrument, trace};
use std::sync::Arc;
use tokio::sync::Mutex;
use super::sync_service_core::SyncService;

// Implement methods related to sending/retrieving sync data on SyncService
impl SyncService {
pub async fn get_next_pending_sync(
    &self,
    device: &Device,
    user: &User,
    pending_resource_ids: Option<Arc<Mutex<Vec<String>>>>,
    current_phase: PhaseType,
    current_span: Span,
) -> Result<Option<SyncPayload>, RepositoryError> {
    let _guard = current_span.enter();
    
    match current_phase {
        PhaseType::UserSync => {
            debug!("Checking for user syncs for UserSync phase");
                self.get_user_sync_for_device(device).await
            }
        PhaseType::DeviceSync => {
            debug!("Checking for device syncs for DeviceSync phase");
            self.get_device_sync_for_device(device).await
        },
        PhaseType::FolderSync => {
            debug!("Checking for folder syncs for FolderSync phase");
            self.get_folder_sync_for_device(device).await
        },
        PhaseType::ResourceSync => {
            debug!("Checking for resource syncs for ResourceSync phase");
            self.get_resource_sync_for_device(device, user).await
        },
        PhaseType::ShareSync => {
            debug!("Checking for share syncs for ShareSync phase");
            self.get_share_sync_for_device(device).await
        },
        PhaseType::UpdateSync => {
            debug!("Checking for pending resource updates");
            if let Some(pending_resources) = pending_resource_ids {
                // Lock the mutex to access the vector
                let mut resources = pending_resources.lock().await;
                // If we have any pending resources, pop one
                if !resources.is_empty() {
                    let resource_id = resources.remove(0); // Pop the first item
                    debug!(
                        resource_id = %resource_id,
                        remaining_resources = resources.len(),
                        "Processing pending resource update"
                    );
                    // Get the resource data for update
                    match self.get_resource_for_update(&resource_id).await {
                        Ok(payload) => {
                            info!(
                                sync_type = "resource_update",
                                resource_id = %resource_id,
                                "Found resource update"
                            );
                            return Ok(Some(payload));
                        },
                        Err(e) => {
                            error!(
                                error = %e,
                                resource_id = %resource_id,
                                "Failed to get resource for update"
                            );
                            return Err(e);
                        }
                    }
                }
            }
            Ok(None)
        },
        PhaseType::DeviceRecordSync => {
            debug!("Checking for device record syncs for DeviceRecordSync phase");
            self.get_unsynced_device_records(device).await
        },
        _ => {
            debug!("No sync data for phase: {:?}", current_phase);
            Ok(None)
        }
    }
}

    #[instrument(
        skip(self, device), 
        fields(
            device_id = %device.id
        ),
        level = "debug"
    )]
    async fn get_device_sync_for_device(
        &self,
        device: &Device,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        debug!("Looking for pending device sync");
        
        match self.sync_repository.get_pending_sync_by_type(&device.id, "device").await {
            Ok(Some((sync_record, device_records, statuses))) => {
                debug!(
                    sync_record_id = %sync_record.id,
                    resource_id = %sync_record.resource_id,
                    device_records = device_records.len(),
                    status_records = statuses.len(),
                    "Found pending device sync"
                );
                
                // Get the device data
                debug!("Retrieving device data");
                match self.device_repository.find_by_id(&sync_record.resource_id).await {
                    Ok(device_data) => {
                        debug!(
                            device_data_id = %device_data.id,
                            "Retrieved device data"
                        );
                        
                        Ok(Some(SyncPayload::DeviceSync {
                            sync_record,
                            device_records,
                            device_record_statuses: statuses,
                            device: device_data,
                        }))
                    },
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %sync_record.resource_id,
                            "Failed to retrieve device data"
                        );
                        Err(e)
                    }
                }
            },
            Ok(None) => {
                debug!("No pending device sync found");
                Ok(None)
            },
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device.id,
                    "Error retrieving pending device sync"
                );
                Err(e)
            }
        }
    }

    #[instrument(
        skip(self, device), 
        fields(
            device_id = %device.id
        ),
        level = "debug"
    )]
    async fn get_user_sync_for_device(
        &self,
        device: &Device,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        debug!("Looking for pending user syncs");
        
        // Get all pending user sync records for this device
        match self.sync_repository.get_all_pending_syncs_by_type(&device.id, "user").await {
            Ok(Some(sync_data)) => {
                debug!(
                    sync_data_count = sync_data.len(),
                    "Found pending user syncs"
                );
                
                // Extract all unique user IDs from the sync records
                let user_ids: Vec<String> = sync_data
                    .iter()
                    .map(|(record, _, _)| record.resource_id.clone())
                    .collect::<std::collections::HashSet<String>>()
                    .into_iter()
                    .collect();
                
                debug!(
                    unique_user_ids = user_ids.len(),
                    "Extracted unique user IDs"
                );

                // Build user_data collection
                debug!("Building user data collection");
                let mut user_data = Vec::new();

                for (i, user_id) in user_ids.iter().enumerate() {
                    trace!(
                        index = i,
                        user_id = %user_id,
                        "Processing user data"
                    );
                    
                    // Get user data
                    match self.user_repository.get_user_by_id(user_id).await {
                        Ok(user) => {
                            trace!(
                                user_id = %user.id,
                                "Retrieved user data"
                            );
                            
                            // Get devices for this user
                            match self.device_repository.get_devices_by_user_id(user_id).await {
                                Ok(user_devices) => {
                                    trace!(
                                        user_id = %user_id,
                                        device_count = user_devices.len(),
                                        "Retrieved user devices"
                                    );
                                    
                                    user_data.push((user, user_devices));
                                },
                                Err(e) => {
                                    error!(
                                        error = %e,
                                        user_id = %user_id,
                                        "Failed to retrieve devices for user"
                                    );
                                    return Err(e);
                                }
                            }
                        },
                        Err(e) => {
                            error!(
                                error = %e,
                                user_id = %user_id,
                                "Failed to retrieve user data"
                            );
                            return Err(e);
                        }
                    }
                }
                
                debug!(
                    user_data_count = user_data.len(),
                    "User data collection built"
                );

                // Return the batch payload
                Ok(Some(SyncPayload::UserSync {
                    sync_data,
                    user_data,
                }))
            },
            Ok(None) => {
                debug!("No pending user syncs found");
                Ok(None)
            },
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device.id,
                    "Error retrieving pending user syncs"
                );
                Err(e)
            }
        }
    }

    #[instrument(
        skip(self, device), 
        fields(
            device_id = %device.id
        ),
        level = "debug"
    )]
    async fn get_folder_sync_for_device(
        &self,
        device: &Device,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        debug!("Looking for pending folder sync");
        
        match self.sync_repository.get_pending_sync_by_type(&device.id, "folder").await {
            Ok(Some((sync_record, device_records, statuses))) => {
                debug!(
                    sync_record_id = %sync_record.id,
                    resource_id = %sync_record.resource_id,
                    device_records = device_records.len(),
                    status_records = statuses.len(),
                    "Found pending folder sync"
                );
                
                // Get the folder data
                debug!("Retrieving folder data");
                match self.folder_repository.find_by_id(&sync_record.resource_id).await {
                    Ok(folder) => {
                        debug!(
                            folder_id = %folder.id,
                            "Retrieved folder data"
                        );
                        
                        Ok(Some(SyncPayload::FolderSync {
                            sync_record,
                            device_records,
                            device_record_statuses: statuses,
                            folder,
                        }))
                    },
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %sync_record.resource_id,
                            "Failed to retrieve folder data"
                        );
                        Err(e)
                    }
                }
            },
            Ok(None) => {
                debug!("No pending folder sync found");
                Ok(None)
            },
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device.id,
                    "Error retrieving pending folder sync"
                );
                Err(e)
            }
        }
    }

    #[instrument(
        skip(self, device, user), 
        fields(
            device_id = %device.id,
            user_id = %user.id
        ),
        level = "debug"
    )]
    async fn get_resource_sync_for_device(
        &self,
        device: &Device,
        user: &User,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        debug!("Looking for pending resource sync");
        
        match self.sync_repository.get_pending_sync_by_type(&device.id, "resource").await {
            Ok(Some((sync_record, device_records, statuses))) => {
                debug!(
                    sync_record_id = %sync_record.id,
                    resource_id = %sync_record.resource_id,
                    device_records = device_records.len(),
                    status_records = statuses.len(),
                    "Found pending resource sync"
                );
                
                // Get the resource with key
                debug!("Retrieving resource with key");
                let resource = match self.resource_repository.find_resource_with_key(&sync_record.resource_id, &user.id).await {
                    Ok(resource) => {
                        debug!(
                            resource_id = %resource.resource.id,
                            "Retrieved resource data"
                        );
                        resource
                    },
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %sync_record.resource_id,
                            user_id = %user.id,
                            "Failed to retrieve resource with key"
                        );
                        return Err(e);
                    }
                };

                debug!("Retrieving vector clocks for resource");
                let vector_clocks = match self.vector_clock_repository.get_vector_clocks_for_resource(&sync_record.resource_id).await {
                    Ok(clocks) => {
                        debug!(
                            clock_count = clocks.len(),
                            "Retrieved vector clocks"
                        );
                        clocks
                    },
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %sync_record.resource_id,
                            "Failed to retrieve vector clocks"
                        );
                        return Err(e);
                    }
                };

                Ok(Some(SyncPayload::ResourceSync {
                    sync_record,
                    device_records,
                    device_record_statuses: statuses,
                    resource,
                    vector_clocks,
                }))
            },
            Ok(None) => {
                debug!("No pending resource sync found");
                Ok(None)
            },
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device.id,
                    "Error retrieving pending resource sync"
                );
                Err(e)
            }
        }
    }

    #[instrument(
        skip(self, device), 
        fields(
            device_id = %device.id
        ),
        level = "debug"
    )]
    async fn get_share_sync_for_device(
        &self,
        device: &Device,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        debug!("Looking for pending share sync");
        
        match self.sync_repository.get_pending_sync_by_type(&device.id, "share").await {
            Ok(Some((sync_record, device_records, statuses))) => {
                debug!(
                    sync_record_id = %sync_record.id,
                    resource_id = %sync_record.resource_id,
                    device_records = device_records.len(),
                    status_records = statuses.len(),
                    "Found pending share sync"
                );
                
                // Get the share record
                debug!("Retrieving share record");
                match self.share_repository.find_by_id(&sync_record.resource_id).await {
                    Ok(share_record) => {
                        debug!(
                            share_record_id = %share_record.id,
                            "Retrieved share record"
                        );
                        
                        Ok(Some(SyncPayload::ShareSync {
                            sync_record,
                            device_records,
                            device_record_statuses: statuses,
                            share_record,
                        }))
                    },
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %sync_record.resource_id,
                            "Failed to retrieve share record"
                        );
                        Err(e)
                    }
                }
            },
            Ok(None) => {
                debug!("No pending share sync found");
                Ok(None)
            },
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device.id,
                    "Error retrieving pending share sync"
                );
                Err(e)
            }
        }
    }

    #[instrument(
        skip(self), 
        fields(
            resource_id = %resource_id
        ),
        level = "debug"
    )]
    async fn get_resource_for_update(
        &self,
        resource_id: &str,
    ) -> Result<SyncPayload, RepositoryError> {
        debug!("Getting resource for update");
        
        let resource = match self.resource_repository.find_by_id_raw(resource_id).await {
            Ok(res) => {
                debug!(
                    resource_id = %res.id,
                    "Retrieved resource data"
                );
                res
            },
            Err(e) => {
                error!(
                    error = %e,
                    resource_id = %resource_id,
                    "Failed to retrieve resource data"
                );
                return Err(e);
            }
        };
        
        let vector_clocks = match self.vector_clock_repository.get_vector_clocks_for_resource(resource_id).await {
            Ok(clocks) => {
                debug!(
                    clock_count = clocks.len(),
                    "Retrieved vector clocks"
                );
                clocks
            },
            Err(e) => {
                error!(
                    error = %e,
                    resource_id = %resource_id,
                    "Failed to retrieve vector clocks"
                );
                return Err(e);
            }
        };
        
        debug!("Prepared resource update payload");
        Ok(SyncPayload::ResourceUpdate {
            resource,
            vector_clocks,
        })
    }

    #[instrument(
        skip(self, device), 
        fields(
            device_id = %device.id
        ),
        level = "debug"
    )]
    async fn get_unsynced_device_records(
        &self,
        device: &Device,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        debug!("Looking for unsynced device records");
        
        match self.sync_repository.get_unsynced_device_sync_records(&device.id).await {
            Ok(unsynced_records) => {
                if !unsynced_records.is_empty() {
                    debug!(
                        record_count = unsynced_records.len(),
                        "Found unsynced device records"
                    );
                    return Ok(Some(SyncPayload::StatusUpdate(unsynced_records)));
                }
                
                debug!("No unsynced device records found");
                Ok(None)
            },
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device.id,
                    "Error retrieving unsynced device records"
                );
                Err(e)
            }
        }
    }

    #[instrument(
        skip(self), 
        fields(
            device_id = %device_id
        ),
        level = "info"
    )]
    pub async fn get_resources_needing_sync(
        &self,
        device_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        info!("Getting resources needing sync");
        
        // Check if device exists
        let device = match self.device_repository.find_by_id(device_id).await {
            Ok(device) => {
                debug!(
                    device_id = %device.id,
                    "Retrieved device data"
                );
                device
            },
            Err(RepositoryError::NotFound) => {
                debug!(
                    device_id = %device_id,
                    "Device not found, returning empty list"
                );
                // Device not found, return empty
                return Ok(Vec::new());
            },
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device_id,
                    "Error retrieving device"
                );
                return Err(e);
            }
        };

        // If device has never synced, return empty list
        if device.last_synced_at.is_none() {
            info!(
                device_id = %device_id,
                "Device has never synced, returning empty list"
            );
            return Ok(Vec::new());
        }

        let last_synced_at = device.last_synced_at.unwrap();
        debug!(
            last_synced_at = ?last_synced_at,
            "Using last synced timestamp"
        );

        // Get all other devices
        debug!("Getting all other devices");
        let all_other_devices = match self.device_repository.get_all_devices_except(&[device_id.to_string()]).await {
            Ok(devices) => {
                debug!(
                    device_count = devices.len(),
                    "Retrieved other devices"
                );
                devices
            },
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device_id,
                    "Error retrieving other devices"
                );
                return Err(e);
            }
        };

        if all_other_devices.is_empty() {
            // No other devices to sync with
            info!(
                device_id = %device_id,
                "No other devices to sync with, returning empty list"
            );
            return Ok(Vec::new());
        }

        debug!("Getting resource IDs for device");
        let device_resource_ids = match self.sync_repository.get_resource_ids_for_device(&device.id).await {
            Ok(ids) => {
                debug!(
                    resource_id_count = ids.len(),
                    "Retrieved resource IDs for device"
                );
                ids
            },
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device_id,
                    "Error retrieving resource IDs for device"
                );
                return Err(e);
            }
        };
        
        // Get resource IDs that need syncing based on vector clocks
        debug!("Getting resource IDs needing updates");
        match self.vector_clock_repository.get_resource_ids_needing_updates(
            &device_resource_ids, 
            last_synced_at, 
            device_id
        ).await {
            Ok(needs_sync) => {
                info!(
                    resources_needing_sync = needs_sync.len(),
                    "Found resources needing sync"
                );
                Ok(needs_sync)
            },
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device_id,
                    "Error determining resources needing updates"
                );
                Err(e)
            }
        }
    }

    #[instrument(
        skip(self), 
        fields(
            user_id = %user_id
        ),
        level = "debug"
    )]
    pub async fn get_payload_for_first_user_sync(
        &self,
        user_id: &str,
    ) -> Result<(User, Vec<Device>), RepositoryError> {
        debug!("Getting payload for first user sync");
        
        let devices = match self.device_repository.get_devices_by_user_id(user_id).await {
            Ok(devices) => {
                debug!(
                    device_count = devices.len(),
                    "Retrieved devices for user"
                );
                devices
            },
            Err(e) => {
                error!(
                    error = %e,
                    user_id = %user_id,
                    "Error retrieving devices for user"
                );
                return Err(e);
            }
        };
        
        let user = match self.user_repository.get_user_by_id(user_id).await {
            Ok(user) => {
                debug!(
                    user_id = %user.id,
                    "Retrieved user data"
                );
                user
            },
            Err(e) => {
                error!(
                    error = %e,
                    user_id = %user_id,
                    "Error retrieving user"
                );
                return Err(e);
            }
        };
        
        info!("Prepared first user sync payload");
        Ok((user, devices))
    }


     #[instrument(
        skip(self, device_id),
        fields(
            device_id = %device_id
        ),
        level = "info"
    )]
    pub async fn get_add_device_record_set(
        &self,
        device_id: &str
    ) -> Result<SyncRecordSet, RepositoryError> {
        info!("Retrieving device sync record set");
        
        // Use the repository method to get all sync data at once
        let sync_data = match self.sync_repository.find_sync_record_set_by_resource(
            device_id,
            &ResourceType::Device.to_string(),
           &OperationType::Create.to_string(), 
        ).await {
            Ok(Some((sync_record, device_records, device_record_statuses))) => {
                debug!(
                    sync_record_id = %sync_record.id,
                    device_records_count = device_records.len(),
                    statuses_count = device_record_statuses.len(),
                    "Retrieved device sync record set"
                );
                
                // Create and return the SyncRecordSet
                let record_set = SyncRecordSet {
                    sync_record,
                    device_records,
                    device_record_statuses,
                };
                
                info!("Device sync record set retrieved successfully");
                Ok(record_set)
            },
            Ok(None) => {
                error!(
                    device_id = %device_id,
                    "No sync record found for device"
                );
                Err(RepositoryError::NotFound)
            },
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device_id,
                    "Error retrieving device sync record set"
                );
                Err(e)
            }
        };
        
        sync_data
    }
}
