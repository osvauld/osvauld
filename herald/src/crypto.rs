//! Cryptographic operations for Herald
//!
//! Provides:
//! - Symmetric encryption: AES-256-GCM
//! - Key exchange: X25519 ECDH
//! - Key derivation: HKDF-SHA256
//! - High-level: ECIES-style seal/open (encrypt for recipient's public key)

use crate::error::{HeraldError, Result};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519Secret};
use zeroize::Zeroize;

/// AES-256-GCM nonce size (96 bits / 12 bytes)
pub const NONCE_SIZE: usize = 12;

/// AES-256 key size (256 bits / 32 bytes)
pub const KEY_SIZE: usize = 32;

/// AES-GCM authentication tag size (128 bits / 16 bytes)
pub const TAG_SIZE: usize = 16;

/// Size of ephemeral public key in sealed messages
pub const EPHEMERAL_KEY_SIZE: usize = 32;

// Key Generation

/// Generate a random AES-256 key
///
/// Returns a cryptographically secure random 32-byte key suitable for
/// symmetric encryption with `encrypt_symmetric`/`decrypt_symmetric`.
pub fn generate_aes_key() -> [u8; KEY_SIZE] {
    let mut key = [0u8; KEY_SIZE];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut key);
    key
}

// Symmetric Encryption (AES-256-GCM)

/// Encrypt data using AES-256-GCM
///
/// Returns: nonce (12 bytes) || ciphertext || tag (16 bytes)
///
/// The nonce is randomly generated and prepended to the output.
pub fn encrypt_symmetric(key: &[u8; KEY_SIZE], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| HeraldError::EncryptionFailed(e.to_string()))?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| HeraldError::EncryptionFailed(e.to_string()))?;

    // Prepend nonce to ciphertext
    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Encrypt data with a specific nonce (use with caution - nonce must be unique!)
///
/// Returns: ciphertext || tag (no nonce prepended)
pub fn encrypt_with_nonce(
    key: &[u8; KEY_SIZE],
    nonce: &[u8; NONCE_SIZE],
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| HeraldError::EncryptionFailed(e.to_string()))?;

    cipher
        .encrypt(Nonce::from_slice(nonce), plaintext)
        .map_err(|e| HeraldError::EncryptionFailed(e.to_string()))
}

/// Decrypt data encrypted with `encrypt_symmetric()`
///
/// Expects: nonce (12 bytes) || ciphertext || tag (16 bytes)
pub fn decrypt_symmetric(key: &[u8; KEY_SIZE], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < NONCE_SIZE + TAG_SIZE {
        return Err(HeraldError::CiphertextTooShort);
    }

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| HeraldError::DecryptionFailed(e.to_string()))?;

    let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);
    let ciphertext_with_tag = &ciphertext[NONCE_SIZE..];

    cipher
        .decrypt(nonce, ciphertext_with_tag)
        .map_err(|e| HeraldError::DecryptionFailed(e.to_string()))
}

/// Decrypt data with explicit nonce
///
/// Expects: ciphertext || tag (no nonce)
pub fn decrypt_with_nonce(
    key: &[u8; KEY_SIZE],
    nonce: &[u8; NONCE_SIZE],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    if ciphertext.len() < TAG_SIZE {
        return Err(HeraldError::CiphertextTooShort);
    }

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| HeraldError::DecryptionFailed(e.to_string()))?;

    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|e| HeraldError::DecryptionFailed(e.to_string()))
}

// Key Exchange (X25519 ECDH)

/// Perform X25519 ECDH key exchange
///
/// Returns: 32-byte shared secret
pub fn ecdh(our_secret: &[u8; 32], their_public: &[u8; 32]) -> [u8; 32] {
    let secret = X25519Secret::from(*our_secret);
    let public = X25519PublicKey::from(*their_public);
    secret.diffie_hellman(&public).to_bytes()
}

/// Generate an ephemeral X25519 keypair
///
/// Returns: (secret_key, public_key)
pub fn generate_ephemeral_keypair() -> ([u8; 32], [u8; 32]) {
    let secret = X25519Secret::random_from_rng(rand::rngs::OsRng);
    let public = X25519PublicKey::from(&secret);

    // Convert secret to bytes (this consumes it, which is fine for ephemeral)
    let secret_bytes: [u8; 32] = secret.to_bytes();
    (secret_bytes, public.to_bytes())
}

// Key Derivation (HKDF-SHA256)

/// Derive a key using HKDF-SHA256
///
/// - `ikm`: Input key material (e.g., ECDH shared secret)
/// - `salt`: Optional salt (can be empty)
/// - `info`: Context string for domain separation
pub fn derive_key(ikm: &[u8], salt: Option<&[u8]>, info: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut output = [0u8; 32];
    hk.expand(info, &mut output)
        .expect("32 bytes is valid for HKDF-SHA256");
    output
}

/// Derive multiple keys using HKDF-SHA256
///
/// Useful for deriving encryption key + MAC key in one go
pub fn derive_keys<const N: usize>(
    ikm: &[u8],
    salt: Option<&[u8]>,
    info: &[u8],
) -> [u8; N] {
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut output = [0u8; N];
    hk.expand(info, &mut output)
        .expect("Output length should be valid for HKDF-SHA256");
    output
}

// High-level: Encrypt for recipient (ECIES-style)

/// Context for ECIES key derivation
const SEAL_CONTEXT: &[u8] = b"herald-seal-v1";

/// Encrypt data for a recipient using their public key (ECIES-style)
///
/// Generates ephemeral keypair, performs ECDH, derives key, encrypts.
///
/// Returns: ephemeral_public (32) || nonce (12) || ciphertext || tag (16)
///
/// # Example
/// ```rust
/// use herald::crypto::{encrypt, decrypt, generate_ephemeral_keypair};
///
/// let (secret, public) = generate_ephemeral_keypair();
/// let sealed = encrypt(&public, b"secret message").unwrap();
/// let opened = decrypt(&secret, &sealed).unwrap();
/// assert_eq!(opened, b"secret message");
/// ```
pub fn encrypt(recipient_public: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    // Generate ephemeral keypair
    let (ephemeral_secret, ephemeral_public) = generate_ephemeral_keypair();

    // ECDH to get shared secret
    let shared_secret = ecdh(&ephemeral_secret, recipient_public);

    // Derive encryption key
    let mut key = derive_key(&shared_secret, None, SEAL_CONTEXT);

    // Encrypt
    let ciphertext = encrypt_symmetric(&key, plaintext)?;

    // Zeroize sensitive material
    key.zeroize();

    // Prepend ephemeral public key
    let mut result = Vec::with_capacity(EPHEMERAL_KEY_SIZE + ciphertext.len());
    result.extend_from_slice(&ephemeral_public);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypt data encrypted with `encrypt()`
///
/// Expects: ephemeral_public (32) || nonce (12) || ciphertext || tag (16)
pub fn decrypt(recipient_secret: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < EPHEMERAL_KEY_SIZE + NONCE_SIZE + TAG_SIZE {
        return Err(HeraldError::CiphertextTooShort);
    }

    // Extract ephemeral public key
    let ephemeral_public: [u8; 32] = ciphertext[..EPHEMERAL_KEY_SIZE]
        .try_into()
        .expect("slice is 32 bytes");

    // ECDH to recover shared secret
    let shared_secret = ecdh(recipient_secret, &ephemeral_public);

    // Derive encryption key
    let mut key = derive_key(&shared_secret, None, SEAL_CONTEXT);

    // Decrypt
    let plaintext = decrypt_symmetric(&key, &ciphertext[EPHEMERAL_KEY_SIZE..])?;

    // Zeroize
    key.zeroize();

    Ok(plaintext)
}

// Transit Encryption (Ephemeral ECDH for peer-to-peer transfer)

/// Context for transit encryption key derivation
const TRANSIT_CONTEXT: &[u8] = b"herald-transit-v1";

/// Encrypt data for peer-to-peer transfer using ephemeral ECDH
///
/// Generates an ephemeral keypair, performs ECDH with recipient's public key,
/// derives a transit key, and encrypts the data.
///
/// Returns: (ephemeral_public, encrypted_data)
/// where encrypted_data = nonce (12) || ciphertext || tag (16)
///
/// **Use case**: Transferring page layers between Owner and Node with forward secrecy.
/// The ephemeral_public must be sent alongside the encrypted data.
pub fn encrypt_for_transfer(
    recipient_public: &[u8; 32],
    plaintext: &[u8],
) -> Result<([u8; 32], Vec<u8>)> {
    // Generate ephemeral keypair for this transfer
    let (ephemeral_secret, ephemeral_public) = generate_ephemeral_keypair();

    // ECDH to get shared secret
    let shared_secret = ecdh(&ephemeral_secret, recipient_public);

    // Derive transit encryption key
    let mut transit_key = derive_key(&shared_secret, None, TRANSIT_CONTEXT);

    // Encrypt with transit key
    let encrypted = encrypt_symmetric(&transit_key, plaintext)?;

    // Zeroize sensitive material
    transit_key.zeroize();

    Ok((ephemeral_public, encrypted))
}

/// Decrypt data received via ephemeral ECDH transfer
///
/// Takes the ephemeral public key from the sender and our secret key,
/// reconstructs the shared secret, derives the transit key, and decrypts.
///
/// Expects: ciphertext = nonce (12) || ciphertext || tag (16)
///
/// **Use case**: Node receiving page layers from Owner.
pub fn decrypt_from_transfer(
    our_secret: &[u8; 32],
    ephemeral_public: &[u8; 32],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    // ECDH to recover shared secret
    let shared_secret = ecdh(our_secret, ephemeral_public);

    // Derive transit encryption key
    let mut transit_key = derive_key(&shared_secret, None, TRANSIT_CONTEXT);

    // Decrypt
    let plaintext = decrypt_symmetric(&transit_key, ciphertext)?;

    // Zeroize
    transit_key.zeroize();

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_symmetric() {
        let key = [42u8; 32];
        let plaintext = b"hello world";

        let ciphertext = encrypt_symmetric(&key, plaintext).unwrap();
        let decrypted = decrypt_symmetric(&key, &ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_empty() {
        let key = [42u8; 32];
        let plaintext = b"";

        let ciphertext = encrypt_symmetric(&key, plaintext).unwrap();
        let decrypted = decrypt_symmetric(&key, &ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_large() {
        let key = [42u8; 32];
        let plaintext = vec![0xABu8; 1_000_000]; // 1MB

        let ciphertext = encrypt_symmetric(&key, &plaintext).unwrap();
        let decrypted = decrypt_symmetric(&key, &ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = [42u8; 32];
        let key2 = [43u8; 32];
        let plaintext = b"hello world";

        let ciphertext = encrypt_symmetric(&key1, plaintext).unwrap();
        let result = decrypt_symmetric(&key2, &ciphertext);

        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = [42u8; 32];
        let plaintext = b"hello world";

        let mut ciphertext = encrypt_symmetric(&key, plaintext).unwrap();
        // Tamper with the ciphertext
        ciphertext[NONCE_SIZE + 5] ^= 0xFF;

        let result = decrypt_symmetric(&key, &ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_ecdh_symmetric() {
        let (secret_a, public_a) = generate_ephemeral_keypair();
        let (secret_b, public_b) = generate_ephemeral_keypair();

        let shared_ab = ecdh(&secret_a, &public_b);
        let shared_ba = ecdh(&secret_b, &public_a);

        assert_eq!(shared_ab, shared_ba);
    }

    #[test]
    fn test_derive_key_deterministic() {
        let ikm = [1u8; 32];

        let key1 = derive_key(&ikm, None, b"context");
        let key2 = derive_key(&ikm, None, b"context");

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_derive_key_different_contexts() {
        let ikm = [1u8; 32];

        let key1 = derive_key(&ikm, None, b"context1");
        let key2 = derive_key(&ikm, None, b"context2");

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let (secret, public) = generate_ephemeral_keypair();
        let plaintext = b"secret message";

        let sealed = encrypt(&public, plaintext).unwrap();
        let opened = decrypt(&secret, &sealed).unwrap();

        assert_eq!(opened, plaintext);
    }

    #[test]
    fn test_encrypt_different_each_time() {
        let (_, public) = generate_ephemeral_keypair();
        let plaintext = b"secret message";

        let sealed1 = encrypt(&public, plaintext).unwrap();
        let sealed2 = encrypt(&public, plaintext).unwrap();

        // Different ephemeral keys = different ciphertexts
        assert_ne!(sealed1, sealed2);
    }

    #[test]
    fn test_ciphertext_length() {
        let key = [42u8; 32];
        let plaintext = b"hello";

        let ciphertext = encrypt_symmetric(&key, plaintext).unwrap();

        // nonce (12) + plaintext (5) + tag (16) = 33
        assert_eq!(ciphertext.len(), NONCE_SIZE + plaintext.len() + TAG_SIZE);
    }

    #[test]
    fn test_sealed_message_length() {
        let (_, public) = generate_ephemeral_keypair();
        let plaintext = b"hello";

        let sealed = encrypt(&public, plaintext).unwrap();

        // ephemeral_public (32) + nonce (12) + plaintext (5) + tag (16) = 65
        assert_eq!(sealed.len(), EPHEMERAL_KEY_SIZE + NONCE_SIZE + plaintext.len() + TAG_SIZE);
    }

    #[test]
    fn test_encrypt_decrypt_for_transfer() {
        // Simulate Owner → Node transfer
        let (node_secret, node_public) = generate_ephemeral_keypair();
        let plaintext = b"layer data to transfer";

        // Owner encrypts for Node
        let (ephemeral_public, encrypted) = encrypt_for_transfer(&node_public, plaintext).unwrap();

        // Node decrypts using ephemeral public and its secret
        let decrypted = decrypt_from_transfer(&node_secret, &ephemeral_public, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_transfer_different_each_time() {
        let (_, node_public) = generate_ephemeral_keypair();
        let plaintext = b"layer data";

        // Each encryption generates new ephemeral key
        let (eph1, enc1) = encrypt_for_transfer(&node_public, plaintext).unwrap();
        let (eph2, enc2) = encrypt_for_transfer(&node_public, plaintext).unwrap();

        // Different ephemeral keys and ciphertexts (forward secrecy)
        assert_ne!(eph1, eph2);
        assert_ne!(enc1, enc2);
    }

    #[test]
    fn test_transfer_wrong_secret_fails() {
        let (_, node_public) = generate_ephemeral_keypair();
        let (wrong_secret, _) = generate_ephemeral_keypair();
        let plaintext = b"secret layer";

        let (ephemeral_public, encrypted) = encrypt_for_transfer(&node_public, plaintext).unwrap();

        // Using wrong secret key should fail
        let result = decrypt_from_transfer(&wrong_secret, &ephemeral_public, &encrypted);
        assert!(result.is_err());
    }
}
