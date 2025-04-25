use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{AsyncTransact, Doc, ReadTxn, StateVector, Update};
/// Get the state vector from a full Yjs document state
///
/// # Arguments
/// * `full_yjs_state` - The complete Yjs state as a byte array
///
/// # Returns
/// * `Result<Vec<u8>, String>` - Encoded state vector, or an error
pub async fn get_state_vector(full_yjs_state: &[u8]) -> Result<Vec<u8>, String> {
    // Create a temporary document
    let doc = Doc::new();

    // Apply the full state to the document
    if !full_yjs_state.is_empty() {
        // Try to determine if this is a v1 or v2 encoded update
        let update = match Update::decode_v1(full_yjs_state) {
            Ok(update) => update,
            Err(_) => {
                // Try v2 if v1 fails
                Update::decode_v2(full_yjs_state).map_err(|e| {
                    format!(
                        "Failed to decode document state using either v1 or v2: {:?}",
                        e
                    )
                })?
            }
        };

        let mut txn = doc.transact_mut().await;
        txn.apply_update(update)
            .map_err(|e| format!("Failed to apply update: {:?}", e))?;
    }

    // Extract the state vector, and encode using v2 to match the JS implementation
    // Using v1 because that's what the JS side uses with Y.encodeStateAsUpdate()
    let txn = doc.transact().await;
    let sv = txn.state_vector();

    Ok(sv.encode_v1())
}

/// Generate updates that need to be applied on a remote peer
///
/// # Arguments
/// * `full_yjs_state` - The complete Yjs state as a byte array
/// * `peer_state_vector` - The state vector from the peer indicating what they already have
///
/// # Returns
/// * `Result<Vec<u8>, String>` - Binary updates the peer needs, or an error
pub async fn generate_updates_for_peer(
    full_yjs_state: &[u8],
    peer_state_vector: &[u8],
) -> Result<Vec<u8>, String> {
    // Create a temporary document
    let doc = Doc::new();

    // Apply the full state to the document
    if !full_yjs_state.is_empty() {
        // Try to determine if this is a v1 or v2 encoded update
        let update = match Update::decode_v1(full_yjs_state) {
            Ok(update) => update,
            Err(_) => {
                // Try v2 if v1 fails
                Update::decode_v2(full_yjs_state).map_err(|e| {
                    format!(
                        "Failed to decode document state using either v1 or v2: {:?}",
                        e
                    )
                })?
            }
        };

        let mut txn = doc.transact_mut().await;
        txn.apply_update(update)
            .map_err(|e| format!("Failed to apply update: {:?}", e))?;
    }

    // Parse the peer's state vector - try both formats
    let sv = match StateVector::decode_v2(peer_state_vector) {
        Ok(sv) => sv,
        Err(_) => {
            // Try v1 if v2 fails
            StateVector::decode_v1(peer_state_vector).map_err(|e| {
                format!(
                    "Failed to decode peer state vector using either v1 or v2: {:?}",
                    e
                )
            })?
        }
    };

    // Generate only the diff the peer needs based on their state vector
    let txn = doc.transact().await;
    Ok(txn.encode_diff_v1(&sv))
}

/// Apply updates to a document and generate only the updates needed by peer
///
/// # Arguments
/// * `full_yjs_state` - The complete local Yjs state as a byte array
/// * `received_updates` - Updates received from the peer to apply
/// * `peer_state_vector` - State vector from the peer indicating what they have
///
/// # Returns
/// * `Result<Vec<u8>, String>` - Updates needed by the peer
pub async fn apply_updates_and_generate_peer_updates(
    full_yjs_state: &[u8],
    received_updates: &[u8],
    peer_state_vector: &[u8],
) -> Result<Vec<u8>, String> {
    // Create a temporary document
    let doc = Doc::new();

    // Apply the full state to the document if it exists
    if !full_yjs_state.is_empty() {
        // Try to determine if this is a v1 or v2 encoded update
        let update = match Update::decode_v1(full_yjs_state) {
            Ok(update) => update,
            Err(_) => {
                // Try v2 if v1 fails
                Update::decode_v2(full_yjs_state)
                    .map_err(|e| format!("Failed to decode document state: {:?}", e))?
            }
        };

        let mut txn = doc.transact_mut().await;
        txn.apply_update(update)
            .map_err(|e| format!("Failed to apply document state: {:?}", e))?;
    }

    // Apply the received updates to the document
    if !received_updates.is_empty() {
        // Try to determine if this is a v1 or v2 encoded update
        let update = match Update::decode_v1(received_updates) {
            Ok(update) => update,
            Err(_) => {
                // Try v2 if v1 fails
                Update::decode_v2(received_updates)
                    .map_err(|e| format!("Failed to decode received updates: {:?}", e))?
            }
        };

        let mut txn = doc.transact_mut().await;
        txn.apply_update(update)
            .map_err(|e| format!("Failed to apply received updates: {:?}", e))?;
    }

    // Parse the peer's state vector
    let sv = match StateVector::decode_v2(peer_state_vector) {
        Ok(sv) => sv,
        Err(_) => {
            // Try v1 if v2 fails
            StateVector::decode_v1(peer_state_vector)
                .map_err(|e| format!("Failed to decode peer state vector: {:?}", e))?
        }
    };

    // Get a transaction for reading
    let txn = doc.transact().await;

    // Generate only the updates the peer needs based on their state vector
    let updates_for_peer = txn.encode_diff_v1(&sv);

    Ok(updates_for_peer)
}
