use anyhow::{Result, anyhow};
use async_trait::async_trait;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use ucan::builder::UcanBuilder;
use ucan::capability::Capability;
use ucan::crypto::KeyMaterial;
use ucan::crypto::did::{DidParser, ED25519_MAGIC_BYTES};

/// Ed25519 KeyMaterial implementation for UCAN library
pub struct Ed25519KeyMaterial {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl Ed25519KeyMaterial {
    pub fn new(signing_key: SigningKey, verifying_key: VerifyingKey) -> Self {
        Self {
            signing_key,
            verifying_key,
        }
    }
}

#[async_trait]
impl KeyMaterial for Ed25519KeyMaterial {
    fn get_jwt_algorithm_name(&self) -> String {
        "EdDSA".to_string()
    }

    async fn get_did(&self) -> Result<String> {
        // Create did:key from Ed25519 public key
        let public_key_bytes = self.verifying_key.as_bytes();

        // Create multicodec prefix for Ed25519 (0xed01)
        let mut multicodec_key = vec![0xed, 0x01];
        multicodec_key.extend_from_slice(public_key_bytes);

        // Use bs58 encoding (same as UCAN crate uses)
        let encoded = bs58::encode(&multicodec_key).into_string();

        Ok(format!("did:key:z{}", encoded))
    }

    async fn sign(&self, payload: &[u8]) -> Result<Vec<u8>> {
        let signature = self.signing_key.sign(payload);
        Ok(signature.to_bytes().to_vec())
    }

    async fn verify(&self, payload: &[u8], signature: &[u8]) -> Result<()> {
        use ed25519_dalek::{Signature, Verifier};

        let signature_bytes: [u8; 64] = signature
            .try_into()
            .map_err(|_| anyhow!("Invalid signature length"))?;

        let sig = Signature::from_bytes(&signature_bytes);

        self.verifying_key
            .verify(payload, &sig)
            .map_err(|e| anyhow!("Signature verification failed: {}", e))
    }
}

/// UCAN Token structure
#[derive(Debug, Clone)]
pub struct UcanToken {
    pub token: String,
    pub issuer_did: String,
    pub audience_did: String,
    pub expires_at: u64,
    pub capabilities: Vec<String>,
}

/// Generate a one-time user share token (Stateless Domain Function)
/// Creates a self-issued token for user sharing (7-day expiry)
pub async fn generate_one_time_user_share_token(
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
) -> Result<UcanToken, String> {
    // 1. Create KeyMaterial for UCAN library
    let key_material = Ed25519KeyMaterial::new(signing_key, verifying_key);

    // 2. Get issuer DID (our DID)
    let issuer_did = key_material
        .get_did()
        .await
        .map_err(|e| format!("Failed to create issuer DID: {}", e))?;

    // 3. Calculate expiration time (7 days)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Failed to get current time: {}", e))?
        .as_secs();
    let expires_at = now + (7 * 24 * 60 * 60); // 7 days in seconds

    // 4. Create capability with empty caveat
    let capability = Capability::new(
        format!("osvauld:user:{}", issuer_did), // Resource URI
        "share".to_string(),                    // Ability
        Value::Null,                            // Caveat (empty)
    );

    // 5. Build UCAN token (self-issued for one-time sharing)
    let ucan_builder_result = UcanBuilder::default()
        .issued_by(&key_material)
        .for_audience(&issuer_did) // Self-issued
        .with_lifetime(expires_at)
        .with_nonce()
        .claiming_capability(capability)
        .build();

    let signable = ucan_builder_result.map_err(|e| format!("Failed to build UCAN token: {}", e))?;

    let signed = signable
        .sign()
        .await
        .map_err(|e| format!("Failed to sign UCAN token: {}", e))?;

    let ucan_token = signed
        .encode()
        .map_err(|e| format!("Failed to encode UCAN token: {}", e))?;

    // 6. Return structured token
    Ok(UcanToken {
        token: ucan_token,
        issuer_did: issuer_did.clone(),
        audience_did: issuer_did, // Self-issued
        expires_at,
        capabilities: vec!["share".to_string()],
    })
}

/// Ed25519 key constructor function for DID parsing
fn ed25519_key_constructor(public_key_bytes: Vec<u8>) -> Result<Box<dyn KeyMaterial>> {
    use ed25519_dalek::VerifyingKey;

    if public_key_bytes.len() != 32 {
        return Err(anyhow!(
            "Invalid Ed25519 public key length: expected 32 bytes, got {}",
            public_key_bytes.len()
        ));
    }

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&public_key_bytes);

    let verifying_key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|e| anyhow!("Failed to create Ed25519 verifying key: {}", e))?;

    // For validation purposes, we only need the verifying key
    // We'll create a dummy signing key since we're only verifying signatures
    let signing_key = SigningKey::generate(&mut OsRng);

    Ok(Box::new(Ed25519KeyMaterial::new(
        signing_key,
        verifying_key,
    )))
}

/// Validate a UCAN token (Stateless Domain Function)
pub async fn validate_ucan_token(token: &str) -> Result<UcanToken, String> {
    // Parse the UCAN token
    let ucan =
        ucan::Ucan::try_from(token).map_err(|e| format!("Failed to parse UCAN token: {}", e))?;

    // Create DID parser for Ed25519 keys
    let mut did_parser = DidParser::new(&[(ED25519_MAGIC_BYTES, ed25519_key_constructor)]);

    // Validate the UCAN token
    // Pass None for current timestamp to use system time
    ucan.validate(None, &mut did_parser)
        .await
        .map_err(|e| format!("UCAN token validation failed: {}", e))?;

    // Check if token is expired
    if let Some(exp) = ucan.expires_at() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("Failed to get current time: {}", e))?
            .as_secs();

        if now >= exp.clone() {
            return Err("UCAN token has expired".to_string());
        }
    }

    // Extract information
    let issuer_did = ucan.issuer().to_string();
    let audience_did = ucan.audience().to_string();
    let expires_at = ucan.expires_at().ok_or("UCAN token has no expiration")?;

    // Extract capabilities
    let capabilities = ucan
        .capabilities()
        .iter()
        .map(|cap| cap.ability.clone())
        .collect();

    Ok(UcanToken {
        token: token.to_string(),
        issuer_did,
        audience_did,
        expires_at,
        capabilities,
    })
}
