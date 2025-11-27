use thiserror::Error;

#[derive(Error, Debug)]
pub enum AlchemistError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },

    #[error("Invalid nonce length: expected {expected}, got {actual}")]
    InvalidNonceLength { expected: usize, actual: usize },

    #[error("Invalid ciphertext: too short")]
    CiphertextTooShort,

    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),
}

pub type Result<T> = std::result::Result<T, AlchemistError>;
