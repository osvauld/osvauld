//! Ephemeral event handling for Scribe actor
//!
//! - Remote ephemeral routing (structured ephemerals)
//! - Local app event emission (PeerSubscribed, PeerUnsubscribed, Ephemeral, StructuredEphemeral)
//! - Peer broadcast via ephemeral channels (cursors, typing, etc.)

use tracing::{debug, info, instrument, warn};

use crate::message::{EphemeralOutbound, PageUpdate};
use crate::policy_compat;
use crate::state::ScribeState;

// Remote Ephemeral Routing

/// Route incoming remote ephemeral to appropriate handler
///
/// Returns true if the message was handled, false if it should be emitted as raw ephemeral
#[instrument(skip(state, payload), fields(page_id = %state.page_id))]
pub fn route_remote_ephemeral(state: &mut ScribeState, from_did: &str, payload: &[u8]) -> bool {
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(payload) else {
        debug!(page_id = %state.page_id, "route_remote_ephemeral: failed to parse JSON");
        return false;
    };

    let Some(msg_type) = json.get("type").and_then(|t| t.as_str()) else {
        debug!(page_id = %state.page_id, "route_remote_ephemeral: no 'type' field in JSON");
        return false;
    };

    debug!(page_id = %state.page_id, msg_type = %msg_type, from_did = %from_did, "route_remote_ephemeral: routing message");

    match msg_type {
        "structured" => {
            let func = json.get("func").and_then(|f| f.as_str());
            let args = json.get("args");
            // Extract original sender's DID from payload (for relayed messages)
            // Fall back to from_did parameter (the immediate peer) if not present
            let original_sender = json
                .get("from_did")
                .and_then(|d| d.as_str())
                .unwrap_or(from_did);
            if let (Some(func), Some(args)) = (func, args) {
                emit_structured_ephemeral(state, original_sender, func, args);
                debug!(page_id = %state.page_id, original_sender = %original_sender, func = %func, "Remote structured ephemeral");
                return true;
            }
            false
        }
        _ => false,
    }
}

// Local App Event Emission

/// Send a PageUpdate to all local app subscribers
///
/// **Context**: Common helper for all event emission — acquires read lock once,
/// iterates subscribers, sends update. Returns count of successful sends.
#[instrument(skip_all, fields(page_id = %state.page_id))]
pub fn emit_to_page_subscribers(state: &ScribeState, update: PageUpdate) -> usize {
    state.emit_page_update_capture(&update);
    let Ok(subs) = state.page_update_subscribers.read() else {
        warn!(page_id = %state.page_id, "Cannot emit to page subscribers - failed to acquire lock");
        return 0;
    };
    let mut sent = 0;
    for tx in subs.iter() {
        if tx.try_send(update.clone()).is_ok() {
            sent += 1;
        }
    }
    sent
}

/// Emit PeerSubscribed to local app subscribers
pub fn emit_peer_subscribed(state: &ScribeState, user_did: &str) {
    let sent = emit_to_page_subscribers(
        state,
        PageUpdate::PeerSubscribed {
            did: user_did.to_string(),
            username: None,
        },
    );
    if sent > 0 {
        info!(user_did = %user_did, "Emitted PeerSubscribed to {} subscribers", sent);
    }
}

/// Emit PeerUnsubscribed to local app subscribers
pub fn emit_peer_unsubscribed(state: &ScribeState, user_did: &str) {
    let sent = emit_to_page_subscribers(
        state,
        PageUpdate::PeerUnsubscribed {
            did: user_did.to_string(),
        },
    );
    if sent > 0 {
        info!(user_did = %user_did, "Emitted PeerUnsubscribed to {} subscribers", sent);
    }
}

/// Emit raw ephemeral to local app subscribers
pub fn emit_raw_ephemeral(state: &ScribeState, user_did: &str, device_id: &str, payload: &[u8]) {
    emit_to_page_subscribers(
        state,
        PageUpdate::Ephemeral {
            user_did: user_did.to_string(),
            device_id: device_id.to_string(),
            payload: payload.to_vec(),
        },
    );
}

/// Emit structured ephemeral to local app subscribers
///
/// **Context**: Called when a structured ephemeral is sent (locally or received from peer)
/// **Note**: Only emits to PageUpdate subscribers, not to PeerActor ephemeral channels
pub fn emit_structured_ephemeral(
    state: &ScribeState,
    from_did: &str,
    func: &str,
    args: &serde_json::Value,
) {
    let sent = emit_to_page_subscribers(
        state,
        PageUpdate::StructuredEphemeral {
            from_did: from_did.to_string(),
            func: func.to_string(),
            args: args.clone(),
        },
    );
    debug!(
        page_id = %state.page_id,
        from_did = %from_did,
        func = %func,
        subscribers = sent,
        "emit_structured_ephemeral: sent to app subscribers"
    );
}

// Remote Ephemeral Dispatch

/// Handle RemoteEphemeral message
///
/// **Context**: Remote peer sent ephemeral data via datagram.
/// **We do**: Route structured ephemerals to app subscribers, emit raw for unrecognized,
/// and relay to other subscribers (node mode).
#[instrument(skip(state, payload), fields(page_id = %state.page_id))]
pub fn handle_remote_ephemeral_message(
    state: &mut ScribeState,
    user_did: Option<&str>,
    device_id: Option<&str>,
    payload: &[u8],
) {
    let payload_str = std::str::from_utf8(payload).unwrap_or("<binary>");
    debug!(
        page_id = %state.page_id,
        user_did = ?user_did,
        device_id = ?device_id,
        payload_len = payload.len(),
        payload = %payload_str,
        our_did = %state.our_did,
        "Scribe received RemoteEphemeral"
    );

    // 1. Forward to local app layer (if we have sender info)
    if let Some(did) = user_did {
        let handled = route_remote_ephemeral(state, did, payload);
        debug!(page_id = %state.page_id, handled = handled, our_did = %state.our_did, "route_remote_ephemeral returned");
        if !handled {
            let dev_id = device_id.unwrap_or_default();
            emit_raw_ephemeral(state, did, dev_id, payload);
        }
    } else {
        debug!(page_id = %state.page_id, "RemoteEphemeral without user_did - not emitting to local app");
    }

    // 2. Relay to other subscribers (node mode relay) - exclude sender
    let exclude = match (user_did, device_id) {
        (Some(d), Some(dev)) => Some((d, dev)),
        _ => None,
    };
    broadcast_ephemeral_to_subscribers(state, payload, exclude);
}

// Structured Ephemeral Handlers

/// Handle SendStructuredEphemeral message
///
/// **Context**: Local Lua app wants to send a structured ephemeral (e.g., typing indicator).
/// **We do**: Validate func against our permit's ephemeral_funcs, emit to local app subscribers,
/// and broadcast to remote peers via ephemeral channels.
///
/// **Returns**: true if the ephemeral was sent, false if rejected by permit.
#[instrument(skip(state, args), fields(page_id = %state.page_id, func = %func))]
pub fn handle_send_structured_ephemeral(
    state: &ScribeState,
    func: &str,
    args: &serde_json::Value,
) -> bool {
    // Log the incoming request
    let has_permit = state.our_permit.is_some();
    let ephemeral_funcs = state
        .our_permit
        .as_ref()
        .map(policy_compat::ephemeral_funcs)
        .unwrap_or_default();
    debug!(
        page_id = %state.page_id,
        func = %func,
        has_permit = has_permit,
        ephemeral_funcs = ?ephemeral_funcs,
        our_did = %state.our_did,
        "SendStructuredEphemeral received"
    );

    // Validate func against our permit's ephemeral_funcs (if set)
    // Empty ephemeral_funcs list means all funcs allowed
    let can_send = state
        .our_permit
        .as_ref()
        .map(|p| policy_compat::can_send_ephemeral(p, func))
        .unwrap_or(true); // No permit = allow (local mode)

    if !can_send {
        warn!(page_id = %state.page_id, func = %func, "Structured ephemeral func not allowed by permit");
        return false;
    }

    // Broadcast as StructuredEphemeral to app subscribers (local UI)
    let our_did = state.our_did.clone();
    emit_structured_ephemeral(state, &our_did, func, args);

    // Also broadcast via ephemeral channel for remote peers
    // PeerActor should be subscribed to this Scribe with ephemeral_tx
    // Include from_did so relayed messages preserve original sender identity
    let payload = serde_json::json!({
        "type": "structured",
        "from_did": our_did,
        "func": func,
        "args": args
    });
    if let Ok(bytes) = serde_json::to_vec(&payload) {
        debug!(
            page_id = %state.page_id,
            func = %func,
            "Broadcasting structured ephemeral to PeerActor subscribers"
        );
        broadcast_ephemeral_to_subscribers(state, &bytes, None);
    }

    true
}

/// Handle RemoteStructuredEphemeral message
///
/// **Context**: Remote peer sent a structured ephemeral via relay.
/// **We do**: Validate func against sender's permit, then emit to local app subscribers.
///
/// **Returns**: true if the ephemeral was accepted, false if rejected.
#[instrument(skip(state, args), fields(page_id = %state.page_id, from_did = %from_did, func = %func))]
pub fn handle_remote_structured_ephemeral(
    state: &ScribeState,
    from_did: &str,
    device_id: &str,
    func: &str,
    args: &serde_json::Value,
) -> bool {
    // Validate func against sender's permit
    let can_send = {
        let subs = state.subscribers.read().ok();
        subs.and_then(|s| {
            s.get(&(from_did.to_string(), device_id.to_string()))
                .map(|info| policy_compat::can_send_ephemeral(&info.permit, func))
        })
        .unwrap_or(false) // Unknown sender = reject
    };

    if !can_send {
        warn!(
            page_id = %state.page_id,
            from_did = %from_did,
            func = %func,
            "Remote structured ephemeral rejected - func not allowed"
        );
        return false;
    }

    // Emit to local app subscribers
    emit_structured_ephemeral(state, from_did, func, args);

    debug!(
        page_id = %state.page_id,
        from_did = %from_did,
        func = %func,
        "Processed remote structured ephemeral"
    );

    true
}

// Peer Broadcast via Ephemeral Channels

/// Broadcast ephemeral data to all subscribed PeerActors (via their ephemeral channels)
///
/// **Context**: Called for:
/// - SendEphemeral from Lua (local user action) - no exclusion
/// - RemoteEphemeral (relay to other peers) - exclude sender
///
/// **exclude_key**: (user_did, device_id) to skip (the sender for relay)
#[instrument(skip(state, payload), fields(page_id = %state.page_id, payload_len = payload.len()))]
pub fn broadcast_ephemeral_to_subscribers(
    state: &ScribeState,
    payload: &[u8],
    exclude_key: Option<(&str, &str)>,
) {
    if let Ok(subscribers) = state.subscribers.read() {
        let total_subs = subscribers.len();
        let mut sent_count = 0;
        let mut skipped_no_channel = 0;
        let mut skipped_excluded = 0;

        for ((did, device), info) in subscribers.iter() {
            // Skip sender (for relay - don't echo back)
            if let Some((ex_did, ex_device)) = exclude_key {
                if did == ex_did && device == ex_device {
                    skipped_excluded += 1;
                    continue;
                }
            }
            // Send via ephemeral channel if available
            if let Some(ref tx) = info.ephemeral_tx {
                match tx.try_send(EphemeralOutbound {
                    page_id: state.page_id.clone(),
                    payload: payload.to_vec(),
                }) {
                    Ok(()) => sent_count += 1,
                    Err(e) => {
                        debug!(
                            page_id = %state.page_id,
                            did = %did,
                            error = %e,
                            "Failed to send ephemeral to subscriber"
                        );
                    }
                }
            } else {
                skipped_no_channel += 1;
            }
        }

        debug!(
            page_id = %state.page_id,
            total_subscribers = total_subs,
            sent = sent_count,
            skipped_no_channel = skipped_no_channel,
            skipped_excluded = skipped_excluded,
            payload_len = payload.len(),
            "Ephemeral broadcast to PeerActors complete"
        );
    } else {
        warn!(page_id = %state.page_id, "Failed to acquire subscribers lock for ephemeral broadcast");
    }
}
