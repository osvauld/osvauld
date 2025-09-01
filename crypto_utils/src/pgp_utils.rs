use crate::errors::PgpError;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use openpgp::{
    armor::{Kind::Signature, Writer as ArmorWriter},
    cert::{CertBuilder, CipherSuite},
    crypto::KeyPair,
    packet::{
        key::{PublicParts, SecretParts, UnspecifiedRole},
        Key,
    },
    parse::{
        stream::{MessageStructure, *},
        Parse,
    },
    policy::{Policy, StandardPolicy},
    serialize::{
        stream::{Message, *},
        Marshal,
    },
    types::{HashAlgorithm, KeyFlags},
    Cert,
};
use sequoia_openpgp::serialize::stream::Encryptor;
use sequoia_openpgp::{self as openpgp};
use std::error::Error;
use std::io::{self, Write};

/// Generate a new PGP certificate with subkeys for signing and encryption
pub fn generate_certificate(username: &str) -> Result<openpgp::Cert> {
    println!("Generating certificate for user: {}", username);

    let (cert, _revocation) = CertBuilder::new()
        .add_userid(username)
        .set_cipher_suite(CipherSuite::Cv25519)
        .add_subkey(KeyFlags::empty().set_signing(), None, None)
        .add_subkey(KeyFlags::empty().set_storage_encryption(), None, None)
        .generate()?;

    println!("Certificate generated. Fingerprint: {}", cert.fingerprint());

    Ok(cert)
}

/// Get the armored (ASCII-armored) public key from a certificate
pub fn get_public_key_armored(cert: &openpgp::Cert) -> Result<String> {
    let mut buf = Vec::new();
    cert.armored().serialize(&mut buf)?;
    Ok(String::from_utf8(buf)?)
}

/// Encrypt text using PGP public key encryption
pub fn encrypt_text_pgp(
    recipient: &Key<PublicParts, UnspecifiedRole>,
    text: &str,
) -> Result<String, anyhow::Error> {
    // Perform the encryption
    let mut encrypted = Vec::new();
    {
        let message = Message::new(&mut encrypted);

        let recipient_obj = openpgp::serialize::stream::Recipient::new(
            openpgp::types::Features::empty(),
            recipient.key_handle(),
            recipient,
        );

        let message = Encryptor::for_recipients(message, vec![recipient_obj])
            .build()
            .map_err(|e| PgpError::EncryptorCreationError(e.to_string()))?;

        let mut writer = LiteralWriter::new(message)
            .build()
            .map_err(|e| PgpError::LiteralWriterCreationError(e.to_string()))?;
        writer.write_all(text.as_bytes())?;
        writer
            .finalize()
            .map_err(|e| PgpError::FinalizationError(e.to_string()))?;
    }

    // Armor the encrypted data
    let mut armored = Vec::new();
    {
        let mut writer = openpgp::armor::Writer::new(&mut armored, openpgp::armor::Kind::Message)
            .map_err(|e| PgpError::ArmorWriterCreationError(e.to_string()))?;
        writer.write_all(&encrypted)?;
        writer
            .finalize()
            .map_err(|e| PgpError::FinalizationError(e.to_string()))?;
    }

    Ok(String::from_utf8(armored)?)
}

/// Decrypt PGP-encrypted text using a secret key
pub fn decrypt_text_pgp(
    policy: &dyn Policy,
    decrypt_key: &openpgp::packet::Key<SecretParts, UnspecifiedRole>,
    ciphertext: &[u8],
) -> Result<Vec<u8>, PgpError> {
    let helper = DecryptionHelperStruct {
        decrypt_key: decrypt_key.clone(),
    };

    let mut decryptor = DecryptorBuilder::from_bytes(ciphertext)
        .map_err(|e| PgpError::DecryptorCreationError(e.to_string()))?
        .with_policy(policy, None, helper)
        .map_err(|e| PgpError::PolicyApplicationError(e.to_string()))?;

    let mut plaintext = Vec::new();
    io::copy(&mut decryptor, &mut plaintext)
        .map_err(|e| PgpError::DecryptionError(e.to_string()))?;

    Ok(plaintext)
}

/// Get a recipient key for encryption from a public key string
pub fn get_recipient(public_key: &str) -> Result<Key<PublicParts, UnspecifiedRole>, PgpError> {
    // Parse the public key bytes into a Cert
    let cert = Cert::from_bytes(&public_key.as_bytes())
        .map_err(|e| PgpError::CertificateParseError(e.to_string()))?;

    // Create a policy for key selection
    let policy = &StandardPolicy::new();
    // Find a suitable encryption key
    cert.keys()
        .with_policy(policy, None)
        .supported()
        .alive()
        .revoked(false)
        .for_storage_encryption()
        .next()
        .map(|key| key.key().clone())
        .ok_or(PgpError::NoSuitableEncryptionKeyError)
}

/// Get a signing keypair from a certificate
pub fn get_signing_keypair(cert: &Cert) -> Result<KeyPair, PgpError> {
    let policy = &StandardPolicy::new();
    let signing_key = cert
        .keys()
        .with_policy(policy, None)
        .for_signing()
        .unencrypted_secret()
        .next()
        .ok_or(PgpError::NoSuitableSigningKeyError)?;

    signing_key
        .key()
        .clone()
        .into_keypair()
        .map_err(|e| PgpError::KeyPairCreationError(e.to_string()))
}

/// Get a decryption key from a certificate
pub fn get_decryption_key(
    cert: &Cert,
) -> Result<openpgp::packet::Key<SecretParts, UnspecifiedRole>, PgpError> {
    let policy = &StandardPolicy::new();

    cert.keys()
        .unencrypted_secret()
        .with_policy(policy, None)
        .supported()
        .alive()
        .revoked(false)
        .for_storage_encryption()
        .next()
        .map(|key| key.key().clone())
        .ok_or(PgpError::NoSuitableDecryptionKeyError)
}

/// Sign a message and return the armored signature
pub fn sign_message(keypair: &KeyPair, message: &str) -> Result<Vec<u8>, PgpError> {
    let mut signature = Vec::new();
    {
        let message_writer = Message::new(&mut signature);
        let signer = Signer::new(message_writer, keypair.clone())
            .map_err(|e| PgpError::SignerCreationError(e.to_string()))?;

        let mut signer = signer
            .detached()
            .build()
            .map_err(|e| PgpError::SignerCreationError(e.to_string()))?;

        signer
            .write_all(message.as_bytes())
            .map_err(|e| PgpError::MessageWriteError(e.to_string()))?;
        signer
            .finalize()
            .map_err(|e| PgpError::SignatureFinalizationError(e.to_string()))?;
    }

    let mut armored_signature = Vec::new();
    {
        let mut armor_writer = ArmorWriter::new(&mut armored_signature, Signature)
            .map_err(|e| PgpError::ArmorWriterCreationError(e.to_string()))?;
        armor_writer
            .write_all(&signature)
            .map_err(|e| PgpError::SignatureWriteError(e.to_string()))?;
        armor_writer
            .finalize()
            .map_err(|e| PgpError::ArmorFinalizationError(e.to_string()))?;
    }

    Ok(armored_signature)
}

/// Verify a detached signature
pub fn verify_signature(
    public_key: &str,
    message: &str,
    signature_b64: &str,
) -> Result<bool, PgpError> {
    // Parse the public key
    let cert = Cert::from_bytes(public_key.as_bytes())
        .map_err(|e| PgpError::CertificateParseError(e.to_string()))?;

    // Decode the base64 signature
    let signature_bytes = general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|e| PgpError::Base64DecodeError(e.to_string()))?;

    // Create a verification helper
    let helper = SignatureVerificationHelper {
        cert,
        verification_successful: false,
    };
    let policy = &StandardPolicy::new();

    // Create the detached verifier
    let mut verifier = DetachedVerifierBuilder::from_bytes(&signature_bytes)
        .map_err(|e| PgpError::VerifierCreationError(e.to_string()))?
        .with_policy(policy, None, helper)
        .map_err(|e| PgpError::PolicyApplicationError(e.to_string()))?;

    // Verify against the message
    verifier
        .verify_bytes(message.as_bytes())
        .map_err(|e| PgpError::VerificationError(e.to_string()))?;

    // Get the verification result from the helper
    Ok(verifier.helper_ref().verification_successful)
}

/// Hash text using SHA-512
pub fn hash_text_sha512(text: &str) -> Result<Vec<u8>, PgpError> {
    let builder = HashAlgorithm::SHA512
        .context()
        .map_err(|e| PgpError::HashContextCreationError(e.to_string()))?;

    let mut context = builder.for_digest();
    context.update(text.as_bytes());

    context
        .into_digest()
        .map_err(|e| PgpError::DigestComputationError(e.to_string()))
}

/// Generate a key ID from a public key
pub fn generate_key_id(public_key: &str) -> Result<String, Box<dyn Error>> {
    let cert = Cert::from_bytes(public_key.as_bytes())?;
    Ok(cert.fingerprint().to_hex().to_lowercase())
}

/// Helper struct for PGP decryption
struct DecryptionHelperStruct {
    decrypt_key: openpgp::packet::Key<
        openpgp::packet::key::SecretParts,
        openpgp::packet::key::UnspecifiedRole,
    >,
}

impl VerificationHelper for DecryptionHelperStruct {
    fn get_certs(
        &mut self,
        _ids: &[sequoia_openpgp::KeyHandle],
    ) -> Result<Vec<sequoia_openpgp::Cert>> {
        Ok(Vec::new())
    }

    fn check(&mut self, _structure: MessageStructure) -> Result<()> {
        Ok(())
    }
}

impl DecryptionHelper for DecryptionHelperStruct {
    fn decrypt(
        &mut self,
        pkesks: &[openpgp::packet::PKESK],
        _skesks: &[openpgp::packet::SKESK],
        sym_algo: Option<openpgp::types::SymmetricAlgorithm>,
        decrypt: &mut dyn FnMut(
            Option<openpgp::types::SymmetricAlgorithm>,
            &openpgp::crypto::SessionKey,
        ) -> bool,
    ) -> openpgp::Result<Option<openpgp::Cert>> {
        if pkesks.is_empty() {
            return Err(anyhow::anyhow!("No PKESKs provided"));
        }

        let mut pair = KeyPair::from(
            self.decrypt_key
                .clone()
                .into_keypair()
                .map_err(|e| PgpError::KeyPairCreationError(e.to_string()))?,
        );

        for pkesk in pkesks {
            if let Some((algo, session_key)) = pkesk.decrypt(&mut pair, sym_algo) {
                if decrypt(algo, &session_key) {
                    return Ok(None);
                }
            }
        }

        Err(anyhow::anyhow!("Decryption failed for all provided PKESKs"))
    }
}

/// Helper struct for signature verification
struct SignatureVerificationHelper {
    cert: Cert,
    verification_successful: bool,
}

impl VerificationHelper for SignatureVerificationHelper {
    fn get_certs(&mut self, _ids: &[openpgp::KeyHandle]) -> openpgp::Result<Vec<Cert>> {
        Ok(vec![self.cert.clone()])
    }

    fn check(&mut self, structure: MessageStructure) -> openpgp::Result<()> {
        for (i, layer) in structure.into_iter().enumerate() {
            match (i, layer) {
                (0, MessageLayer::SignatureGroup { results }) => {
                    for result in results {
                        match result {
                            Ok(_) => {
                                self.verification_successful = true;
                                return Ok(());
                            }
                            Err(_) => continue,
                        }
                    }
                    return Err(anyhow::anyhow!("All signature verifications failed"));
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unexpected message structure for detached signature"
                    ));
                }
            }
        }

        if self.verification_successful {
            Ok(())
        } else {
            Err(anyhow::anyhow!("No signature group found"))
        }
    }
}
