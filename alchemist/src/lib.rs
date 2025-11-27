//! Alchemist: Cryptographic transformations for Osvauld
//!
//! Pure transformation layer - takes keys and data, returns transformed bytes.
//! No identity or key management (see `herald` for that).
//!
//! # Features
//!
//! - **Symmetric encryption**: AES-256-GCM with random nonces
//! - **Key exchange**: X25519 ECDH
//! - **Key derivation**: HKDF-SHA256
//! - **High-level**: ECIES-style seal/open
//!
//! # Example
//!
//! ```rust
//! use alchemist::{encrypt, decrypt, seal, open, derive_key, ecdh};
//!
//! // Direct encryption with known key
//! let key = [42u8; 32];
//! let ciphertext = encrypt(&key, b"hello").unwrap();
//! let plaintext = decrypt(&key, &ciphertext).unwrap();
//!
//! // Encrypt for recipient (ECIES-style)
//! let (secret, public) = alchemist::generate_ephemeral_keypair();
//! let sealed = seal(&public, b"secret").unwrap();
//! let opened = open(&secret, &sealed).unwrap();
//! ```

pub mod crypto;
pub mod error;

pub use crypto::{
    // Symmetric encryption
    encrypt,
    encrypt_with_nonce,
    decrypt,
    decrypt_with_nonce,
    // Key exchange
    ecdh,
    generate_ephemeral_keypair,
    // Key derivation
    derive_key,
    derive_keys,
    // High-level ECIES
    seal,
    open,
    // Constants
    NONCE_SIZE,
    KEY_SIZE,
    TAG_SIZE,
};

pub use error::{AlchemistError, Result};
