//! Utility functions for the shell

use std::collections::HashSet;

use crate::NodeInfo;
use butler::{ConnectionType, SovereignNode};

/// Format timestamp as "X hours/days ago" or "Never"
pub fn format_timestamp(ts: Option<i64>) -> String {
    match ts {
        Some(millis) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            let diff_secs = (now - millis) / 1000;

            if diff_secs < 60 {
                "Just now".to_string()
            } else if diff_secs < 3600 {
                format!("{} min ago", diff_secs / 60)
            } else if diff_secs < 86400 {
                format!("{} hours ago", diff_secs / 3600)
            } else {
                format!("{} days ago", diff_secs / 86400)
            }
        }
        None => "Never".to_string(),
    }
}

/// Convert SovereignNode to Slint NodeInfo
pub fn sovereign_node_to_node_info(node: &SovereignNode) -> NodeInfo {
    NodeInfo {
        id: node.node_id.clone().into(),
        name: node.name.clone().into(),
        is_connected: node.is_connected,
        last_connected: format_timestamp(node.last_connected_at).into(),
        connection_type: match node.connection_type {
            ConnectionType::Owner => "Owner".into(),
            ConnectionType::Viewer => "Viewer".into(),
        },
        is_published: false,
    }
}

/// Convert SovereignNode to Slint NodeInfo with published state
///
/// `published_node_dids` is a set of DIDs that have published this space.
/// We check if node.did is in the set to determine is_published.
pub fn sovereign_node_to_node_info_with_published(
    node: &SovereignNode,
    published_node_dids: &HashSet<String>,
) -> NodeInfo {
    NodeInfo {
        id: node.node_id.clone().into(),
        name: node.name.clone().into(),
        is_connected: node.is_connected,
        last_connected: format_timestamp(node.last_connected_at).into(),
        connection_type: match node.connection_type {
            ConnectionType::Owner => "Owner".into(),
            ConnectionType::Viewer => "Viewer".into(),
        },
        is_published: published_node_dids.contains(&node.did),
    }
}
