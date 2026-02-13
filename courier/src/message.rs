//! Protocol message types for P2P communication
//!
//! All messages exchanged between peers are defined here.
//! courier owns serialization/deserialization.
//!
//! ## Wire Format
//!
//! Each message is encoded as: `[u16 tag (big-endian)][bincode payload]`
//! The outer transport wraps in `[u32 len][payload]`.
//!
//! Tags are grouped by category:
//! - 0x00xx: Handshake
//! - 0x01xx: Sync
//! - 0x02xx: Publishing
//! - 0x03xx: Viewer
//! - 0x04xx: Assets
//! - 0x05xx: Consent
//! - 0xFFxx: Errors

use serde::{Deserialize, Serialize};

/// Protocol version for this wire format
pub const PROTOCOL_VERSION: u16 = 2;

// Re-export LayerType from domains (single source of truth)
pub use domains::LayerType;

// --- Tag Constants ---

pub mod tags {
    // Handshake (0x00xx)
    pub const HELLO: u16 = 0x0001;
    pub const WELCOME: u16 = 0x0002;
    pub const PERMIT_GRANT: u16 = 0x0003;
    pub const ACK: u16 = 0x0004;
    pub const REJECTED: u16 = 0x0005;

    // Sync (0x01xx)
    pub const SYNC_OFFER: u16 = 0x0100;
    pub const SYNC_ACCEPT: u16 = 0x0101;
    pub const SYNC_ACK: u16 = 0x0102;
    pub const SYNC_RESET: u16 = 0x0103;
    pub const SYNC_SNAPSHOT: u16 = 0x0104;

    // Publishing (0x02xx)
    pub const PUBLISH_SPACE: u16 = 0x0200;
    pub const PUBLISH_SPACE_ACK: u16 = 0x0201;
    pub const PAGE_ANNOUNCE: u16 = 0x0202;
    pub const PAGE_ANNOUNCE_ACK: u16 = 0x0203;
    pub const PERMIT_UPDATE: u16 = 0x0204;
    pub const PUBLISH_ERROR: u16 = 0x020F;

    // Viewer (0x03xx)
    pub const SPACE_REQUEST: u16 = 0x0300;
    pub const SPACE_DATA: u16 = 0x0301;
    pub const SPACE_DATA_ACK: u16 = 0x0302;
    pub const SPACE_REQUEST_ERROR: u16 = 0x030F;
    pub const GET_LINK_REQ: u16 = 0x0310;
    pub const GET_LINK_RESP: u16 = 0x0311;

    // Assets (0x04xx)
    pub const ASSET_PREPARE: u16 = 0x0400;
    pub const ASSET_READY: u16 = 0x0401;
    pub const ASSET_ACK: u16 = 0x0402;

    // Consent (0x05xx)
    pub const SYNC_CONSENT_GRANT: u16 = 0x0500;
    pub const SYNC_CONSENT_ACK: u16 = 0x0501;
    pub const LAYER_CONSENT_GRANT: u16 = 0x0502;
    pub const LAYER_CONSENT_ACK: u16 = 0x0503;

    // Layer Permits (extends Publishing 0x02xx)
    pub const LAYER_PERMIT: u16 = 0x0205;

    // Errors (0xFFxx)
    pub const ERROR: u16 = 0xFF00;
}

// --- Independent Message Structs ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloMsg {
    pub protocol_version: u16,
    pub did: String,
    pub username: String,
    pub public_key: [u8; 32],
    pub encryption_key: [u8; 32],
    pub signature: Vec<u8>,
    pub timestamp: i64,
    pub permit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WelcomeMsg {
    pub protocol_version: u16,
    pub node_id: String,
    pub node_public_key: [u8; 32],
    pub node_encryption_key: [u8; 32],
    pub signature: Vec<u8>,
    pub timestamp: i64,
    pub permit_for_peer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermitGrantMsg {
    pub permit_for_node: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectedMsg {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOfferMsg {
    pub page_id: String,
    pub layer_name: String,
    pub layer_type: LayerType,
    pub data: Vec<u8>,
    pub state_vector: Vec<u8>,
    pub ephemeral_public: [u8; 32],
    #[serde(default)]
    pub authority_permit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAcceptMsg {
    pub page_id: String,
    pub layer_name: String,
    pub state_vector: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAckMsg {
    pub page_id: String,
    pub layer_name: String,
    pub state_vector: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResetMsg {
    pub page_id: String,
    pub layer_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSnapshotMsg {
    pub page_id: String,
    pub layer_name: String,
    pub layer_type: LayerType,
    pub snapshot: Vec<u8>,
    pub state_vector: Vec<u8>,
    pub ephemeral_public: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPrepareMsg {
    pub page_id: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetReadyMsg {
    pub page_id: String,
    pub hash: String,
    pub iroh_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetAckMsg {
    pub page_id: String,
    pub hash: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishSpaceMsg {
    pub request_id: String,
    pub space: PublishedSpace,
    pub space_permit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishSpaceAckMsg {
    pub request_id: String,
    pub permit: String,
    pub pages: Vec<String>,
}

/// Announces a page (meta + permits only, no layer data)
///
/// **Context**: Replaces monolithic PublishPage. Layers delivered via Scribe subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageAnnounceMsg {
    pub request_id: String,
    pub page: PublishedPageMeta,
    pub page_permit: String,
    pub owner_permit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageAnnounceAckMsg {
    pub request_id: String,
    pub page_id: String,
    pub permit: String,
}

/// Push updated permit for existing subscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermitUpdateMsg {
    pub permit: String,
    pub scope: PermitScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermitScope {
    Space { space_id: String },
    Page { page_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishErrorMsg {
    pub request_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetShareableLinkRequestMsg {
    pub request_id: String,
    pub space_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetShareableLinkResponseMsg {
    pub request_id: String,
    pub space_id: String,
    pub permit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceRequestMsg {
    pub request_id: String,
    pub space_id: String,
    pub viewer_did: String,
    pub viewer_public_key: [u8; 32],
    pub viewer_encryption_key: [u8; 32],
    pub viewer_permit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceDataMsg {
    pub request_id: String,
    pub space_id: String,
    pub delegated_permit: String,
    pub space: PublishedSpace,
    /// Full page metadata for viewer to create page shells
    pub pages: Vec<PublishedPageMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceDataAckMsg {
    pub request_id: String,
    pub space_id: String,
    pub delegated_permit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceRequestErrorMsg {
    pub request_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConsentGrantMsg {
    pub request_id: String,
    pub space_id: String,
    pub space_consent_permit: String,
    pub page_consent_permits: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConsentAckMsg {
    pub request_id: String,
    pub space_id: String,
}

/// Layer permit for a dynamic layer (Node → Viewer)
///
/// **Context**: Node detected a new dynamic layer, issues per-layer UCAN permits
/// **Viewer receives**: Stores alongside page permit, then issues layer consent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerPermitMsg {
    pub request_id: String,
    pub page_id: String,
    pub layer_name: String,
    pub permit: String,
}

/// Viewer consents to sync a specific dynamic layer (Viewer → Node)
///
/// **Context**: Viewer received LayerPermit, now consents to receive sync
/// **Node stores**: Layer consent to authorize future sync for this layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConsentGrantMsg {
    pub request_id: String,
    pub page_id: String,
    pub layer_name: String,
    pub consent_permit: String,
}

/// Node acknowledges viewer's layer consent (Node → Viewer)
///
/// **Context**: Node stored viewer's layer consent, sync can now start
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConsentAckMsg {
    pub request_id: String,
    pub page_id: String,
    pub layer_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMsg {
    pub id: Option<String>,
    pub code: ErrorCode,
    pub message: String,
}

// --- Internal Dispatch Enum ---

/// Protocol messages for peer-to-peer communication
///
/// This enum is the internal dispatch target. Serialization goes through
/// tagged format (not bincode enum).
#[derive(Debug, Clone)]
pub enum Message {
    // Handshake
    Hello(HelloMsg),
    Welcome(WelcomeMsg),
    PermitGrant(PermitGrantMsg),
    Ack,
    Rejected(RejectedMsg),

    // Sync Protocol (3-Step)
    SyncOffer(SyncOfferMsg),
    SyncAccept(SyncAcceptMsg),
    SyncAck(SyncAckMsg),
    SyncReset(SyncResetMsg),
    SyncSnapshot(SyncSnapshotMsg),

    // Assets
    AssetPrepare(AssetPrepareMsg),
    AssetReady(AssetReadyMsg),
    AssetAck(AssetAckMsg),

    // Publishing
    PublishSpace(PublishSpaceMsg),
    PublishSpaceAck(PublishSpaceAckMsg),
    PageAnnounce(PageAnnounceMsg),
    PageAnnounceAck(PageAnnounceAckMsg),
    PermitUpdate(PermitUpdateMsg),
    PublishError(PublishErrorMsg),

    // Shareable Links
    GetShareableLinkRequest(GetShareableLinkRequestMsg),
    GetShareableLinkResponse(GetShareableLinkResponseMsg),

    // Viewer Space Request
    SpaceRequest(SpaceRequestMsg),
    SpaceData(SpaceDataMsg),
    SpaceDataAck(SpaceDataAckMsg),
    SpaceRequestError(SpaceRequestErrorMsg),

    // Sync Consent
    SyncConsentGrant(SyncConsentGrantMsg),
    SyncConsentAck(SyncConsentAckMsg),

    // Layer Permits + Consent
    LayerPermit(LayerPermitMsg),
    LayerConsentGrant(LayerConsentGrantMsg),
    LayerConsentAck(LayerConsentAckMsg),

    // Errors
    Error(ErrorMsg),
}

impl Message {
    /// Get the message variant name (for logging)
    pub fn name(&self) -> &'static str {
        match self {
            Message::Hello(_) => "Hello",
            Message::Welcome(_) => "Welcome",
            Message::PermitGrant(_) => "PermitGrant",
            Message::Ack => "Ack",
            Message::Rejected(_) => "Rejected",
            Message::SyncOffer(_) => "SyncOffer",
            Message::SyncAccept(_) => "SyncAccept",
            Message::SyncAck(_) => "SyncAck",
            Message::SyncReset(_) => "SyncReset",
            Message::SyncSnapshot(_) => "SyncSnapshot",
            Message::AssetPrepare(_) => "AssetPrepare",
            Message::AssetReady(_) => "AssetReady",
            Message::AssetAck(_) => "AssetAck",
            Message::PublishSpace(_) => "PublishSpace",
            Message::PublishSpaceAck(_) => "PublishSpaceAck",
            Message::PageAnnounce(_) => "PageAnnounce",
            Message::PageAnnounceAck(_) => "PageAnnounceAck",
            Message::PermitUpdate(_) => "PermitUpdate",
            Message::PublishError(_) => "PublishError",
            Message::GetShareableLinkRequest(_) => "GetShareableLinkRequest",
            Message::GetShareableLinkResponse(_) => "GetShareableLinkResponse",
            Message::SpaceRequest(_) => "SpaceRequest",
            Message::SpaceData(_) => "SpaceData",
            Message::SpaceDataAck(_) => "SpaceDataAck",
            Message::SpaceRequestError(_) => "SpaceRequestError",
            Message::SyncConsentGrant(_) => "SyncConsentGrant",
            Message::SyncConsentAck(_) => "SyncConsentAck",
            Message::LayerPermit(_) => "LayerPermit",
            Message::LayerConsentGrant(_) => "LayerConsentGrant",
            Message::LayerConsentAck(_) => "LayerConsentAck",
            Message::Error(_) => "Error",
        }
    }

    /// Get page_id and layer_name context for trace enrichment (sync messages only)
    pub fn context(&self) -> (Option<&str>, Option<&str>) {
        match self {
            Message::SyncOffer(m) => (Some(&m.page_id), Some(&m.layer_name)),
            Message::SyncAccept(m) => (Some(&m.page_id), Some(&m.layer_name)),
            Message::SyncAck(m) => (Some(&m.page_id), Some(&m.layer_name)),
            Message::SyncReset(m) => (Some(&m.page_id), Some(&m.layer_name)),
            Message::SyncSnapshot(m) => (Some(&m.page_id), Some(&m.layer_name)),
            Message::LayerPermit(m) => (Some(&m.page_id), Some(&m.layer_name)),
            Message::LayerConsentGrant(m) => (Some(&m.page_id), Some(&m.layer_name)),
            Message::LayerConsentAck(m) => (Some(&m.page_id), Some(&m.layer_name)),
            _ => (None, None),
        }
    }

    /// Serialize message to tagged wire format: [u16 tag][bincode payload]
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        match self {
            Message::Hello(m) => encode_tagged(tags::HELLO, m),
            Message::Welcome(m) => encode_tagged(tags::WELCOME, m),
            Message::PermitGrant(m) => encode_tagged(tags::PERMIT_GRANT, m),
            Message::Ack => encode_tagged(tags::ACK, &()),
            Message::Rejected(m) => encode_tagged(tags::REJECTED, m),
            Message::SyncOffer(m) => encode_tagged(tags::SYNC_OFFER, m),
            Message::SyncAccept(m) => encode_tagged(tags::SYNC_ACCEPT, m),
            Message::SyncAck(m) => encode_tagged(tags::SYNC_ACK, m),
            Message::SyncReset(m) => encode_tagged(tags::SYNC_RESET, m),
            Message::SyncSnapshot(m) => encode_tagged(tags::SYNC_SNAPSHOT, m),
            Message::AssetPrepare(m) => encode_tagged(tags::ASSET_PREPARE, m),
            Message::AssetReady(m) => encode_tagged(tags::ASSET_READY, m),
            Message::AssetAck(m) => encode_tagged(tags::ASSET_ACK, m),
            Message::PublishSpace(m) => encode_tagged(tags::PUBLISH_SPACE, m),
            Message::PublishSpaceAck(m) => encode_tagged(tags::PUBLISH_SPACE_ACK, m),
            Message::PageAnnounce(m) => encode_tagged(tags::PAGE_ANNOUNCE, m),
            Message::PageAnnounceAck(m) => encode_tagged(tags::PAGE_ANNOUNCE_ACK, m),
            Message::PermitUpdate(m) => encode_tagged(tags::PERMIT_UPDATE, m),
            Message::PublishError(m) => encode_tagged(tags::PUBLISH_ERROR, m),
            Message::GetShareableLinkRequest(m) => encode_tagged(tags::GET_LINK_REQ, m),
            Message::GetShareableLinkResponse(m) => encode_tagged(tags::GET_LINK_RESP, m),
            Message::SpaceRequest(m) => encode_tagged(tags::SPACE_REQUEST, m),
            Message::SpaceData(m) => encode_tagged(tags::SPACE_DATA, m),
            Message::SpaceDataAck(m) => encode_tagged(tags::SPACE_DATA_ACK, m),
            Message::SpaceRequestError(m) => encode_tagged(tags::SPACE_REQUEST_ERROR, m),
            Message::SyncConsentGrant(m) => encode_tagged(tags::SYNC_CONSENT_GRANT, m),
            Message::SyncConsentAck(m) => encode_tagged(tags::SYNC_CONSENT_ACK, m),
            Message::LayerPermit(m) => encode_tagged(tags::LAYER_PERMIT, m),
            Message::LayerConsentGrant(m) => encode_tagged(tags::LAYER_CONSENT_GRANT, m),
            Message::LayerConsentAck(m) => encode_tagged(tags::LAYER_CONSENT_ACK, m),
            Message::Error(m) => encode_tagged(tags::ERROR, m),
        }
    }

    /// Deserialize message from tagged wire format
    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        let (tag, payload) = decode_tag(data)?;
        dispatch_message(tag, payload)
    }
}

/// Encode a message with its tag: [u16 big-endian tag][bincode payload]
fn encode_tagged<T: Serialize>(tag: u16, msg: &T) -> Result<Vec<u8>, bincode::Error> {
    let payload = bincode::serialize(msg)?;
    let mut buf = Vec::with_capacity(2 + payload.len());
    buf.extend_from_slice(&tag.to_be_bytes());
    buf.extend_from_slice(&payload);
    Ok(buf)
}

/// Decode tag from wire data, returning (tag, payload_slice)
fn decode_tag(data: &[u8]) -> Result<(u16, &[u8]), bincode::Error> {
    if data.len() < 2 {
        return Err(bincode::Error::from(bincode::ErrorKind::SizeLimit));
    }
    let tag = u16::from_be_bytes([data[0], data[1]]);
    Ok((tag, &data[2..]))
}

/// Dispatch tagged payload to the correct Message variant
fn dispatch_message(tag: u16, payload: &[u8]) -> Result<Message, bincode::Error> {
    match tag {
        tags::HELLO => Ok(Message::Hello(bincode::deserialize(payload)?)),
        tags::WELCOME => Ok(Message::Welcome(bincode::deserialize(payload)?)),
        tags::PERMIT_GRANT => Ok(Message::PermitGrant(bincode::deserialize(payload)?)),
        tags::ACK => {
            let _: () = bincode::deserialize(payload)?;
            Ok(Message::Ack)
        }
        tags::REJECTED => Ok(Message::Rejected(bincode::deserialize(payload)?)),
        tags::SYNC_OFFER => Ok(Message::SyncOffer(bincode::deserialize(payload)?)),
        tags::SYNC_ACCEPT => Ok(Message::SyncAccept(bincode::deserialize(payload)?)),
        tags::SYNC_ACK => Ok(Message::SyncAck(bincode::deserialize(payload)?)),
        tags::SYNC_RESET => Ok(Message::SyncReset(bincode::deserialize(payload)?)),
        tags::SYNC_SNAPSHOT => Ok(Message::SyncSnapshot(bincode::deserialize(payload)?)),
        tags::ASSET_PREPARE => Ok(Message::AssetPrepare(bincode::deserialize(payload)?)),
        tags::ASSET_READY => Ok(Message::AssetReady(bincode::deserialize(payload)?)),
        tags::ASSET_ACK => Ok(Message::AssetAck(bincode::deserialize(payload)?)),
        tags::PUBLISH_SPACE => Ok(Message::PublishSpace(bincode::deserialize(payload)?)),
        tags::PUBLISH_SPACE_ACK => Ok(Message::PublishSpaceAck(bincode::deserialize(payload)?)),
        tags::PAGE_ANNOUNCE => Ok(Message::PageAnnounce(bincode::deserialize(payload)?)),
        tags::PAGE_ANNOUNCE_ACK => Ok(Message::PageAnnounceAck(bincode::deserialize(payload)?)),
        tags::PERMIT_UPDATE => Ok(Message::PermitUpdate(bincode::deserialize(payload)?)),
        tags::PUBLISH_ERROR => Ok(Message::PublishError(bincode::deserialize(payload)?)),
        tags::GET_LINK_REQ => Ok(Message::GetShareableLinkRequest(bincode::deserialize(
            payload,
        )?)),
        tags::GET_LINK_RESP => Ok(Message::GetShareableLinkResponse(bincode::deserialize(
            payload,
        )?)),
        tags::SPACE_REQUEST => Ok(Message::SpaceRequest(bincode::deserialize(payload)?)),
        tags::SPACE_DATA => Ok(Message::SpaceData(bincode::deserialize(payload)?)),
        tags::SPACE_DATA_ACK => Ok(Message::SpaceDataAck(bincode::deserialize(payload)?)),
        tags::SPACE_REQUEST_ERROR => Ok(Message::SpaceRequestError(bincode::deserialize(payload)?)),
        tags::SYNC_CONSENT_GRANT => Ok(Message::SyncConsentGrant(bincode::deserialize(payload)?)),
        tags::SYNC_CONSENT_ACK => Ok(Message::SyncConsentAck(bincode::deserialize(payload)?)),
        tags::LAYER_PERMIT => Ok(Message::LayerPermit(bincode::deserialize(payload)?)),
        tags::LAYER_CONSENT_GRANT => Ok(Message::LayerConsentGrant(bincode::deserialize(payload)?)),
        tags::LAYER_CONSENT_ACK => Ok(Message::LayerConsentAck(bincode::deserialize(payload)?)),
        tags::ERROR => Ok(Message::Error(bincode::deserialize(payload)?)),
        _ => Err(bincode::Error::from(bincode::ErrorKind::Custom(format!(
            "Unknown message tag: 0x{:04X}",
            tag
        )))),
    }
}

// --- Publishing Types ---

/// Space metadata for publishing (transport-layer representation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedSpace {
    pub id: String,
    pub name: String,
    pub parent_space_id: Option<String>,
    pub owner_did: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Page metadata for publishing (without encrypted_key - that's local)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedPageMeta {
    pub id: String,
    pub space_id: String,
    pub name: String,
    pub owner_did: String,
    pub is_private: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Error codes for protocol errors
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    Unauthorized,
    NotFound,
    AccessDenied,
    InvalidMessage,
    Internal,
    TimestampExpired,
    PermitInvalid,
}

/// Connection string structure
///
/// This is what the node prints on startup for the owner to scan/enter.
/// Contains everything needed to establish first connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionString {
    pub node_public_key: [u8; 32],
    pub node_encryption_key: [u8; 32],
    pub device_public_key: [u8; 32],
    pub permit: String,
}

impl ConnectionString {
    pub fn new(
        node_public_key: [u8; 32],
        node_encryption_key: [u8; 32],
        device_public_key: [u8; 32],
        permit: String,
    ) -> Self {
        Self {
            node_public_key,
            node_encryption_key,
            device_public_key,
            permit,
        }
    }

    pub fn encode(&self) -> Result<String, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            json.as_bytes(),
        ))
    }

    pub fn decode(encoded: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)?;
        let json = String::from_utf8(bytes)?;
        let conn: Self = serde_json::from_str(&json)?;
        Ok(conn)
    }
}

// --- Ephemeral Datagram ---

/// Ephemeral datagram - opaque payload routed by page_id
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralDatagram {
    pub page_id: String,
    pub payload: Vec<u8>,
}

impl EphemeralDatagram {
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_tagged_roundtrip_hello() {
        let msg = Message::Hello(HelloMsg {
            protocol_version: PROTOCOL_VERSION,
            did: "did:key:test".to_string(),
            username: "alice".to_string(),
            public_key: [1u8; 32],
            encryption_key: [2u8; 32],
            signature: vec![4, 5, 6],
            timestamp: 1234567890,
            permit: "ucan_token_here".to_string(),
        });

        let bytes = msg.to_bytes().unwrap();

        // Verify tag is first 2 bytes
        assert_eq!(u16::from_be_bytes([bytes[0], bytes[1]]), tags::HELLO);

        let deserialized = Message::from_bytes(&bytes).unwrap();
        match deserialized {
            Message::Hello(h) => {
                assert_eq!(h.did, "did:key:test");
                assert_eq!(h.username, "alice");
                assert_eq!(h.protocol_version, PROTOCOL_VERSION);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_tagged_roundtrip_ack() {
        let msg = Message::Ack;
        let bytes = msg.to_bytes().unwrap();
        assert_eq!(u16::from_be_bytes([bytes[0], bytes[1]]), tags::ACK);

        let deserialized = Message::from_bytes(&bytes).unwrap();
        assert!(matches!(deserialized, Message::Ack));
    }

    #[test]
    fn test_tagged_roundtrip_sync_offer() {
        let msg = Message::SyncOffer(SyncOfferMsg {
            page_id: "page-123".to_string(),
            layer_name: "page-123/data".to_string(),
            layer_type: LayerType::Data,
            data: vec![1, 2, 3],
            state_vector: vec![4, 5],
            ephemeral_public: [6u8; 32],
            authority_permit: None,
        });

        let bytes = msg.to_bytes().unwrap();
        let deserialized = Message::from_bytes(&bytes).unwrap();
        match deserialized {
            Message::SyncOffer(s) => {
                assert_eq!(s.page_id, "page-123");
                assert_eq!(s.layer_type, LayerType::Data);
                assert_eq!(s.data, vec![1, 2, 3]);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_tagged_roundtrip_page_announce() {
        let msg = Message::PageAnnounce(PageAnnounceMsg {
            request_id: "req-1".to_string(),
            page: PublishedPageMeta {
                id: "p1".to_string(),
                space_id: "s1".to_string(),
                name: "Test Page".to_string(),
                owner_did: "did:key:owner".to_string(),
                is_private: false,
                created_at: 0,
                updated_at: 0,
            },
            page_permit: "permit-1".to_string(),
            owner_permit: "owner-permit-1".to_string(),
        });

        let bytes = msg.to_bytes().unwrap();
        assert_eq!(
            u16::from_be_bytes([bytes[0], bytes[1]]),
            tags::PAGE_ANNOUNCE
        );

        let deserialized = Message::from_bytes(&bytes).unwrap();
        match deserialized {
            Message::PageAnnounce(p) => {
                assert_eq!(p.page.id, "p1");
                assert_eq!(p.page_permit, "permit-1");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_tagged_roundtrip_permit_update() {
        let msg = Message::PermitUpdate(PermitUpdateMsg {
            permit: "new-permit".to_string(),
            scope: PermitScope::Page {
                page_id: "p1".to_string(),
            },
        });

        let bytes = msg.to_bytes().unwrap();
        let deserialized = Message::from_bytes(&bytes).unwrap();
        match deserialized {
            Message::PermitUpdate(p) => {
                assert_eq!(p.permit, "new-permit");
                match p.scope {
                    PermitScope::Page { page_id } => assert_eq!(page_id, "p1"),
                    _ => panic!("Wrong scope"),
                }
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_tagged_roundtrip_layer_permit() {
        let msg = Message::LayerPermit(LayerPermitMsg {
            request_id: "req-1".to_string(),
            page_id: "page-1".to_string(),
            layer_name: "page-1/channels/did:key:alice/general/messages".to_string(),
            permit: "layer-permit-token".to_string(),
        });

        let bytes = msg.to_bytes().unwrap();
        assert_eq!(u16::from_be_bytes([bytes[0], bytes[1]]), tags::LAYER_PERMIT);

        let deserialized = Message::from_bytes(&bytes).unwrap();
        match deserialized {
            Message::LayerPermit(m) => {
                assert_eq!(m.page_id, "page-1");
                assert_eq!(
                    m.layer_name,
                    "page-1/channels/did:key:alice/general/messages"
                );
                assert_eq!(m.permit, "layer-permit-token");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_tagged_roundtrip_layer_consent_grant() {
        let msg = Message::LayerConsentGrant(LayerConsentGrantMsg {
            request_id: "req-2".to_string(),
            page_id: "page-1".to_string(),
            layer_name: "page-1/channels/did:key:alice/general/messages".to_string(),
            consent_permit: "consent-token".to_string(),
        });

        let bytes = msg.to_bytes().unwrap();
        assert_eq!(
            u16::from_be_bytes([bytes[0], bytes[1]]),
            tags::LAYER_CONSENT_GRANT
        );

        let deserialized = Message::from_bytes(&bytes).unwrap();
        match deserialized {
            Message::LayerConsentGrant(m) => {
                assert_eq!(m.page_id, "page-1");
                assert_eq!(
                    m.layer_name,
                    "page-1/channels/did:key:alice/general/messages"
                );
                assert_eq!(m.consent_permit, "consent-token");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_tagged_roundtrip_layer_consent_ack() {
        let msg = Message::LayerConsentAck(LayerConsentAckMsg {
            request_id: "req-3".to_string(),
            page_id: "page-1".to_string(),
            layer_name: "page-1/channels/did:key:alice/general/messages".to_string(),
        });

        let bytes = msg.to_bytes().unwrap();
        assert_eq!(
            u16::from_be_bytes([bytes[0], bytes[1]]),
            tags::LAYER_CONSENT_ACK
        );

        let deserialized = Message::from_bytes(&bytes).unwrap();
        match deserialized {
            Message::LayerConsentAck(m) => {
                assert_eq!(m.page_id, "page-1");
                assert_eq!(
                    m.layer_name,
                    "page-1/channels/did:key:alice/general/messages"
                );
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_unknown_tag_returns_error() {
        let mut data = vec![0xFF, 0xFE]; // unknown tag 0xFFFE
        data.extend_from_slice(&bincode::serialize(&()).unwrap());
        assert!(Message::from_bytes(&data).is_err());
    }

    #[test]
    fn test_layer_type_from_name() {
        assert_eq!(
            LayerType::from_layer_name("page-1/app:Canvas"),
            LayerType::App
        );
        assert_eq!(
            LayerType::from_layer_name("page-1/static:logo.png"),
            LayerType::Static
        );
        assert_eq!(
            LayerType::from_layer_name("page-1/products"),
            LayerType::Data
        );
        assert_eq!(
            LayerType::from_layer_name("page-1/messages"),
            LayerType::Data
        );
        assert_eq!(LayerType::from_layer_name("app:Canvas"), LayerType::App);
    }

    #[test]
    fn test_ephemeral_datagram_serialization() {
        let datagram = EphemeralDatagram {
            page_id: "page-123".to_string(),
            payload: b"{\"type\":\"cursor\",\"x\":100.5,\"y\":200.5}".to_vec(),
        };

        let bytes = datagram.to_bytes().unwrap();
        let deserialized = EphemeralDatagram::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.page_id, "page-123");
        assert_eq!(deserialized.payload, datagram.payload);
    }

    #[test]
    fn test_message_names_are_unique() {
        let messages: Vec<Message> = vec![
            Message::Hello(HelloMsg {
                protocol_version: PROTOCOL_VERSION,
                did: "did".to_string(),
                username: "u".to_string(),
                public_key: [0; 32],
                encryption_key: [0; 32],
                signature: vec![],
                timestamp: 0,
                permit: "p".to_string(),
            }),
            Message::Welcome(WelcomeMsg {
                protocol_version: PROTOCOL_VERSION,
                node_id: "n".to_string(),
                node_public_key: [0; 32],
                node_encryption_key: [0; 32],
                signature: vec![],
                timestamp: 0,
                permit_for_peer: "p".to_string(),
            }),
            Message::PermitGrant(PermitGrantMsg {
                permit_for_node: "p".to_string(),
            }),
            Message::Ack,
            Message::Rejected(RejectedMsg {
                reason: "r".to_string(),
            }),
            Message::SyncOffer(SyncOfferMsg {
                page_id: "p".to_string(),
                layer_name: "l".to_string(),
                layer_type: LayerType::Data,
                data: vec![],
                state_vector: vec![],
                ephemeral_public: [0; 32],
                authority_permit: None,
            }),
            Message::SyncAccept(SyncAcceptMsg {
                page_id: "p".to_string(),
                layer_name: "l".to_string(),
                state_vector: vec![],
            }),
            Message::SyncAck(SyncAckMsg {
                page_id: "p".to_string(),
                layer_name: "l".to_string(),
                state_vector: vec![],
            }),
            Message::Error(ErrorMsg {
                id: None,
                code: ErrorCode::Internal,
                message: "e".to_string(),
            }),
        ];

        let names: Vec<&str> = messages.iter().map(|m| m.name()).collect();
        let unique_names: std::collections::HashSet<&str> = names.iter().cloned().collect();
        assert_eq!(
            names.len(),
            unique_names.len(),
            "Message names should be unique"
        );
    }

    // --- Property-Based Tests ---

    prop_compose! {
        fn arb_ephemeral_datagram()(
            page_id in "[a-z0-9]{8,16}",
            payload in prop::collection::vec(any::<u8>(), 0..100),
        ) -> EphemeralDatagram {
            EphemeralDatagram { page_id, payload }
        }
    }

    proptest! {
        #[test]
        fn test_ephemeral_roundtrip(datagram in arb_ephemeral_datagram()) {
            let serialized = datagram.to_bytes().unwrap();
            let deserialized = EphemeralDatagram::from_bytes(&serialized).unwrap();
            prop_assert_eq!(datagram.page_id, deserialized.page_id);
            prop_assert_eq!(datagram.payload, deserialized.payload);
        }
    }

    prop_compose! {
        fn arb_hello()(
            did in "[a-z0-9:]{10,30}",
            username in "[a-zA-Z]{3,12}",
            public_key in prop::array::uniform32(any::<u8>()),
            encryption_key in prop::array::uniform32(any::<u8>()),
            signature in prop::collection::vec(any::<u8>(), 32..64),
            timestamp in any::<i64>(),
            permit in "[a-zA-Z0-9]{20,50}",
        ) -> Message {
            Message::Hello(HelloMsg {
                protocol_version: PROTOCOL_VERSION,
                did, username, public_key, encryption_key, signature, timestamp, permit,
            })
        }
    }

    prop_compose! {
        fn arb_sync_offer()(
            page_id in "[a-z0-9]{8,16}",
            layer_name in "[a-z0-9/_]{4,20}",
            data in prop::collection::vec(any::<u8>(), 0..100),
            state_vector in prop::collection::vec(any::<u8>(), 0..50),
            ephemeral_public in prop::array::uniform32(any::<u8>()),
        ) -> Message {
            Message::SyncOffer(SyncOfferMsg {
                page_id, layer_name,
                layer_type: LayerType::Data,
                data, state_vector, ephemeral_public,
                authority_permit: None,
            })
        }
    }

    prop_compose! {
        fn arb_sync_accept()(
            page_id in "[a-z0-9]{8,16}",
            layer_name in "[a-z0-9/_]{4,20}",
            state_vector in prop::collection::vec(any::<u8>(), 0..50),
        ) -> Message {
            Message::SyncAccept(SyncAcceptMsg { page_id, layer_name, state_vector })
        }
    }

    proptest! {
        #[test]
        fn test_hello_roundtrip(msg in arb_hello()) {
            let serialized = msg.to_bytes().unwrap();
            let deserialized = Message::from_bytes(&serialized).unwrap();
            if let (Message::Hello(m1), Message::Hello(m2)) = (&msg, &deserialized) {
                prop_assert_eq!(&m1.did, &m2.did);
                prop_assert_eq!(&m1.username, &m2.username);
                prop_assert_eq!(&m1.permit, &m2.permit);
                prop_assert_eq!(&m1.public_key, &m2.public_key);
                prop_assert_eq!(&m1.encryption_key, &m2.encryption_key);
                prop_assert_eq!(&m1.timestamp, &m2.timestamp);
                prop_assert_eq!(&m1.protocol_version, &m2.protocol_version);
            } else {
                prop_assert!(false, "Deserialization changed message type");
            }
        }

        #[test]
        fn test_sync_offer_roundtrip(msg in arb_sync_offer()) {
            let serialized = msg.to_bytes().unwrap();
            let deserialized = Message::from_bytes(&serialized).unwrap();
            if let (Message::SyncOffer(m1), Message::SyncOffer(m2)) = (&msg, &deserialized) {
                prop_assert_eq!(&m1.page_id, &m2.page_id);
                prop_assert_eq!(&m1.layer_name, &m2.layer_name);
                prop_assert_eq!(&m1.layer_type, &m2.layer_type);
                prop_assert_eq!(&m1.data, &m2.data);
                prop_assert_eq!(&m1.state_vector, &m2.state_vector);
                prop_assert_eq!(&m1.ephemeral_public, &m2.ephemeral_public);
            } else {
                prop_assert!(false, "Deserialization changed message type");
            }
        }

        #[test]
        fn test_sync_accept_roundtrip(msg in arb_sync_accept()) {
            let serialized = msg.to_bytes().unwrap();
            let deserialized = Message::from_bytes(&serialized).unwrap();
            if let (Message::SyncAccept(m1), Message::SyncAccept(m2)) = (&msg, &deserialized) {
                prop_assert_eq!(&m1.page_id, &m2.page_id);
                prop_assert_eq!(&m1.layer_name, &m2.layer_name);
                prop_assert_eq!(&m1.state_vector, &m2.state_vector);
            } else {
                prop_assert!(false, "Deserialization changed message type");
            }
        }
    }
}
