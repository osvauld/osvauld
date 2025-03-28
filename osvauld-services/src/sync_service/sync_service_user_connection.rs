use osvauld_core::models::device::Device;
use osvauld_core::models::sync_record::{SyncRecord, SyncRecordSet};
use osvauld_core::models::user::User;
use osvauld_core::repositories::RepositoryError;

use super::sync_service_core::SyncService;

impl SyncService {
    pub async fn process_first_user_connection(
        &self,
        user: &User,
        devices: &Vec<Device>,
        current_user_id: &str,
        current_device_id: &str,
    ) -> Result<(User, Vec<Device>, SyncRecordSet), RepositoryError> {
        let user_devices = self
            .device_repository
            .get_devices_by_user_except(current_user_id, &[current_device_id.to_string()])
            .await?;
        let mut new_user = user.clone();
        new_user.first_sync = false;
        let current_user = self.user_repository.get_user_by_id(current_user_id).await?;

        let user_addition_record = SyncRecord::create_user_sync_record(
            user.id.clone(),
            current_device_id.to_string(),
            &user_devices,
        );
        self.db
            .sync_add_new_user(&new_user, devices, &user_addition_record)
            .await?;
        Ok((current_user, user_devices, user_addition_record))
    }

    pub async fn process_first_user_connection_response(
        &self,
        user: &User,
        devices: &Vec<Device>,
        user_addition_record: &SyncRecordSet,
        current_user_id: &str,
        current_device_id: &str,
    ) -> Result<SyncRecordSet, RepositoryError> {
        let user_devices = self
            .device_repository
            .get_devices_by_user_except(current_user_id, &[current_device_id.to_string()])
            .await?;
        let remote_user_addition_record = SyncRecord::create_user_sync_record(
            user.id.clone(),
            current_device_id.to_string(),
            &user_devices,
        );

        self.db
            .complete_new_user_add(
                &user.id,
                devices,
                &remote_user_addition_record,
                &user_addition_record,
            )
            .await?;
        Ok(remote_user_addition_record)
    }

    pub async fn handle_user_add_ack(
        &self,
        remote_user_id: &str,
        user_addition_records: &SyncRecordSet,
    ) -> Result<(), RepositoryError> {
        // Use the abstracted DB function
        self.db
            .handle_user_add_ack(remote_user_id, &user_addition_records)
            .await?;
        Ok(())
    }
}
