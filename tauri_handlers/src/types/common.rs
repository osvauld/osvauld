use butler::Space;
use serde::{Deserialize, Serialize};

/// Known user response (for sharing, contacts list)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownUserResponse {
    pub user_id: String,
    pub username: String,
    pub public_key: String,
}

/// Sovereign node response (for publishing, node selection)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SovereignNodeResponse {
    pub node_id: String,
    pub username: String,
    pub user_public_key: String,
    pub device_public_key: String,
    pub is_connected: bool,
    pub last_connected_at: Option<i64>,
}

/// Base response type used by shared handlers
/// Projects can extend this with additional variants
#[derive(Serialize)]
#[serde(untagged)]
pub enum BaseCryptoResponse {
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
    /// Seed phrase (mnemonic) for account recovery
    SeedPhrase {
        mnemonic: String,
    },
    Folders(Vec<FolderResponse>),
    Spaces(Vec<SpaceResponse>),
    ResourcesMetadata(Vec<ResourceMetadata>),
    SearchedResourceIds(Vec<String>),
    FolderCreated(Space),
    SpaceCreated(Space),
    Success,
    UpdateResources,
    ResourceCreated(ResourceMetadata),
    ResourceUpdated(ResourceMetadata),
    SelectedResourceResponse(ResourceResponse),
    GetKnownUsers(Vec<KnownUserResponse>),
    GetSovereignNodes(Vec<SovereignNodeResponse>),
    UserDetailsForShare(String),
    OneTimePermit(OneTimePermitOut),
    Users(Vec<KnownUserResponse>),
    // Node-related responses
    NodeRegistered(crate::handlers::node::NodeInfoResponse),
}

// ========== Input Types (all shared) ==========

#[derive(Deserialize)]
pub struct SavePassphraseInput {
    pub username: String,
    pub passphrase: String,
}

#[derive(Deserialize, Serialize)]
pub struct OneTimePermitOut {
    pub permit: String,
    pub permit_pub_key: String,
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
    pub permit_template_json: String,
    pub metadata_json: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResourceInput {
    pub id: String,
    pub data: String,
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
    pub folder_template_json: String,
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
    pub recipient_role: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default: bool,
}

// ========== Space Types (new terminology) ==========

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSpaceInput {
    pub name: String,
    pub description: String,
    pub folder_template_json: String, // Still using folder template for compatibility
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftDeleteSpace {
    pub space_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceShareUsersInput {
    pub space_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareSpace {
    pub space_id: String,
    pub user_id: String,
    pub recipient_role: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMetadata {
    pub id: String,
    pub title: String,
    pub resource_type: String,
    pub folder_id: String,
    pub last_modified: i64,
    pub favourite: bool,
    pub preview: Option<String>,
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
    /// The permit (UCAN token) for authorization
    pub permit: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SyncResourceInput {
    pub resource_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RequestFolderResourcesInput {
    pub folder_id: String,
}
