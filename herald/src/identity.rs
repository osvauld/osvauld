use crate::error::{HeraldError, Result};
use bip39::{Language, Mnemonic};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use sha2::Sha256;
use std::sync::Arc;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519Secret};
use zeroize::Zeroize;

/// Context strings for HKDF derivation
const SIGNING_CONTEXT: &[u8] = b"herald-ed25519-signing-v1";
const ENCRYPTION_CONTEXT: &[u8] = b"herald-x25519-encryption-v1";
const DEVICE_CONTEXT: &[u8] = b"herald-ed25519-device-v1";

/// Multicodec prefix for Ed25519 public keys (0xed01 in varint encoding)
/// This is required for proper did:key encoding per the did:key spec
const ED25519_MAGIC_BYTES: &[u8] = &[0xed, 0x01];

/// Inner identity data (not directly exposed)
struct IdentityInner {
    signing_key: SigningKey,
    encryption_secret: X25519Secret,
    device_secret: [u8; 32],  // For Iroh P2P - separate from signing key
    did: String,
}

impl Drop for IdentityInner {
    fn drop(&mut self) {
        // SigningKey handles its own zeroization
        // X25519Secret handles its own zeroization
    }
}

/// Identity - holds signing and encryption keys derived from BIP39
///
/// Designed for async usage:
/// - Clone is cheap (Arc internally)
/// - All methods are &self (no mutation)
/// - Safe to wrap in RwLock if needed
/// - Safe to pass across threads
#[derive(Clone)]
pub struct Identity {
    inner: Arc<IdentityInner>,
}

impl Identity {
    /// Generate a new identity with a fresh mnemonic
    ///
    /// Returns (identity, mnemonic_phrase)
    pub fn generate() -> Result<(Self, String)> {
        let mut entropy = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut entropy);

        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|e| HeraldError::InvalidMnemonic(e.to_string()))?;

        entropy.zeroize();

        let phrase = mnemonic.to_string();
        let identity = Self::from_mnemonic(&phrase, None)?;

        Ok((identity, phrase))
    }

    /// Create identity from existing mnemonic phrase
    ///
    /// Optional password for additional entropy (BIP39 passphrase)
    pub fn from_mnemonic(phrase: &str, password: Option<&str>) -> Result<Self> {
        let mnemonic = Mnemonic::parse_in(Language::English, phrase)
            .map_err(|e| HeraldError::InvalidMnemonic(e.to_string()))?;

        // BIP39: mnemonic + password -> 512-bit seed
        let seed = mnemonic.to_seed(password.unwrap_or(""));

        Self::from_seed(&seed)
    }

    /// Create identity from raw seed bytes (64 bytes)
    pub fn from_seed(seed: &[u8]) -> Result<Self> {
        if seed.len() < 32 {
            return Err(HeraldError::KeyDerivation("Seed too short".into()));
        }

        // Derive signing key using HKDF
        let hk = Hkdf::<Sha256>::new(None, seed);
        let mut signing_bytes = [0u8; 32];
        hk.expand(SIGNING_CONTEXT, &mut signing_bytes)
            .map_err(|e| HeraldError::KeyDerivation(e.to_string()))?;

        let signing_key = SigningKey::from_bytes(&signing_bytes);
        signing_bytes.zeroize();

        // Derive encryption key using HKDF (different context)
        let mut encryption_bytes = [0u8; 32];
        hk.expand(ENCRYPTION_CONTEXT, &mut encryption_bytes)
            .map_err(|e| HeraldError::KeyDerivation(e.to_string()))?;

        let encryption_secret = X25519Secret::from(encryption_bytes);
        encryption_bytes.zeroize();

        // Derive device key using HKDF (for Iroh P2P, separate from signing)
        let mut device_bytes = [0u8; 32];
        hk.expand(DEVICE_CONTEXT, &mut device_bytes)
            .map_err(|e| HeraldError::KeyDerivation(e.to_string()))?;

        // Generate DID from public signing key with multicodec prefix
        let verifying_key = signing_key.verifying_key();
        let did_bytes = [ED25519_MAGIC_BYTES, verifying_key.as_bytes()].concat();
        let did = format!("did:key:z{}", bs58::encode(&did_bytes).into_string());

        Ok(Self {
            inner: Arc::new(IdentityInner {
                signing_key,
                encryption_secret,
                device_secret: device_bytes,
                did,
            }),
        })
    }

    /// Create identity from raw secret keys (for restoring from encrypted storage)
    ///
    /// This is used after decrypting stored keys, NOT from mnemonic.
    pub fn from_secret_keys(
        signing_secret: [u8; 32],
        encryption_secret: [u8; 32],
        device_secret: [u8; 32],
    ) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(&signing_secret);
        let encryption_secret = X25519Secret::from(encryption_secret);

        // Generate DID from public signing key with multicodec prefix
        let verifying_key = signing_key.verifying_key();
        let did_bytes = [ED25519_MAGIC_BYTES, verifying_key.as_bytes()].concat();
        let did = format!("did:key:z{}", bs58::encode(&did_bytes).into_string());

        Ok(Self {
            inner: Arc::new(IdentityInner {
                signing_key,
                encryption_secret,
                device_secret,
                did,
            }),
        })
    }

    /// Get the DID (decentralized identifier)
    pub fn did(&self) -> &str {
        &self.inner.did
    }

    /// Derive a DID from a public signing key (Ed25519, 32 bytes)
    ///
    /// This is useful when receiving a peer's public key and needing to identify them.
    pub fn did_from_public_key(public_key: &[u8; 32]) -> String {
        let did_bytes = [ED25519_MAGIC_BYTES, public_key.as_slice()].concat();
        format!("did:key:z{}", bs58::encode(&did_bytes).into_string())
    }

    /// Extract public key bytes from a DID
    ///
    /// Reverses `did_from_public_key`: parses DID, base58 decodes, removes multicodec prefix.
    /// Returns 32-byte Ed25519 public key.
    pub fn public_key_from_did(did: &str) -> Result<[u8; 32]> {
        // Remove "did:key:z" prefix
        let encoded = did
            .strip_prefix("did:key:z")
            .ok_or_else(|| HeraldError::InvalidPublicKey("Invalid DID format: must start with 'did:key:z'".into()))?;

        // Base58 decode
        let did_bytes = bs58::decode(encoded)
            .into_vec()
            .map_err(|e| HeraldError::InvalidPublicKey(format!("Invalid base58 in DID: {}", e)))?;

        // Verify and remove multicodec prefix (0xed 0x01)
        if did_bytes.len() != 34 || did_bytes[0] != 0xed || did_bytes[1] != 0x01 {
            return Err(HeraldError::InvalidPublicKey("Invalid DID: wrong length or multicodec prefix".into()));
        }

        let pubkey: [u8; 32] = did_bytes[2..]
            .try_into()
            .map_err(|_| HeraldError::InvalidPublicKey("Invalid public key length".into()))?;

        Ok(pubkey)
    }

    /// Convert base64 public key to DID format
    ///
    /// Input: "PvtlPXdb41VEPceTuviS/IrTl37daFmzyFjJicD1qDg=" (base64)
    /// Output: "did:key:z6MkiXXX..." (DID)
    ///
    /// Use this at storage boundary: receive base64 from transport, store as DID.
    pub fn did_from_base64_pubkey(base64_pubkey: &str) -> Result<String> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let pubkey_bytes = STANDARD
            .decode(base64_pubkey)
            .map_err(|e| HeraldError::InvalidPublicKey(format!("Invalid base64: {}", e)))?;

        let pubkey_32: [u8; 32] = pubkey_bytes
            .try_into()
            .map_err(|_| HeraldError::InvalidPublicKey("Public key must be 32 bytes".into()))?;

        Ok(Self::did_from_public_key(&pubkey_32))
    }

    /// Convert DID to base64 public key format
    ///
    /// Input: "did:key:z6MkiXXX..."
    /// Output: "PvtlPXdb41VEPceTuviS/..." (base64)
    ///
    /// Use this when transport layer needs base64 format.
    pub fn base64_pubkey_from_did(did: &str) -> Result<String> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let pubkey_bytes = Self::public_key_from_did(did)?;
        Ok(STANDARD.encode(pubkey_bytes))
    }

    /// Get the public signing key (Ed25519, 32 bytes)
    pub fn public_signing_key(&self) -> [u8; 32] {
        self.inner.signing_key.verifying_key().to_bytes()
    }

    /// Get the public encryption key (X25519, 32 bytes)
    pub fn public_encryption_key(&self) -> [u8; 32] {
        X25519PublicKey::from(&self.inner.encryption_secret).to_bytes()
    }

    /// Get the secret signing key (Ed25519, 32 bytes)
    ///
    /// WARNING: Handle with care - this is sensitive key material.
    /// Used for encrypting keys for storage.
    pub fn secret_signing_key(&self) -> [u8; 32] {
        self.inner.signing_key.to_bytes()
    }

    /// Get the secret encryption key (X25519, 32 bytes)
    ///
    /// WARNING: Handle with care - this is sensitive key material.
    /// Used for encrypting keys for storage.
    pub fn secret_encryption_key(&self) -> [u8; 32] {
        self.inner.encryption_secret.to_bytes()
    }

    /// Get the public device key (Ed25519, 32 bytes)
    ///
    /// This is the device-specific key for Iroh P2P networking.
    /// Separate from the signing key to allow device-specific identity.
    pub fn public_device_key(&self) -> [u8; 32] {
        let device_signing_key = SigningKey::from_bytes(&self.inner.device_secret);
        device_signing_key.verifying_key().to_bytes()
    }

    /// Get the secret device key (Ed25519, 32 bytes)
    ///
    /// WARNING: Handle with care - this is sensitive key material.
    /// Used for Iroh P2P endpoint binding.
    pub fn secret_device_key(&self) -> [u8; 32] {
        self.inner.device_secret
    }

    /// Sign data with Ed25519
    pub fn sign(&self, data: &[u8]) -> [u8; 64] {
        self.inner.signing_key.sign(data).to_bytes()
    }

    /// Verify a signature against our public key
    pub fn verify(&self, data: &[u8], signature: &[u8; 64]) -> bool {
        let sig = match Signature::from_bytes(signature) {
            sig => sig,
        };
        self.inner
            .signing_key
            .verifying_key()
            .verify(data, &sig)
            .is_ok()
    }

    /// Verify a signature against any public key (static method)
    pub fn verify_with_key(public_key: &[u8; 32], data: &[u8], signature: &[u8; 64]) -> bool {
        let Ok(verifying_key) = VerifyingKey::from_bytes(public_key) else {
            return false;
        };
        let sig = Signature::from_bytes(signature);
        verifying_key.verify(data, &sig).is_ok()
    }

    /// Perform X25519 ECDH key exchange
    ///
    /// Returns shared secret (32 bytes)
    pub fn ecdh(&self, their_public: &[u8; 32]) -> [u8; 32] {
        let their_public = X25519PublicKey::from(*their_public);
        self.inner.encryption_secret.diffie_hellman(&their_public).to_bytes()
    }

    /// Derive a symmetric key from shared secret using HKDF
    ///
    /// Use this after ECDH to get an encryption key
    pub fn derive_key(shared_secret: &[u8], context: &[u8]) -> [u8; 32] {
        let hk = Hkdf::<Sha256>::new(None, shared_secret);
        let mut output = [0u8; 32];
        hk.expand(context, &mut output)
            .expect("32 bytes is valid for HKDF-SHA256");
        output
    }

    // =========================================================================
    // Convenience methods for encryption (uses crypto module)
    // =========================================================================

    /// Encrypt data for a recipient using their public encryption key
    ///
    /// Uses X25519 ECIES: generates ephemeral keypair, ECDH, AES-GCM encrypt.
    /// The recipient can decrypt with their secret encryption key.
    ///
    /// # Example
    /// ```rust
    /// use herald::Identity;
    ///
    /// let (alice, _) = Identity::generate().unwrap();
    /// let (bob, _) = Identity::generate().unwrap();
    ///
    /// let sealed = alice.encrypt_for(&bob.public_encryption_key(), b"hello").unwrap();
    /// let opened = bob.decrypt_sealed(&sealed).unwrap();
    /// assert_eq!(opened, b"hello");
    /// ```
    pub fn encrypt_for(&self, recipient_public: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
        crate::crypto::encrypt(recipient_public, plaintext)
    }

    /// Decrypt data that was encrypted for us
    ///
    /// Uses our secret encryption key to decrypt ECIES-sealed data.
    pub fn decrypt_sealed(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        crate::crypto::decrypt(&self.secret_encryption_key(), ciphertext)
    }

    /// Encrypt data for ourselves (self-encryption)
    ///
    /// Useful for encrypting data that only we can decrypt later.
    pub fn encrypt_for_self(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        crate::crypto::encrypt(&self.public_encryption_key(), plaintext)
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("did", &self.inner.did)
            .field("public_signing_key", &hex::encode(self.public_signing_key()))
            .field("public_encryption_key", &hex::encode(self.public_encryption_key()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_identity() {
        let (identity, mnemonic) = Identity::generate().unwrap();

        // Mnemonic should be 24 words
        assert_eq!(mnemonic.split_whitespace().count(), 24);

        // DID should start with did:key:z
        assert!(identity.did().starts_with("did:key:z"));

        // Keys should be 32 bytes
        assert_eq!(identity.public_signing_key().len(), 32);
        assert_eq!(identity.public_encryption_key().len(), 32);
    }

    #[test]
    fn test_deterministic_derivation() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let id1 = Identity::from_mnemonic(phrase, None).unwrap();
        let id2 = Identity::from_mnemonic(phrase, None).unwrap();

        assert_eq!(id1.public_signing_key(), id2.public_signing_key());
        assert_eq!(id1.public_encryption_key(), id2.public_encryption_key());
        assert_eq!(id1.did(), id2.did());
    }

    #[test]
    fn test_password_changes_keys() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let id1 = Identity::from_mnemonic(phrase, None).unwrap();
        let id2 = Identity::from_mnemonic(phrase, Some("secret")).unwrap();

        assert_ne!(id1.public_signing_key(), id2.public_signing_key());
    }

    #[test]
    fn test_sign_verify() {
        let (identity, _) = Identity::generate().unwrap();
        let data = b"hello world";

        let signature = identity.sign(data);
        assert!(identity.verify(data, &signature));

        // Wrong data should fail
        assert!(!identity.verify(b"wrong data", &signature));
    }

    #[test]
    fn test_verify_with_key() {
        let (identity, _) = Identity::generate().unwrap();
        let data = b"hello world";
        let signature = identity.sign(data);

        let public_key = identity.public_signing_key();
        assert!(Identity::verify_with_key(&public_key, data, &signature));
    }

    #[test]
    fn test_ecdh() {
        let (alice, _) = Identity::generate().unwrap();
        let (bob, _) = Identity::generate().unwrap();

        let alice_shared = alice.ecdh(&bob.public_encryption_key());
        let bob_shared = bob.ecdh(&alice.public_encryption_key());

        // Both should derive the same shared secret
        assert_eq!(alice_shared, bob_shared);
    }

    #[test]
    fn test_derive_key() {
        let shared_secret = [42u8; 32];

        let key1 = Identity::derive_key(&shared_secret, b"context1");
        let key2 = Identity::derive_key(&shared_secret, b"context2");

        // Different contexts should give different keys
        assert_ne!(key1, key2);

        // Same context should give same key
        let key1_again = Identity::derive_key(&shared_secret, b"context1");
        assert_eq!(key1, key1_again);
    }

    #[test]
    fn test_clone_is_cheap() {
        let (identity, _) = Identity::generate().unwrap();
        let cloned = identity.clone();

        // Should be same identity
        assert_eq!(identity.did(), cloned.did());
        assert_eq!(identity.public_signing_key(), cloned.public_signing_key());
    }

    #[test]
    fn test_encrypt_for_and_decrypt() {
        let (alice, _) = Identity::generate().unwrap();
        let (bob, _) = Identity::generate().unwrap();

        let plaintext = b"hello bob from alice";
        let sealed = alice.encrypt_for(&bob.public_encryption_key(), plaintext).unwrap();
        let opened = bob.decrypt_sealed(&sealed).unwrap();

        assert_eq!(opened, plaintext);
    }

    #[test]
    fn test_encrypt_for_self() {
        let (identity, _) = Identity::generate().unwrap();

        let plaintext = b"secret data for myself";
        let sealed = identity.encrypt_for_self(plaintext).unwrap();
        let opened = identity.decrypt_sealed(&sealed).unwrap();

        assert_eq!(opened, plaintext);
    }

    #[test]
    fn test_wrong_recipient_cannot_decrypt() {
        let (alice, _) = Identity::generate().unwrap();
        let (bob, _) = Identity::generate().unwrap();
        let (eve, _) = Identity::generate().unwrap();

        let plaintext = b"secret message for bob";
        let sealed = alice.encrypt_for(&bob.public_encryption_key(), plaintext).unwrap();

        // Eve should not be able to decrypt
        let result = eve.decrypt_sealed(&sealed);
        assert!(result.is_err());
    }
}
