use crate::domains::models::resource::Resource;
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
    pub fn create_completion_records(
        share_id: String,
        current_user_id: String,
        other_users: &[User],
    ) -> ShareStatusChangeSet {
        Self::create_status_change_records(
            share_id,
            current_user_id,
            other_users,
            ShareStatus::Completed,
        )
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
}
