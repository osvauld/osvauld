use crate::errors::{CryptoError, UcanError};
use crate::ucan_utils;
use std::future::Future;

/// Validate a connect token
pub async fn validate_connect_token(
    token: &str,
    presenter_ucan_pub: &str,
    verifier_ucan_pub: &str,
    verifier_user_id: &str,
    domain: &str,
) -> Result<bool, CryptoError> {
    let ucan = ucan_utils::validate_structure(token).await?;
    let required_resource = format!("{}:user-connect:{}", domain, verifier_user_id);

    if ucan_utils::is_one_time_connect_token(&ucan, domain) {
        ucan_utils::verify_did_key_match(ucan.issuer(), verifier_ucan_pub)?;
        ucan_utils::check_capability(&ucan, &required_resource, "use")?;
    } else {
        ucan_utils::validate_audience(&ucan, presenter_ucan_pub)?;
        ucan_utils::validate_embedded_proof_chain(
            token,
            verifier_user_id,
            verifier_ucan_pub,
            domain,
            None,
        )
        .await?;
    }

    Ok(true)
}

/// Validate authority for update operations
pub async fn validate_authority_for_update<F, Fut>(
    ucan_token: &str,
    peer_ucan_pub: &str,
    root_ucan_pub: &str,
    resource_id: &str,
    domain: &str,
    proof_resolver: &F,
) -> Result<bool, CryptoError>
where
    F: Fn(&str) -> Fut + Send + Sync,
    Fut: Future<Output = Result<String, UcanError>> + Send + 'static,
{
    let ucan = ucan_utils::validate_structure(ucan_token)
        .await
        .map_err(CryptoError::UcanError)?;

    ucan_utils::validate_audience(&ucan, peer_ucan_pub).map_err(CryptoError::UcanError)?;
    let resource_uri = format!("{}:resource:{}", domain, resource_id);
    ucan_utils::validate_ucan_permission(
        &ucan,
        root_ucan_pub,
        proof_resolver,
        &resource_uri,
        &"crud/update".to_string(),
    )
    .await
    .map(|_| true)
    .map_err(CryptoError::UcanError)
}

/// Get CID from UCAN token
pub fn get_cid_from_ucan_token(ucan_token: &str) -> Result<String, CryptoError> {
    Ok(ucan_utils::get_ucan_cid(ucan_token)?)
}

/// Get role from UCAN token
pub async fn get_role_from_ucan_token(ucan_token: &str) -> Result<String, CryptoError> {
    let ucan = ucan_utils::validate_structure(ucan_token).await?;
    Ok(ucan_utils::get_role_from_token(&ucan))
}

/// Extract resource_id from UCAN token
pub async fn extract_resource_id_from_ucan_token(
    ucan_token: &str,
) -> Result<String, CryptoError> {
    let ucan = ucan_utils::validate_structure(ucan_token).await?;
    Ok(ucan_utils::extract_resource_id_from_ucan(&ucan)?)
}
