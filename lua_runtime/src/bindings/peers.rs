//! Lua peers bindings
//!
//! Provides peers:count() to check how many peers are subscribed to this page.
//! Useful for apps to decide when to broadcast (avoid ephemeral spam).

use mlua::{UserData, UserDataMethods};
use ractor::ActorRef;
use tracing::debug;

use butler::ScribeMessage;

use super::block_on_async;

/// Peers bindings for Lua
///
/// **Context**: Scripts need to check if anyone is listening before broadcasting.
/// **Usage**: `if peers:count() > 0 then broadcast_position() end`
pub struct PeersBindings {
    scribe_ref: ActorRef<ScribeMessage>,
}

impl PeersBindings {
    pub fn new(scribe_ref: ActorRef<ScribeMessage>) -> Self {
        Self { scribe_ref }
    }
}

impl UserData for PeersBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // peers:count() -> number
        // Returns the number of subscribers to this page's Scribe
        methods.add_method("count", |_, this, ()| {
            let (tx, rx) = tokio::sync::oneshot::channel();

            this.scribe_ref
                .cast(ScribeMessage::GetSubscriberCount { reply: tx })
                .map_err(|e| {
                    mlua::Error::RuntimeError(format!("Failed to send GetSubscriberCount: {}", e))
                })?;

            let count = block_on_async(async { rx.await.unwrap_or(0) })?;

            debug!(count = count, "peers:count() returned");
            Ok(count)
        });
    }
}
