//! Identity - Our own user identity data
//!
//! Stored in Sled as:
//! - identity/self → IdentityData (public info)
//! - identity/keystore → EncryptedKeyStore (encrypted mnemonic)

use chrono::Local;
use serde::{Deserialize, Serialize};

/// UserInfo - Minimal user info for UI display and handshakes
///
/// Derived from Identity when needed, not stored.
/// Contains both signing (public_key) and encryption keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub did: String,
    pub username: String,
    /// Ed25519 signing public key (32 bytes)
    pub public_key: Vec<u8>,
    /// X25519 encryption public key (32 bytes)
    pub encryption_key: Vec<u8>,
}

/// IdentityData - Our own user identity (public info)
///
/// This is the "self" user - the owner of this node/device.
/// Stored in Sled identity tree under key "self".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityData {
    /// Our DID (did:key:z...)
    pub did: String,
    /// Ed25519 public signing key (32 bytes)
    pub signing_public_key: Vec<u8>,
    /// X25519 public encryption key (32 bytes)
    pub encryption_public_key: Vec<u8>,
    /// Ed25519 public device key for Iroh P2P (32 bytes)
    #[serde(default)]
    pub device_public_key: Vec<u8>,
    /// Display username
    pub username: String,
    /// When this identity was created
    pub created_at: i64,
}

impl IdentityData {
    pub fn new(
        did: String,
        signing_public_key: Vec<u8>,
        encryption_public_key: Vec<u8>,
        device_public_key: Vec<u8>,
        username: String,
    ) -> Self {
        Self {
            did,
            signing_public_key,
            encryption_public_key,
            device_public_key,
            username,
            created_at: Local::now().timestamp_millis(),
        }
    }
}

/// Argon2 parameters for key derivation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argon2Params {
    /// Memory cost in KiB (default: 65536 = 64MB)
    pub m_cost: u32,
    /// Time cost / iterations (default: 3)
    pub t_cost: u32,
    /// Parallelism (default: 4)
    pub p_cost: u32,
}

impl Default for Argon2Params {
    fn default() -> Self {
        Self {
            m_cost: 65536, // 64 MB
            t_cost: 3,
            p_cost: 4,
        }
    }
}

/// EncryptedKeyStore - Encrypted identity keys for storage
///
/// Stored in Sled identity tree under key "keystore".
/// Keys are encrypted with AES-256-GCM using a key derived
/// from the user's passphrase via Argon2.
///
/// NOTE: Mnemonic is shown to user once and NOT stored.
/// Only the derived keys are encrypted and stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedKeyStore {
    /// AES-GCM encrypted Ed25519 signing key
    /// Format: nonce (12 bytes) || ciphertext (32 bytes) || tag (16 bytes)
    pub encrypted_signing_key: Vec<u8>,
    /// AES-GCM encrypted X25519 encryption key
    /// Format: nonce (12 bytes) || ciphertext (32 bytes) || tag (16 bytes)
    pub encrypted_encryption_key: Vec<u8>,
    /// AES-GCM encrypted Ed25519 device key for Iroh P2P
    /// Format: nonce (12 bytes) || ciphertext (32 bytes) || tag (16 bytes)
    #[serde(default)]
    pub encrypted_device_key: Vec<u8>,
    /// Argon2 salt for passphrase → AES key derivation (16 bytes)
    pub salt: Vec<u8>,
    /// Argon2 parameters used
    pub argon2_params: Argon2Params,
}

impl EncryptedKeyStore {
    pub fn new(
        encrypted_signing_key: Vec<u8>,
        encrypted_encryption_key: Vec<u8>,
        encrypted_device_key: Vec<u8>,
        salt: Vec<u8>,
    ) -> Self {
        Self {
            encrypted_signing_key,
            encrypted_encryption_key,
            encrypted_device_key,
            salt,
            argon2_params: Argon2Params::default(),
        }
    }

    pub fn with_params(
        encrypted_signing_key: Vec<u8>,
        encrypted_encryption_key: Vec<u8>,
        encrypted_device_key: Vec<u8>,
        salt: Vec<u8>,
        params: Argon2Params,
    ) -> Self {
        Self {
            encrypted_signing_key,
            encrypted_encryption_key,
            encrypted_device_key,
            salt,
            argon2_params: params,
        }
    }
}
