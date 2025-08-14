use crate::errors::PgpError;
use anyhow::Result;
use openpgp::{
    cert::Cert,
    crypto::KeyPair,
    packet::signature::SignatureBuilder,
    parse::{
        stream::{MessageStructure, *},
        Parse,
    },
    policy::StandardPolicy,
    serialize::stream::{Message as StreamMessage, Signer},
};
use sequoia_openpgp::{self as openpgp};
use std::io::{self, Write};
use std::time::Duration;

/// Create cleartext signature with expiry
///
/// This creates a cleartext signed message
pub fn sign_message_cleartext(
    keypair: &KeyPair,
    message: &str,
    expiry_duration: Option<Duration>,
) -> Result<String, PgpError> {
    let mut signed_message = Vec::new();

    {
        let message_writer = StreamMessage::new(&mut signed_message);

        // Create the Signer, using a template if expiry is needed.
        let signer = if let Some(duration) = expiry_duration {
            // 1. Create a SignatureBuilder to act as a template.
            let template = SignatureBuilder::new(openpgp::types::SignatureType::Text)
                // 2. Set the validity period on the TEMPLATE, not the Signer.
                .set_signature_validity_period(duration)
                .map_err(|e| PgpError::SignerCreationError(e.to_string()))?;

            // 3. Create the Signer using the configured template.
            Signer::with_template(message_writer, keypair.clone(), template)
                .map_err(|e| PgpError::SignerCreationError(e.to_string()))?
        } else {
            // No expiry, so no template is needed.
            Signer::new(message_writer, keypair.clone())
                .map_err(|e| PgpError::SignerCreationError(e.to_string()))?
        };

        // Use cleartext mode
        let mut signer = signer
            .cleartext()
            .build()
            .map_err(|e| PgpError::SignerCreationError(e.to_string()))?;

        // Write the message content
        signer
            .write_all(message.as_bytes())
            .map_err(|e| PgpError::MessageWriteError(e.to_string()))?;

        // Finalize the signer
        signer
            .finalize()
            .map_err(|e| PgpError::SignatureFinalizationError(e.to_string()))?;
    }

    String::from_utf8(signed_message).map_err(|e| PgpError::Utf8ConversionError(e.to_string()))
}

/// Verify cleartext signature and extract the original message
///
/// This function verifies a cleartext signed message and returns both
/// the verification result and the extracted original message text.
/// The VerifierBuilder automatically extracts the cleartext during verification.
pub fn verify_message_cleartext(
    public_key: &str,
    cleartext_signed_message: &str,
) -> Result<(bool, Option<String>), PgpError> {
    // Parse the public key
    let cert = Cert::from_bytes(public_key.as_bytes())
        .map_err(|e| PgpError::CertificateParseError(e.to_string()))?;

    // Create verification helper
    let helper = CleartextVerificationHelper::new(cert);
    let policy = &StandardPolicy::new();

    // Create the cleartext message verifier
    let mut verifier = VerifierBuilder::from_bytes(cleartext_signed_message.as_bytes())
        .map_err(|e| PgpError::VerifierCreationError(e.to_string()))?
        .with_policy(policy, None, helper)
        .map_err(|e| PgpError::PolicyApplicationError(e.to_string()))?;

    // Extract the message content - VerifierBuilder does this automatically
    let mut extracted_message = Vec::new();
    match io::copy(&mut verifier, &mut extracted_message) {
        Ok(_) => {
            let message_text = String::from_utf8(extracted_message)
                .map_err(|e| PgpError::Utf8ConversionError(e.to_string()))?;
            Ok((
                verifier.helper_ref().verification_successful,
                Some(message_text),
            ))
        }
        Err(_) => {
            // Verification failed or couldn't extract message
            Ok((false, None))
        }
    }
}

/// Helper struct for cleartext signature verification
struct CleartextVerificationHelper {
    cert: Cert,
    verification_successful: bool,
}

impl CleartextVerificationHelper {
    fn new(cert: Cert) -> Self {
        Self {
            cert,
            verification_successful: false,
        }
    }
}

impl VerificationHelper for CleartextVerificationHelper {
    fn get_certs(&mut self, _ids: &[openpgp::KeyHandle]) -> openpgp::Result<Vec<Cert>> {
        Ok(vec![self.cert.clone()])
    }

    fn check(&mut self, structure: MessageStructure) -> openpgp::Result<()> {
        // For cleartext signatures, we expect:
        // Layer 0: SignatureGroup (the signatures)
        // The message content is extracted directly by the verifier

        for (i, layer) in structure.into_iter().enumerate() {
            match (i, layer) {
                (0, MessageLayer::SignatureGroup { results }) => {
                    // Check if any signature verification succeeded
                    for result in results {
                        if result.is_ok() {
                            self.verification_successful = true;
                        }
                    }
                }
                _ => {
                    // For cleartext signatures, we typically only expect the signature group
                    // The literal data is handled differently in cleartext format
                }
            }
        }

        Ok(())
    }
}
