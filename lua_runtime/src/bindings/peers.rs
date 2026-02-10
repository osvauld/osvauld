//! Lua peers bindings
//!
//! Provides peers:count() to check how many peers are subscribed to this page.
//! Useful for apps to decide when to broadcast (avoid ephemeral spam).

use mlua::{UserData, UserDataMethods};
use std::sync::Arc;
use tracing::debug;

use crate::scribe_handle::ScribeHandle;

/// Peers bindings for Lua
///
/// **Context**: Scripts need to check if anyone is listening before broadcasting.
/// **Usage**: `if peers:count() > 0 then broadcast_position() end`
pub struct PeersBindings {
    scribe: Arc<dyn ScribeHandle>,
}

impl PeersBindings {
    pub fn new(scribe: Arc<dyn ScribeHandle>) -> Self {
        Self { scribe }
    }
}

impl UserData for PeersBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // peers:count() -> number
        // Returns the number of subscribers to this page's Scribe
        methods.add_method("count", |_, this, ()| {
            let count = this.scribe.get_subscriber_count();

            debug!(count = count, "peers:count() returned");
            Ok(count)
        });
    }
}
