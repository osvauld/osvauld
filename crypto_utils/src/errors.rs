use std::io;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum AesError {
    #[error("Key derivation failed: {0}")]
    KeyDerivationError(String),

    #[error("Cipher creation failed: {0}")]
    CipherCreationError(String),

    #[error("Encryption failed: {0}")]
    EncryptionError(String),

    #[error("Decryption failed: {0}")]
    DecryptionError(String),

    #[error("Failed to parse certificate: {0}")]
    CertificateParseError(String),
    #[error("Failed to decode: {0}")]
    Base64DecodeError(String),
    #[error("Failed to decode: {0}")]
    Utf8ConversionError(String),
}

#[derive(Error, Debug)]
pub enum PgpError {
    #[error("Failed to create hash context: {0}")]
    HashContextCreationError(String),

    #[error("Failed to compute digest: {0}")]
    DigestComputationError(String),

    #[error("Failed to create encryptor: {0}")]
    EncryptorCreationError(String),

    #[error("Failed to create literal writer: {0}")]
    LiteralWriterCreationError(String),

    #[error("Failed to finalize encryption: {0}")]
    FinalizationError(String),

    #[error("Failed to create armor writer: {0}")]
    ArmorWriterCreationError(String),

    #[error("Failed to create decryptor: {0}")]
    DecryptorCreationError(String),

    #[error("Failed to apply policy: {0}")]
    PolicyApplicationError(String),

    #[error("Decryption failed: {0}")]
    DecryptionError(String),
    #[error("No suitable encryption key found")]
    NoSuitableEncryptionKeyError,

    #[error("No suitable signing key found")]
    NoSuitableSigningKeyError,

    #[error("No suitable decryption key found")]
    NoSuitableDecryptionKeyError,

    #[error("Key pair creation failed: {0}")]
    KeyPairCreationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("Failed to create signer: {0}")]
    SignerCreationError(String),
    #[error("Failed to write message: {0}")]
    MessageWriteError(String),
    #[error("Failed to finalize signature: {0}")]
    SignatureFinalizationError(String),
    #[error("Failed to write signature: {0}")]
    SignatureWriteError(String),
    #[error("Failed to finalize armored signature: {0}")]
    ArmorFinalizationError(String),
    #[error("Failed to parse certificate: {0}")]
    CertificateParseError(String),
    #[error("Base64 decode error: {0}")]
    Base64DecodeError(String),
    #[error("Verifier creation error: {0}")]
    VerifierCreationError(String),
    #[error("Verification error: {0}")]
    VerificationError(String),
    #[error("utf conversion error: {0}")]
    Utf8ConversionError(String),
    #[error("failed to validate the signature: {0}")]
    InvalidSignature(String),
}

#[derive(Error, Debug)]
pub enum CryptoUtilsError {
    #[error("No certificate stored in context")]
    NoCertificateError,

    #[error("Failed to lock global context: {0}")]
    ContextLockError(String),

    #[error("Missing field value")]
    MissingFieldValue,

    #[error("Invalid salt length")]
    InvalidSaltLength,

    #[error("Failed to get signing key: {0}")]
    SigningKeyError(String),
    #[error("Failed to sign message: {0}")]
    SigningError(String),
    #[error("UTF-8 conversion error: {0}")]
    Utf8ConversionError(String),
    #[error("Failed to get decryption key: {0}")]
    CertificateDecryptionError(String),
    #[error("Failed to get decryption key: {0}")]
    Other(String),
}

#[derive(Error, Debug)]
pub enum UcanError {
    #[error("UCAN creation failed: {0}")]
    CreationError(String),

    #[error("UCAN signature error: {0}")]
    SignatureError(String),

    #[error("UCAN encoding error: {0}")]
    EncodingError(String),

    #[error("UCAN decoding error: {0}")]
    DecodingError(String),

    #[error("UCAN expired")]
    ExpiredError,

    #[error("Invalid UCAN format: {0}")]
    FormatError(String),

    #[error("DID creation error: {0}")]
    DidError(String),

    #[error("Ed25519 key extraction error: {0}")]
    KeyExtractionError(String),

    #[error("Crypto utils error: {0}")]
    CryptoUtilsError(#[from] CryptoUtilsError),
    #[error("Failed to parse UCAN token: {0}")]
    ParseError(String),
    #[error("UCAN validation failed: {0}")]
    ValidationError(String),
    #[error("The presenter of the token is not its intended audience.")]
    InvalidAudience,
    #[error("Failed to decode DID string: {0}")]
    DidDecodeError(String),
    #[error("Failed to decode base64 key: {0}")]
    Base64DecodeError(String),
    #[error("Invalid DID format: {0}")]
    InvalidDidFormat(String),
    #[error("The provided public key does not match the key in the DID.")]
    PublicKeyMismatch,
    #[error("The required capability was not found in the UCAN.")]
    CapabilityNotFound,
    #[error("A proof is required for this UCAN, but none was found.")]
    ProofRequired,

    #[error("The UCAN's proof chain is invalid: {0}")]
    ProofChainInvalid(String),

    #[error("Failed to convert ucan to CID {0}")]
    UcanCidConvertionFailed(String),
    #[error(
        "Delegation is not permitted; the parent UCAN lacks the required 'ucan/share' capability."
    )]
    DelegationNotPermitted,
}

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("AES error: {0}")]
    AesError(#[from] AesError),

    #[error("PGP error: {0}")]
    PgpError(#[from] PgpError),

    #[error("Crypto utils error: {0}")]
    CryptoUtilsError(#[from] CryptoUtilsError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("Base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("Certificate error: {0}")]
    CertError(String),

    #[error("UCAN error: {0}")]
    UcanError(#[from] UcanError),

    #[error("Other error: {0}")]
    Other(String),
}
