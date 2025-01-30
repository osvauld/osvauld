use serde::{Deserialize, Serialize};

use crate::domains::models::folder::Folder;
#[derive(Serialize)]
#[serde(untagged)]
pub enum CryptoResponse {
    IsSignedUp {
        isSignedUp: bool,
    },
    Error(String),
    SavePassphrase {
        signature: String,
        username: String,
        deviceKey: String,
        encryptionKey: String,
    },
    CheckPvtKeyLoaded(bool),
    PublicKey(String),
    Signature(String),

    SignatureResponse {
        signature: String,
    },
    DecryptedText(String),
    ImportedCertificate {
        certificate: String,
        publicKey: String,
        salt: String,
    },
    ChangedPassphrase(String),
    ExportedCertificate(String),
    Folders(Vec<FolderResponse>),
    Credentials(Vec<CredentialResponse>),
    FolderCreated(Folder),
    Success,
}

#[derive(Deserialize)]
pub struct SavePassphraseInput {
    pub username: String,
    pub passphrase: String,
    pub challenge: String,
}

#[derive(Deserialize)]
pub struct LoadPvtKeyInput {
    pub passphrase: String,
}

#[derive(Deserialize)]
pub struct SignChallengeInput {
    pub challenge: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCredentialInput {
    pub credential_payload: String,
    pub folder_id: String,
    pub credential_type: String,
}

#[derive(Deserialize)]
pub struct HashAndSignInput {
    pub message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCredentialInput {
    pub credential_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleFavInput {
    pub credential_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLastAccessedInput {
    pub credential_id: String,
}
#[derive(Deserialize, Debug)]
pub struct AddDeviceInput {
    pub certificate: String,
    pub passphrase: String,
    pub ticket: String,
}

#[derive(Deserialize)]
pub struct ExportedCertificate {
    pub passphrase: String,
}
#[derive(Deserialize)]
pub struct GetAllCredentials {
    pub favourite: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]

pub struct PasswordChangeInput {
    pub old_password: String,
    pub new_password: String,
}

pub struct CredentialType {
    pub credential_id: String,
    pub credential_type: String,
    pub data: String,
    pub folder_id: String,
    pub signature: String,
    pub permission: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddFolderInput {
    pub name: String,
    pub description: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftDeleteFolder {
    pub folder_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SavePassphraseResponse {
    pub signature: String,
    pub username: String,
    pub public_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]

pub struct FolderResponse {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCredentialForFolderInput {
    pub folder_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CredentialResponse {
    pub id: String,
    pub data: serde_json::Value,
    pub favourite: bool,
    pub last_accessed: i64,
    pub folder_id: String,
}
