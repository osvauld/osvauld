//! Reusable guards and validation helpers for peer_actor handlers

use crate::coordinator::CourierMode;
use crate::state::PeerState;

/// Check mode and return error message if mismatched
pub fn require_mode(actual: CourierMode, expected: CourierMode, action: &str) -> Option<&'static str> {
    if actual != expected {
        tracing::warn!("{} called in {:?} mode, expected {:?}", action, actual, expected);
        return Some("Wrong mode");
    }
    None
}

/// Check if User mode, returns error message if not
pub fn require_user_mode(actual: CourierMode, action: &str) -> Option<&'static str> {
    require_mode(actual, CourierMode::User, action)
}

/// Check if Node mode, returns error message if not
pub fn require_node_mode(actual: CourierMode, action: &str) -> Option<&'static str> {
    require_mode(actual, CourierMode::Node, action)
}

/// Check authenticated and return peer info (did, username)
pub fn require_auth<'a>(state: &'a PeerState) -> Result<(&'a str, &'a str), &'static str> {
    match state {
        PeerState::Authenticated { did, username, .. } => Ok((did, username)),
        _ => {
            tracing::warn!("Action requires authentication, current state: {}", state.name());
            Err("Not authenticated")
        }
    }
}

/// Parse and validate permit
pub fn parse_permit(token: &str) -> Result<gurkha::Permit, String> {
    gurkha::Permit::from_token(token).map_err(|e| format!("Invalid permit: {:?}", e))
}

/// Convert butler::Space → message::PublishedSpace
pub fn to_published_space(space: &butler::Space) -> crate::message::PublishedSpace {
    crate::message::PublishedSpace {
        id: space.id.clone(),
        name: space.name.clone(),
        parent_space_id: space.parent_space_id.clone(),
        owner_did: space.owner_did.clone(),
        description: space.description.clone(),
        created_at: space.created_at,
        updated_at: space.updated_at,
    }
}

/// Convert butler::PageMeta → message::PublishedPageMeta
pub fn to_published_page_meta(meta: &butler::PageMeta) -> crate::message::PublishedPageMeta {
    crate::message::PublishedPageMeta {
        id: meta.id.clone(),
        space_id: meta.space_id.clone(),
        name: meta.name.clone(),
        owner_did: meta.owner_did.clone(),
        is_private: meta.is_private,
        created_at: meta.created_at,
        updated_at: meta.updated_at,
    }
}

/// Convert message::PublishedSpace → butler::Space
pub fn from_published_space(
    space: &crate::message::PublishedSpace,
) -> butler::Space {
    butler::Space {
        id: space.id.clone(),
        name: space.name.clone(),
        parent_space_id: space.parent_space_id.clone(),
        owner_did: space.owner_did.clone(),
        is_default: false,
        description: space.description.clone(),
        created_at: space.created_at,
        updated_at: space.updated_at,
    }
}

/// Convert message::PublishedPageMeta → butler::PageMeta
pub fn from_published_page_meta(meta: &crate::message::PublishedPageMeta) -> butler::PageMeta {
    butler::PageMeta {
        id: meta.id.clone(),
        space_id: meta.space_id.clone(),
        name: meta.name.clone(),
        encrypted_key: Vec::new(),
        owner_did: meta.owner_did.clone(),
        is_private: meta.is_private,
        created_at: meta.created_at,
        updated_at: meta.updated_at,
    }
}
