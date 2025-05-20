use super::sync_service_core::SyncService;
use osvauld_core::models::p2p::DeviceConnection;
use osvauld_core::models::sync_record::{SyncRecord, SyncRecordSet};
use osvauld_core::models::sync_types::{OperationType, ResourceType, SyncOperations};
use osvauld_core::models::device::Device;
use osvauld_core::models::user::User;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::RepositoryError;
use tracing::{Span, debug, error, info, instrument};

impl SyncService {
#[instrument(
    skip(self, payload, current_span), 
    fields(
        user_id = %user_id,
        device_id = %current_device_id,
        payload_type = ?std::mem::discriminant(payload)
    ),
    level = "info"
)]
pub async fn process_device_connection_payload(
    &self,
    payload: &DeviceConnection,
    user_id: &str,
    current_device_id: &str,
    current_span: Span,
) -> Result<Option<DeviceConnection>, RepositoryError> {
    let _guard = current_span.enter();
    info!("Processing device connection payload");
    
    match payload {
        DeviceConnection::Request { device, sync_record_set } => {
            info!(
                device_id = %device.id,
                "Processing device connection request"
            );
            
            // Process request and generate comprehensive response
            match self.process_first_device_connection(
                device,
                sync_record_set,
                user_id,
                current_device_id,
            ).await {
                Ok(response) => {
                    info!("Device connection request processed successfully");
                    Ok(Some(response))
                },
                Err(e) => {
                    error!(error = %e, "Failed to process device connection request");
                    Err(e)
                }
            }
        },
        
        DeviceConnection::Response { 
            user_devices, 
                sync_record_sets,
                external_users,
                external_devices
        } => {
            info!(
                user_devices_count = user_devices.len(),
                external_device_count = external_devices.len(),
                record_count = sync_record_sets.len(),
                "Processing device connection response"
            );
            
            // Process response and send acknowledgment
            match self.process_device_connection_response(
                user_devices,
                    external_devices,
                    external_users,
                sync_record_sets,
                user_id,
                current_device_id,
            ).await {
                Ok(ack) => {
                    info!("Device connection response processed successfully");
                    Ok(Some(ack))
                },
                Err(e) => {
                    error!(error = %e, "Failed to process device connection response");
                    Err(e)
                }
            }
        },
        
        DeviceConnection::Acknowledgment {operations        } => {
            info!(
                completion_records = operations.len(),
                "Processing device connection acknowledgment"
            );
            
            // Process acknowledgment and generate complete message
            match self.handle_device_connection_ack(
                    operations,
                current_device_id,
            ).await {
                Ok(complete) => {
                    info!("Device connection acknowledgment processed successfully");
                    Ok(Some(complete))
                },
                Err(e) => {
                    error!(error = %e, "Failed to process device connection acknowledgment");
                    Err(e)
                }
            }
        },
        
        DeviceConnection::Complete {
            device_record_status_ids,
        } => {
            info!(
                status_id_count = device_record_status_ids.len(),
                "Processing device connection complete"
            );
            
            // Process complete message (final step)
            match self.process_device_connection_complete(
                device_record_status_ids,
                current_device_id,
            ).await {
                Ok(_) => {
                    info!("Device connection complete processed successfully");
                    Ok(None) // No further response needed
                },
                Err(e) => {
                    error!(error = %e, "Failed to process device connection complete");
                    Err(e)
                }
            }
        }
    }
}
#[instrument(
    skip(self, device, sync_record_set), 
    fields(
        device_id = %device.id,
        user_id = %user_id,
        current_device_id = %current_device_id
    ),
    level = "info"
)]
async fn process_first_device_connection(
    &self,
    device: &Device,
    sync_record_set: &SyncRecordSet,
    user_id: &str,
    current_device_id: &str,
) -> Result<DeviceConnection, RepositoryError> {
    info!("Processing first device connection request");
   let mut all_devices = Vec::new(); 
    let user_devices = self.device_repository.get_devices_by_user_id(user_id).await?;
    debug!(device_count = &user_devices.len(), "Retrieved user devices");
    all_devices.extend(user_devices.clone());
    let external_users = self.user_repository.get_known_users().await?;
    let external_user_ids: Vec<String> = external_users.iter().map(|user| user.id.clone()).collect();
    let external_devices = self.device_repository.get_devices_by_user_ids(&external_user_ids).await?; 
        all_devices.extend(external_devices);
    let all_sync_and_device_records = self.sync_repository.get_all_sync_records_with_device_records().await?;
    
    let device_sync_set = SyncRecord::create_initial_device_sync_records(
        device.clone(),
        current_device_id.to_string(),
        &all_sync_and_device_records,
            &all_devices,
        sync_record_set.clone(),
    );
    
let resource_ids: Vec<String> = all_sync_and_device_records
    .iter()
    .filter_map(|record_pair| {
        if record_pair.sync_record.resource_type == ResourceType::Resource 
           && record_pair.sync_record.operation_type == OperationType::Create {
            Some(record_pair.sync_record.resource_id.clone())
        } else {
            None
        }
    })
    .collect();
    let vector_clocks = ResourceVectorClock::create_entires_for_new_device(&resource_ids, &device.id);
    
    self.db.save_device_sync(device, &device_sync_set, &vector_clocks).await?;
    let sync_record_sets= self.sync_repository.get_sync_records_for_new_device(user_id, &device.id).await?;
    let external_devices = self.device_repository.get_devices_by_user_except(user_id, &[device.id.clone()]).await?;
    Ok(DeviceConnection::Response { user_devices, sync_record_sets, external_devices, external_users })
}


    #[instrument(
    skip(self, user_devices, external_users, external_devices, sync_record_sets), 
    fields(
        user_id = %user_id,
        current_device_id = %current_device_id,
        device_count = user_devices.len(),
        record_set_count = sync_record_sets.len()
    ),
    level = "info"
)]
pub async fn process_device_connection_response(
    &self,
    user_devices: &Vec<Device>,
    external_devices: &Vec<Device>,
    external_users: &Vec<User>,
    sync_record_sets: &Vec<SyncRecordSet>,
    user_id: &str,
    current_device_id: &str,
) -> Result<DeviceConnection, RepositoryError> {
    info!("Processing device connection response");
    
    // Get current user's devices to include in the processing
    debug!("Getting current user's devices");
    //currently there is only one device
    let current_device = match self.device_repository.get_devices_by_user_id(user_id).await {
        Ok(devices) => {
            debug!(device_count = devices.len(), "Retrieved user devices");
            devices
        },
        Err(e) => {
            error!(error = %e, "Failed to get user devices");
            return Err(e);
        }
    };
    
    let all_devices = Vec::new();
    all_devices.extend(current_device);
    all_devices.extend(user_devices.clone());
    all_devices.extend(external_devices.clone());
    
    // Initialize collections for database operations
    let mut record_sets_to_add = Vec::new();
    let mut operations_to_apply = Vec::new();
    let mut return_payload = Vec::new();
        for sync_record_set in sync_record_sets {
        debug!(
            record_id = %sync_record_set.sync_record.id,
            "Processing sync record set"
        );
        
        // Process the sync record
        let (merge_result, record_exists) = match self.prepare_common_sync_data(
            &sync_record_set.sync_record,
            &sync_record_set.device_records,
            &sync_record_set.device_record_statuses,
            current_device_id,
            all_devices.clone(),
        ).await {
            Ok(result) => result,
            Err(e) => return Err(e),
        };
        
        // Collect operations for database transaction
        if !record_exists {
            // Create record set for new record
            let record_set = SyncRecordSet {
                sync_record: sync_record_set.sync_record.clone(),
                device_records: merge_result.local_operations.records_to_add.clone(),
                device_record_statuses: merge_result.local_operations.status_records_to_add.clone(),
            };
            
            record_sets_to_add.push(record_set);
        } else {
            // Apply operations for existing record
            operations_to_apply.push(merge_result.local_operations.clone());
        }
            return_payload.push(merge_result.remote_operations);
        }
        self.db.commit_device_connection_response(devices, &record_sets_to_add, &operations_to_apply).await?;
        Ok(DeviceConnection::Acknowledgment { operations: return_payload })
    }
#[instrument(
    skip(self, operations), 
    fields(
        operation_count = operations.len(),
        current_device_id = %current_device_id
    ),
    level = "info"
)]
async fn handle_device_connection_ack(
    &self,
    operations: &Vec<SyncOperations>,
    current_device_id: &str,
) -> Result<DeviceConnection, RepositoryError> {
    info!("Processing device connection acknowledgment");
    
    let mut all_device_record_status_ids = Vec::new();
    
    // Process each operation in the vector
    for (i, operation) in operations.iter().enumerate() {
        debug!(
            index = i,
            records_to_add = operation.records_to_add.len(),
            status_records_to_add = operation.status_records_to_add.len(),
            "Processing operation"
        );
        
        // Process this operation and collect record IDs
        match self.process_fullsync_acknowledgment(operation.clone(), current_device_id).await {
            Ok(Some(device_record_ids)) => {
                debug!(
                    record_count = device_record_ids.len(),
                    "Adding device record IDs to result"
                );
                all_device_record_status_ids.extend(device_record_ids);
            },
            Ok(None) => {
                debug!("No device record IDs to add for this operation");
            },
            Err(e) => {
                error!(
                    error = %e,
                    "Failed to process operation"
                );
                return Err(e);
            }
        }
    }
    
    info!(
        total_status_ids = all_device_record_status_ids.len(),
        "Device connection acknowledgment processed successfully"
    );
    
    // Return the Complete message with all collected record IDs
    Ok(DeviceConnection::Complete {
        device_record_status_ids: all_device_record_status_ids,
    })
}

}
