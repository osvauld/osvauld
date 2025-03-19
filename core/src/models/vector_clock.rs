use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Single vector clock entry in the database (one row per device/resource)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceVectorClock {
    pub id: String,
    pub resource_id: String,
    pub device_id: String,
    pub clock_value: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ResourceVectorClock {
    // Create a new vector clock entry
    pub fn new(resource_id: String, device_id: String, clock_value: u64) -> Self {
        let now = Local::now().timestamp_millis();

        Self {
            id: Uuid::new_v4().to_string(),
            resource_id,
            device_id,
            clock_value,
            created_at: now,
            updated_at: now,
        }
    }

    // Create a new entry with clock value 1
    pub fn initialize(resource_id: &str, device_id: &str) -> Self {
        Self::new(resource_id.to_string(), device_id.to_string(), 1)
    }

    // Create vector clock entries for a new resource with multiple devices
    // The source device gets clock value 1, others get 0
    pub fn create_initial_entries(
        resource_id: &str,
        device_ids: &[String],
        source_device_id: &str,
    ) -> Vec<ResourceVectorClock> {
        let mut entries = Vec::new();

        for device_id in device_ids {
            let clock_value = if device_id == source_device_id { 1 } else { 0 };
            entries.push(ResourceVectorClock::new(
                resource_id.to_string(),
                device_id.clone(),
                clock_value,
            ));
        }

        entries
    }

    //function to create vector clocks for resource when a new device imported by user
    pub fn create_entires_for_new_device(
        resource_ids: &[String],
        device_id: &str,
    ) -> Vec<ResourceVectorClock> {
        let mut vector_clocks = Vec::new();

        for resource_id in resource_ids {
            vector_clocks.push(ResourceVectorClock::new(
                resource_id.clone(),
                device_id.to_string(),
                0, // Initialize with 0 since the new device doesn't have the resource yet
            ));
        }

        vector_clocks
    }
    // Merge two sets of vector clock entries
    // Returns a complete analysis of what needs to be updated where
    pub fn merge(
        local_entries: &[ResourceVectorClock],
        remote_entries: &[ResourceVectorClock],
    ) -> MergeResult {
        let mut update_local = Vec::new();
        let mut add_local = Vec::new();
        let mut update_remote = Vec::new();
        let mut add_remote = Vec::new();

        let mut local_needs_update = false;
        let mut remote_needs_update = false;

        // Process entries from remote
        for remote in remote_entries {
            let local_entry = local_entries
                .iter()
                .find(|e| e.device_id == remote.device_id);

            match local_entry {
                Some(local) => {
                    // Entry exists on both sides
                    if remote.clock_value > local.clock_value {
                        // Remote has higher value - update local
                        update_local.push(remote.clone());
                        local_needs_update = true;
                    } else if local.clock_value > remote.clock_value {
                        // Local has higher value - update remote
                        update_remote.push(local.clone());
                        remote_needs_update = true;
                    }
                }
                None => {
                    // Entry only in remote - add to local
                    add_local.push(remote.clone());
                    local_needs_update = true;
                }
            }
        }

        // Check for entries only in local
        for local in local_entries {
            let remote_has_entry = remote_entries
                .iter()
                .any(|e| e.device_id == local.device_id);

            if !remote_has_entry {
                // Entry only in local - add to remote
                add_remote.push(local.clone());
                remote_needs_update = true;
            }
        }

        MergeResult {
            update_local,
            add_local,
            update_remote,
            add_remote,
            local_needs_update,
            remote_needs_update,
        }
    }
}

// Result of merging vector clocks
#[derive(Debug, Clone)]
pub struct MergeResult {
    pub update_local: Vec<ResourceVectorClock>, // Entries to update locally
    pub add_local: Vec<ResourceVectorClock>,    // New entries to add locally
    pub update_remote: Vec<ResourceVectorClock>, // Entries to update remotely
    pub add_remote: Vec<ResourceVectorClock>,   // New entries to add remotely
    pub local_needs_update: bool,               // Does local need resource data update
    pub remote_needs_update: bool,              // Does remote need resource data update
}
