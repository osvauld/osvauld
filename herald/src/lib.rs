//! Herald: Identity and cryptography for Osvauld
//!
//! Provides:
//! - BIP39 mnemonic generation and recovery
//! - Ed25519 signing keys (identity, UCANs)
//! - X25519 encryption keys (derived from same seed)
//! - Passphrase-based key encryption (Argon2 + AES-GCM)
//! - ECIES encryption (encrypt for recipient's public key)
//! - ECDH key exchange
//! - HKDF key derivation
//!
//! Designed for async usage - Identity is Clone (Arc internally),
//! all methods are &self, safe to pass across threads.
//!
//! # Signup Example
//!
//! ```rust
//! use herald::keystore;
//!
//! // Generate identity and encrypt keys with passphrase
//! let (encrypted_keys, mnemonic) = keystore::generate_and_encrypt("user_passphrase").unwrap();
//!
//! // Show mnemonic to user ONCE for backup, then discard
//! println!("Backup phrase: {}", mnemonic);
//!
//! // Store encrypted_keys in database (Butler/Sled)
//! // encrypted_keys contains: encrypted signing key, encrypted encryption key, salt, public keys
//! ```
//!
//! # Login Example
//!
//! ```rust
//! use herald::keystore;
//!
//! // Load encrypted_keys from database...
//! # let (encrypted_keys, _) = keystore::generate_and_encrypt("passphrase").unwrap();
//!
//! // Decrypt and restore identity
//! let identity = keystore::decrypt_and_restore(&encrypted_keys, "passphrase").unwrap();
//!
//! // Now you can sign, encrypt, etc.
//! let signature = identity.sign(b"hello");
//! ```
//!
//! # Encryption Example
//!
//! ```rust
//! use herald::{Identity, crypto};
//!
//! let (alice, _) = Identity::generate().unwrap();
//! let (bob, _) = Identity::generate().unwrap();
//!
//! // Encrypt for Bob using his public key
//! let sealed = alice.encrypt_for(&bob.public_encryption_key(), b"hello bob").unwrap();
//!
//! // Bob decrypts using his identity
//! let opened = bob.decrypt_sealed(&sealed).unwrap();
//! assert_eq!(opened, b"hello bob");
//! ```
//!
//! # Recovery Example
//!
//! ```rust
//! use herald::keystore;
//!
//! // User enters their backup mnemonic on a new device
//! let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
//! let encrypted_keys = keystore::recover_from_mnemonic(mnemonic, None, "new_passphrase").unwrap();
//!
//! // Store encrypted_keys - same identity recovered
//! ```

pub mod crypto;
pub mod error;
pub mod identity;
pub mod keystore;

pub use error::{HeraldError, Result};
pub use identity::Identity;
pub use keystore::EncryptedKeys;

// Re-export commonly used crypto functions
pub use crypto::{
    encrypt, decrypt,
    generate_aes_key, encrypt_symmetric, decrypt_symmetric,
    // Key exchange and derivation
    ecdh, derive_key, generate_ephemeral_keypair,
    // Transit encryption (peer-to-peer)
    encrypt_for_transfer, decrypt_from_transfer,
};
