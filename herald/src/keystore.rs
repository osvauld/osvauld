//! KeyStore - Passphrase-based encryption for identity keys
//!
//! Handles:
//! - Encrypting derived keys with passphrase (Argon2 → AES-GCM)
//! - Decrypting keys and returning Identity
//! - Recovery from mnemonic

use crate::error::{HeraldError, Result};
use crate::identity::Identity;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroize;

/// AES-256-GCM nonce size
const NONCE_SIZE: usize = 12;
/// Argon2 salt size
pub const SALT_SIZE: usize = 16;

/// Default Argon2 parameters (OWASP 2024 recommendations)
pub const DEFAULT_M_COST: u32 = 65536;  // 64 MB
pub const DEFAULT_T_COST: u32 = 3;       // 3 iterations
pub const DEFAULT_P_COST: u32 = 4;       // 4 parallel lanes

/// Encrypted keys ready for storage
///
/// Contains encrypted secret keys + public keys + Argon2 params.
/// The mnemonic is NOT stored - only shown to user once.
#[derive(Debug, Clone)]
pub struct EncryptedKeys {
    /// AES-GCM encrypted signing key: nonce (12) || ciphertext (32) || tag (16)
    pub encrypted_signing_key: Vec<u8>,
    /// AES-GCM encrypted encryption key: nonce (12) || ciphertext (32) || tag (16)
    pub encrypted_encryption_key: Vec<u8>,
    /// AES-GCM encrypted device key: nonce (12) || ciphertext (32) || tag (16)
    pub encrypted_device_key: Vec<u8>,
    /// Ed25519 public signing key (32 bytes)
    pub public_signing_key: [u8; 32],
    /// X25519 public encryption key (32 bytes)
    pub public_encryption_key: [u8; 32],
    /// Ed25519 public device key for Iroh P2P (32 bytes)
    pub public_device_key: [u8; 32],
    /// DID derived from public signing key
    pub did: String,
    /// Argon2 salt (16 bytes)
    pub salt: Vec<u8>,
    /// Argon2 memory cost in KiB
    pub m_cost: u32,
    /// Argon2 time cost (iterations)
    pub t_cost: u32,
    /// Argon2 parallelism
    pub p_cost: u32,
}

impl EncryptedKeys {
    /// Create with default Argon2 parameters
    fn new(
        encrypted_signing_key: Vec<u8>,
        encrypted_encryption_key: Vec<u8>,
        encrypted_device_key: Vec<u8>,
        public_signing_key: [u8; 32],
        public_encryption_key: [u8; 32],
        public_device_key: [u8; 32],
        did: String,
        salt: Vec<u8>,
    ) -> Self {
        Self {
            encrypted_signing_key,
            encrypted_encryption_key,
            encrypted_device_key,
            public_signing_key,
            public_encryption_key,
            public_device_key,
            did,
            salt,
            m_cost: DEFAULT_M_COST,
            t_cost: DEFAULT_T_COST,
            p_cost: DEFAULT_P_COST,
        }
    }
}

/// Generate a new identity, encrypt keys, return mnemonic for user backup
///
/// Returns: (EncryptedKeys, mnemonic_phrase)
///
/// - EncryptedKeys: store this in database
/// - mnemonic_phrase: show ONCE to user for backup, then discard
pub fn generate_and_encrypt(passphrase: &str) -> Result<(EncryptedKeys, String)> {
    // Generate new identity with BIP39 mnemonic
    let (identity, mnemonic) = Identity::generate()?;

    // Encrypt the keys
    let encrypted = encrypt_identity(&identity, passphrase)?;

    Ok((encrypted, mnemonic))
}

/// Encrypt an existing identity's keys with passphrase
///
/// Used for:
/// - Initial signup (after generating identity)
/// - Recovery from mnemonic (re-encrypting with new passphrase)
pub fn encrypt_identity(identity: &Identity, passphrase: &str) -> Result<EncryptedKeys> {
    // Generate salt
    let mut salt = vec![0u8; SALT_SIZE];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut salt);

    // Derive AES key using Argon2
    let mut aes_key = derive_key_argon2(
        passphrase,
        &salt,
        DEFAULT_M_COST,
        DEFAULT_T_COST,
        DEFAULT_P_COST,
    )?;

    // Get secret keys from identity
    let signing_secret = identity.secret_signing_key();
    let encryption_secret = identity.secret_encryption_key();
    let device_secret = identity.secret_device_key();

    // Encrypt all keys
    let encrypted_signing = encrypt_aes_gcm(&aes_key, &signing_secret)?;
    let encrypted_encryption = encrypt_aes_gcm(&aes_key, &encryption_secret)?;
    let encrypted_device = encrypt_aes_gcm(&aes_key, &device_secret)?;

    // Zeroize AES key
    aes_key.zeroize();

    Ok(EncryptedKeys::new(
        encrypted_signing,
        encrypted_encryption,
        encrypted_device,
        identity.public_signing_key(),
        identity.public_encryption_key(),
        identity.public_device_key(),
        identity.did().to_string(),
        salt,
    ))
}

/// Decrypt keys and restore Identity
///
/// Used for login - takes stored EncryptedKeys and passphrase,
/// returns the Identity ready for use.
pub fn decrypt_and_restore(encrypted: &EncryptedKeys, passphrase: &str) -> Result<Identity> {
    // Derive AES key from passphrase using stored Argon2 params
    let mut aes_key = derive_key_argon2(
        passphrase,
        &encrypted.salt,
        encrypted.m_cost,
        encrypted.t_cost,
        encrypted.p_cost,
    )?;

    // Decrypt all keys
    let signing_secret = decrypt_aes_gcm(&aes_key, &encrypted.encrypted_signing_key)?;
    let encryption_secret = decrypt_aes_gcm(&aes_key, &encrypted.encrypted_encryption_key)?;
    let device_secret = decrypt_aes_gcm(&aes_key, &encrypted.encrypted_device_key)?;

    // Zeroize AES key
    aes_key.zeroize();

    // Restore identity from secret keys
    let signing_key: [u8; 32] = signing_secret
        .try_into()
        .map_err(|_| HeraldError::InvalidSecretKey)?;
    let encryption_key: [u8; 32] = encryption_secret
        .try_into()
        .map_err(|_| HeraldError::InvalidSecretKey)?;
    let device_key: [u8; 32] = device_secret
        .try_into()
        .map_err(|_| HeraldError::InvalidSecretKey)?;

    Identity::from_secret_keys(signing_key, encryption_key, device_key)
}

/// Recover identity from mnemonic and encrypt with new passphrase
///
/// Used when user sets up on a new device using their backup mnemonic.
///
/// Returns: EncryptedKeys to store
pub fn recover_from_mnemonic(
    mnemonic: &str,
    bip39_password: Option<&str>,
    new_passphrase: &str,
) -> Result<EncryptedKeys> {
    // Restore identity from mnemonic
    let identity = Identity::from_mnemonic(mnemonic, bip39_password)?;

    // Encrypt with new passphrase
    encrypt_identity(&identity, new_passphrase)
}

/// Change passphrase for encrypted keys
///
/// Decrypts with old passphrase, re-encrypts with new passphrase.
pub fn change_passphrase(
    encrypted: &EncryptedKeys,
    old_passphrase: &str,
    new_passphrase: &str,
) -> Result<EncryptedKeys> {
    // Decrypt with old passphrase
    let identity = decrypt_and_restore(encrypted, old_passphrase)?;

    // Re-encrypt with new passphrase (generates new salt)
    encrypt_identity(&identity, new_passphrase)
}

// Internal helpers

fn derive_key_argon2(
    passphrase: &str,
    salt: &[u8],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> Result<[u8; 32]> {
    let params = Params::new(m_cost, t_cost, p_cost, Some(32))
        .map_err(|e| HeraldError::KeyDerivation(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut output = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut output)
        .map_err(|e| HeraldError::KeyDerivation(e.to_string()))?;

    // Argon2 allocates m_cost KiB (default 64 MB) internally for hashing.
    // glibc doesn't return those pages to the OS after freeing.
    // Force the allocator to release them now.
    #[cfg(target_os = "linux")]
    unsafe {
        libc::malloc_trim(0);
    }

    Ok(output)
}

fn encrypt_aes_gcm(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| HeraldError::KeyDerivation(e.to_string()))?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| HeraldError::KeyDerivation(format!("Encryption failed: {}", e)))?;

    // Prepend nonce
    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

fn decrypt_aes_gcm(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < NONCE_SIZE + 16 {
        return Err(HeraldError::KeyDerivation("Ciphertext too short".into()));
    }

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| HeraldError::KeyDerivation(e.to_string()))?;

    let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);
    let ciphertext_with_tag = &ciphertext[NONCE_SIZE..];

    cipher
        .decrypt(nonce, ciphertext_with_tag)
        .map_err(|_| HeraldError::InvalidSignature) // Wrong passphrase
}

