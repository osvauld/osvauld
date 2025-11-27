//! Permit Service
//!
//! Core service for permit token generation and delegation.
//! Holds cached Ed25519 keys for efficient permit operations.
//!
//! Usage:
//! ```rust
//! let permit_service = PermitService::new(signing_key, verifying_key);
//! let (token, cid) = permit_service.issue_one_time("sthalam", "owner").await?;
//! ```

use crate::errors::{ServiceError, ServiceResult};
use crate::parser::Permit;
use crate::decision;
use crate::crypto;
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::{SigningKey, VerifyingKey};
use tracing::{debug, error, info, instrument, warn};

/// Permit Service - holds cached keys for efficient permit token operations.
///
/// Keys are decrypted once and cached in this struct.
/// This service is initialized once at login and passed around as Arc<RwLock<PermitService>>.
pub struct PermitService {
    signing_key: Option<SigningKey>,
    verifying_key: Option<VerifyingKey>,
}

impl PermitService {
    /// Create a new PermitService with no loaded keys
    pub fn new() -> Self {
        Self {
            signing_key: None,
            verifying_key: None,
        }
    }

    /// Load keys into the PermitService (called during login)
    #[instrument(skip(self, signing_key, verifying_key))]
    pub fn load_keys(&mut self, signing_key: SigningKey, verifying_key: VerifyingKey) {
        debug!("🔐 Loading permit keys into service");
        self.signing_key = Some(signing_key);
        self.verifying_key = Some(verifying_key);
        info!("✓ Permit keys loaded successfully");
    }

    /// Check if keys are loaded
    pub fn is_loaded(&self) -> bool {
        self.signing_key.is_some() && self.verifying_key.is_some()
    }

    /// Clear the loaded keys
    #[instrument(skip(self))]
    pub fn clear_keys(&mut self) {
        debug!("🧹 Clearing permit keys from service");
        self.signing_key = None;
        self.verifying_key = None;
        info!("✓ Permit keys cleared");
    }

    /// Get references to the keys (returns error if not loaded)
    fn get_keys(&self) -> ServiceResult<(&SigningKey, &VerifyingKey)> {
        match (&self.signing_key, &self.verifying_key) {
            (Some(sk), Some(vk)) => Ok((sk, vk)),
            _ => Err(ServiceError::KeysNotLoaded),
        }
    }
}

impl Default for PermitService {
    fn default() -> Self {
        Self::new()
    }
}

impl PermitService {
    // ==================== CONNECTION TOKENS ====================

    /// Issue a one-time connection token
    /// Used for initial device pairing (QR codes, connection strings).
    /// Facts-only approach - relationship determines permissions.
    #[instrument(skip(self), fields(relationship = %relationship, token_type = "one_time"))]
    pub async fn issue_one_time(
        &self,
        relationship: &str,
    ) -> ServiceResult<(String, String)> {
        debug!("🔐 Issuing one-time connection token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        let decision = decision::decide_one_time_token(
            verifying_key,
            relationship,
        )?;
        debug!("✓ Token decision created");

        let (token, cid) = crypto::sign_permit(
            signing_key,
            verifying_key,
            &decision,
        ).await?;

        info!("✓ One-time token generated: cid={}", cid);
        Ok((token, cid))
    }

    /// Issue a peer connection token
    /// Establishes bidirectional peer connection with signed permit.
    /// Facts-only approach - no domain needed, relationship determines permissions.
    #[instrument(skip(self, peer_pubkey), fields(relationship = %relationship, token_type = "peer_connection"))]
    pub async fn issue_peer_connection(
        &self,
        peer_pubkey: &str,
        relationship: &str,
    ) -> ServiceResult<String> {
        debug!("🔐 Issuing peer connection token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        let decision = decision::decide_peer_connection(
            verifying_key,
            peer_pubkey,
            relationship,
        )?;

        let (token, _cid) = crypto::sign_permit(
            signing_key,
            verifying_key,
            &decision,
        ).await?;

        info!("✓ Peer connection token generated");
        Ok(token)
    }

    /// Issue a viewer authentication token
    /// Facts-only approach - no domain needed.
    #[instrument(skip(self), fields(resource_id = %resource_id, token_type = "viewer_auth"))]
    pub async fn issue_viewer_auth(
        &self,
        resource_id: &str,
    ) -> ServiceResult<String> {
        debug!("🔐 Issuing viewer authentication token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        let decision = decision::decide_viewer_auth(
            verifying_key,
            resource_id,
        )?;

        let (token, _cid) = crypto::sign_permit(
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
    /// Facts-only approach - no domain needed.
    #[instrument(skip(self), fields(folder_id = %folder_id, token_type = "folder_viewer_auth"))]
    pub async fn issue_folder_viewer_auth(
        &self,
        folder_id: &str,
    ) -> ServiceResult<(String, String)> {
        debug!("🔐 Issuing folder viewer authentication token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        let decision = decision::decide_folder_viewer_auth(
            verifying_key,
            folder_id,
        )?;
        debug!("✓ Token decision created");

        let (token, cid) = crypto::sign_permit(
            signing_key,
            verifying_key,
            &decision,
        ).await?;

        info!("✓ Folder viewer auth token generated: cid={}", cid);
        Ok((token, cid))
    }

    // ==================== RESOURCE TOKENS ====================

    /// Issue resource owner token
    /// Facts-only approach - no domain needed.
    #[instrument(skip(self, permit_template_json), fields(resource_id = %resource_id, token_type = "resource_owner"))]
    pub async fn issue_owner_token(
        &self,
        resource_id: &str,
        permit_template_json: &str,
    ) -> ServiceResult<(String, String)> {
        debug!("🔐 Issuing resource owner token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        debug!("🔍 Parsing resource template");
        let decision = decision::decide_owner_token(
            verifying_key,
            resource_id,
            permit_template_json,
        )?;
        debug!("✓ Token decision created");

        let (token, cid) = crypto::sign_permit(
            signing_key,
            verifying_key,
            &decision,
        ).await?;

        info!("✓ Resource owner token generated: cid={}", cid);
        Ok((token, cid))
    }

    /// Unified resource delegation (replaces role-specific methods)
    ///
    /// Delegates a resource to any audience using a template key.
    /// Template key determines the delegation pattern (e.g., "node", "user", "viewer").
    ///
    /// # Arguments
    /// * `delegator_token` - Token of the delegator (must have share_resource capability)
    /// * `template_key` - Template key to use for delegation ("node", "user", "viewer", etc.)
    /// * `audience_pubkey` - Public key of the delegatee
    ///
    /// # Returns
    /// * `Ok((token_string, cid))` - The delegated token and its CID
    pub async fn delegate_resource(
        &self,
        delegator_token: &str,
        template_key: &str,
        audience_pubkey: &str,
    ) -> ServiceResult<(String, String)> {
        let (signing_key, verifying_key) = self.get_keys()?;

        // Extract delegation template from delegator token
        let template = decision::extract_template_from_token(delegator_token, template_key)?;

        let parsed_permit = Permit::from_token(delegator_token)?;

        // Extract resource ID from facts (facts-only approach)
        let resource_id = parsed_permit
            .get_fact("resource_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ServiceError::InvalidPermit("Missing resource_id in token facts".to_string())
            })?
            .to_string();

        // Validate token has share_resource permission (from facts.operations)
        // Operations are now strings: "allow"/"deny" instead of booleans
        let has_share = parsed_permit
            .get_fact("operations")
            .and_then(|ops| ops.as_object())
            .and_then(|ops| ops.get("share_resource"))
            .and_then(|v| v.as_str())
            .map(|s| s == "allow")
            .unwrap_or(false);

        if !has_share {
            return Err(ServiceError::ValidationError(
                "Token does not have share_resource permission".to_string()
            ));
        }

        // Create delegation decision (no domain needed)
        let delegation_decision = decision::decide_delegation(
            &template,
            &resource_id,
            "resource",
            audience_pubkey,
            Some(delegator_token),
        )?;

        // Use builder to create token
        let builder = crate::builder::GurkhaPermitBuilder::new(signing_key, verifying_key);
        let (token, cid) = builder.build(delegation_decision.into()).await?;

        Ok((token, cid))
    }


    // ==================== FOLDER TOKENS ====================

    /// Issue folder owner token
    /// Facts-only approach - no domain needed.
    #[instrument(skip(self, permit_template_json), fields(folder_id = %folder_id, token_type = "folder_owner"))]
    pub async fn issue_folder_owner_token(
        &self,
        folder_id: &str,
        permit_template_json: &str,
    ) -> ServiceResult<(String, String)> {
        debug!("🔐 Issuing folder owner token");

        let (signing_key, verifying_key) = self.get_keys()?;
        debug!("✓ Keys validated");

        debug!("🔍 Parsing folder template");
        let decision = decision::decide_folder_owner_token(
            verifying_key,
            folder_id,
            permit_template_json,
        )?;
        debug!("✓ Token decision created");

        let (token, cid) = crypto::sign_permit(
            signing_key,
            verifying_key,
            &decision,
        ).await?;

        info!("✓ Folder owner token generated: cid={}", cid);
        Ok((token, cid))
    }

    /// Unified folder delegation (replaces role-specific methods)
    ///
    /// Delegates a folder to any audience using a template key.
    /// Template key determines the delegation pattern (e.g., "node", "user", "viewer").
    ///
    /// # Arguments
    /// * `delegator_token` - Token of the delegator (must have add_resources capability)
    /// * `template_key` - Template key to use for delegation ("node", "user", "viewer", etc.)
    /// * `audience_pubkey` - Public key of the delegatee
    ///
    /// # Returns
    /// * `Ok((token_string, cid))` - The delegated token and its CID
    pub async fn delegate_folder(
        &self,
        delegator_token: &str,
        template_key: &str,
        audience_pubkey: &str,
    ) -> ServiceResult<(String, String)> {
        let (signing_key, verifying_key) = self.get_keys()?;

        // Extract delegation template from delegator token
        let template = decision::extract_template_from_token(delegator_token, template_key)?;

        let parsed_permit = Permit::from_token(delegator_token)?;

        // Extract folder ID from facts (facts-only approach)
        let folder_id = parsed_permit
            .get_fact("folder_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ServiceError::InvalidPermit("Missing folder_id in token facts".to_string())
            })?
            .to_string();

        // Validate token has add_resources permission (from facts.operations)
        // Operations are now strings: "allow"/"deny" instead of booleans
        let has_add_resources = parsed_permit
            .get_fact("operations")
            .and_then(|ops| ops.as_object())
            .and_then(|ops| ops.get("add_resources"))
            .and_then(|v| v.as_str())
            .map(|s| s == "allow")
            .unwrap_or(false);

        if !has_add_resources {
            return Err(ServiceError::ValidationError(
                "Token does not have add_resources permission".to_string()
            ));
        }

        // Create delegation decision (no domain needed, folder_id added by decide_delegation)
        let delegation_decision = decision::decide_delegation(
            &template,
            &folder_id,
            "folder",
            audience_pubkey,
            Some(delegator_token),
        )?;

        // Use builder to create token
        let builder = crate::builder::GurkhaPermitBuilder::new(signing_key, verifying_key);
        let (token, cid) = builder.build(delegation_decision.into()).await?;

        Ok((token, cid))
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
    #[instrument(skip(self, permit_token))]
    pub fn extract_resource_id(&self, permit_token: &str) -> ServiceResult<String> {
        debug!("🔍 Extracting resource ID from token");
        let permit = Permit::from_token(permit_token).map_err(|e| {
            error!("❌ Failed to parse token: {}", e);
            e
        })?;

        let resource_id = permit.resource_id()
            .ok_or_else(|| {
                error!("❌ Cannot extract resource_id from token");
                ServiceError::InvalidPermit("Cannot extract resource_id".to_string())
            })?;

        debug!("✓ Resource ID extracted: {}", resource_id);
        Ok(resource_id)
    }

    /// Extract folder ID from token
    #[instrument(skip(self, permit_token))]
    pub fn extract_folder_id(&self, permit_token: &str) -> ServiceResult<String> {
        debug!("🔍 Extracting folder ID from token");
        let permit = Permit::from_token(permit_token).map_err(|e| {
            error!("❌ Failed to parse token: {}", e);
            e
        })?;

        let folder_id = permit.folder_id()
            .ok_or_else(|| {
                error!("❌ Cannot extract folder_id from token");
                ServiceError::InvalidPermit("Cannot extract folder_id".to_string())
            })?;

        debug!("✓ Folder ID extracted: {}", folder_id);
        Ok(folder_id)
    }

    /// Extract document capabilities from token
    ///
    /// V3: Reads from facts.documents map instead of parsing URI capabilities
    #[instrument(skip(self, permit_token))]
    pub async fn extract_capabilities(&self, permit_token: &str) -> ServiceResult<Vec<(String, String)>> {
        debug!("🔍 Extracting document capabilities from token facts");
        let permit = Permit::from_token(permit_token).map_err(|e| {
            error!("❌ Failed to parse token: {}", e);
            e
        })?;

        let mut doc_capabilities = Vec::new();

        // V3: Extract from facts.documents map
        // Format: { doc_name: { type: "crdt"|"asset", capability: "collaborator"|"viewer" } }
        if let Some(documents) = permit.get_fact("documents").and_then(|v| v.as_object()) {
            for (doc_name, doc_info) in documents {
                if let Some(capability) = doc_info.as_object()
                    .and_then(|obj| obj.get("capability"))
                    .and_then(|v| v.as_str())
                {
                    doc_capabilities.push((doc_name.clone(), capability.to_string()));
                }
            }
        }

        debug!("✓ Extracted {} document capabilities", doc_capabilities.len());
        Ok(doc_capabilities)
    }

    /// Validate permit structure
    #[instrument(skip(self, permit_token))]
    pub async fn validate_permit_structure(&self, permit_token: &str) -> ServiceResult<()> {
        debug!("🔍 Validating permit structure");
        Permit::from_token(permit_token).map_err(|e| {
            error!("❌ Invalid permit structure: {}", e);
            e
        })?;
        debug!("✓ Permit structure valid");
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
            proofs: dd.proofs,
            proof_tokens: dd.proof_tokens,
        }
    }
}
