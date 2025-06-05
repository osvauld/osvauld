use super::sync_service_core::SyncService;
use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::{DeviceConnection, NetworkSyncPayload};
use osvauld_core::models::sync_record::{SyncRecord, SyncRecordSet};
use osvauld_core::models::sync_types::{OperationType, ResourceType, SyncOperations};
use osvauld_core::models::user::User;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::RepositoryError;
use std::collections::HashSet;
use tracing::{Span, debug, error, info, instrument};

#[derive(Debug)]
pub struct ProcessedRecordResult {
    pub updated_record_set: SyncRecordSet,
    pub local_operations: SyncOperations,
    pub remote_operations: SyncOperations,
    pub record_exists: bool,
}
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
            DeviceConnection::Request {
                device,
                sync_record_set,
            } => {
                info!(
                    device_id = %device.id,
                    "Processing device connection request"
                );

                // Process request and generate comprehensive response
                match self
                    .process_first_device_connection(
                        device,
                        sync_record_set,
                        user_id,
                        current_device_id,
                    )
                    .await
                {
                    Ok(response) => {
                        info!("Device connection request processed successfully");
                        Ok(Some(response))
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to process device connection request");
                        Err(e)
                    }
                }
            }

            DeviceConnection::Response {
                user_devices,
                sync_record_sets,
                external_users,
                external_devices,
            } => {
                info!(
                    user_devices_count = user_devices.len(),
                    external_device_count = external_devices.len(),
                    record_count = sync_record_sets.len(),
                    "Processing device connection response"
                );

                // Process response and send acknowledgment
                match self
                    .process_device_connection_response(
                        user_devices,
                        external_devices,
                        external_users,
                        sync_record_sets,
                        user_id,
                        current_device_id,
                    )
                    .await
                {
                    Ok(ack) => {
                        info!("Device connection response processed successfully");
                        Ok(Some(ack))
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to process device connection response");
                        Err(e)
                    }
                }
            }

            DeviceConnection::Acknowledgment { operations } => {
                info!(
                    completion_records = operations.len(),
                    "Processing device connection acknowledgment"
                );

                // Process acknowledgment and generate complete message
                match self
                    .handle_device_connection_ack(operations, current_device_id)
                    .await
                {
                    Ok(complete) => {
                        info!("Device connection acknowledgment processed successfully");
                        Ok(Some(complete))
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to process device connection acknowledgment");
                        Err(e)
                    }
                }
            }

            DeviceConnection::Complete {
                device_record_status_ids,
            } => {
                info!(
                    status_id_count = device_record_status_ids.len(),
                    "Processing device connection complete"
                );

                // Process complete message (final step)
                match self
                    .process_device_connection_complete(
                        device_record_status_ids,
                        current_device_id,
                        current_span.clone(),
                    )
                    .await
                {
                    Ok(_) => {
                        info!("Device connection complete processed successfully");
                        Ok(None) // No further response needed
                    }
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
        let user_devices = self
            .device_repository
            .get_devices_by_user_id(user_id)
            .await?;
        debug!(device_count = &user_devices.len(), "Retrieved user devices");
        all_devices.extend(user_devices.clone());
        let external_users = self.user_repository.get_known_users().await?;
        let external_user_ids: Vec<String> =
            external_users.iter().map(|user| user.id.clone()).collect();
        let external_devices = self
            .device_repository
            .get_devices_by_user_ids(&external_user_ids)
            .await?;
        all_devices.extend(external_devices.clone());
        let all_sync_and_device_records = self
            .sync_repository
            .get_all_sync_records_with_device_records()
            .await?;

        let device_sync_set = SyncRecord::create_initial_device_sync_records(
            device.clone(),
            current_device_id.to_string(),
            &all_sync_and_device_records,
            &user_devices,
            sync_record_set.clone(),
        );
        let network_awareness_set = SyncRecord::create_network_device_sync_record(
            device.id.clone(),
            current_device_id.to_string(),
            &all_devices,
        );

        let resource_ids: Vec<String> = all_sync_and_device_records
            .iter()
            .filter_map(|record_pair| {
                if record_pair.sync_record.resource_type == ResourceType::Resource
                    && record_pair.sync_record.operation_type == OperationType::Create
                {
                    Some(record_pair.sync_record.resource_id.clone())
                } else {
                    None
                }
            })
            .collect();
        let vector_clocks =
            ResourceVectorClock::create_entries_for_new_device(&resource_ids, &device.id);

        self.db
            .save_device_sync(
                device,
                &device_sync_set,
                &vector_clocks,
                &network_awareness_set,
            )
            .await?;
        let sync_record_sets = self
            .sync_repository
            .get_sync_records_for_new_device(user_id, &device.id)
            .await?;
        Ok(DeviceConnection::Response {
            user_devices,
            sync_record_sets,
            external_devices,
            external_users,
        })
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
            }
            Err(e) => {
                error!(error = %e, "Failed to get user devices");
                return Err(e);
            }
        };

        let mut all_devices = Vec::new();
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
            let (merge_result, record_exists) = match self
                .prepare_common_sync_data(
                    &sync_record_set.sync_record,
                    &sync_record_set.device_records,
                    &sync_record_set.device_record_statuses,
                    current_device_id,
                    all_devices.clone(),
                )
                .await
            {
                Ok(result) => result,
                Err(e) => return Err(e),
            };

            // Collect operations for database transaction
            if !record_exists {
                // Create record set for new record
                let record_set = SyncRecordSet {
                    sync_record: sync_record_set.sync_record.clone(),
                    device_records: merge_result.local_operations.records_to_add.clone(),
                    device_record_statuses: merge_result
                        .local_operations
                        .status_records_to_add
                        .clone(),
                };

                record_sets_to_add.push(record_set);
            } else {
                // Apply operations for existing record
                operations_to_apply.push(merge_result.local_operations.clone());
            }
            return_payload.push(merge_result.remote_operations);
        }
        self.db
            .commit_device_connection_response(
                &user_devices,
                &external_devices,
                &external_users,
                &record_sets_to_add,
                &operations_to_apply,
            )
            .await?;
        Ok(DeviceConnection::Acknowledgment {
            operations: return_payload,
        })
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
            match self
                .process_fullsync_acknowledgment(operation.clone(), current_device_id)
                .await
            {
                Ok(Some(device_record_ids)) => {
                    debug!(
                        record_count = device_record_ids.len(),
                        "Adding device record IDs to result"
                    );
                    all_device_record_status_ids.extend(device_record_ids);
                }
                Ok(None) => {
                    debug!("No device record IDs to add for this operation");
                }
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
    #[instrument(
    skip(self, device_record_status_ids, current_device_id, current_span),
    fields(
        current_device_id = %current_device_id,
        status_id_count = device_record_status_ids.len()
    ),
    level = "info"
)]
    pub async fn process_device_connection_complete(
        &self,
        device_record_status_ids: &[String],
        current_device_id: &str,
        current_span: Span,
    ) -> Result<(), RepositoryError> {
        self.handle_ack_complete(device_record_status_ids.to_vec(), current_span)
            .await
    }

    #[instrument(
        skip(self, current_span),
        fields(peer_device_id = %peer_device_id),
        level = "info"
    )]
    pub async fn get_network_sync_request_payload(
        &self,
        peer_device_id: &str,
        current_span: Span,
    ) -> Result<NetworkSyncPayload, RepositoryError> {
        let _guard = current_span.enter();

        let new_device_record_sets = self
            .sync_repository
            .get_all_pending_syncs_by_type(peer_device_id, "device", "create")
            .await?
            .unwrap_or_default();

        // 2. Get user sync records
        let new_user_record_sets = self
            .sync_repository
            .get_all_pending_syncs_by_type(peer_device_id, "user", "create")
            .await?
            .unwrap_or_default();

        // 3. Get network device sync records
        let network_device_record_sets = self
            .sync_repository
            .get_all_pending_syncs_by_type(peer_device_id, "network_device", "create")
            .await?
            .unwrap_or_default();
        let device_ids = SyncRecordSet::extract_resource_ids(&new_device_record_sets);
        let unknown_devices = self
            .device_repository
            .get_devices_by_ids(&device_ids)
            .await?;
        let known_devices = self
            .device_repository
            .get_all_devices_except(&device_ids)
            .await?;
        let user_ids = SyncRecordSet::extract_resource_ids(&new_user_record_sets);
        let new_users = self
            .user_repository
            .get_users_and_devices_by_user_ids(&user_ids)
            .await?;
        let network_device_ids = SyncRecordSet::extract_resource_ids(&network_device_record_sets);
        let network_devices = self
            .device_repository
            .get_devices_by_ids(&network_device_ids)
            .await?;

        Ok(NetworkSyncPayload::Request {
            new_device_record_sets,
            known_devices,
            unknown_devices,
            new_user_record_sets,
            new_users,
            network_device_record_sets,
            network_devices,
        })
    }

    #[instrument(
        skip(self, payload, current_span),
        fields(
            user_id = %user_id,
            device_id = %current_device_id,
            payload_type = ?std::mem::discriminant(payload)
        ),
        level = "info"
    )]
    pub async fn process_network_sync_payload(
        &self,
        payload: &NetworkSyncPayload,
        user_id: &str,
        current_device_id: &str,
        peer_device_id: &str,
        current_span: Span,
    ) -> Result<Option<NetworkSyncPayload>, RepositoryError> {
        let _guard = current_span.enter();
        info!("Processing device sync payload");

        match payload {
            NetworkSyncPayload::Request {
                new_device_record_sets,
                known_devices,
                unknown_devices,
                new_user_record_sets,
                new_users,
                network_device_record_sets,
                network_devices,
            } => {
                // Process the request and generate response
                match self
                    .process_device_sync_request(
                        new_device_record_sets,
                        known_devices,
                        unknown_devices,
                        new_user_record_sets,
                        new_users,
                        network_device_record_sets,
                        network_devices,
                        user_id,
                        current_device_id,
                        peer_device_id,
                    )
                    .await
                {
                    Ok(response) => {
                        info!("Device sync request processed successfully");
                        Ok(Some(response))
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to process device sync request");
                        Err(e)
                    }
                }
            }

            NetworkSyncPayload::Response { .. } => {
                info!("Processing device sync response (Phase 2) - TODO");
                // TODO: Implement Phase 2 processing
                Ok(None)
            }

            NetworkSyncPayload::SyncRecordExchange { .. } => {
                info!("Processing sync record exchange (Phase 3) - TODO");
                // TODO: Implement Phase 3 processing
                Ok(None)
            }

            NetworkSyncPayload::DeviceRecordRequest { .. } => {
                info!("Processing device record request (Phase 4) - TODO");
                // TODO: Implement Phase 4 processing
                Ok(None)
            }

            NetworkSyncPayload::DeviceRecordExchange { .. } => {
                info!("Processing device record exchange (Phase 5) - TODO");
                // TODO: Implement Phase 5 processing
                Ok(None)
            }

            NetworkSyncPayload::Acknowledgment { .. } => {
                info!("Processing acknowledgment (Phase 6) - TODO");
                // TODO: Implement Phase 6 processing
                Ok(None)
            }

            NetworkSyncPayload::Complete => {
                info!("Device sync complete");
                Ok(None)
            }
        }
    }

    /// Process Phase 1: Device Discovery Request
    #[instrument(skip_all, level = "info")]
    async fn process_device_sync_request(
        &self,
        remote_device_record_sets: &[SyncRecordSet],
        initiator_devices_known_by_acceptor: &[Device],
        initiator_devices_unknown_to_acceptor: &[Device],
        remote_user_record_sets: &[SyncRecordSet],
        remote_unknown_users: &Vec<(User, Vec<Device>)>,
        remote_network_device_record_sets: &[SyncRecordSet],
        remote_network_devices: &[Device],
        user_id: &str,
        current_device_id: &str,
        peer_device_id: &str,
    ) -> Result<NetworkSyncPayload, RepositoryError> {
        info!("Processing device sync request - identifying new devices");

        // Step 1: Get what B thinks A doesn't know about (B's perspective)
        debug!("Getting what B thinks A doesn't know about");
        let acceptor_payload = self
            .get_network_sync_request_payload(
                peer_device_id, // A's device ID - get pending syncs for A
                tracing::Span::current(),
            )
            .await?;

        let (
            local_device_record_sets,
            acceptor_devices_unknown_to_initiator,
            acceptor_devices_known_by_initiator,
            local_user_record_sets,
            acceptor_unknown_users,
            local_network_device_record_sets,
            local_network_devices,
        ) = if let NetworkSyncPayload::Request {
            new_device_record_sets,
            known_devices,
            unknown_devices,
            new_user_record_sets,
            new_users,
            network_device_record_sets,
            network_devices,
        } = acceptor_payload
        {
            // Early termination check - both sides have empty unknown lists
            
            (
                new_device_record_sets,
                unknown_devices,
                known_devices,
                new_user_record_sets,
                new_users,
                network_device_record_sets,
                network_devices,
            )
        } else {
            return Err(RepositoryError::CustomError(
                "Expected Request payload".to_string(),
            ));
        };

        debug!("Categorizing devices into 4 buckets using both A and B perspectives");
        let all_acceptor_devices: Vec<Device> = acceptor_devices_known_by_initiator
            .iter()
            .chain(acceptor_devices_unknown_to_initiator.iter())
            .cloned()
            .collect();

        let all_initiator_devices: Vec<Device> = initiator_devices_unknown_to_acceptor
            .iter()
            .chain(initiator_devices_known_by_acceptor.iter())
            .cloned()
            .collect();

        let mut seen = HashSet::new();
        let all_devices: Vec<Device> = all_acceptor_devices
            .iter()
            .chain(all_initiator_devices.iter())
            .filter(|device| seen.insert(device.id.clone()))
            .cloned()
            .collect();

        // What B doesn't know vs awareness gap (from A's request)
        let (devices_acceptor_doesnt_know, _devices_initiator_thinks_acceptor_doesnt_know) = self
            .categorize_devices_from_perspective(
                initiator_devices_unknown_to_acceptor,
                &all_acceptor_devices,
            )
            .await;

        // What A doesn't know vs awareness gap (from B's request)
        let (devices_initiator_doesnt_know, _devices_acceptor_thinks_initiator_doesnt_know) = self
            .categorize_devices_from_perspective(
                &acceptor_devices_unknown_to_initiator,
                &all_initiator_devices,
            )
            .await;

        // Process remote device record sets
        let mut updated_remote_device_record_sets = Vec::new();
        let mut all_local_operations = SyncOperations::new();
        let mut all_remote_operations = SyncOperations::new();

        for remote_record in remote_device_record_sets {
            let result = self
                .process_remote_sync_record_set(
                    remote_record,
                    &devices_acceptor_doesnt_know,
                    &all_devices,
                    current_device_id,
                )
                .await?;

            updated_remote_device_record_sets.push(result.updated_record_set);
            all_local_operations.merge(result.local_operations);
            all_remote_operations.merge(result.remote_operations);
        }

        // Process remote user record sets
        let mut updated_remote_user_record_sets = Vec::new();
        for remote_record in remote_user_record_sets {
            let result = self
                .process_remote_sync_record_set(
                    remote_record,
                    &devices_acceptor_doesnt_know,
                    &all_devices,
                    current_device_id,
                )
                .await?;

            updated_remote_user_record_sets.push(result.updated_record_set);
            all_local_operations.merge(result.local_operations);
            all_remote_operations.merge(result.remote_operations);
        }

        // Process remote network device record sets
        let mut updated_remote_network_device_record_sets = Vec::new();
        for remote_record in remote_network_device_record_sets {
            let result = self
                .process_remote_sync_record_set(
                    remote_record,
                    &devices_acceptor_doesnt_know,
                    &all_devices,
                    current_device_id,
                )
                .await?;

            updated_remote_network_device_record_sets.push(result.updated_record_set);
            all_local_operations.merge(result.local_operations);
            all_remote_operations.merge(result.remote_operations);
        }

        // Process local device record sets (for response payload)
        let mut to_merge_local_record_sets = Vec::new();
        let mut updated_local_device_record_sets = Vec::new();
        for local_record in &local_device_record_sets {
            let is_true_unknown_device = devices_initiator_doesnt_know
                .iter()
                .any(|device| device.id == local_record.sync_record.resource_id);

            if is_true_unknown_device {
                let (updated_record, local_ops) = Self::process_local_record_set(
                    local_record,
                    initiator_devices_unknown_to_acceptor,
                    &all_devices,
                    current_device_id,
                )
                .await?;

                updated_local_device_record_sets.push(updated_record);
                all_local_operations.merge(local_ops);
            }else {
                to_merge_local_record_sets.push(local_record);
            }
        }

        // Process local user record sets (for response payload)
        let mut updated_local_user_record_sets = Vec::new();
        for local_record in &local_user_record_sets {
            let (updated_record, local_ops) = Self::process_local_record_set(
                local_record,
                initiator_devices_unknown_to_acceptor,
                &all_devices,
                current_device_id,
            )
            .await?;

            updated_local_user_record_sets.push(updated_record);
            all_local_operations.merge(local_ops);
        }

        // Process local network device record sets (for response payload)
        let mut updated_local_network_device_record_sets = Vec::new();
        for local_record in &local_network_device_record_sets {
            let (updated_record, local_ops) = Self::process_local_record_set(
                local_record,
                initiator_devices_unknown_to_acceptor,
                &all_devices,
                current_device_id,
            )
            .await?;

            updated_local_network_device_record_sets.push(updated_record);
            all_local_operations.merge(local_ops);
        }

        todo!()
        // Return response payload
        // Ok(NetworkSyncPayload::Response {
        //     // Device sync data
        //     device_record_sets: updated_local_device_record_sets,
        //     unknown_devices: devices_initiator_doesnt_know,
        //     known_devices: all_initiator_devices
        //         .into_iter()
        //         .filter(|d| !devices_initiator_doesnt_know.iter().any(|unknown| unknown.id == d.id))
        //         .collect(),
        //
        //     // User sync data
        //     user_record_sets: updated_local_user_record_sets,
        //     unknown_users: acceptor_unknown_users,
        //     known_users: acceptor_known_users,
        //
        //     // Network device sync data
        //     network_device_record_sets: updated_local_network_device_record_sets,
        //
        //     // Operations for remote to apply
        //     remote_operations: all_remote_operations,
        // })
    }
    async fn categorize_devices_from_perspective(
        &self,
        sender_thinks_receiver_doesnt_know: &[Device], // Devices A is sending to B
        all_receiver_devices: &[Device],               // All devices B knows about
    ) -> (Vec<Device>, Vec<Device>) {
        // (unknown, awareness_gap)

        let receiver_device_ids: std::collections::HashSet<String> =
            all_receiver_devices.iter().map(|d| d.id.clone()).collect();

        let mut unknown_devices = Vec::new();
        let mut awareness_gap_devices = Vec::new();

        for device in sender_thinks_receiver_doesnt_know {
            if receiver_device_ids.contains(&device.id) {
                // Receiver actually knows this device - awareness gap
                awareness_gap_devices.push(device.clone());
            } else {
                // Receiver actually doesn't know - unknown device
                unknown_devices.push(device.clone());
            }
        }

        (unknown_devices, awareness_gap_devices)
    }

    async fn process_remote_sync_record_set(
        &self,
        record_set: &SyncRecordSet,
        unknown_devices_on_other_side: &[Device],
        all_devices: &[Device],
        current_device_id: &str,
    ) -> Result<ProcessedRecordResult, RepositoryError> {
        let mut updated_record = SyncRecordSet::empty(record_set.sync_record.clone());
        let mut local_operations = SyncOperations::new();
        let mut remote_operations = SyncOperations::new();
        let does_record_exist_locally = self
            .sync_repository
            .get_sync_record_by_id(&record_set.sync_record.id)
            .await?;
        let mut record_exists = false;

        if does_record_exist_locally.is_none() {
            // Record doesn't exist locally - this is a true unknown

            // Step 1: Process device records and create completion
            let (
                processed_device_records,
                processed_record_status,
                updated_record_ids,
                updated_status_ids,
            ) = SyncRecord::process_device_records(
                &record_set.device_records,
                &record_set.device_record_statuses,
                current_device_id,
            );

            updated_record.extend_device_records_and_statuses(
                processed_device_records,
                processed_record_status,
            );

            remote_operations.add_ids_to_update(updated_record_ids, updated_status_ids);

            // Step 2: Create completion record
            let completion_record = SyncRecord::create_completion_records(
                updated_record.sync_record.id.clone(),
                current_device_id.to_string(),
                &all_devices,
            );

            updated_record.add_device_record_with_statuses(
                completion_record.device_record.clone(),
                completion_record.device_record_statuses.clone(),
            );

            remote_operations.add_single_record_with_statuses(
                completion_record.device_record,
                completion_record.device_record_statuses,
            );

            // Step 3: Create cross-device records if there are unknown devices on other side
            if !unknown_devices_on_other_side.is_empty() {
                let (device_record_set, device_record_statuses) =
                    SyncRecord::create_cross_device_records_for_targets(
                        &record_set.sync_record.id,
                        &record_set.device_records,
                        &all_devices,
                        &unknown_devices_on_other_side,
                        current_device_id,
                    );

                let mut combined_statuses = device_record_set.device_record_statuses.clone();
                combined_statuses.extend(device_record_statuses.clone());

                updated_record.extend_device_records_and_statuses(
                    device_record_set.device_records.clone(),
                    combined_statuses.clone(),
                );

                remote_operations
                    .add_records_and_statuses(device_record_set.device_records, combined_statuses);
            }
        } else {
            record_exists = true;
            // Record exists locally - merge with existing records
            let (local_device_records, local_device_statuses) = self
                .sync_repository
                .get_device_records_and_statuses_by_sync_record(&record_set.sync_record.id)
                .await?;

            let merged_records = SyncRecord::merge_sync_records(
                &local_device_records,
                &local_device_statuses,
                &record_set.device_records,
                &record_set.device_record_statuses,
                current_device_id,
            );

            // Add remote operations (what to send back to remote)
            remote_operations.add_records_and_statuses(
                merged_records.remote_operations.records_to_add,
                merged_records.remote_operations.status_records_to_add,
            );
            remote_operations.add_ids_to_update(
                merged_records.remote_operations.record_ids_to_update,
                merged_records.remote_operations.status_ids_to_update,
            );

            local_operations.add_records_and_statuses(
                merged_records.local_operations.records_to_add,
                merged_records.local_operations.status_records_to_add,
            );

            // Add local operations (what to update in our own database)
            local_operations.add_ids_to_update(
                merged_records.local_operations.record_ids_to_update,
                merged_records.local_operations.status_ids_to_update,
            );
            #[derive(Debug, Clone)]
            pub enum ProcessingMode {
                Remote, // Processing incoming records from remote peer
                Local,  // Processing local records for outgoing payload
            }
        }
        Ok(ProcessedRecordResult {
            updated_record_set: updated_record,
            local_operations,
            remote_operations,
            record_exists,
        })
    }

    pub async fn process_local_record_set(
        record_set: &SyncRecordSet,
        unknown_devices_on_other_side: &[Device],
        all_devices: &[Device],
        current_device_id: &str,
    ) -> Result<(SyncRecordSet, SyncOperations), RepositoryError> {
        let mut updated_record = SyncRecordSet::empty(record_set.sync_record.clone());
        let mut local_operations = SyncOperations::new();
        updated_record.extend_device_records_and_statuses(
            record_set.device_records.clone(),
            record_set.device_record_statuses.clone(),
        );

        // Create cross-device records only if there are unknown devices on remote side
        if !unknown_devices_on_other_side.is_empty() {
            let (device_record_set, device_record_statuses) =
                SyncRecord::create_cross_device_records_for_targets(
                    &record_set.sync_record.id,
                    &record_set.device_records,
                    &all_devices,
                    &unknown_devices_on_other_side,
                    current_device_id,
                );

            let mut combined_statuses = device_record_set.device_record_statuses.clone();
            combined_statuses.extend(device_record_statuses.clone());

            updated_record.extend_device_records_and_statuses(
                device_record_set.device_records.clone(),
                combined_statuses.clone(),
            );

            local_operations
                .add_records_and_statuses(device_record_set.device_records, combined_statuses);
        }
        Ok((updated_record, local_operations))
    }
}
