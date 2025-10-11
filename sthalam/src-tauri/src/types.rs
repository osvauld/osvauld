use osvauld_core::models::device::Device;
use serde::{Deserialize, Serialize};

use osvauld_core::models::folder::Folder;
use osvauld_core::models::user::User;

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
        #[serde(rename = "userId")]
        user_id: String,
    },
    CheckPvtKeyLoaded(bool),
    PublicKey(String),
    User {
        username: String,
        #[serde(rename = "publicKey")]
        public_key: String,
        #[serde(rename = "userId")]
        user_id: String,
    },
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
    UserDetails {
        #[serde(rename = "userId")]
        user_id: String,
        #[serde(rename = "deviceId")]
        device_id: String,
        username: String,
        #[serde(rename = "publicKey")]
        public_key: String,
        #[serde(rename = "deviceKey")]
        device_key: String,
    },
    UserId(String),
    ChangedPassphrase(String),
    ExportedCertificate(String),
    Folders(Vec<FolderResponse>),
    Resources(Vec<ResourceResponse>),
    SearchedResourceIds(Vec<String>),
    FolderCreated(Folder),
    Success,
    UpdateResources,
    ResourceCreated(String),
    SelectedResourceResponse(ResourceResponse),
    SelectedResourceResponse2(ResourceResponse2),
    CreatedKnownUser {
        user: User,
        device: Device,
    },
    GetKnownUsers(Vec<User>),
    UserDetailsForShare(String),
    OneTimeUcanToken(UcanOneTimeTokenOut),
    Users(Vec<User>),
    ShareToken(crate::handlers::website_handler::ShareTokenResponse),
    WebsiteState(Vec<u8>),
}

#[derive(Deserialize)]
pub struct SavePassphraseInput {
    pub username: String,
    pub passphrase: String,
}

#[derive(Deserialize, Serialize)]
pub struct UcanOneTimeTokenOut {
    pub ucan_token: String,
    pub ucan_pub_key: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePreview {
    pub id: String,
    pub preview: String,
    pub title: String,
    pub folder_id: String,
    pub favourite: bool,
    pub last_modified: i64,
    pub last_accessed: i64,
}
#[derive(Deserialize)]
pub struct LoadPvtKeyInput {
    pub passphrase: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddResourceInput {
    pub resource_payload: String,
    pub folder_id: String,
    pub resource_type: String,
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
    pub username: String,
    pub device_id: String,
    pub passphrase: String,
}

#[derive(Deserialize)]
pub struct ExportedCertificate {
    pub passphrase: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordChangeInput {
    pub old_password: String,
    pub new_password: String,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderShareUsersInput {
    pub folder_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareFolder {
    pub folder_id: String,
    pub user_id: String,
    pub permissions: Vec<(String, String)>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetResourceForFolderInput {
    pub folder_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceResponse {
    pub id: String,
    pub data: serde_json::Value,
    pub favourite: bool,
    pub last_accessed: i64,
    pub folder_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceResponse2 {
    pub id: String,
    pub data: serde_json::Value,
    pub favourite: bool,
    pub last_accessed: i64,
    pub folder_id: String,
    pub resource_type: String,
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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ShareResource {
    pub user_id: String,
    pub resource_id: String,
    pub permissions: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize)]
pub struct UserDetails {
    pub user_public_key: String,
    pub device_public_key: String,
    pub username: String,
    pub ucan_token: String,
    pub ucan_pub_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectToWebsiteInput {
    pub connection_string: String,
}
