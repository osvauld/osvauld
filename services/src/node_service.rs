use crate::errors::ServiceResult;
use crypto_utils::CryptoUtils;
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Generate a folder token for public access via sovereign node
/// This creates a connection string that viewers can use to access the folder
pub async fn generate_folder_token(
    folder_id: &str,
    domain: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<(String, String)> {
    info!(
        "Generating folder token for folder {} in domain {}",
        folder_id, domain
    );

    // Get the encrypted UCAN private key from the store
    let encrypted_ucan_pvt_key = repo_ctx.store_repo.get_ucan_key().await?;

    let crypto = crypto_utils.read().await;

    // Generate the token with view capability
    let capability_prefix = format!("{}:folder:{}", domain, folder_id);

    let ucan_token = crypto
        .generate_public_folder_view_token(&encrypted_ucan_pvt_key, folder_id, &capability_prefix)
        .await?;

    // Get the public UCAN key
    let ucan_public_key = crypto
        .get_public_ucan_key(&encrypted_ucan_pvt_key)
        .await?;

    info!("Successfully generated folder token for folder {}", folder_id);

    Ok((ucan_token, ucan_public_key))
}
