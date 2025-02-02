use crate::domains::models::credential::Credential;
use crate::domains::models::device::Device;
use crate::domains::models::sync_types::{OperationType, ResourceType, SyncStatus};
use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug)]
pub struct SyncRecordSet {
    pub sync_record: SyncRecord,
    pub device_records: Vec<DeviceRecord>,
    pub device_record_statuses: Vec<DeviceRecordStatus>,
}
#[derive(Debug)]
pub struct StatusChangeSet {
    pub device_records: Vec<DeviceRecord>,
    pub device_record_statuses: Vec<DeviceRecordStatus>,
}
#[derive(Debug)]
pub struct InitialDeviceSyncSet {
    pub device_records: Vec<DeviceRecord>,
    pub device_record_statuses: Vec<DeviceRecordStatus>,
}
impl SyncRecord {
    pub fn create_folder_sync_record(
        resource_id: String,
        current_device_id: String,
        other_devices: &[Device],
    ) -> SyncRecordSet {
        SyncRecord::create_sync_records(
            resource_id,
            ResourceType::Folder,
            OperationType::Create,
            current_device_id,
            other_devices,
        )
    }

    pub fn create_credential_sync_record(
        resource_id: String,
        current_device_id: String,
        other_devices: &[Device],
    ) -> SyncRecordSet {
        SyncRecord::create_sync_records(
            resource_id,
            ResourceType::Credential,
            OperationType::Create,
            current_device_id,
            other_devices,
        )
    }

    pub fn create_soft_delete_credential_records(
        credential_id: String,
        current_device_id: String,
        devices: &[Device],
    ) -> SyncRecordSet {
        Self::create_sync_records(
            credential_id,
            ResourceType::Credential,
            OperationType::SoftDelete,
            current_device_id,
            devices,
        )
    }

    pub fn create_soft_delete_folder_records(
        folder_id: String,
        credentials: Vec<Credential>,
        current_device_id: String,
        devices: &[Device],
    ) -> Vec<SyncRecordSet> {
        let mut sync_sets = Vec::new();

        // Create sync records for credentials first
        for credential in credentials {
            let credential_sync_set = Self::create_soft_delete_credential_records(
                credential.id,
                current_device_id.clone(),
                devices,
            );
            sync_sets.push(credential_sync_set);
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
        other_devices: &[Device],
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

        // Create a device record for the current device (always completed)
        let mut device_records = vec![DeviceRecord {
            id: Uuid::new_v4().to_string(),
            sync_record_id: sync_record.id.clone(),
            device_id: current_device_id.clone(),
            status: SyncStatus::Completed,
            synced: true, // Current device is always synced
            created_at: now,
            updated_at: now,
        }];

        // Create device records for other devices (all pending)
        let other_device_records: Vec<DeviceRecord> = other_devices
            .iter()
            .map(|device| DeviceRecord {
                id: Uuid::new_v4().to_string(),
                sync_record_id: sync_record.id.clone(),
                device_id: device.id.clone(),
                status: SyncStatus::Pending,
                synced: false, // Other devices start unsynced
                created_at: now,
                updated_at: now,
            })
            .collect();
        device_records.extend(other_device_records);

        // Create status records for each device record
        let mut device_record_statuses = Vec::new();

        // For each device record, create status entries for current device and other devices
        for device_record in &device_records {
            // Current device always knows about all device records
            device_record_statuses.push(DeviceRecordStatus {
                id: Uuid::new_v4().to_string(),
                device_record_id: device_record.id.clone(),
                aware_device_id: current_device_id.clone(),
                synced: true, // Current device is aware of all records
                created_at: now,
                updated_at: now,
            });

            // Other devices start unaware of all records
            for other_device in other_devices {
                device_record_statuses.push(DeviceRecordStatus {
                    id: Uuid::new_v4().to_string(),
                    device_record_id: device_record.id.clone(),
                    aware_device_id: other_device.id.clone(),
                    synced: false, // Other devices start unaware
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
        synced_device_id: String,
        current_device_id: String,
        other_devices: &[Device],
    ) -> StatusChangeSet {
        SyncRecord::create_status_change_records(
            sync_id,
            synced_device_id,
            current_device_id,
            other_devices,
            SyncStatus::Completed,
        )
    }
    fn create_status_change_records(
        sync_id: String,
        synced_device_id: String,
        current_device_id: String,
        other_devices: &[Device],
        status: SyncStatus,
    ) -> StatusChangeSet {
        let now = Local::now().timestamp_millis();

        // Create single device record for the device that just synced
        let device_record = DeviceRecord {
            id: Uuid::new_v4().to_string(),
            sync_record_id: sync_id,
            device_id: synced_device_id,
            status,
            synced: true,
            created_at: now,
            updated_at: now,
        };

        let mut device_record_statuses = Vec::new();

        // Current device knows about this new status
        device_record_statuses.push(DeviceRecordStatus {
            id: Uuid::new_v4().to_string(),
            device_record_id: device_record.id.clone(),
            aware_device_id: current_device_id.clone(),
            synced: true,
            created_at: now,
            updated_at: now,
        });

        // Create status records for other devices
        for other_device in other_devices {
            if other_device.id != current_device_id {
                device_record_statuses.push(DeviceRecordStatus {
                    id: Uuid::new_v4().to_string(),
                    device_record_id: device_record.id.clone(),
                    aware_device_id: other_device.id.clone(),
                    synced: false,
                    created_at: now,
                    updated_at: now,
                });
            }
        }

        StatusChangeSet {
            device_records: vec![device_record],
            device_record_statuses,
        }
    }
    pub fn create_initial_device_sync_records(
        new_device_id: String,
        current_device_id: String,
        existing_sync_records: &[SyncRecord],
        all_devices: &[Device],
    ) -> InitialDeviceSyncSet {
        let now = Local::now().timestamp_millis();

        // Create pending device records for the new device for each existing sync
        let device_records: Vec<DeviceRecord> = existing_sync_records
            .iter()
            .map(|existing_sync| DeviceRecord {
                id: Uuid::new_v4().to_string(),
                sync_record_id: existing_sync.id.clone(),
                device_id: new_device_id.clone(),
                status: SyncStatus::Pending,
                synced: false,
                created_at: now,
                updated_at: now,
            })
            .collect();

        // Create status records for each device record
        let mut device_record_statuses = Vec::new();

        for device_record in &device_records {
            // For each device record, we need status records for all devices
            for device in all_devices {
                device_record_statuses.push(DeviceRecordStatus {
                    id: Uuid::new_v4().to_string(),
                    device_record_id: device_record.id.clone(),
                    aware_device_id: device.id.clone(),
                    // Only the current device starts as synced
                    synced: device.id == current_device_id,
                    created_at: now,
                    updated_at: now,
                });
            }

            // Also add a status record for the new device
            device_record_statuses.push(DeviceRecordStatus {
                id: Uuid::new_v4().to_string(),
                device_record_id: device_record.id.clone(),
                aware_device_id: new_device_id.clone(),
                synced: false, // New device starts unaware
                created_at: now,
                updated_at: now,
            });
        }

        InitialDeviceSyncSet {
            device_records,
            device_record_statuses,
        }
    }
}
