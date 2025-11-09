use crate::errors::UcanError;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use log::info;
use openpgp::{
    packet::key::{SecretParts, UnspecifiedRole},
    policy::StandardPolicy,
    types::PublicKeyAlgorithm,
    Cert,
};
use sequoia_openpgp::{self as openpgp, crypto::mpi::SecretKeyMaterial};
use serde_json::json;
use std::future::Future;
use std::boxed::Box;
use ucan::{
    builder::UcanBuilder,
    capability::Capability,
    crypto::{
        did::{DidParser, ED25519_MAGIC_BYTES},
        JwtSignatureAlgorithm, KeyMaterial,
    },
    Ucan,
};
/// Constructor function for DID parser - converts bytes to Ed25519KeyMaterial
pub fn bytes_to_ed25519_key(bytes: Vec<u8>) -> Result<Box<dyn KeyMaterial>> {
    let key_bytes: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("Invalid key length: expected 32 bytes"))?;

    let public_key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|e| anyhow!("Invalid Ed25519 public key: {}", e))?;

    Ok(Box::new(Ed25519KeyMaterial(public_key, None)))
}

#[derive(Clone)]
pub struct Ed25519KeyMaterial(pub VerifyingKey, pub Option<SigningKey>);

impl Ed25519KeyMaterial {
    pub fn new(signing_key: SigningKey, verifying_key: VerifyingKey) -> Self {
        Self(verifying_key, Some(signing_key))
    }
}

#[cfg_attr(target_arch="wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl KeyMaterial for Ed25519KeyMaterial {
    fn get_jwt_algorithm_name(&self) -> String {
        JwtSignatureAlgorithm::EdDSA.to_string()
    }

    async fn get_did(&self) -> Result<String> {
        let bytes = [ED25519_MAGIC_BYTES, self.0.as_bytes()].concat();
        Ok(format!("did:key:z{}", bs58::encode(bytes).into_string()))
    }

    async fn sign(&self, payload: &[u8]) -> Result<Vec<u8>> {
        match &self.1 {
            Some(private_key) => {
                use ed25519_dalek::Signer;
                let signature = private_key.sign(payload);
                Ok(signature.to_bytes().to_vec())
            }
            None => Err(anyhow!("No private key; cannot sign data")),
        }
    }

    async fn verify(&self, payload: &[u8], signature: &[u8]) -> Result<()> {
        let sig_array: [u8; 64] = signature
            .try_into()
            .map_err(|_| anyhow!("Invalid signature length"))?;
        let signature = Signature::from_bytes(&sig_array);

        use ed25519_dalek::Verifier;
        self.0
            .verify(payload, &signature)
            .map_err(|e| anyhow!("Could not verify signature: {:?}", e))?;

        Ok(())
    }
}
/// Derive Ed25519 UCAN keys from PGP certificate using deterministic derivation
pub fn derive_ucan_keys_from_pgp(cert: &Cert) -> Result<(SigningKey, VerifyingKey), UcanError> {
    let policy = &StandardPolicy::new();

    // Get the primary key - this should be Ed25519 with Cv25519 cipher suite
    let primary_key = cert.primary_key();

    // Verify it's Ed25519
    if primary_key.key().pk_algo() != PublicKeyAlgorithm::EdDSA {
        return Err(UcanError::KeyExtractionError(
            "Primary key is not Ed25519".to_string(),
        ));
    }

    // Get the secret key material from primary key
    for key_amalgamation in cert.keys().with_policy(policy, None) {
        let key = key_amalgamation.key();

        // Check if this is the primary key
        if key.fingerprint() == primary_key.key().fingerprint() {
            // Try to convert to secret key
            if let Ok(secret_key) = key.clone().parts_into_secret() {
                if secret_key.has_unencrypted_secret() {
                    return derive_ucan_key_from_pgp_secret(&secret_key);
                } else {
                    return Err(UcanError::KeyExtractionError(
                        "Primary key is encrypted - decrypt certificate first".to_string(),
                    ));
                }
            }
        }
    }

    Err(UcanError::KeyExtractionError(
        "Could not access primary key secret material".to_string(),
    ))
}

/// Derive UCAN Ed25519 key from PGP secret key using deterministic derivation
fn derive_ucan_key_from_pgp_secret(
    secret_key: &openpgp::packet::Key<SecretParts, UnspecifiedRole>,
) -> Result<(SigningKey, VerifyingKey), UcanError> {
    // Get the secret key material
    let secret_material = secret_key.secret();

    // Check if it's unencrypted
    if let openpgp::packet::key::SecretKeyMaterial::Unencrypted(unencrypted) = secret_material {
        // Map over the secret material to extract Ed25519 keys
        let result = unencrypted.map(|mpis| {
            match mpis {
                SecretKeyMaterial::EdDSA { scalar } => {
                    // Get the PGP private key bytes
                    let pgp_private_key = scalar.value();

                    if pgp_private_key.len() != 32 {
                        return Err(UcanError::KeyExtractionError(format!(
                            "Expected 32 bytes for Ed25519 secret key, got {}",
                            pgp_private_key.len()
                        )));
                    }

                    // Derive UCAN key from PGP key using Argon2
                    derive_ucan_signing_key(pgp_private_key)
                }
                _ => Err(UcanError::KeyExtractionError(
                    "Not an Ed25519 key".to_string(),
                )),
            }
        });

        result
    } else {
        Err(UcanError::KeyExtractionError(
            "Secret key is encrypted".to_string(),
        ))
    }
}

/// Derive UCAN Ed25519 signing key from PGP private key using Argon2
fn derive_ucan_signing_key(
    pgp_private_key: &[u8],
) -> Result<(SigningKey, VerifyingKey), UcanError> {
    use argon2::Argon2;

    // Use Argon2 for key derivation with domain separation
    let info = b"osvauld-ucan-ed25519-v1";
    let salt = b"osvauld-ucan-salt-v1-16bytes"; // Argon2 needs at least 8 bytes, 16 is good

    // Ensure salt is exactly 16 bytes for Argon2
    let mut salt_array = [0u8; 16];
    let salt_len = salt.len().min(16);
    salt_array[..salt_len].copy_from_slice(&salt[..salt_len]);

    // Derive 32 bytes for Ed25519 private key using Argon2
    let mut derived_key = [0u8; 32];

    // Use info as additional input by combining with pgp_private_key
    let mut input = Vec::with_capacity(pgp_private_key.len() + info.len());
    input.extend_from_slice(pgp_private_key);
    input.extend_from_slice(info);

    Argon2::default()
        .hash_password_into(&input, &salt_array, &mut derived_key)
        .map_err(|e| UcanError::KeyExtractionError(format!("Argon2 derivation failed: {}", e)))?;

    // Create Ed25519 signing key from derived bytes
    let signing_key = SigningKey::from_bytes(&derived_key);
    let verifying_key = signing_key.verifying_key();

    Ok((signing_key, verifying_key))
}

/// Generate a one-time UCAN token for user connection (root capability)
pub async fn generate_one_time_connection_token(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    capability_str: &str,
    role: &str,
) -> Result<String, UcanError> {
    let key_material = Ed25519KeyMaterial::new(signing_key.clone(), verifying_key.clone());
    let expiry_seconds = 24 * 60 * 60;
    let capability = Capability::from((capability_str, "use", &json!({})));
    let ucan = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience("*")
        .with_lifetime(expiry_seconds)
        .claiming_capability(capability)
        .with_fact("role", role.to_string())
        .build()
        .map_err(|e| UcanError::KeyExtractionError(format!("UCAN build error: {}", e)))?
        .sign()
        .await
        .map_err(|e| UcanError::KeyExtractionError(format!("UCAN signing error: {}", e)))?;

    let token = ucan
        .encode()
        .map_err(|e| UcanError::KeyExtractionError(format!("UCAN encoding error: {}", e)))?;

    Ok(token)
}

/// Check if a UCAN token is a one-time connect token
pub fn is_one_time_connect_token(ucan: &Ucan, domain: &str) -> bool {
    let is_wildcard_audience = ucan.audience() == "*";
    let required_prefix = format!("{}:user-connect", domain);
    let has_connect_capability = ucan
        .capabilities()
        .iter()
        .any(|cap| cap.resource.starts_with(&required_prefix) && cap.ability == "use");

    is_wildcard_audience && has_connect_capability
}

/// Extract the user role from a UCAN token
/// Returns the role string from the token's facts, or "viewer" as default
pub fn get_role_from_token(ucan: &Ucan) -> String {
    ucan.facts()
        .as_ref()
        .and_then(|facts| facts.get("role"))
        .and_then(|value| value.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "viewer".to_string())
}

/// Generate a delegation and connection token
/// This creates a token  that grants both connection and delegation capabilities
/// Issued directly by root authority (not delegated from one-time token)
pub async fn generate_delegation_and_connection_token(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    issuer_user_id: &str,
    audience_ucan_pub_key: &str,
    domain: &str,
    lifetime_seconds: u64,
    role: &str,
    additional_capabilities: Vec<(String, String)>, // [(resource, ability)]
) -> Result<String, UcanError> {
    let key_material = Ed25519KeyMaterial::new(signing_key.clone(), verifying_key.clone());
    let audience_did = pub_key_b64_to_did(audience_ucan_pub_key)?;
    let connect_resource = format!("{}:user-connect:{}", domain, issuer_user_id);
    let share_resource = format!("{}:user-share:{}", domain, issuer_user_id);
    let connect_cap = Capability::from((connect_resource.as_str(), "use", &json!({})));
    let share_cap = Capability::from((share_resource.as_str(), "use", &json!({})));

    let mut builder = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(&audience_did)
        .with_lifetime(lifetime_seconds)
        .claiming_capability(connect_cap)
        .claiming_capability(share_cap);

    // Add additional capabilities
    for (resource, ability) in additional_capabilities {
        let cap = Capability::from((resource.as_str(), ability.as_str(), &json!({})));
        builder = builder.claiming_capability(cap);
    }

    let ucan = builder
        .with_fact("role", role.to_string())
        .build()
        .map_err(|e| UcanError::CreationError(e.to_string()))?
        .sign()
        .await
        .map_err(|e| UcanError::SignatureError(e.to_string()))?;
    let token = ucan
        .encode()
        .map_err(|e| UcanError::EncodingError(e.to_string()))?;

    Ok(token)
}

/// Parses a UCAN token string and performs basic structural and cryptographic
/// validation, including signature and expiration checks.
///
/// ### Arguments
/// * `token` - The raw UCAN token string.
///
/// ### Returns
/// A `Result` containing the parsed and validated `Ucan` object, or a
/// `UcanError` on failure.
pub async fn validate_structure(token: &str) -> Result<Ucan, UcanError> {
    let ucan = Ucan::try_from(token).map_err(|e| UcanError::ParseError(e.to_string()))?;
    let key_constructors: &[(
        &'static [u8],
        fn(Vec<u8>) -> Result<Box<dyn ucan::crypto::KeyMaterial>>,
    )] = &[(ED25519_MAGIC_BYTES, crate::ucan_utils::bytes_to_ed25519_key)];
    let mut did_parser = DidParser::new(key_constructors);

    let now = chrono::Utc::now().timestamp() as u64;
    ucan.validate(Some(now), &mut did_parser)
        .await
        .map_err(|e| UcanError::ValidationError(e.to_string()))?;

    Ok(ucan)
}
/// Validates that the presenter of a UCAN is its intended audience by comparing
/// the raw public key from the `aud` field with the presenter's public key.
///
/// ### Arguments
/// * `ucan` - A reference to the parsed and structurally valid UCAN.
/// * `presenter_ucan_pub_b64` - The base64-encoded public key of the peer presenting the token.
///
/// ### Returns
/// A `Result` that is empty on success, or a `UcanError` on failure.
pub fn validate_audience(ucan: &Ucan, presenter_ucan_pub_b64: &str) -> Result<(), UcanError> {
    verify_did_key_match(ucan.audience(), presenter_ucan_pub_b64).map_err(|e| match e {
        UcanError::PublicKeyMismatch => UcanError::InvalidAudience,
        _ => e,
    })
}
pub fn get_ucan_cid(ucan_token: &str) -> Result<String, UcanError> {
    let ucan = Ucan::try_from(ucan_token).map_err(|e| UcanError::ParseError(e.to_string()))?;
    let token_cid = ucan
        .to_cid(UcanBuilder::<Ed25519KeyMaterial>::default_hasher())
        .map_err(|e| UcanError::UcanCidConvertionFailed(e.to_string()))?;
    Ok(token_cid.to_string())
}
/// Verifies if the public key in a did:key string matches a base64-encoded key.
pub fn verify_did_key_match(did: &str, key_b64: &str) -> Result<(), UcanError> {
    info!("did {}", did);
    let key_from_did = if did.starts_with("did:key:z") {
        let decoded_did = bs58::decode(&did[9..])
            .into_vec()
            .map_err(|e| UcanError::DidError(e.to_string()))?;

        if decoded_did.len() <= 2 {
            return Err(UcanError::FormatError("DID is too short".to_string()));
        }
        decoded_did[2..].to_vec()
    } else {
        return Err(UcanError::FormatError("Unsupported DID method".to_string()));
    };

    let key_from_b64 = general_purpose::STANDARD
        .decode(key_b64)
        .map_err(|e| UcanError::DecodingError(e.to_string()))?;

    if key_from_did != key_from_b64 {
        return Err(UcanError::PublicKeyMismatch);
    }

    Ok(())
}

/// Verifies that the UCAN contains a capability that grants the required permission.
/// This function supports wildcard matching for hierarchical resources (e.g., a
/// capability for "my-app:*" grants permission for "my-app:feature-a").
///
/// ### Arguments
/// * `ucan` - A reference to the parsed and structurally valid UCAN.
/// * `required_resource` - The resource string that permission is required for.
/// * `required_ability` - The ability string that is required.
///
/// ### Returns
/// A `Result` that is empty on success, or a `UcanError::CapabilityNotFound` on failure.
pub fn check_capability(
    ucan: &Ucan,
    required_resource: &str,
    required_ability: &str,
) -> Result<(), UcanError> {
    for capability in ucan.capabilities().iter() {
        if capability.ability == required_ability {
            let cap_resource = capability.resource;

            if cap_resource.ends_with('*') {
                let prefix = &cap_resource[..cap_resource.len() - 1];
                if required_resource.starts_with(prefix) {
                    return Ok(());
                }
            } else {
                if cap_resource == required_resource {
                    return Ok(());
                }
            }
        }
    }

    //  If the loop completes, no suitable capability was found.
    Err(UcanError::CapabilityNotFound)
}

/// Extract the domain prefix from a UCAN's capabilities.
///
/// This function looks for resource or folder capabilities in the UCAN and extracts
/// the domain prefix (e.g., "livnote" from "livnote:resource:abc123").
///
/// ### Arguments
/// * `ucan` - The UCAN object to extract the domain from.
/// * `resource_type` - The type of resource to look for ("resource" or "folder").
///
/// ### Returns
/// The domain prefix string, or an error if no matching capability is found.
pub fn extract_domain_from_ucan(ucan: &Ucan, resource_type: &str) -> Result<String, UcanError> {
    for capability in ucan.capabilities().iter() {
        let cap_resource = capability.resource;

        // Look for the resource_type pattern in the URI
        // Format: "domain:resource_type:id" or "domain:resource_type:*"
        if let Some(middle_pos) = cap_resource.find(&format!(":{}", resource_type)) {
            // Extract everything before ":resource_type"
            let domain = &cap_resource[..middle_pos];
            if !domain.is_empty() {
                return Ok(domain.to_string());
            }
        }
    }

    Err(UcanError::CapabilityNotFound)
}

/// Extract folder_id from UCAN and verify it has add_resources capability
///
/// Looks for capability matching "domain:folder:folder_id" pattern and checks
/// if it has the "add_resources" ability.
///
/// ### Arguments
/// * `ucan` - The UCAN object to extract from
/// * `domain` - The domain to match (e.g., "sthalam")
///
/// ### Returns
/// The folder_id string if found with add_resources capability
pub fn extract_folder_id_with_add_resources_capability(
    ucan: &Ucan,
    domain: &str,
) -> Result<String, UcanError> {
    let folder_pattern = format!("{}:folder:", domain);

    for capability in ucan.capabilities().iter() {
        let cap_resource = capability.resource;

        if cap_resource.starts_with(&folder_pattern) && capability.ability == "add_resources" {
            // Extract folder_id from "domain:folder:folder_id"
            if let Some(folder_id) = cap_resource.strip_prefix(&folder_pattern) {
                if !folder_id.is_empty() && folder_id != "*" {
                    return Ok(folder_id.to_string());
                }
            }
        }
    }

    Err(UcanError::CapabilityNotFound)
}

fn pub_key_b64_to_did(key_b64: &str) -> Result<String, UcanError> {
    let key_bytes = general_purpose::STANDARD
        .decode(key_b64)
        .map_err(|e| UcanError::DecodingError(e.to_string()))?;

    // Prepend the Ed25519 multicodec prefix
    let did_bytes = [ED25519_MAGIC_BYTES, key_bytes.as_slice()].concat();

    // bs58 encode and format as a did:key
    let did = format!("did:key:z{}", bs58::encode(did_bytes).into_string());
    Ok(did)
}

/// Generates a "root" UCAN for a new resource, issued by the owner to themselves.
///
/// This token grants full permissions and serves as the root of authority for
/// any future delegations.
pub async fn generate_resource_owner_ucan(
    owner_signing_key: &SigningKey,
    owner_verifying_key: &VerifyingKey,
    resource_id: &str,
    capability_prefix: &str,
) -> Result<(String, String), UcanError> {
    // 1. Create KeyMaterial for the owner using the struct from ucan_utils.rs.
    let key_material =
        Ed25519KeyMaterial::new(owner_signing_key.clone(), owner_verifying_key.clone());

    // 2. The issuer and audience are the same for the owner's root token.
    // The get_did method is defined by the KeyMaterial trait.
    let owner_did = key_material
        .get_did()
        .await
        .map_err(|e| UcanError::DidError(e.to_string()))?;

    // 3. A root token should have a very long lifetime.
    let long_lifetime = 30 * 365 * 24 * 60 * 60; // 30 years in seconds

    // 4. Define the full set of capabilities for the owner.
    let resource_uri = format!("{}:resource:{}", capability_prefix, resource_id);
    let capabilities = vec![
        Capability::from((resource_uri.as_str(), "crud/read", &json!({}))),
        Capability::from((resource_uri.as_str(), "crud/update", &json!({}))),
        Capability::from((resource_uri.as_str(), "crud/delete", &json!({}))),
        Capability::from((resource_uri.as_str(), "ucan/share", &json!({}))),
    ];

    // 5. Build the UCAN using the builder definition provided.
    let mut builder = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(&owner_did)
        .with_lifetime(long_lifetime);

    // Add each capability to the builder.
    for cap in capabilities {
        builder = builder.claiming_capability(cap);
    }

    // Finalize the builder, sign it, and encode it as a string.
    let ucan = builder
        .build()
        .map_err(|e| UcanError::CreationError(e.to_string()))?
        .sign()
        .await
        .map_err(|e| UcanError::SignatureError(e.to_string()))?;
    let token_cid = ucan
        .to_cid(UcanBuilder::<Ed25519KeyMaterial>::default_hasher())
        .map_err(|e| UcanError::UcanCidConvertionFailed(e.to_string()))?;
    // 6. Return the encoded token string.
    let token_str = ucan
        .encode()
        .map_err(|e| UcanError::EncodingError(e.to_string()))?;
    Ok((token_str, token_cid.to_string()))
}

/// Generates a flexible "root" UCAN for a resource with custom templates from frontend
///
/// Loro Migration - Phase 2: Accepts UCAN template JSON from frontend containing
/// owner_template and viewer_template.
///
/// # Arguments
/// * `owner_signing_key` - Owner's Ed25519 signing key
/// * `owner_verifying_key` - Owner's Ed25519 verifying key
/// * `resource_id` - UUID of the resource
/// * `capability_prefix` - Domain prefix (e.g., "sthalam.com")
/// * `ucan_template_json` - JSON string with owner_template and viewer_template
/// * `expiry_seconds` - Optional expiry in seconds (defaults to 30 years)
///
/// # Expected Template JSON Format
/// ```json
/// {
///   "owner_template": {
///     "capabilities": {"main_doc": "crud/merge", ...},
///     "no_update_from_node": [],
///     "dont_send_to_node": []
///   },
///   "viewer_template": {
///     "capabilities": {"main_doc": "crud/readonly", ...},
///     "no_update_from_node": ["main_doc"],
///     "dont_send_to_node": ["main_doc"]
///   }
/// }
/// ```
///
/// # Returns
/// * `Ok((token_string, token_cid))` - The encoded UCAN and its CID
/// * `Err(UcanError)` - Parse or creation error
pub async fn generate_flexible_resource_owner_ucan(
    owner_signing_key: &SigningKey,
    owner_verifying_key: &VerifyingKey,
    resource_id: &str,
    capability_prefix: &str,
    ucan_template_json: &str,
    expiry_seconds: Option<u64>,
) -> Result<(String, String), UcanError> {
    // 1. Parse the template JSON
    let template_value: serde_json::Value = serde_json::from_str(ucan_template_json)
        .map_err(|e| UcanError::TemplateInvalid(format!("Failed to parse template JSON: {}", e)))?;

    let template_obj = template_value.as_object()
        .ok_or_else(|| UcanError::TemplateInvalid("Template must be an object".to_string()))?;

    // 2. Extract owner_template
    let owner_template_value = template_obj.get("owner_template")
        .ok_or_else(|| UcanError::TemplateInvalid("Missing 'owner_template'".to_string()))?;

    let owner_template_obj = owner_template_value.as_object()
        .ok_or_else(|| UcanError::TemplateInvalid("owner_template must be an object".to_string()))?;

    let owner_capabilities_value = owner_template_obj.get("capabilities")
        .ok_or_else(|| UcanError::TemplateInvalid("owner_template missing 'capabilities'".to_string()))?;

    let owner_capabilities_obj = owner_capabilities_value.as_object()
        .ok_or_else(|| UcanError::TemplateInvalid("owner_template capabilities must be an object".to_string()))?;

    // 3. Create KeyMaterial for the owner
    let key_material = Ed25519KeyMaterial::new(owner_signing_key.clone(), owner_verifying_key.clone());

    let owner_did = key_material
        .get_did()
        .await
        .map_err(|e| UcanError::DidError(e.to_string()))?;

    // 4. Set lifetime (default 30 years or custom)
    let lifetime = expiry_seconds.unwrap_or(30 * 365 * 24 * 60 * 60);

    // 5. Build capabilities from owner_template
    let mut capabilities = Vec::new();
    for (doc_name, ability_value) in owner_capabilities_obj {
        let ability = ability_value.as_str()
            .ok_or_else(|| UcanError::TemplateInvalid(format!("Ability for {} must be a string", doc_name)))?;

        let resource_uri = format!("{}:resource:{}:{}", capability_prefix, resource_id, doc_name);
        capabilities.push(Capability::from((resource_uri.as_str(), ability, &json!({}))));
    }

    // 6. Build the UCAN with capabilities
    let mut builder = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(&owner_did)
        .with_lifetime(lifetime);

    for cap in capabilities {
        builder = builder.claiming_capability(cap);
    }

    // 7. Add both templates to facts
    builder = builder.with_fact("owner_template", owner_template_value.clone());

    let viewer_template_value = template_obj.get("viewer_template")
        .ok_or_else(|| UcanError::TemplateInvalid("Missing 'viewer_template'".to_string()))?;
    builder = builder.with_fact("viewer_template", viewer_template_value.clone());

    // 7a. Add role fact (owner vs viewer)
    builder = builder.with_fact("role", json!("owner"));

    // 7b. Add doc_types if present in owner_template
    if let Some(doc_types) = owner_template_obj.get("doc_types") {
        builder = builder.with_fact("doc_types", doc_types.clone());
    }

    // 7c. Add document list for resource parsing
    let doc_names: Vec<String> = owner_capabilities_obj.keys()
        .map(|k| k.to_string())
        .collect();
    builder = builder.with_fact("docs", json!(doc_names));

    // 8. Build, sign, and encode
    let ucan = builder
        .build()
        .map_err(|e| UcanError::CreationError(e.to_string()))?
        .sign()
        .await
        .map_err(|e| UcanError::SignatureError(e.to_string()))?;

    let token_cid = ucan
        .to_cid(UcanBuilder::<Ed25519KeyMaterial>::default_hasher())
        .map_err(|e| UcanError::UcanCidConvertionFailed(e.to_string()))?;

    let token_str = ucan
        .encode()
        .map_err(|e| UcanError::EncodingError(e.to_string()))?;

    Ok((token_str, token_cid.to_string()))
}

/// Generates a folder owner UCAN with templates for role-based delegation
///
/// This function creates a folder UCAN with templates embedded in the facts section.
/// The templates define capabilities for different roles (owner_template and node_template)
/// which are used during folder sharing to determine what capabilities to delegate.
///
/// # Arguments
/// * `owner_signing_key` - The signing key of the folder owner
/// * `owner_verifying_key` - The verifying key of the folder owner
/// * `folder_id` - The ID of the folder
/// * `capability_prefix` - The domain prefix (e.g., "sthalam")
/// * `folder_template_json` - JSON string containing owner_template and node_template
/// * `expiry_seconds` - Token lifetime in seconds (None = 30 years)
///
/// # Template JSON Structure
/// ```json
/// {
///   "owner_template": {
///     "capabilities": {
///       "add_resources": "...",
///       "crud/read": "...",
///       "share_folder": "..."
///     }
///   },
///   "node_template": {
///     "capabilities": {
///       "add_resources": "...",
///       "crud/read": "..."
///     }
///   }
/// }
/// ```
pub async fn generate_folder_ucan_with_template(
    owner_signing_key: &SigningKey,
    owner_verifying_key: &VerifyingKey,
    folder_id: &str,
    capability_prefix: &str,
    folder_template_json: &str,
    expiry_seconds: Option<u64>,
) -> Result<(String, String), UcanError> {
    // 1. Parse the template JSON
    let template_value: serde_json::Value = serde_json::from_str(folder_template_json)
        .map_err(|e| UcanError::TemplateInvalid(format!("Failed to parse folder template JSON: {}", e)))?;

    let template_obj = template_value.as_object()
        .ok_or_else(|| UcanError::TemplateInvalid("Folder template must be an object".to_string()))?;

    // 2. Extract owner_template
    let owner_template_value = template_obj.get("owner_template")
        .ok_or_else(|| UcanError::TemplateInvalid("Missing 'owner_template'".to_string()))?;

    let owner_template_obj = owner_template_value.as_object()
        .ok_or_else(|| UcanError::TemplateInvalid("owner_template must be an object".to_string()))?;

    let owner_capabilities_value = owner_template_obj.get("capabilities")
        .ok_or_else(|| UcanError::TemplateInvalid("owner_template missing 'capabilities'".to_string()))?;

    let owner_capabilities_obj = owner_capabilities_value.as_object()
        .ok_or_else(|| UcanError::TemplateInvalid("owner_template capabilities must be an object".to_string()))?;

    // 3. Create KeyMaterial for the owner
    let key_material = Ed25519KeyMaterial::new(owner_signing_key.clone(), owner_verifying_key.clone());

    let owner_did = key_material
        .get_did()
        .await
        .map_err(|e| UcanError::DidError(e.to_string()))?;

    // 4. Set lifetime (default 30 years or custom)
    let lifetime = expiry_seconds.unwrap_or(30 * 365 * 24 * 60 * 60);

    // 5. Build capabilities from owner_template
    let folder_uri = format!("{}:folder:{}", capability_prefix, folder_id);
    let mut capabilities = Vec::new();

    for (capability_name, _) in owner_capabilities_obj {
        capabilities.push(Capability::from((folder_uri.as_str(), capability_name.as_str(), &json!({}))));
    }

    // 6. Build the UCAN with capabilities
    let mut builder = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(&owner_did)
        .with_lifetime(lifetime);

    for cap in capabilities {
        builder = builder.claiming_capability(cap);
    }

    // 7. Add both templates to facts
    builder = builder.with_fact("owner_template", owner_template_value.clone());

    let node_template_value = template_obj.get("node_template")
        .ok_or_else(|| UcanError::TemplateInvalid("Missing 'node_template'".to_string()))?;
    builder = builder.with_fact("node_template", node_template_value.clone());

    // 8. Build, sign, and encode
    let ucan = builder
        .build()
        .map_err(|e| UcanError::CreationError(e.to_string()))?
        .sign()
        .await
        .map_err(|e| UcanError::SignatureError(e.to_string()))?;

    let token_cid = ucan
        .to_cid(UcanBuilder::<Ed25519KeyMaterial>::default_hasher())
        .map_err(|e| UcanError::UcanCidConvertionFailed(e.to_string()))?;

    let token_str = ucan
        .encode()
        .map_err(|e| UcanError::EncodingError(e.to_string()))?;

    Ok((token_str, token_cid.to_string()))
}

/// Generates a flexible UCAN token for a resource with custom permissions and expiry
///
/// This token can grant various levels of access to a resource.
/// Supports custom expiry times and multiple capability types.
///
/// # Arguments
/// * `owner_signing_key` - The signing key of the resource owner
/// * `owner_verifying_key` - The verifying key of the resource owner
/// * `resource_id` - The ID of the resource to share
/// * `capability_prefix` - The domain prefix (e.g., "sthalam")
/// * `expiry_seconds` - Token lifetime in seconds (None = infinite/30 years)
/// * `capabilities` - List of capabilities to grant (e.g., ["view/public", "crud/update", "ucan/share"])
/// * `audience` - Target audience DID or "*" for public
///
/// # Example capabilities:
/// - "view/public" - Read-only public access
/// - "crud/read" - Authenticated read
/// - "crud/update" - Edit permissions
/// - "crud/delete" - Delete permissions
/// - "ucan/share" - Can reshare to others
pub async fn generate_flexible_resource_token(
    owner_signing_key: &SigningKey,
    owner_verifying_key: &VerifyingKey,
    resource_id: &str,
    capability_prefix: &str,
    expiry_seconds: Option<u64>,
    capabilities: Vec<&str>,
    audience: &str,
) -> Result<String, UcanError> {
    // 1. Create KeyMaterial for the owner
    let key_material =
        Ed25519KeyMaterial::new(owner_signing_key.clone(), owner_verifying_key.clone());

    // 2. Set lifetime (default to 30 years if None for "infinite")
    let lifetime = expiry_seconds.unwrap_or(30 * 365 * 24 * 60 * 60);

    // 3. Build capabilities from the provided list
    let resource_uri = format!("{}:resource:{}", capability_prefix, resource_id);
    let mut builder = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(audience)
        .with_lifetime(lifetime);

    // Add each capability to the builder
    for capability_str in capabilities {
        let capability = Capability::from((resource_uri.as_str(), capability_str, &json!({})));
        builder = builder.claiming_capability(capability);
    }

    // 4. Build and sign the UCAN
    let ucan = builder
        .build()
        .map_err(|e| UcanError::CreationError(e.to_string()))?
        .sign()
        .await
        .map_err(|e| UcanError::SignatureError(e.to_string()))?;

    // 5. Return the encoded token string
    let token_str = ucan
        .encode()
        .map_err(|e| UcanError::EncodingError(e.to_string()))?;

    Ok(token_str)
}

/// Generates a public view-only UCAN token for a resource (convenience wrapper)
///
/// This token grants read-only access to a resource for public viewing.
/// Uses wildcard audience (*) and only grants "view/public" capability.
/// Default expiry: 30 days
pub async fn generate_public_view_token(
    owner_signing_key: &SigningKey,
    owner_verifying_key: &VerifyingKey,
    resource_id: &str,
    capability_prefix: &str,
) -> Result<String, UcanError> {
    generate_flexible_resource_token(
        owner_signing_key,
        owner_verifying_key,
        resource_id,
        capability_prefix,
        Some(30 * 24 * 60 * 60), // 30 days
        vec!["view/public"],
        "*", // Public wildcard audience
    )
    .await
}

/// Generates a flexible UCAN token for a folder with custom permissions and expiry
///
/// This token grants access to all resources in a folder.
/// Supports custom expiry times and multiple capability types.
///
/// # Arguments
/// * `owner_signing_key` - The signing key of the folder owner
/// * `owner_verifying_key` - The verifying key of the folder owner
/// * `folder_id` - The ID of the folder to share
/// * `capability_prefix` - The domain prefix (e.g., "sthalam")
/// * `expiry_seconds` - Token lifetime in seconds (None = infinite/30 years)
/// * `capabilities` - List of capabilities to grant
/// * `audience` - Target audience DID or "*" for public
/// * `role` - The role to embed in the token (e.g., "viewer", "user", "owner")
pub async fn generate_flexible_folder_token(
    owner_signing_key: &SigningKey,
    owner_verifying_key: &VerifyingKey,
    folder_id: &str,
    capability_prefix: &str,
    expiry_seconds: Option<u64>,
    capabilities: Vec<&str>,
    audience: &str,
    role: &str,
) -> Result<String, UcanError> {
    // 1. Create KeyMaterial for the owner
    let key_material =
        Ed25519KeyMaterial::new(owner_signing_key.clone(), owner_verifying_key.clone());

    // 2. Set lifetime (default to 30 years if None for "infinite")
    let lifetime = expiry_seconds.unwrap_or(30 * 365 * 24 * 60 * 60);

    // 3. Build capabilities for both folder and its resources
    let folder_uri = format!("{}:folder:{}", capability_prefix, folder_id);
    let resource_wildcard = format!("{}:resource:{}/*", capability_prefix, folder_id);

    let mut builder = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(audience)
        .with_lifetime(lifetime)
        .with_fact("role", role.to_string());

    // Add folder capabilities
    for capability_str in &capabilities {
        let folder_cap = Capability::from((folder_uri.as_str(), *capability_str, &json!({})));
        builder = builder.claiming_capability(folder_cap);
    }

    // Add resource wildcard capabilities
    for capability_str in capabilities {
        let resource_cap =
            Capability::from((resource_wildcard.as_str(), capability_str, &json!({})));
        builder = builder.claiming_capability(resource_cap);
    }

    // 4. Build and sign the UCAN
    let ucan = builder
        .build()
        .map_err(|e| UcanError::CreationError(e.to_string()))?
        .sign()
        .await
        .map_err(|e| UcanError::SignatureError(e.to_string()))?;

    // 5. Return the encoded token string
    let token_str = ucan
        .encode()
        .map_err(|e| UcanError::EncodingError(e.to_string()))?;

    Ok(token_str)
}

/// Generates a public view-only UCAN token for a folder (convenience wrapper)
///
/// This token grants read-only access to all resources in a folder for public viewing.
/// Uses wildcard audience and only grants "view/public" capability.
/// Default expiry: 30 days
pub async fn generate_public_folder_view_token(
    owner_signing_key: &SigningKey,
    owner_verifying_key: &VerifyingKey,
    folder_id: &str,
    capability_prefix: &str,
) -> Result<String, UcanError> {
    generate_flexible_folder_token(
        owner_signing_key,
        owner_verifying_key,
        folder_id,
        capability_prefix,
        Some(30 * 24 * 60 * 60), // 30 days
        vec!["view/public"],
        "*",      // Public wildcard audience
        "viewer", // Role for public folder viewers
    )
    .await
}

pub async fn validate_ucan_permission<F, Fut>(
    ucan: &Ucan,
    verifier_ucan_pub_b64: &str,
    proof_resolver: &F,
    required_resource: &str,
    required_ability: &str,
) -> Result<(), UcanError>
where
    F: Fn(&str) -> Fut + Send + Sync,
    Fut: Future<Output = Result<String, UcanError>> + Send + 'static,
{
    let issuer_did = ucan.issuer();
    // 1. Base Case: Check if the UCAN was issued directly by the verifier.
    match verify_did_key_match(issuer_did, verifier_ucan_pub_b64) {
        Ok(()) => return Ok(()), // Success! Direct authority confirmed.
        Err(UcanError::PublicKeyMismatch) => {
            // This is expected for a delegated UCAN. We proceed to check proofs.
        }
        Err(e) => return Err(e), // Propagate other errors.
    }

    // 2. Recursive Step: If not issued by the verifier, a proof is required.
    if let Some(proofs_vec) = ucan.proofs() {
        if proofs_vec.is_empty() {
            return Err(UcanError::ProofRequired);
        }

        for proof_cid_string in proofs_vec {
            // a. Look up the CID directly in the resolver.
            let parent_token_string = match proof_resolver(proof_cid_string).await {
                Ok(token) => token,
                Err(_) => continue, // If resolver fails to find the proof, try the next one.
            };

            // b. Parse the resolved parent token string.
            let parent_ucan = match validate_structure(&parent_token_string).await {
                Ok(ucan) => ucan,
                Err(_) => continue,
            };

            // c. Validate the parent UCAN.
            if parent_ucan.audience() != ucan.issuer() {
                continue;
            }

            if check_capability(&parent_ucan, required_resource, required_ability).is_err() {
                return Err(UcanError::DelegationNotPermitted);
            }

            // d. Make the recursive call, passing the map down the chain.
            let recursive_call = validate_ucan_permission(
                &parent_ucan,
                verifier_ucan_pub_b64,
                proof_resolver,
                required_resource,
                required_ability,
            );
            if Box::pin(recursive_call).await.is_ok() {
                return Ok(());
            }
        }
    } else {
        return Err(UcanError::ProofRequired);
    }

    Err(UcanError::ProofChainInvalid(
        "No valid proof chain leads back to the verifier.".to_string(),
    ))
}

/// Generates a delegated UCAN, using a parent UCAN string as proof.
///
/// This function creates a new link in a delegation chain by using the standard
/// CID-based proof mechanism. It returns a tuple of the new `(token_string, token_cid)`.
pub async fn generate_delegated_ucan(
    delegator_signing_key: &SigningKey,
    delegator_verifying_key: &VerifyingKey,
    recipient_ucan_pub_key: &str,
    permissions: Vec<(String, String)>, // The specific permissions to grant
    proof_ucan_string: &str,            // The parent UCAN token string
    template_value: Option<serde_json::Value>, // Template to include in facts
    recipient_role: &str,                       // Role (owner, node, viewer)
    doc_types_value: Option<serde_json::Value>, // Doc types mapping
    docs_list: Option<Vec<String>>,              // List of doc names
) -> Result<(String, String), UcanError> {
    // 1. Parse the parent UCAN string to use it as a proof object.
    let authority_ucan =
        Ucan::try_from(proof_ucan_string).map_err(|e| UcanError::ParseError(e.to_string()))?;

    // 2. Set up the delegator's key material.
    let delegator_key_material = Ed25519KeyMaterial::new(
        delegator_signing_key.clone(),
        delegator_verifying_key.clone(),
    );
    // 3. Get the recipient's DID from their public key.
    let recipient_did = pub_key_b64_to_did(recipient_ucan_pub_key)?;

    // 4. Create the Vec<Capability> from the input permissions.
    let capabilities: Vec<Capability> = permissions
        .into_iter()
        .map(|(resource, ability)| {
            Capability::from((resource.as_str(), ability.as_str(), &json!({})))
        })
        .collect();
    // 5. Set a 30-year lifetime.
    let long_lifetime = 30 * 365 * 24 * 60 * 60;

    // 6. Build the new UCAN with capabilities and facts
    let mut builder = UcanBuilder::default()
        .issued_by(&delegator_key_material)
        .for_audience(&recipient_did)
        .with_lifetime(long_lifetime)
        .claiming_capabilities(&capabilities)
        .witnessed_by(&authority_ucan, None);

    // 7. Add template to facts based on role
    if let Some(template) = template_value {
        let template_key = match recipient_role {
            "owner" | "node" => "owner_template",
            "viewer" => "viewer_template",
            _ => "owner_template", // default
        };
        builder = builder.with_fact(template_key, template);
    }

    // 8. Add role to facts
    builder = builder.with_fact("role", recipient_role.to_string());

    // 9. Add doc_types to facts if provided
    if let Some(doc_types) = doc_types_value {
        builder = builder.with_fact("doc_types", doc_types);
    }

    // 10. Add docs list to facts if provided
    if let Some(docs) = docs_list {
        builder = builder.with_fact("docs", docs);
    }

    // 11. Build and sign the UCAN
    let ucan = builder
        .build()
        .map_err(|e| UcanError::CreationError(e.to_string()))?
        .sign()
        .await
        .map_err(|e| UcanError::SignatureError(e.to_string()))?;

    // 7. Get the final token string and its CID to be stored.
    let token_string = ucan
        .encode()
        .map_err(|e| UcanError::EncodingError(e.to_string()))?;

    let token_cid = ucan
        .to_cid(UcanBuilder::<Ed25519KeyMaterial>::default_hasher())
        .map_err(|e| UcanError::UcanCidConvertionFailed(e.to_string()))?
        .to_string();

    Ok((token_string, token_cid))
}
/// Generate a delegated user connection token with embedded proof
pub async fn generate_delegated_user_connection_token(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    target_user_id: &str,
    audience_ucan_pub_key: &str,
    domain: &str,
    parent_token: &str,
) -> Result<String, UcanError> {
    let key_material = Ed25519KeyMaterial::new(signing_key.clone(), verifying_key.clone());
    let audience_did = pub_key_b64_to_did(audience_ucan_pub_key)?;

    let connect_resource = format!("{}:user-connect:{}", domain, target_user_id);
    let connect_capability = Capability::from((connect_resource.as_str(), "use", &json!({})));

    let lifetime = 30 * 365 * 24 * 60 * 60; // 30 years

    // Calculate CID of parent token for standard prf field
    let parent_ucan =
        Ucan::try_from(parent_token).map_err(|e| UcanError::ParseError(e.to_string()))?;

    let ucan = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(&audience_did)
        .with_lifetime(lifetime)
        .claiming_capability(connect_capability)
        .witnessed_by(&parent_ucan, None) // Standard CID reference
        .with_fact("proof", parent_token.to_string()) // Embedded proof
        .build()
        .map_err(|e| UcanError::CreationError(e.to_string()))?
        .sign()
        .await
        .map_err(|e| UcanError::SignatureError(e.to_string()))?;

    let token_string = ucan
        .encode()
        .map_err(|e| UcanError::EncodingError(e.to_string()))?;

    Ok(token_string)
}
pub async fn validate_embedded_proof_chain(
    proof_token: &str,
    target_user_id: &str,              // For capability validation
    target_user_ucan_pub: &str,        // For root authority verification
    domain: &str,                      // For domain consistency
    expected_issuer_did: Option<&str>, // Current token's issuer should match this
) -> Result<(), UcanError> {
    // 1. Parse and structurally validate
    let proof_ucan = validate_structure(proof_token).await?;

    // 2. Verify issuer matches expected (audience of parent token)
    if let Some(expected_did) = expected_issuer_did {
        if proof_ucan.audience() != expected_did {
            return Err(UcanError::InvalidIssuer);
        }
    }

    // 3. Check capability for target user
    let required_resource = format!("{}:user-connect:{}", domain, target_user_id);
    check_capability(&proof_ucan, &required_resource, "use")?;

    // 4. Base case: Check if issued by target user (root authority)
    if verify_did_key_match(proof_ucan.issuer(), target_user_ucan_pub).is_ok() {
        return Ok(()); // Reached root authority
    }
    if let Some(facts_map) = proof_ucan.facts() {
        if let Some(proof_value) = facts_map.get("proof") {
            if let Some(nested_proof) = proof_value.as_str() {
                let nested_proof_cid = get_ucan_cid(nested_proof)?;
                if let Some(prf_cids) = proof_ucan.proofs().as_ref() {
                    if !prf_cids.contains(&nested_proof_cid) {
                        return Err(UcanError::ProofChainInvalid("CID mismatch".to_string()));
                    }
                }

                return Box::pin(validate_embedded_proof_chain(
                    nested_proof,
                    target_user_id,
                    target_user_ucan_pub,
                    domain,
                    Some(proof_ucan.issuer()),
                ))
                .await;
            }
        }
    }

    // No proof chain and not root authority = invalid
    Err(UcanError::ProofChainInvalid(
        "Chain doesn't reach root".to_string(),
    ))
}

// ============================================================================
// Loro Migration - Phase 1.3: UCAN Extraction for Resource Sync
// ============================================================================

/// Template structure extracted from UCAN facts
/// Used by Node to create viewer UCANs with the same structure
#[derive(Debug, Clone)]
pub struct UcanTemplate {
    /// Document capabilities: doc_name -> ability (e.g., "main_doc" -> "crud/readonly")
    pub capabilities: std::collections::HashMap<String, String>,
    /// Documents that viewer should not accept updates for (local-only)
    pub no_update_from_node: Vec<String>,
    /// Documents that viewer should not send to node (local-only)
    pub dont_send_to_node: Vec<String>,
}

/// Loro Migration - Phase 2: UCAN Facts with Owner and Viewer Templates
///
/// Facts structure contains two templates:
/// - owner_template: Full capabilities for owner and node
/// - viewer_template: Restricted capabilities for viewers
#[derive(Debug, Clone)]
pub struct UcanFacts {
    /// Template for owner and node (full access)
    pub owner_template: UcanTemplate,
    /// Template for viewers (restricted access)
    pub viewer_template: UcanTemplate,
}

/// Extract raw facts section from UCAN token
///
/// # Arguments
/// * `token` - The UCAN token string
///
/// # Returns
/// * `Ok(Some(facts_map))` - Facts section as JSON map
/// * `Ok(None)` - No facts section in UCAN (valid, just no facts)
/// * `Err(UcanError)` - Parse error
pub fn extract_facts(token: &str) -> Result<Option<serde_json::Map<String, serde_json::Value>>, UcanError> {
    let ucan = Ucan::try_from(token).map_err(|e| UcanError::ParseError(e.to_string()))?;
    Ok(extract_facts_from_ucan(&ucan))
}

/// Internal helper: Extract facts from parsed Ucan object
fn extract_facts_from_ucan(ucan: &Ucan) -> Option<serde_json::Map<String, serde_json::Value>> {
    ucan.facts().as_ref().map(|facts| {
        // Convert BTreeMap to serde_json::Map
        facts.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    })
}

/// Extract template from UCAN facts section
///
/// Template is used by Node to create viewer UCANs with proper doc structure and sync rules.
///
/// # Expected Facts Structure
/// ```json
/// {
///   "fct": {
///     "template": {
///       "capabilities": {
///         "main_doc": "crud/readonly",
///         "comments": "crud/merge"
///       },
///       "no_update_from_node": ["uiState", "cartDoc"],
///       "dont_send_to_node": ["uiState", "cartDoc"]
///     }
///   }
/// }
/// ```
///
/// # Arguments
/// * `token` - The UCAN token string
///
/// # Returns
/// * `Ok(Some(template))` - Template found and parsed
/// * `Ok(None)` - No template in facts (not all UCANs have templates)
/// * `Err(UcanError::TemplateInvalid)` - Template exists but malformed
pub fn extract_template(token: &str) -> Result<Option<UcanTemplate>, UcanError> {
    let ucan = Ucan::try_from(token).map_err(|e| UcanError::ParseError(e.to_string()))?;
    extract_template_from_ucan(&ucan)
}

/// Internal helper: Extract template from parsed Ucan object
fn extract_template_from_ucan(ucan: &Ucan) -> Result<Option<UcanTemplate>, UcanError> {
    let facts = match extract_facts_from_ucan(ucan) {
        Some(f) => f,
        None => return Ok(None), // No facts = no template (valid)
    };

    // Check if template exists in facts
    let template_value = match facts.get("template") {
        Some(t) => t,
        None => return Ok(None), // No template in facts (valid)
    };

    // Parse template object
    let template_obj = template_value.as_object()
        .ok_or_else(|| UcanError::TemplateInvalid("Template must be an object".to_string()))?;

    // Extract capabilities
    let capabilities_value = template_obj.get("capabilities")
        .ok_or_else(|| UcanError::TemplateInvalid("Missing 'capabilities' field".to_string()))?;

    let capabilities_obj = capabilities_value.as_object()
        .ok_or_else(|| UcanError::TemplateInvalid("capabilities must be an object".to_string()))?;

    let mut capabilities = std::collections::HashMap::new();
    for (doc_name, ability_value) in capabilities_obj {
        let ability = ability_value.as_str()
            .ok_or_else(|| UcanError::TemplateInvalid(format!("Ability for {} must be a string", doc_name)))?;
        capabilities.insert(doc_name.clone(), ability.to_string());
    }

    // Extract no_update_from_node (optional, defaults to empty)
    let no_update_from_node = template_obj.get("no_update_from_node")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Extract dont_send_to_node (optional, defaults to empty)
    let dont_send_to_node = template_obj.get("dont_send_to_node")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(Some(UcanTemplate {
        capabilities,
        no_update_from_node,
        dont_send_to_node,
    }))
}

/// Extract document capabilities from UCAN token
///
/// Parses capabilities in the format: `domain:resource:resource_id:doc_name`
/// Returns a map of doc_name -> ability (e.g., "main_doc" -> "crud/readonly")
///
/// # Expected Capability Format
/// ```json
/// {
///   "cap": {
///     "sthalam:resource:abc123:main_doc": {"crud/readonly": [{}]},
///     "sthalam:resource:abc123:comments": {"crud/merge": [{}]},
///     "sthalam:resource:abc123:submissions": {"crud/appendonly": [{}]}
///   }
/// }
/// ```
///
/// # Arguments
/// * `token` - The UCAN token string
///
/// # Returns
/// * `Ok(HashMap)` - Map of doc_name to ability
/// * `Err(UcanError::NoDocumentCapabilities)` - No document capabilities found
/// * `Err(UcanError::ParseError)` - Failed to parse token
pub fn extract_doc_capabilities(token: &str) -> Result<std::collections::HashMap<String, String>, UcanError> {
    let ucan = Ucan::try_from(token).map_err(|e| UcanError::ParseError(e.to_string()))?;
    extract_doc_capabilities_from_ucan(&ucan)
}

/// Internal helper: Extract doc capabilities from parsed Ucan object
fn extract_doc_capabilities_from_ucan(ucan: &Ucan) -> Result<std::collections::HashMap<String, String>, UcanError> {
    let mut doc_capabilities = std::collections::HashMap::new();

    for capability in ucan.capabilities().iter() {
        let cap_resource = capability.resource;

        // Look for resource pattern with doc name: "domain:resource:resource_id:doc_name"
        if cap_resource.contains(":resource:") {
            let parts: Vec<&str> = cap_resource.split(':').collect();

            // Must have exactly 4 parts: [domain, "resource", resource_id, doc_name]
            if parts.len() == 4 && parts[1] == "resource" {
                let doc_name = parts[3];

                // Get the ability (first ability in the map)
                let ability = capability.ability;

                doc_capabilities.insert(doc_name.to_string(), ability.to_string());
            }
        }
    }

    if doc_capabilities.is_empty() {
        return Err(UcanError::NoDocumentCapabilities);
    }

    Ok(doc_capabilities)
}

// ============================================================================
// Loro Migration - Phase 2: Extract Owner and Viewer Templates
// ============================================================================

/// Extract UcanFacts containing both owner and viewer templates
///
/// # Expected Facts Structure
/// ```json
/// {
///   "owner_template": {
///     "capabilities": {"main_doc": "crud/merge", ...},
///     "no_update_from_node": [],
///     "dont_send_to_node": []
///   },
///   "viewer_template": {
///     "capabilities": {"main_doc": "crud/readonly", ...},
///     "no_update_from_node": ["main_doc"],
///     "dont_send_to_node": ["main_doc"]
///   }
/// }
/// ```
///
/// # Arguments
/// * `token` - The UCAN token string
///
/// # Returns
/// * `Ok(UcanFacts)` - Both templates found and parsed
/// * `Err(UcanError)` - Missing or malformed templates
pub fn extract_ucan_facts(token: &str) -> Result<UcanFacts, UcanError> {
    let ucan = Ucan::try_from(token).map_err(|e| UcanError::ParseError(e.to_string()))?;
    extract_ucan_facts_from_ucan(&ucan)
}

/// Internal helper: Extract UcanFacts from parsed Ucan object
fn extract_ucan_facts_from_ucan(ucan: &Ucan) -> Result<UcanFacts, UcanError> {
    let facts = extract_facts_from_ucan(ucan)
        .ok_or_else(|| UcanError::TemplateNotFound)?;

    // Extract owner_template
    let owner_template = extract_template_from_facts(&facts, "owner_template")?;

    // Extract viewer_template
    let viewer_template = extract_template_from_facts(&facts, "viewer_template")?;

    Ok(UcanFacts {
        owner_template,
        viewer_template,
    })
}

/// Extract a specific template from facts by key
fn extract_template_from_facts(
    facts: &serde_json::Map<String, serde_json::Value>,
    template_key: &str,
) -> Result<UcanTemplate, UcanError> {
    let template_value = facts.get(template_key)
        .ok_or_else(|| UcanError::TemplateNotFound)?;

    let template_obj = template_value.as_object()
        .ok_or_else(|| UcanError::TemplateInvalid(format!("{} must be an object", template_key)))?;

    // Extract capabilities
    let capabilities_value = template_obj.get("capabilities")
        .ok_or_else(|| UcanError::TemplateInvalid(format!("{} missing 'capabilities' field", template_key)))?;

    let capabilities_obj = capabilities_value.as_object()
        .ok_or_else(|| UcanError::TemplateInvalid(format!("{} capabilities must be an object", template_key)))?;

    let mut capabilities = std::collections::HashMap::new();
    for (doc_name, ability_value) in capabilities_obj {
        let ability = ability_value.as_str()
            .ok_or_else(|| UcanError::TemplateInvalid(format!("Ability for {} must be a string", doc_name)))?;
        capabilities.insert(doc_name.clone(), ability.to_string());
    }

    // Extract no_update_from_node (optional, defaults to empty)
    let no_update_from_node = template_obj.get("no_update_from_node")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Extract dont_send_to_node (optional, defaults to empty)
    let dont_send_to_node = template_obj.get("dont_send_to_node")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(UcanTemplate {
        capabilities,
        no_update_from_node,
        dont_send_to_node,
    })
}

/// Extract only the owner template from UCAN facts
///
/// # Arguments
/// * `token` - The UCAN token string
///
/// # Returns
/// * `Ok(UcanTemplate)` - Owner template found and parsed
/// * `Err(UcanError)` - Missing or malformed template
pub fn extract_owner_template(token: &str) -> Result<UcanTemplate, UcanError> {
    let ucan = Ucan::try_from(token).map_err(|e| UcanError::ParseError(e.to_string()))?;
    let facts = extract_facts_from_ucan(&ucan)
        .ok_or_else(|| UcanError::TemplateNotFound)?;
    extract_template_from_facts(&facts, "owner_template")
}

/// Extract only the viewer template from UCAN facts
///
/// # Arguments
/// * `token` - The UCAN token string
///
/// # Returns
/// * `Ok(UcanTemplate)` - Viewer template found and parsed
/// * `Err(UcanError)` - Missing or malformed template
pub fn extract_viewer_template(token: &str) -> Result<UcanTemplate, UcanError> {
    let ucan = Ucan::try_from(token).map_err(|e| UcanError::ParseError(e.to_string()))?;
    let facts = extract_facts_from_ucan(&ucan)
        .ok_or_else(|| UcanError::TemplateNotFound)?;
    extract_template_from_facts(&facts, "viewer_template")
}
