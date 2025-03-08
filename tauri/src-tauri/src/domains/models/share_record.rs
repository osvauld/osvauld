use crate::domains::models::resource::ResourceKeyPair;
use crate::domains::models::share_types::{ShareOperation, ShareStatus};
use crate::domains::models::user::User;
use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareRecord {
    pub id: String,
    pub resource_id: String,
    pub shared_by_user_id: String,
    pub operation_type: ShareOperation,
    pub created_at: i64,
    pub updated_at: i64,
}
pub struct SharePayloadResult {
    pub data: Option<ResourceKeyPair>,
    pub user_records: Vec<UserRecord>,
    pub user_record_statuses: Vec<UserRecordStatus>,
    pub share_record: Option<ShareRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRecord {
    pub id: String,
    pub share_record_id: String,
    pub user_id: String,
    pub status: ShareStatus,
    pub synced: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRecordStatus {
    pub id: String,
    pub user_record_id: String,
    pub aware_user_id: String,
    pub synced: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct ShareRecordSet {
    pub share_record: ShareRecord,
    pub user_records: Vec<UserRecord>,
    pub user_record_statuses: Vec<UserRecordStatus>,
}

pub struct UserRecordSet {
    pub user_records: Vec<UserRecord>,
    pub user_record_statuses: Vec<UserRecordStatus>,
}

#[derive(Debug)]
pub struct ShareStatusChangeSet {
    pub user_record: UserRecord,
    pub user_record_statuses: Vec<UserRecordStatus>,
}

impl ShareRecord {
    pub fn create_share_record(
        resource_id: String,
        shared_by_user_id: String,
        shared_with_users: &[User],
        operation_type: ShareOperation,
    ) -> ShareRecordSet {
        let now = Local::now().timestamp_millis();

        // Create the main share record
        let share_record = ShareRecord {
            id: Uuid::new_v4().to_string(),
            resource_id,
            shared_by_user_id: shared_by_user_id.clone(),
            operation_type,
            created_at: now,
            updated_at: now,
        };

        let mut user_records = Vec::new();

        // Create user records for all users
        for user in shared_with_users {
            let is_owner = user.id == shared_by_user_id;

            user_records.push(UserRecord {
                id: Uuid::new_v4().to_string(),
                share_record_id: share_record.id.clone(),
                user_id: user.id.clone(),
                status: if is_owner {
                    ShareStatus::Completed
                } else {
                    ShareStatus::Pending
                },
                synced: is_owner, // Owner is synced, others are not
                created_at: now,
                updated_at: now,
            });
        }

        // Create status records for each user record
        let mut user_record_statuses = Vec::new();

        // For EACH user record, create a status entry for EACH user
        for user_record in &user_records {
            for user in shared_with_users {
                // Determine if this status record refers to the owner
                let is_status_for_owner = user.id == shared_by_user_id;

                // Determine if this is a user's status for their own record
                let is_users_own_record = user.id == user_record.user_id;

                // Create the status record
                user_record_statuses.push(UserRecordStatus {
                    id: Uuid::new_v4().to_string(),
                    user_record_id: user_record.id.clone(),
                    aware_user_id: user.id.clone(),

                    // The owner is aware of all records
                    // Other users are only initially aware of their own records
                    synced: is_status_for_owner || is_users_own_record,

                    created_at: now,
                    updated_at: now,
                });
            }
        }

        ShareRecordSet {
            share_record,
            user_records,
            user_record_statuses,
        }
    }

    fn create_status_change_records(
        share_id: String,
        current_user_id: String,
        other_users: &[User],
        status: ShareStatus,
    ) -> ShareStatusChangeSet {
        let now = Local::now().timestamp_millis();

        // Create single user record for the user that just processed the share
        let user_record = UserRecord {
            id: Uuid::new_v4().to_string(),
            share_record_id: share_id,
            user_id: current_user_id.clone(),
            status,
            synced: true,
            created_at: now,
            updated_at: now,
        };

        let mut user_record_statuses = Vec::new();

        // Current user knows about this new status
        user_record_statuses.push(UserRecordStatus {
            id: Uuid::new_v4().to_string(),
            user_record_id: user_record.id.clone(),
            aware_user_id: current_user_id.clone(),
            synced: true,
            created_at: now,
            updated_at: now,
        });

        // Create status records for other users
        for other_user in other_users {
            user_record_statuses.push(UserRecordStatus {
                id: Uuid::new_v4().to_string(),
                user_record_id: user_record.id.clone(),
                aware_user_id: other_user.id.clone(),
                synced: false,
                created_at: now,
                updated_at: now,
            });
        }

        ShareStatusChangeSet {
            user_record,
            user_record_statuses,
        }
    }

    pub fn process_user_records(
        user_records: &[UserRecord],
        user_statuses: &[UserRecordStatus],
        current_user_id: &str,
    ) -> (Vec<UserRecord>, Vec<UserRecordStatus>) {
        let updated_records = user_records
            .iter()
            .map(|record| {
                let mut r = record.clone();
                if r.user_id == current_user_id {
                    r.synced = true;
                }
                r
            })
            .collect();

        let updated_statuses = user_statuses
            .iter()
            .map(|status| {
                let mut s = status.clone();
                if s.aware_user_id == current_user_id {
                    s.synced = true;
                }
                s
            })
            .collect();

        (updated_records, updated_statuses)
    }

    pub fn create_owner_share_record(&self, resource_id: String, user: User) -> ShareRecordSet {
        // Create a share record set where the owner has already completed the share
        ShareRecord::create_share_record(
            resource_id,
            user.id.clone(),
            &[user], // The owner is both the sharer and target
            ShareOperation::Share,
        )
    }

    pub fn create_user_update_records(
        share_record_id: String,
        target_users: &[User],
        current_user_id: String,
        all_users: &[User],
    ) -> UserRecordSet {
        let now = Local::now().timestamp_millis();
        let mut user_records = Vec::new();
        let mut user_record_statuses = Vec::new();

        // Create user records for all target users that need to sync again
        for target_user in target_users {
            let user_record = UserRecord {
                id: Uuid::new_v4().to_string(),
                share_record_id: share_record_id.clone(),
                user_id: target_user.id.clone(),
                status: ShareStatus::Pending,
                synced: false,
                created_at: now,
                updated_at: now,
            };

            // Create status records for EACH user in all_users
            // This ensures every user has awareness status for every record
            for aware_user in all_users {
                // Determine the initial sync status:
                // - The current user (updater) is aware of all records
                // - Other users are initially unaware
                let is_synced = aware_user.id == current_user_id;

                user_record_statuses.push(UserRecordStatus {
                    id: Uuid::new_v4().to_string(),
                    user_record_id: user_record.id.clone(),
                    aware_user_id: aware_user.id.clone(),
                    synced: is_synced,
                    created_at: now,
                    updated_at: now,
                });
            }

            // Add the user record to our collection
            user_records.push(user_record);
        }

        UserRecordSet {
            user_records,
            user_record_statuses,
        }
    }

    pub fn generate_share_user_records(
        user: User,
        current_user_id: String,
        existing_user_records: &[UserRecord],
    ) -> UserRecordSet {
        let now = Local::now().timestamp_millis();
        let existing_user_ids: Vec<String> = existing_user_records
            .iter()
            .map(|record| record.user_id.clone())
            .collect();
        let share_record_id: String = existing_user_records[0].share_record_id.clone();
        let user_record = UserRecord {
            id: Uuid::new_v4().to_string(),
            share_record_id,
            user_id: user.id.clone(),
            status: ShareStatus::Pending,
            synced: false,
            created_at: now,
            updated_at: now,
        };
        // Create status records for the new user record (for all existing users + new user)
        let mut user_record_statuses: Vec<UserRecordStatus> = existing_user_ids
            .iter()
            .chain(std::iter::once(&user.id))
            .map(|aware_user_id| UserRecordStatus {
                id: Uuid::new_v4().to_string(),
                user_record_id: user_record.id.clone(),
                aware_user_id: aware_user_id.clone(),
                synced: aware_user_id == &current_user_id,
                created_at: now,
                updated_at: now,
            })
            .collect();
        // Now create status records for all existing user records but with the new user as aware_user_id
        let new_status_for_existing_records: Vec<UserRecordStatus> = existing_user_records
            .iter()
            .map(|existing_record| UserRecordStatus {
                id: Uuid::new_v4().to_string(),
                user_record_id: existing_record.id.clone(),
                aware_user_id: user.id.clone(), // New user is now aware of this record
                synced: false,                  // Initially not synced
                created_at: now,
                updated_at: now,
            })
            .collect();

        // Extend the status records with the new ones
        user_record_statuses.extend(new_status_for_existing_records);

        UserRecordSet {
            user_records: vec![user_record],
            user_record_statuses,
        }
    }

    //function to process payload when resoucrce doesn't exist locally
    pub fn process_payload_for_new_resource(
        user_records: &Vec<UserRecord>,
        user_record_statuses: &Vec<UserRecordStatus>,
        current_user_id: &str,
    ) -> (Vec<UserRecord>, Vec<UserRecordStatus>) {
        // Create copies of the records to modify
        let mut updated_user_records = user_records.clone();
        let mut updated_status_records = user_record_statuses.clone();

        // Find and update the current user's record
        for user_record in &mut updated_user_records {
            if user_record.user_id == current_user_id {
                // Mark as synced and completed
                user_record.synced = true;
                user_record.status = ShareStatus::Completed;
                user_record.updated_at = chrono::Local::now().timestamp_millis();
            }
        }

        // Find and update status records for the current user
        for status in &mut updated_status_records {
            if status.aware_user_id == current_user_id {
                // Mark as synced
                status.synced = true;
                status.updated_at = chrono::Local::now().timestamp_millis();
            }
        }

        (updated_user_records, updated_status_records)
    }
}
