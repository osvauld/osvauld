use crate::models::device::Device;
use crate::models::resource::Resource;
use crate::models::sync_types::{OperationType, ResourceType, SyncMergeResult, SyncStatus};
use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub enum SyncUpdateData {
    FullSyncSet(SyncRecordSet),
    DeviceRecordSet(DeviceRecordSet),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRecord {
    pub id: String,
    pub resource_id: String,
    pub resource_type: ResourceType,
    pub operation_type: OperationType,
    pub source_device_id: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecord {
    pub id: String,
    pub sync_record_id: String,
    pub device_id: String,
    pub status: SyncStatus,
    pub synced: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecordStatus {
    pub id: String,
    pub device_record_id: String,
    pub aware_device_id: String,
    pub synced: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRecordSet {
    pub sync_record: SyncRecord,
    pub device_records: Vec<DeviceRecord>,
    pub device_record_statuses: Vec<DeviceRecordStatus>,
}

pub struct DeviceRecordSet {
    pub device_records: Vec<DeviceRecord>,
    pub device_record_statuses: Vec<DeviceRecordStatus>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatusChangeSet {
    pub device_record: DeviceRecord,
    pub device_record_statuses: Vec<DeviceRecordStatus>,
}
#[derive(Debug)]
pub struct InitialDeviceSyncSet {
    pub sync_record: SyncRecord,
    pub device_records: Vec<DeviceRecord>,
    pub device_record_statuses: Vec<DeviceRecordStatus>,
}
impl SyncRecord {
    pub fn create_folder_sync_record(
        resource_id: String,
        current_device_id: String,
        devices: &[Device],
    ) -> SyncRecordSet {
        SyncRecord::create_sync_records(
            resource_id,
            ResourceType::Folder,
            OperationType::Create,
            current_device_id,
            devices,
        )
    }

    pub fn create_resource_sync_record(
        resource_id: String,
        current_device_id: String,
        devices: &[Device],
    ) -> SyncRecordSet {
        SyncRecord::create_sync_records(
            resource_id,
            ResourceType::Resource,
            OperationType::Create,
            current_device_id,
            devices,
        )
    }

    pub fn create_share_sync_record(
        resource_id: String,
        current_device_id: String,
        devices: &[Device],
    ) -> SyncRecordSet {
        SyncRecord::create_sync_records(
            resource_id,
            ResourceType::Share,
            OperationType::Create,
            current_device_id,
            devices,
        )
    }
    pub fn create_soft_delete_resource_records(
        resource_id: String,
        current_device_id: String,
        devices: &[Device],
    ) -> SyncRecordSet {
        Self::create_sync_records(
            resource_id,
            ResourceType::Resource,
            OperationType::SoftDelete,
            current_device_id,
            devices,
        )
    }
    pub fn create_signup_device(device: Device) -> SyncRecordSet {
        SyncRecord::create_sync_records(
            device.id.clone(),
            ResourceType::Device,
            OperationType::Create,
            device.id.clone(),
            &[device],
        )
    }

    pub fn create_soft_delete_folder_records(
        folder_id: String,
        resources: Vec<Resource>,
        current_device_id: String,
        devices: &[Device],
    ) -> Vec<SyncRecordSet> {
        let mut sync_sets = Vec::new();

        // Create sync records for resources first
        for resource in resources {
            let resource_sync_set = Self::create_soft_delete_resource_records(
                resource.id,
                current_device_id.clone(),
                devices,
            );
            sync_sets.push(resource_sync_set);
        }

        // Create sync record for folder
        let folder_sync_set = Self::create_sync_records(
            folder_id,
            ResourceType::Folder,
            OperationType::SoftDelete,
            current_device_id,
            devices,
        );
        sync_sets.push(folder_sync_set);

        sync_sets
    }
    fn create_sync_records(
        resource_id: String,
        resource_type: ResourceType,
        operation_type: OperationType,
        current_device_id: String,
        devices: &[Device],
    ) -> SyncRecordSet {
        let now = Local::now().timestamp_millis();

        // Create the main sync record first
        let sync_record = SyncRecord {
            id: Uuid::new_v4().to_string(),
            resource_id,
            resource_type,
            operation_type,
            source_device_id: current_device_id.clone(),
            created_at: now,
            updated_at: now,
        };

        // Create device records for all devices
        let device_records: Vec<DeviceRecord> = devices
            .iter()
            .map(|device| DeviceRecord {
                id: Uuid::new_v4().to_string(),
                sync_record_id: sync_record.id.clone(),
                device_id: device.id.clone(),
                // Current device is completed, others are pending
                status: if device.id == current_device_id {
                    SyncStatus::Completed
                } else {
                    SyncStatus::Pending
                },
                // Current device is synced, others are not
                synced: device.id == current_device_id,
                created_at: now,
                updated_at: now,
            })
            .collect();

        // Create status records for each device record
        let mut device_record_statuses = Vec::new();

        // For each device record, create status entries for all devices
        for device_record in &device_records {
            // Create status records for each device being aware of this record
            for device in devices {
                device_record_statuses.push(DeviceRecordStatus {
                    id: Uuid::new_v4().to_string(),
                    device_record_id: device_record.id.clone(),
                    aware_device_id: device.id.clone(),
                    // Current device is aware of all records
                    synced: device.id == current_device_id,
                    created_at: now,
                    updated_at: now,
                });
            }
        }

        SyncRecordSet {
            sync_record,
            device_records,
            device_record_statuses,
        }
    }

    pub fn create_completion_records(
        sync_id: String,
        current_device_id: String,
        devices: &[Device],
    ) -> StatusChangeSet {
        SyncRecord::create_status_change_records(
            sync_id,
            current_device_id,
            devices,
            SyncStatus::Completed,
        )
    }

    pub fn process_completion_record(
        completion_record: &StatusChangeSet,
        device_id: &str,
    ) -> (StatusChangeSet, Option<String>) {
        // Create a new StatusChangeSet to hold the modified records
        let mut processed_record = StatusChangeSet {
            device_record: completion_record.device_record.clone(),
            device_record_statuses: Vec::new(),
        };

        // Variable to hold the ID of the device record if it matches the current device
        let mut matched_device_record_id = None;

        // Process each device record status
        for status in &completion_record.device_record_statuses {
            let mut new_status = status.clone();

            // If this status is for the current device, mark it as synced
            if status.aware_device_id == device_id {
                new_status.synced = true;

                // If we haven't already found a match, store the device record ID
                if matched_device_record_id.is_none() {
                    matched_device_record_id = Some(status.id.clone());
                }
            }

            // Add the (potentially modified) status to our result
            processed_record.device_record_statuses.push(new_status);
        }

        // Return both the processed record and the matching device record ID (if any)
        (processed_record, matched_device_record_id)
    }
    fn create_status_change_records(
        sync_id: String,
        current_device_id: String,
        devices: &[Device],
        status: SyncStatus,
    ) -> StatusChangeSet {
        let now = Local::now().timestamp_millis();

        // Create single device record for the device that just synced
        let device_record = DeviceRecord {
            id: Uuid::new_v4().to_string(),
            sync_record_id: sync_id,
            device_id: current_device_id.clone(),
            status,
            synced: true,
            created_at: now,
            updated_at: now,
        };

        let mut device_record_statuses = Vec::new();

        // Create status records for all devices
        for device in devices {
            device_record_statuses.push(DeviceRecordStatus {
                id: Uuid::new_v4().to_string(),
                device_record_id: device_record.id.clone(),
                aware_device_id: device.id.clone(),
                // Current device knows about this status, others don't
                synced: device.id == current_device_id,
                created_at: now,
                updated_at: now,
            });
        }

        StatusChangeSet {
            device_record,
            device_record_statuses,
        }
    }

    pub fn create_initial_device_sync_records(
        new_device: Device,
        current_device_id: String,
        existing_sync_records: &[SyncRecord],
        all_devices: &[Device],
        new_device_record: SyncRecordSet,
    ) -> SyncRecordSet {
        let now = Local::now().timestamp_millis();

        // Keep the original sync record for the new device
        let device_sync_record = new_device_record.sync_record;
        let mut device_records = Vec::new();
        let mut device_record_statuses = Vec::new();

        // 1. Add the original device record and status from new_device_record
        // These represent the new device's own record
        device_records.extend(new_device_record.device_records);
        device_record_statuses.extend(new_device_record.device_record_statuses);

        // 2. Create device records for existing devices to acknowledge the new device
        for device in all_devices {
            let status = if device.id == current_device_id {
                SyncStatus::Completed
            } else {
                SyncStatus::Pending
            };

            let device_record = DeviceRecord {
                id: Uuid::new_v4().to_string(),
                sync_record_id: device_sync_record.id.clone(),
                device_id: device.id.clone(),
                status,
                synced: device.id == current_device_id,
                created_at: now,
                updated_at: now,
            };

            // Create status records for this device record
            // Both the current device and all existing devices need to be aware
            for aware_device in all_devices.iter().chain(std::iter::once(&new_device)) {
                device_record_statuses.push(DeviceRecordStatus {
                    id: Uuid::new_v4().to_string(),
                    device_record_id: device_record.id.clone(),
                    aware_device_id: aware_device.id.clone(),
                    synced: aware_device.id == current_device_id,
                    created_at: now,
                    updated_at: now,
                });
            }

            device_records.push(device_record);
        }

        // 3. Create device records for all existing sync records for the new device
        for existing_sync in existing_sync_records {
            let device_record = DeviceRecord {
                id: Uuid::new_v4().to_string(),
                sync_record_id: existing_sync.id.clone(),
                device_id: new_device.id.clone(),
                status: SyncStatus::Pending,
                synced: false,
                created_at: now,
                updated_at: now,
            };

            // Create status records for the existing sync records
            // Both current device and all existing devices need to be aware
            for device in all_devices.iter().chain(std::iter::once(&new_device)) {
                device_record_statuses.push(DeviceRecordStatus {
                    id: Uuid::new_v4().to_string(),
                    device_record_id: device_record.id.clone(),
                    aware_device_id: device.id.clone(),
                    synced: device.id == current_device_id,
                    created_at: now,
                    updated_at: now,
                });
            }

            device_records.push(device_record);
        }

        SyncRecordSet {
            sync_record: device_sync_record,
            device_records,
            device_record_statuses,
        }
    }

    pub fn process_device_records(
        device_records: &[DeviceRecord],
        device_statuses: &[DeviceRecordStatus],
        current_device_id: &str,
    ) -> (
        Vec<DeviceRecord>,
        Vec<DeviceRecordStatus>,
        Vec<String>,
        Vec<String>,
    ) {
        let mut updated_record_ids = Vec::new();
        let mut updated_status_ids = Vec::new();

        let updated_records = device_records
            .iter()
            .map(|record| {
                let mut r = record.clone();
                if r.device_id == current_device_id {
                    r.synced = true;
                    updated_record_ids.push(r.id.clone());
                }
                r
            })
            .collect();

        let updated_statuses = device_statuses
            .iter()
            .map(|status| {
                let mut s = status.clone();
                if s.aware_device_id == current_device_id {
                    s.synced = true;
                    updated_status_ids.push(s.id.clone());
                }
                s
            })
            .collect();

        (
            updated_records,
            updated_statuses,
            updated_record_ids,
            updated_status_ids,
        )
    }

    pub fn create_device_sync_records(
        sync_record_id: String,
        target_devices: &[Device],
        current_device_id: String,
    ) -> DeviceRecordSet {
        let now = Local::now().timestamp_millis();
        let mut device_records = Vec::new();
        let mut device_record_statuses = Vec::new();

        // Create device records for all target devices that need to sync again
        for device in target_devices {
            let device_record = DeviceRecord {
                id: Uuid::new_v4().to_string(),
                sync_record_id: sync_record_id.clone(),
                device_id: device.id.clone(),
                status: SyncStatus::Pending,
                synced: false,
                created_at: now,
                updated_at: now,
            };

            // Create status records for each device record
            // Current device knows about these records
            device_record_statuses.push(DeviceRecordStatus {
                id: Uuid::new_v4().to_string(),
                device_record_id: device_record.id.clone(),
                aware_device_id: current_device_id.clone(),
                synced: true,
                created_at: now,
                updated_at: now,
            });

            // Each target device also needs a status record (starts unaware)
            device_record_statuses.push(DeviceRecordStatus {
                id: Uuid::new_v4().to_string(),
                device_record_id: device_record.id.clone(),
                aware_device_id: device.id.clone(),
                synced: false,
                created_at: now,
                updated_at: now,
            });

            // Add the device record to our collection
            device_records.push(device_record);
        }

        DeviceRecordSet {
            device_records,
            device_record_statuses,
        }
    }

    pub fn create_user_sync_record(
        user_id: String,
        current_device_id: String,
        devices: &[Device],
    ) -> SyncRecordSet {
        SyncRecord::create_sync_records(
            user_id,
            ResourceType::User,
            OperationType::Create,
            current_device_id,
            devices,
        )
    }

    pub fn merge_sync_records(
        local_device_records: &[DeviceRecord],
        local_device_statuses: &[DeviceRecordStatus],
        remote_device_records: &[DeviceRecord],
        remote_device_statuses: &[DeviceRecordStatus],
        current_device_id: &str,
    ) -> SyncMergeResult {
        let mut result = SyncMergeResult::new();

        // Process Device Records
        // 1. Check remote records against local records
        for remote_record in remote_device_records {
            let matching_local_record = local_device_records
                .iter()
                .find(|local_record| local_record.id == remote_record.id);

            match matching_local_record {
                Some(local_record) => {
                    // Record exists on both sides - check sync flags
                    if remote_record.synced && !local_record.synced {
                        // Remote has synced=true but local has synced=false
                        // Update local record to synced=true
                        result
                            .local_operations
                            .record_ids_to_update
                            .push(local_record.id.clone());
                    }

                    if local_record.synced && !remote_record.synced {
                        // Local has synced=true but remote has synced=false
                        // Update remote record to synced=true
                        result
                            .remote_operations
                            .record_ids_to_update
                            .push(remote_record.id.clone());
                    }
                }
                None => {
                    // Record exists on remote but not local - add to local
                    result
                        .local_operations
                        .records_to_add
                        .push(remote_record.clone());
                }
            }
        }

        // 2. Check local records against remote records
        for local_record in local_device_records {
            let matching_remote_record = remote_device_records
                .iter()
                .find(|remote_record| remote_record.id == local_record.id);

            if matching_remote_record.is_none() {
                // Record exists on local but not remote - add to remote
                result
                    .remote_operations
                    .records_to_add
                    .push(local_record.clone());
            }
        }

        // Process Device Record Statuses
        // 1. Check remote statuses against local statuses
        for remote_status in remote_device_statuses {
            let matching_local_status = local_device_statuses
                .iter()
                .find(|local_status| local_status.id == remote_status.id);

            match matching_local_status {
                Some(local_status) => {
                    // Status exists on both sides - check sync flags

                    // Special case: If status is for current device and synced=false
                    // Always update it to synced=true
                    if remote_status.aware_device_id == current_device_id && !remote_status.synced {
                        result
                            .local_operations
                            .status_ids_to_update
                            .push(local_status.id.clone());
                        result
                            .remote_operations
                            .status_ids_to_update
                            .push(remote_status.id.clone());
                        continue;
                    }

                    if remote_status.synced && !local_status.synced {
                        // Remote has synced=true but local has synced=false
                        // Update local status to synced=true
                        result
                            .local_operations
                            .status_ids_to_update
                            .push(local_status.id.clone());
                    }

                    if local_status.synced && !remote_status.synced {
                        // Local has synced=true but remote has synced=false
                        // Update remote status to synced=true
                        result
                            .remote_operations
                            .status_ids_to_update
                            .push(remote_status.id.clone());
                    }
                }
                None => {
                    // Status exists on remote but not local - add to local

                    // If this status is for the current device, ensure it's marked as synced
                    let mut status_to_add = remote_status.clone();
                    if status_to_add.aware_device_id == current_device_id {
                        status_to_add.synced = true;
                    }

                    result
                        .local_operations
                        .status_records_to_add
                        .push(status_to_add);
                }
            }
        }

        // 2. Check local statuses against remote statuses
        for local_status in local_device_statuses {
            let matching_remote_status = remote_device_statuses
                .iter()
                .find(|remote_status| remote_status.id == local_status.id);

            if matching_remote_status.is_none() {
                // Status exists on local but not remote - add to remote
                result
                    .remote_operations
                    .status_records_to_add
                    .push(local_status.clone());
            }
        }

        result
    }
    pub fn create_resource_share_records(
        sync_record_id: String,
        current_device_id: String,
        recipient_devices: &[Device],
        all_device_ids: &[String],
    ) -> DeviceRecordSet {
        let now = Local::now().timestamp_millis();
        let mut device_records = Vec::new();
        let mut device_record_statuses = Vec::new();

        // Create device records for all recipient devices
        for recipient_device in recipient_devices {
            // Create a device record for each recipient device
            let device_record = DeviceRecord {
                id: Uuid::new_v4().to_string(),
                sync_record_id: sync_record_id.clone(),
                device_id: recipient_device.id.clone(),
                status: SyncStatus::Pending,
                synced: false,
                created_at: now,
                updated_at: now,
            };

            // Create status records for all devices in the system
            for aware_device_id in all_device_ids {
                device_record_statuses.push(DeviceRecordStatus {
                    id: Uuid::new_v4().to_string(),
                    device_record_id: device_record.id.clone(),
                    aware_device_id: aware_device_id.clone(),
                    // Only the current device has synced=true initially
                    synced: aware_device_id == &current_device_id,
                    created_at: now,
                    updated_at: now,
                });
            }

            // Add the device record to our collection
            device_records.push(device_record);
        }

        DeviceRecordSet {
            device_records,
            device_record_statuses,
        }
    }

    pub fn create_device_records_for_sync_records(
        sync_record_ids: &[String],
        new_device_ids: &[String],
        current_device_id: &str,
        all_device_ids: &[String],
    ) -> Vec<DeviceRecordSet> {
        let now = Local::now().timestamp_millis();
        let mut device_record_sets = Vec::new();

        // For each sync record, create device records for all new devices
        for sync_record_id in sync_record_ids {
            let mut device_records = Vec::new();
            let mut device_record_statuses = Vec::new();

            // Create device records for all new devices
            for new_device_id in new_device_ids {
                let device_record = DeviceRecord {
                    id: Uuid::new_v4().to_string(),
                    sync_record_id: sync_record_id.clone(),
                    device_id: new_device_id.clone(),
                    status: SyncStatus::Pending,
                    synced: false,
                    created_at: now,
                    updated_at: now,
                };

                // Create status records for this device record for all devices
                for aware_device_id in all_device_ids {
                    device_record_statuses.push(DeviceRecordStatus {
                        id: Uuid::new_v4().to_string(),
                        device_record_id: device_record.id.clone(),
                        aware_device_id: aware_device_id.clone(),
                        synced: aware_device_id == current_device_id,
                        created_at: now,
                        updated_at: now,
                    });
                }

                device_records.push(device_record);
            }

            device_record_sets.push(DeviceRecordSet {
                device_records,
                device_record_statuses,
            });
        }

        device_record_sets
    }
}
