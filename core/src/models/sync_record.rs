use crate::models::device::Device;
use crate::models::resource::Resource;
use crate::models::sync_types::{OperationType, ResourceType, SyncStatus};
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

#[derive(Debug, Clone)]
pub struct SyncRecordSet {
    pub sync_record: SyncRecord,
    pub device_records: Vec<DeviceRecord>,
    pub device_record_statuses: Vec<DeviceRecordStatus>,
}

pub struct DeviceRecordSet {
    pub device_records: Vec<DeviceRecord>,
    pub device_record_statuses: Vec<DeviceRecordStatus>,
}

#[derive(Debug)]
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

    pub fn create_resource_sync_record(
        resource_id: String,
        current_device_id: String,
        other_devices: &[Device],
    ) -> SyncRecordSet {
        SyncRecord::create_sync_records(
            resource_id,
            ResourceType::Resource,
            OperationType::Create,
            current_device_id,
            other_devices,
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
            &[][..],
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
        current_device_id: String,
        other_devices: &[Device],
    ) -> StatusChangeSet {
        SyncRecord::create_status_change_records(
            sync_id,
            current_device_id,
            other_devices,
            SyncStatus::Completed,
        )
    }
    fn create_status_change_records(
        sync_id: String,
        current_device_id: String,
        other_devices: &[Device],
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
            device_record_statuses.push(DeviceRecordStatus {
                id: Uuid::new_v4().to_string(),
                device_record_id: device_record.id.clone(),
                aware_device_id: other_device.id.clone(),
                synced: false,
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
    ) -> (Vec<DeviceRecord>, Vec<DeviceRecordStatus>) {
        let updated_records = device_records
            .iter()
            .map(|record| {
                let mut r = record.clone();
                if r.device_id == current_device_id {
                    r.synced = true;
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
                }
                s
            })
            .collect();

        (updated_records, updated_statuses)
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
}
