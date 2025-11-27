use crate::error::{AlchemistError, Result};
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

// =============================================================================
// Symmetric Encryption (AES-256-GCM)
// =============================================================================

/// Encrypt data using AES-256-GCM
///
/// Returns: nonce (12 bytes) || ciphertext || tag (16 bytes)
///
/// The nonce is randomly generated and prepended to the output.
pub fn encrypt(key: &[u8; KEY_SIZE], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AlchemistError::EncryptionFailed(e.to_string()))?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| AlchemistError::EncryptionFailed(e.to_string()))?;

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
        .map_err(|e| AlchemistError::EncryptionFailed(e.to_string()))?;

    cipher
        .encrypt(Nonce::from_slice(nonce), plaintext)
        .map_err(|e| AlchemistError::EncryptionFailed(e.to_string()))
}

/// Decrypt data encrypted with `encrypt()`
///
/// Expects: nonce (12 bytes) || ciphertext || tag (16 bytes)
pub fn decrypt(key: &[u8; KEY_SIZE], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < NONCE_SIZE + TAG_SIZE {
        return Err(AlchemistError::CiphertextTooShort);
    }

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AlchemistError::DecryptionFailed(e.to_string()))?;

    let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);
    let ciphertext_with_tag = &ciphertext[NONCE_SIZE..];

    cipher
        .decrypt(nonce, ciphertext_with_tag)
        .map_err(|e| AlchemistError::DecryptionFailed(e.to_string()))
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
        return Err(AlchemistError::CiphertextTooShort);
    }

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AlchemistError::DecryptionFailed(e.to_string()))?;

    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|e| AlchemistError::DecryptionFailed(e.to_string()))
}

// =============================================================================
// Key Exchange (X25519 ECDH)
// =============================================================================

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

// =============================================================================
// Key Derivation (HKDF-SHA256)
// =============================================================================

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

// =============================================================================
// High-level: Encrypt for recipient (ECIES-style)
// =============================================================================

/// Encrypt data for a recipient using their public key (ECIES-style)
///
/// Generates ephemeral keypair, performs ECDH, derives key, encrypts.
///
/// Returns: ephemeral_public (32) || nonce (12) || ciphertext || tag (16)
pub fn seal(recipient_public: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    // Generate ephemeral keypair
    let (ephemeral_secret, ephemeral_public) = generate_ephemeral_keypair();

    // ECDH to get shared secret
    let shared_secret = ecdh(&ephemeral_secret, recipient_public);

    // Derive encryption key
    let mut key = derive_key(&shared_secret, None, b"alchemist-seal-v1");

    // Encrypt
    let ciphertext = encrypt(&key, plaintext)?;

    // Zeroize sensitive material
    key.zeroize();

    // Prepend ephemeral public key
    let mut result = Vec::with_capacity(32 + ciphertext.len());
    result.extend_from_slice(&ephemeral_public);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypt data sealed with `seal()`
///
/// Expects: ephemeral_public (32) || nonce (12) || ciphertext || tag (16)
pub fn open(our_secret: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < 32 + NONCE_SIZE + TAG_SIZE {
        return Err(AlchemistError::CiphertextTooShort);
    }

    // Extract ephemeral public key
    let ephemeral_public: [u8; 32] = ciphertext[..32]
        .try_into()
        .expect("slice is 32 bytes");

    // ECDH to recover shared secret
    let shared_secret = ecdh(our_secret, &ephemeral_public);

    // Derive encryption key
    let mut key = derive_key(&shared_secret, None, b"alchemist-seal-v1");

    // Decrypt
    let plaintext = decrypt(&key, &ciphertext[32..])?;

    // Zeroize
    key.zeroize();

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = [42u8; 32];
        let plaintext = b"hello world";

        let ciphertext = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_empty() {
        let key = [42u8; 32];
        let plaintext = b"";

        let ciphertext = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_large() {
        let key = [42u8; 32];
        let plaintext = vec![0xABu8; 1_000_000]; // 1MB

        let ciphertext = encrypt(&key, &plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = [42u8; 32];
        let key2 = [43u8; 32];
        let plaintext = b"hello world";

        let ciphertext = encrypt(&key1, plaintext).unwrap();
        let result = decrypt(&key2, &ciphertext);

        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = [42u8; 32];
        let plaintext = b"hello world";

        let mut ciphertext = encrypt(&key, plaintext).unwrap();
        // Tamper with the ciphertext
        ciphertext[NONCE_SIZE + 5] ^= 0xFF;

        let result = decrypt(&key, &ciphertext);
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
    fn test_seal_open() {
        let (secret, public) = generate_ephemeral_keypair();
        let plaintext = b"secret message";

        let sealed = seal(&public, plaintext).unwrap();
        let opened = open(&secret, &sealed).unwrap();

        assert_eq!(opened, plaintext);
    }

    #[test]
    fn test_seal_different_each_time() {
        let (_, public) = generate_ephemeral_keypair();
        let plaintext = b"secret message";

        let sealed1 = seal(&public, plaintext).unwrap();
        let sealed2 = seal(&public, plaintext).unwrap();

        // Different ephemeral keys = different ciphertexts
        assert_ne!(sealed1, sealed2);
    }

    #[test]
    fn test_ciphertext_length() {
        let key = [42u8; 32];
        let plaintext = b"hello";

        let ciphertext = encrypt(&key, plaintext).unwrap();

        // nonce (12) + plaintext (5) + tag (16) = 33
        assert_eq!(ciphertext.len(), NONCE_SIZE + plaintext.len() + TAG_SIZE);
    }
}
