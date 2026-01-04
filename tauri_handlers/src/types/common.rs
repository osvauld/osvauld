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

/// Base response type for all handlers
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
    SeedPhrase {
        mnemonic: String,
    },
    // Space responses
    Spaces(Vec<SpaceResponse>),
    SpaceCreated(Space),
    // Page responses
    PagesMetadata(Vec<PageMetadata>),
    PageCreated(PageMetadata),
    PageOpened(PageResponse),
    Success,
    GetKnownUsers(Vec<KnownUserResponse>),
    GetSovereignNodes(Vec<SovereignNodeResponse>),
    UserDetailsForShare(String),
    OneTimePermit(OneTimePermitOut),
    Users(Vec<KnownUserResponse>),
    // NodeRegistered removed - node identity stored in IDENTITY table
}

// ========== Auth Input Types ==========

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

#[derive(Deserialize)]
pub struct LoadPvtKeyInput {
    pub passphrase: String,
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

// ========== Space Types ==========

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpaceInput {
    pub name: String,
    pub description: String,
    pub space_template_json: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSpaceInput {
    pub space_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareSpaceInput {
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
    pub is_default: bool,
}

// ========== Page Types ==========

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePageInput {
    pub space_id: String,
    pub permit_template_json: String,
    pub metadata_json: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPageInput {
    pub page_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosePageInput {
    pub page_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyUpdateInput {
    pub page_id: String,
    pub layer_name: String,
    pub update: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PageMetadata {
    pub id: String,
    pub title: String,
    pub page_type: String,
    pub space_id: String,
    pub last_modified: i64,
    pub favourite: bool,
    pub preview: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PageResponse {
    pub id: String,
    pub data: serde_json::Value,
    pub favourite: bool,
    pub last_accessed: i64,
    pub space_id: String,
    pub has_wasm: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePageWasmInput {
    pub page_id: String,
    /// Base64-encoded WASM bytes
    pub wasm_base64: String,
}

// ========== User Types ==========

#[derive(Serialize, Deserialize)]
pub struct UserDetails {
    pub user_public_key: String,
    pub device_public_key: String,
    pub username: String,
    pub permit: String,
}
