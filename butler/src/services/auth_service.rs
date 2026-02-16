//! Auth Service - Signup, Login, Recovery using Herald + RedbStore
//!
//! Handles:
//! - Signup: Generate identity, encrypt keys, store in redb
//! - Login: Load encrypted keys, decrypt with passphrase, return Identity
//! - Recovery: Restore from mnemonic, encrypt with new passphrase

use crate::error::{ButlerError, Result};
use crate::models::{Argon2Params, EncryptedKeyStore, IdentityData};
use crate::storage::RedbStore;
use herald::{keystore, EncryptedKeys, Identity};
use std::sync::Arc;
use tracing::instrument;

/// Signup result - contains identity and mnemonic for user backup
pub struct SignupResult {
    /// The active identity (signing + encryption keys loaded)
    pub identity: Identity,
    /// Mnemonic phrase - show ONCE to user, then discard
    pub mnemonic: String,
}

/// Signup a new user
///
/// 1. Generates BIP39 mnemonic → Identity
/// 2. Encrypts keys with passphrase (Argon2 + AES)
/// 3. Stores IdentityData + EncryptedKeyStore in redb
/// 4. Returns Identity + mnemonic (for user backup)
#[instrument(skip(store, passphrase), fields(username = %username))]
pub fn signup(store: &RedbStore, username: &str, passphrase: &str) -> Result<SignupResult> {
    // Check if already signed up
    if store.is_signed_up()? {
        return Err(ButlerError::already_signed_up());
    }

    // Generate identity and encrypt keys
    let (encrypted_keys, mnemonic) = keystore::generate_and_encrypt(passphrase)
        .map_err(|e| ButlerError::crypto_error(e.to_string()))?;

    // Create IdentityData (public info)
    let identity_data = IdentityData::new(
        encrypted_keys.did.clone(),
        encrypted_keys.public_signing_key.to_vec(),
        encrypted_keys.public_encryption_key.to_vec(),
        encrypted_keys.public_device_key.to_vec(),
        username.to_string(),
    );

    // Create EncryptedKeyStore (for Sled storage)
    let keystore = EncryptedKeyStore::with_params(
        encrypted_keys.encrypted_signing_key.clone(),
        encrypted_keys.encrypted_encryption_key.clone(),
        encrypted_keys.encrypted_device_key.clone(),
        encrypted_keys.salt.clone(),
        Argon2Params {
            m_cost: encrypted_keys.m_cost,
            t_cost: encrypted_keys.t_cost,
            p_cost: encrypted_keys.p_cost,
        },
    );

    // Store in Sled
    store.set_identity(&identity_data)?;
    store.set_keystore(&keystore)?;
    store.flush()?;

    // Restore identity for use (we already have the keys in memory)
    let identity = keystore::decrypt_and_restore(&encrypted_keys, passphrase)
        .map_err(|e| ButlerError::crypto_error(e.to_string()))?;

    Ok(SignupResult { identity, mnemonic })
}

/// Login - decrypt keys and return Identity
#[instrument(skip_all)]
pub fn login(store: &RedbStore, passphrase: &str) -> Result<Identity> {
    // Load identity data
    let identity_data = store
        .get_identity()?
        .ok_or_else(ButlerError::not_signed_up)?;

    // Load encrypted keystore
    let keystore = store
        .get_keystore()?
        .ok_or_else(ButlerError::not_signed_up)?;

    // Convert to Herald's EncryptedKeys format
    let encrypted_keys = EncryptedKeys {
        encrypted_signing_key: keystore.encrypted_signing_key,
        encrypted_encryption_key: keystore.encrypted_encryption_key,
        encrypted_device_key: keystore.encrypted_device_key,
        public_signing_key: identity_data
            .signing_public_key
            .clone()
            .try_into()
            .map_err(|_| ButlerError::crypto_error("Invalid public signing key length"))?,
        public_encryption_key: identity_data
            .encryption_public_key
            .clone()
            .try_into()
            .map_err(|_| ButlerError::crypto_error("Invalid public encryption key length"))?,
        public_device_key: identity_data
            .device_public_key
            .clone()
            .try_into()
            .map_err(|_| ButlerError::crypto_error("Invalid public device key length"))?,
        did: identity_data.did.clone(),
        salt: keystore.salt,
        m_cost: keystore.argon2_params.m_cost,
        t_cost: keystore.argon2_params.t_cost,
        p_cost: keystore.argon2_params.p_cost,
    };

    // Decrypt and restore identity
    let identity = keystore::decrypt_and_restore(&encrypted_keys, passphrase)
        .map_err(|_| ButlerError::invalid_passphrase())?;

    Ok(identity)
}

/// Check if user is signed up
#[instrument(skip_all)]
pub fn is_signed_up(store: &RedbStore) -> Result<bool> {
    store.is_signed_up()
}

/// Get identity data (public info) without decrypting keys
#[instrument(skip_all)]
pub fn get_identity_data(store: &RedbStore) -> Result<Option<IdentityData>> {
    store.get_identity()
}

/// Recover from mnemonic on a new device
///
/// 1. Restore Identity from mnemonic
/// 2. Encrypt keys with new passphrase
/// 3. Store in redb
#[instrument(skip(store, mnemonic, passphrase), fields(username = %username))]
pub fn recover(
    store: &RedbStore,
    username: &str,
    mnemonic: &str,
    passphrase: &str,
) -> Result<Identity> {
    // Check if already signed up
    if store.is_signed_up()? {
        return Err(ButlerError::already_signed_up());
    }

    // Recover identity from mnemonic and encrypt with new passphrase
    let encrypted_keys = keystore::recover_from_mnemonic(mnemonic, None, passphrase)
        .map_err(|e| ButlerError::crypto_error(e.to_string()))?;

    // Create IdentityData
    let identity_data = IdentityData::new(
        encrypted_keys.did.clone(),
        encrypted_keys.public_signing_key.to_vec(),
        encrypted_keys.public_encryption_key.to_vec(),
        encrypted_keys.public_device_key.to_vec(),
        username.to_string(),
    );

    // Create EncryptedKeyStore
    let keystore = EncryptedKeyStore::with_params(
        encrypted_keys.encrypted_signing_key.clone(),
        encrypted_keys.encrypted_encryption_key.clone(),
        encrypted_keys.encrypted_device_key.clone(),
        encrypted_keys.salt.clone(),
        Argon2Params {
            m_cost: encrypted_keys.m_cost,
            t_cost: encrypted_keys.t_cost,
            p_cost: encrypted_keys.p_cost,
        },
    );

    // Store in Sled
    store.set_identity(&identity_data)?;
    store.set_keystore(&keystore)?;
    store.flush()?;

    // Restore identity for use
    let identity = keystore::decrypt_and_restore(&encrypted_keys, passphrase)
        .map_err(|e| ButlerError::crypto_error(e.to_string()))?;

    Ok(identity)
}

/// Change passphrase
///
/// 1. Decrypt with old passphrase
/// 2. Re-encrypt with new passphrase
/// 3. Update redb
#[instrument(skip_all)]
pub fn change_passphrase(
    store: &RedbStore,
    old_passphrase: &str,
    new_passphrase: &str,
) -> Result<()> {
    // Load identity data
    let identity_data = store
        .get_identity()?
        .ok_or_else(ButlerError::not_signed_up)?;

    // Load encrypted keystore
    let keystore = store
        .get_keystore()?
        .ok_or_else(ButlerError::not_signed_up)?;

    // Convert to Herald format
    let encrypted_keys = EncryptedKeys {
        encrypted_signing_key: keystore.encrypted_signing_key,
        encrypted_encryption_key: keystore.encrypted_encryption_key,
        encrypted_device_key: keystore.encrypted_device_key,
        public_signing_key: identity_data
            .signing_public_key
            .clone()
            .try_into()
            .map_err(|_| ButlerError::crypto_error("Invalid public signing key length"))?,
        public_encryption_key: identity_data
            .encryption_public_key
            .clone()
            .try_into()
            .map_err(|_| ButlerError::crypto_error("Invalid public encryption key length"))?,
        public_device_key: identity_data
            .device_public_key
            .clone()
            .try_into()
            .map_err(|_| ButlerError::crypto_error("Invalid public device key length"))?,
        did: identity_data.did.clone(),
        salt: keystore.salt,
        m_cost: keystore.argon2_params.m_cost,
        t_cost: keystore.argon2_params.t_cost,
        p_cost: keystore.argon2_params.p_cost,
    };

    // Change passphrase
    let new_encrypted =
        keystore::change_passphrase(&encrypted_keys, old_passphrase, new_passphrase)
            .map_err(|_| ButlerError::invalid_passphrase())?;

    // Update keystore
    let new_keystore = EncryptedKeyStore::with_params(
        new_encrypted.encrypted_signing_key,
        new_encrypted.encrypted_encryption_key,
        new_encrypted.encrypted_device_key,
        new_encrypted.salt,
        Argon2Params {
            m_cost: new_encrypted.m_cost,
            t_cost: new_encrypted.t_cost,
            p_cost: new_encrypted.p_cost,
        },
    );

    store.set_keystore(&new_keystore)?;
    store.flush()?;

    Ok(())
}
