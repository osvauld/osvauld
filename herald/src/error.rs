use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeraldError {
    // Identity errors
    #[error("Invalid mnemonic phrase: {0}")]
    InvalidMnemonic(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("Invalid secret key")]
    InvalidSecretKey,

    // Crypto errors
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid ciphertext: too short")]
    CiphertextTooShort,

    #[error("Invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },

    #[error("Invalid nonce length: expected {expected}, got {actual}")]
    InvalidNonceLength { expected: usize, actual: usize },
}

pub type Result<T> = std::result::Result<T, HeraldError>;
