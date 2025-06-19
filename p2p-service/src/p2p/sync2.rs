use crate::p2p::peer_connection::PeerConnection;

use osvauld_core::models::DeviceManifestRequestPayload;

use tracing::{debug, error, info, instrument, Span};

// Helper method signatures to reduce repeated patterns
impl PeerConnection {
    pub async fn handle_manifest_request_payload(
        payload: &DeviceManifestRequestPayload,
    ) -> Result<(), String> {
        todo!()
    }
}
