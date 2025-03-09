use serde::{Deserialize, Serialize};

use crate::domains::models::folder::Folder;
use crate::domains::models::resource::DecryptedResource;
use crate::domains::models::user::User;

#[derive(Serialize)]
#[serde(untagged)]
pub enum CryptoResponse {
    IsSignedUp {
        #[serde(rename = "isSignedUp")]
        is_signed_up: bool,
    },
    Error(String),
    SavePassphrase {
        username: String,
        #[serde(rename = "deviceKey")]
        device_key: String,
        #[serde(rename = "encryptionKey")]
        encryption_key: String,
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
        #[serde(rename = "publicKey")]
        public_key: String,
        salt: String,
    },
    UserId(String),
    ChangedPassphrase(String),
    ExportedCertificate(String),
    Folders(Vec<FolderResponse>),
    Resources(Vec<ResourceResponse>),
    FolderCreated(Folder),
    Success,
    UpdateResources,
    ResourceCreateted(String),
    GetResourceResponse(DecryptedResource),
    CreatedKnownUser(User),
    GetKnownUsers(Vec<User>),
}

#[derive(Deserialize)]
pub struct SavePassphraseInput {
    pub username: String,
    pub passphrase: String,
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
pub struct AddResourceInput {
    pub resource_payload: String,
    pub folder_id: String,
    pub resource_type: String,
}

#[derive(Deserialize)]
pub struct HashAndSignInput {
    pub message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResourceInput {
    pub resource_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleFavInput {
    pub resource_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLastAccessedInput {
    pub resource_id: String,
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
pub struct GetAllResources {
    pub favourite: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordChangeInput {
    pub old_password: String,
    pub new_password: String,
}

// pub struct ResourceType {
//     pub resource_id: String,
//     pub resource_type: String,
//     pub data: String,
//     pub folder_id: String,
//     pub signature: String,
//     pub permission: String,
// }
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
pub struct GetResourceForFolderInput {
    pub folder_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResourceResponse {
    pub id: String,
    pub data: serde_json::Value,
    pub favourite: bool,
    pub last_accessed: i64,
    pub folder_id: String,
}

#[derive(Deserialize, Clone)]
pub struct UpdateResources {
    pub id: String,
    pub data: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetResource {
    pub resource_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddKnownUser {
    pub nickname: String,
    pub public_key: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ShareResource {
    pub public_key: String,
    pub resource_id: String,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InitiateFirstConnectionInput {
    pub ticket: String,
    pub user_id: String,
}
