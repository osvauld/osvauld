use crate::application::services::{CredentialService, SyncService};
use crate::database::schema::sync_records::credential_id;
use crate::domains::models::sync_record::ResourceType;
use crate::types::{
    AddCredentialInput, CredentialResponse, CryptoResponse, DeleteCredentialInput,
    GetAllCredentials, GetCredentialForFolderInput, ToggleFavInput, UpdateLastAccessedInput,
};
use log::info;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn handle_add_credential(
    input: AddCredentialInput,
    credential_service: State<'_, Arc<CredentialService>>,
    sync_service: State<'_, Arc<SyncService>>,
) -> Result<CryptoResponse, String> {
    let credential = credential_service
        .add_credential(
            input.credential_payload,
            input.credential_type,
            input.folder_id,
        )
        .await
        .map_err(|e| e.to_string())?;
    sync_service
        .add_credential_to_sync(credential)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_get_credentials_for_folder(
    input: GetCredentialForFolderInput,
    credential_service: State<'_, Arc<CredentialService>>,
) -> Result<CryptoResponse, String> {
    let credentials = credential_service
        .get_credentials_for_folder(input.folder_id)
        .await
        .map_err(|e| e.to_string())?;

    let credential_responses = credentials
        .into_iter()
        .map(|cred| CredentialResponse {
            id: cred.id,
            data: cred.data,
            favourite: cred.favourite,
            last_accessed: cred.last_accessed,
            folder_id: cred.folder_id,
        })
        .collect();

    Ok(CryptoResponse::Credentials(credential_responses))
}

#[tauri::command]
pub async fn soft_delete_credential(
    input: DeleteCredentialInput,
    credential_service: State<'_, Arc<CredentialService>>,
    sync_service: State<'_, Arc<SyncService>>,
) -> Result<(), String> {
    info!("deleting credential {}", input.credential_id);
    credential_service
        .delete_credential(input.credential_id.clone())
        .await
        .map_err(|e| e.to_string())?;
    sync_service
        .add_soft_deletion_sync_record(input.credential_id, ResourceType::Credential)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn toggle_fav(
    input: ToggleFavInput,
    credential_service: State<'_, Arc<CredentialService>>,
) -> Result<CryptoResponse, String> {
    credential_service
        .toggle_fav(input.credential_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}
#[tauri::command]
pub async fn update_last_accessed(
    input: UpdateLastAccessedInput,
    credential_service: State<'_, Arc<CredentialService>>,
) -> Result<CryptoResponse, String> {
    credential_service
        .update_last_accessed(input.credential_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn get_all_credentials(
    credential_service: State<'_, Arc<CredentialService>>,
    input: GetAllCredentials,
) -> Result<CryptoResponse, String> {
    let credentials = credential_service
        .get_all_credentials(input.favourite)
        .await
        .map_err(|e| e.to_string())?;
    let credential_responses = credentials
        .into_iter()
        .map(|cred| CredentialResponse {
            id: cred.id,
            data: cred.data,
            favourite: cred.favourite,
            last_accessed: cred.last_accessed,
            folder_id: cred.folder_id,
        })
        .collect();

    Ok(CryptoResponse::Credentials(credential_responses))
}
