//! Validation service - high-level business validation logic for UCAN tokens
//!
//! This module provides business-level validation functions that combine
//! multiple crypto_utils operations with application-specific rules.

use crate::errors::ServiceResult;
use crypto_utils;

/// Validate that a peer has the capability to add folders
///
/// This validates:
/// - The peer connection token structure is valid
/// - The peer has `{domain}:add_folder` capability with `use` ability
///
/// # Arguments
/// * `peer_connection_token` - The peer's connection UCAN token
/// * `domain` - The domain to check (e.g., "sthalam")
///
/// # Returns
/// * `Ok(())` if validation succeeds
/// * `Err` with descriptive error if validation fails
pub async fn validate_peer_can_add_folder(
    peer_connection_token: &str,
    domain: &str,
) -> ServiceResult<()> {
    // 1. Validate token structure
    let peer_ucan = crypto_utils::ucan_utils::validate_structure(peer_connection_token)
        .await
        .map_err(|e| {
            crate::errors::FolderServiceError::Validation(format!(
                "Invalid peer connection token: {}",
                e
            ))
        })?;

    // 2. Check for add_folder capability
    let add_folder_resource = format!("{}:add_folder", domain);
    crypto_utils::ucan_utils::check_capability(&peer_ucan, &add_folder_resource, "use")
        .map_err(|_| {
            crate::errors::FolderServiceError::Validation(format!(
                "Peer lacks {} capability",
                add_folder_resource
            ))
        })?;

    Ok(())
}

/// Validate that a peer can add resources to a folder
///
/// This validates:
/// - The owner's folder UCAN structure is valid
/// - The owner has `add_resources` capability for the specific folder
/// - The folder_id in the UCAN matches the expected folder_id
///
/// # Arguments
/// * `owner_folder_ucan` - The owner's folder UCAN token
/// * `expected_folder_id` - The folder_id that should match the UCAN
/// * `domain` - The domain to check (e.g., "sthalam")
///
/// # Returns
/// * `Ok(())` if all validations pass
/// * `Err` with descriptive error if any validation fails
pub async fn validate_peer_can_add_resources(
    owner_folder_ucan: &str,
    expected_folder_id: &str,
    domain: &str,
) -> ServiceResult<()> {
    // 1. Validate owner's folder UCAN structure
    let folder_ucan = crypto_utils::ucan_utils::validate_structure(owner_folder_ucan)
        .await
        .map_err(|e| {
            crate::errors::ResourceServiceError::UcanError(format!(
                "Invalid owner folder UCAN: {}",
                e
            ))
        })?;

    // 3. Extract folder_id and verify add_resources capability
    let folder_id_from_ucan =
        crypto_utils::ucan_utils::extract_folder_id_with_add_resources_capability(
            &folder_ucan, domain,
        )
        .map_err(|_| {
            crate::errors::ResourceServiceError::UcanError(
                "Owner's folder UCAN lacks add_resources capability or no folder found"
                    .to_string(),
            )
        })?;

    // 4. Verify folder_id matches expected folder_id
    if folder_id_from_ucan != expected_folder_id {
        return Err(crate::errors::ResourceServiceError::UcanError(format!(
            "Folder ID mismatch: UCAN has {}, expected {}",
            folder_id_from_ucan, expected_folder_id
        ))
        .into());
    }

    Ok(())
}

/// Validate that a requester has access to a resource via their folder UCAN
///
/// This validates:
/// - The folder UCAN structure is valid
/// - The folder UCAN has proper capabilities (add_resources)
/// - The resource's folder_id matches the folder_id in the UCAN
///
/// Used when a peer requests a resource - validates they should have access
/// based on folder permissions.
///
/// # Arguments
/// * `folder_ucan` - Requester's folder UCAN token
/// * `resource_folder_id` - The folder_id that the resource belongs to
/// * `domain` - The domain to check (e.g., "sthalam")
///
/// # Returns
/// * `Ok(())` if requester has valid folder access
/// * `Err` with descriptive error if validation fails
pub async fn validate_folder_access_for_resource(
    folder_ucan: &str,
    resource_folder_id: &str,
    domain: &str,
) -> ServiceResult<()> {
    // Reuse existing validation - folder UCAN must have add_resources capability
    // and folder_id must match the resource's folder
    validate_peer_can_add_resources(folder_ucan, resource_folder_id, domain).await
}
