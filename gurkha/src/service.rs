//! UCAN Service
//!
//! Core service for token generation and delegation.
//! Holds cached Ed25519 keys for efficient UCAN operations.
//!
//! Usage:
//! ```rust
//! let ucan_service = UcanService::new(signing_key, verifying_key);
//! let (token, cid) = ucan_service.issue_one_time("sthalam", "owner").await?;
//! ```

use crate::errors::{ServiceError, ServiceResult};
use crate::parser::GenericUcan;
use crate::token::{
    FolderShareToken, FolderViewerToken, ResourceShareToken, ResourceViewerToken,
};
use crate::decision;
use crate::crypto;
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::{SigningKey, VerifyingKey};
use tracing::{debug, error, info, instrument, warn};

/// UCAN Service - holds cached keys for efficient token operations
///
/// Keys are decrypted once and cached in this struct.
/// This service is initialized once at login and passed around as Arc<RwLock<UcanService>>.
pub struct UcanService {
    signing_key: Option<SigningKey>,
    verifying_key: Option<VerifyingKey>,
}

impl UcanService {
    /// Create a new UcanService with no loaded keys
    pub fn new() -> Self {
        Self {
            signing_key: None,
            verifying_key: None,
        }
    }

    /// Load keys into the UcanService (called during login)
    #[instrument(skip(self, signing_key, verifying_key))]
    pub fn load_keys(&mut self, signing_key: SigningKey, verifying_key: VerifyingKey) {
        debug!("🔐 Loading UCAN keys into service");
        self.signing_key = Some(signing_key);
        self.verifying_key = Some(verifying_key);
        info!("✓ UCAN keys loaded successfully");
    }

    /// Check if keys are loaded
    pub fn is_loaded(&self) -> bool {
        self.signing_key.is_some() && self.verifying_key.is_some()
    }

    /// Clear the loaded keys
    #[instrument(skip(self))]
    pub fn clear_keys(&mut self) {
        debug!("🧹 Clearing UCAN keys from service");
        self.signing_key = None;
        self.verifying_key = None;
        info!("✓ UCAN keys cleared");
    }

    /// Get references to the keys (returns error if not loaded)
    fn get_keys(&self) -> ServiceResult<(&SigningKey, &VerifyingKey)> {
        match (&self.signing_key, &self.verifying_key) {
            (Some(sk), Some(vk)) => Ok((sk, vk)),
            _ => Err(ServiceError::KeysNotLoaded),
        }
    }
}

impl Default for UcanService {
    fn default() -> Self {
        Self::new()
    }
}

impl UcanService {
    // ==================== CONNECTION TOKENS ====================

    /// Issue a one-time connection token
    /// Used for initial device pairing (QR codes, connection strings).
    #[instrument(skip(self), fields(capability = %capability_str, role = %role, token_type = "one_time"))]
    pub async fn issue_one_time(
        &self,
        capability_str: &str,
        role: &str,
    ) -> ServiceResult<(String, String)> {
        debug!("🔐 Issuing one-time connection token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        let decision = decision::decide_one_time_token(
            verifying_key,
            capability_str,
            role,
        )?;
        debug!("✓ Token decision created");

        let (token, cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &decision,
        ).await?;

        info!("✓ One-time token generated: cid={}", cid);
        Ok((token, cid))
    }

    /// Issue a peer connection token
    /// Establishes bidirectional peer connection with signed UCAN.
    #[instrument(skip(self, peer_pubkey), fields(domain = %domain, role = %role, token_type = "peer_connection"))]
    pub async fn issue_peer_connection(
        &self,
        peer_pubkey: &str,
        domain: &str,
        role: &str,
    ) -> ServiceResult<String> {
        debug!("🔐 Issuing peer connection token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        let decision = decision::decide_peer_connection(
            verifying_key,
            domain,
            peer_pubkey,
            role,
        )?;

        let (token, _cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &decision,
        ).await?;

        info!("✓ Peer connection token generated");
        Ok(token)
    }

    /// Issue a viewer authentication token
    #[instrument(skip(self), fields(resource_id = %resource_id, domain = %domain, token_type = "viewer_auth"))]
    pub async fn issue_viewer_auth(
        &self,
        resource_id: &str,
        domain: &str,
    ) -> ServiceResult<String> {
        debug!("🔐 Issuing viewer authentication token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        let decision = decision::decide_viewer_auth(
            verifying_key,
            resource_id,
            domain,
        )?;

        let (token, _cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &decision,
        ).await?;

        info!("✓ Viewer auth token generated");
        Ok(token)
    }

    /// Issue a folder viewer authentication token (for shareable links)
    ///
    /// This is a one-time token with wildcard audience that viewers use to connect.
    /// Used in the FolderTokenRequest flow to generate shareable connection strings.
    #[instrument(skip(self), fields(folder_id = %folder_id, domain = %domain, token_type = "folder_viewer_auth"))]
    pub async fn issue_folder_viewer_auth(
        &self,
        folder_id: &str,
        domain: &str,
    ) -> ServiceResult<(String, String)> {
        debug!("🔐 Issuing folder viewer authentication token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        let decision = decision::decide_folder_viewer_auth(
            verifying_key,
            folder_id,
            domain,
        )?;
        debug!("✓ Token decision created");

        let (token, cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &decision,
        ).await?;

        info!("✓ Folder viewer auth token generated: cid={}", cid);
        Ok((token, cid))
    }

    // ==================== RESOURCE TOKENS ====================

    /// Issue resource owner token
    #[instrument(skip(self, ucan_template_json), fields(resource_id = %resource_id, domain = %domain, token_type = "resource_owner"))]
    pub async fn issue_owner_token(
        &self,
        resource_id: &str,
        domain: &str,
        ucan_template_json: &str,
    ) -> ServiceResult<(String, String)> {
        debug!("🔐 Issuing resource owner token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        debug!("🔍 Parsing resource template");
        let decision = decision::decide_owner_token(
            verifying_key,
            resource_id,
            domain,
            ucan_template_json,
        )?;
        debug!("✓ Token decision created");

        let (token, cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &decision,
        ).await?;

        info!("✓ Resource owner token generated: cid={}", cid);
        Ok((token, cid))
    }

    /// Delegate resource to node (Owner → Node)
    pub async fn delegate_resource_to_node(
        &self,
        owner_token: &str,
        node_pub_key: &str,
    ) -> ServiceResult<ResourceShareToken> {
        let (signing_key, verifying_key) = self.get_keys()?;

        // Extract delegation template from owner token
        let template = decision::extract_template_from_token(owner_token, "node")?;
        let domain = decision::extract_domain_from_token(owner_token)?;

        let parsed_ucan = GenericUcan::from_token(owner_token)?;
        let ucan_ref = parsed_ucan.parsed();

        // Extract resource ID from token
        let resource_id = crate::extractors::extract_id_from_resource_type(
            ucan_ref,
            &domain,
            "resource",
        )?;

        // Validate token has share_resource permission
        crate::extractors::validate_has_capability(
            ucan_ref,
            &domain,
            "resource",
            "share_resource",
        )?;

        let delegation_decision = decision::decide_delegation_to_node(
            &template,
            &domain,
            &resource_id,
            node_pub_key,
        )?;

        let (token, _cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &delegation_decision.into(),
        ).await?;

        ResourceShareToken::from_token(&token)
            .map_err(|e| ServiceError::InvalidUcan(format!("Invalid ResourceShareToken: {}", e)))
    }

    /// Delegate resource to user (Node → User or Owner → User)
    pub async fn delegate_resource_to_user(
        &self,
        delegator_token: &str,
        user_pub_key: &str,
    ) -> ServiceResult<ResourceShareToken> {
        let (signing_key, verifying_key) = self.get_keys()?;

        let template = decision::extract_template_from_token(delegator_token, "user")?;
        let domain = decision::extract_domain_from_token(delegator_token)?;

        let parsed_ucan = GenericUcan::from_token(delegator_token)?;
        let ucan_ref = parsed_ucan.parsed();

        // Extract resource ID from token
        let resource_id = crate::extractors::extract_id_from_resource_type(
            ucan_ref,
            &domain,
            "resource",
        )?;

        // Validate token has share_resource permission
        crate::extractors::validate_has_capability(
            ucan_ref,
            &domain,
            "resource",
            "share_resource",
        )?;

        let delegation_decision = decision::decide_delegation_to_user(
            &template,
            &domain,
            &resource_id,
            user_pub_key,
        )?;

        let (token, _cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &delegation_decision.into(),
        ).await?;

        ResourceShareToken::from_token(&token)
            .map_err(|e| ServiceError::InvalidUcan(format!("Invalid ResourceShareToken: {}", e)))
    }

    /// Delegate resource to viewer (User → Viewer)
    pub async fn delegate_resource_to_viewer(
        &self,
        delegator_token: &str,
        viewer_pub_key: &str,
    ) -> ServiceResult<ResourceViewerToken> {
        let template = decision::extract_template_from_token(delegator_token, "viewer")?;
        let domain = decision::extract_domain_from_token(delegator_token)?;

        let parsed_ucan = GenericUcan::from_token(delegator_token)?;
        let ucan_ref = parsed_ucan.parsed();

        // Extract resource ID from token
        let resource_id = crate::extractors::extract_id_from_resource_type(
            ucan_ref,
            &domain,
            "resource",
        )?;

        // Validate token has share_resource permission
        crate::extractors::validate_has_capability(
            ucan_ref,
            &domain,
            "resource",
            "share_resource",
        )?;

        let (signing_key, verifying_key) = self.get_keys()?;

        let delegation_decision = decision::decide_delegation_to_viewer(
            &template,
            &domain,
            &resource_id,
            viewer_pub_key,
        )?;

        let (token, _cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &delegation_decision.into(),
        ).await?;

        ResourceViewerToken::from_token(&token)
            .map_err(|e| ServiceError::InvalidUcan(format!("Invalid ResourceViewerToken: {}", e)))
    }

    // ==================== FOLDER TOKENS ====================

    /// Issue folder owner token
    #[instrument(skip(self, ucan_template_json), fields(folder_id = %folder_id, domain = %domain, token_type = "folder_owner"))]
    pub async fn issue_folder_owner_token(
        &self,
        folder_id: &str,
        domain: &str,
        ucan_template_json: &str,
    ) -> ServiceResult<(String, String)> {
        debug!("🔐 Issuing folder owner token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        debug!("🔍 Parsing folder template");
        let decision = decision::decide_folder_owner_token(
            verifying_key,
            folder_id,
            domain,
            ucan_template_json,
        )?;
        debug!("✓ Token decision created");

        let (token, cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &decision,
        ).await?;

        info!("✓ Folder owner token generated: cid={}", cid);
        Ok((token, cid))
    }

    /// Delegate folder to node (Owner → Node)
    #[instrument(skip(self, delegator_token, node_pub_key), fields(node_pub_key = %node_pub_key))]
    pub async fn delegate_folder_to_node(
        &self,
        delegator_token: &str,
        node_pub_key: &str,
    ) -> ServiceResult<FolderShareToken> {
        debug!("🔐 Delegating folder to node");
        debug!("Delegator token (first 100 chars): {}...", &delegator_token[..delegator_token.len().min(100)]);

        let (signing_key, verifying_key) = self.get_keys()?;

        let template = decision::extract_template_from_token(delegator_token, "node")?;
        debug!("✓ Template extracted for role: node");

        let domain = decision::extract_domain_from_token(delegator_token)?;
        debug!("✓ Domain extracted: {}", domain);

        let parsed_ucan = GenericUcan::from_token(delegator_token)?;
        let ucan_ref = parsed_ucan.parsed();

        debug!("Attempting to extract folder_id from token...");
        let folder_id = crate::extractors::extract_id_from_resource_type(
            ucan_ref,
            &domain,
            "folder",
        )?;
        debug!("✓ Folder ID extracted: {}", folder_id);

        // Validate token has add_resources permission for folder operations
        crate::extractors::validate_has_capability(
            ucan_ref,
            &domain,
            "folder",
            "add_resources",
        )?;
        debug!("✓ Token has add_resources permission");

        let delegation_decision = decision::decide_folder_delegation(
            &template,
            &domain,
            &folder_id,
            node_pub_key,
            "node",
        )?;

        let (token, _cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &delegation_decision.into(),
        ).await?;

        FolderShareToken::from_token(&token)
            .map_err(|e| ServiceError::InvalidUcan(format!("Invalid FolderShareToken: {}", e)))
    }

    /// Delegate folder to user (Node → User or Owner → User)
    pub async fn delegate_folder_to_user(
        &self,
        delegator_token: &str,
        user_pub_key: &str,
    ) -> ServiceResult<FolderShareToken> {
        let (signing_key, verifying_key) = self.get_keys()?;

        let template = decision::extract_template_from_token(delegator_token, "user")?;
        let domain = decision::extract_domain_from_token(delegator_token)?;

        let parsed_ucan = GenericUcan::from_token(delegator_token)?;
        let ucan_ref = parsed_ucan.parsed();

        let folder_id = crate::extractors::extract_id_from_resource_type(
            ucan_ref,
            &domain,
            "folder",
        )?;

        // Validate token has add_resources permission
        crate::extractors::validate_has_capability(
            ucan_ref,
            &domain,
            "folder",
            "add_resources",
        )?;

        let delegation_decision = decision::decide_folder_delegation(
            &template,
            &domain,
            &folder_id,
            user_pub_key,
            "user",
        )?;

        let (token, _cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &delegation_decision.into(),
        ).await?;

        FolderShareToken::from_token(&token)
            .map_err(|e| ServiceError::InvalidUcan(format!("Invalid FolderShareToken: {}", e)))
    }

    /// Delegate folder to viewer (User → Viewer)
    pub async fn delegate_folder_to_viewer(
        &self,
        delegator_token: &str,
        viewer_pub_key: &str,
    ) -> ServiceResult<FolderViewerToken> {
        let (signing_key, verifying_key) = self.get_keys()?;

        let template = decision::extract_template_from_token(delegator_token, "viewer")?;
        let domain = decision::extract_domain_from_token(delegator_token)?;

        let parsed_ucan = GenericUcan::from_token(delegator_token)?;
        let ucan_ref = parsed_ucan.parsed();

        let folder_id = crate::extractors::extract_id_from_resource_type(
            ucan_ref,
            &domain,
            "folder",
        )?;

        // Validate token has add_resources permission
        crate::extractors::validate_has_capability(
            ucan_ref,
            &domain,
            "folder",
            "add_resources",
        )?;

        let delegation_decision = decision::decide_folder_delegation(
            &template,
            &domain,
            &folder_id,
            viewer_pub_key,
            "viewer",
        )?;

        let (token, _cid) = crypto::sign_ucan(
            signing_key,
            verifying_key,
            &delegation_decision.into(),
        ).await?;

        FolderViewerToken::from_token(&token)
            .map_err(|e| ServiceError::InvalidUcan(format!("Invalid FolderViewerToken: {}", e)))
    }

    // ==================== EXTRACTION & UTILITIES ====================

    /// Get public key from cached verifying key
    #[instrument(skip(self))]
    pub fn get_public_key(&self) -> ServiceResult<String> {
        debug!("🔍 Getting public key from cached verifying key");
        let (_signing_key, verifying_key) = self.get_keys()?;
        let pub_key = general_purpose::STANDARD.encode(verifying_key.as_bytes());
        debug!("✓ Public key extracted");
        Ok(pub_key)
    }

    /// Extract resource ID from token
    #[instrument(skip(self, ucan_token))]
    pub fn extract_resource_id(&self, ucan_token: &str) -> ServiceResult<String> {
        debug!("🔍 Extracting resource ID from token");
        let ucan = GenericUcan::from_token(ucan_token).map_err(|e| {
            error!("❌ Failed to parse token: {}", e);
            e
        })?;

        let resource_id = ucan.resource_id()
            .ok_or_else(|| {
                error!("❌ Cannot extract resource_id from token");
                ServiceError::InvalidUcan("Cannot extract resource_id".to_string())
            })?;

        debug!("✓ Resource ID extracted: {}", resource_id);
        Ok(resource_id)
    }

    /// Extract folder ID from token
    #[instrument(skip(self, ucan_token))]
    pub fn extract_folder_id(&self, ucan_token: &str) -> ServiceResult<String> {
        debug!("🔍 Extracting folder ID from token");
        let ucan = GenericUcan::from_token(ucan_token).map_err(|e| {
            error!("❌ Failed to parse token: {}", e);
            e
        })?;

        let folder_id = ucan.folder_id()
            .ok_or_else(|| {
                error!("❌ Cannot extract folder_id from token");
                ServiceError::InvalidUcan("Cannot extract folder_id".to_string())
            })?;

        debug!("✓ Folder ID extracted: {}", folder_id);
        Ok(folder_id)
    }

    /// Extract document capabilities from token
    #[instrument(skip(self, ucan_token))]
    pub async fn extract_capabilities(&self, ucan_token: &str) -> ServiceResult<Vec<(String, String)>> {
        use crate::uri::ParsedCapabilityUri;

        debug!("🔍 Extracting document capabilities from token");
        let ucan = GenericUcan::from_token(ucan_token).map_err(|e| {
            error!("❌ Failed to parse token: {}", e);
            e
        })?;

        let mut doc_capabilities = Vec::new();

        // Extract capabilities that are resource-specific (documents)
        for cap in ucan.parsed_capabilities() {
            if let ParsedCapabilityUri::Resource(resource_cap) = cap {
                // For resources, the doc_name is the document identifier
                // and we'll use "write" as the default ability for now
                doc_capabilities.push((resource_cap.doc_name().to_string(), "write".to_string()));
            }
        }

        debug!("✓ Extracted {} document capabilities", doc_capabilities.len());
        Ok(doc_capabilities)
    }

    /// Validate UCAN structure
    #[instrument(skip(self, ucan_token))]
    pub async fn validate_ucan_structure(&self, ucan_token: &str) -> ServiceResult<()> {
        debug!("🔍 Validating UCAN structure");
        GenericUcan::from_token(ucan_token).map_err(|e| {
            error!("❌ Invalid UCAN structure: {}", e);
            e
        })?;
        debug!("✓ UCAN structure valid");
        Ok(())
    }
}

// Helper trait to convert DelegationDecision to TokenDecision
impl From<crate::decision::DelegationDecision> for crate::decision::TokenDecision {
    fn from(dd: crate::decision::DelegationDecision) -> Self {
        crate::decision::TokenDecision {
            audience: dd.audience,
            capabilities: dd.capabilities,
            facts: dd.facts,
            expiry: None,
        }
    }
}
