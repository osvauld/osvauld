//! Validation handle for remote validation service
//!
//! **Context**: Enables Scribe to delegate validation to external service (kunki)
//! **Pattern**: Channel-based RPC - send request, await reply via oneshot
//! **Why**: Allows validation.lua to run with presence_lib in kunki's LuaRuntime

use tokio::sync::{mpsc, oneshot};
use tracing::instrument;

use domains::JsonOp;

/// Validation request sent to validation service
#[derive(Debug)]
pub struct ValidationRequest {
    pub page_id: String,
    pub layer_name: String,
    pub ops: Vec<JsonOp>,
    pub from_did: String,
    pub role: String,
    pub reply: oneshot::Sender<Result<(bool, Option<String>), String>>,
}

/// Handle for sending validation requests to external service
///
/// **Context**: Scribe holds this handle to delegate validation to kunki
/// **Pattern**: mpsc channel for requests, oneshot for replies
/// **Fallback**: If service unavailable (viewer/owner mode), validation can be skipped
#[derive(Clone)]
pub struct ValidationHandle {
    tx: mpsc::Sender<ValidationRequest>,
}

impl ValidationHandle {
    /// Create a new ValidationHandle from a channel sender
    ///
    /// **Context**: Created by validation service (kunki) and passed to Scribe
    pub fn new(tx: mpsc::Sender<ValidationRequest>) -> Self {
        Self { tx }
    }

    /// Validate operations via external service
    ///
    /// **Context**: Called by Scribe when remote update arrives
    /// **Returns**: (passed, error_message) - passed=true if validation succeeded
    /// **Timeout**: Handled by caller (Scribe adds 5s timeout)
    #[instrument(skip(self, ops))]
    pub async fn validate_ops(
        &self,
        page_id: &str,
        layer_name: &str,
        ops: &[JsonOp],
        from_did: &str,
        role: &str,
    ) -> Result<(bool, Option<String>), String> {
        let (reply_tx, reply_rx) = oneshot::channel();

        let request = ValidationRequest {
            page_id: page_id.to_string(),
            layer_name: layer_name.to_string(),
            ops: ops.to_vec(),
            from_did: from_did.to_string(),
            role: role.to_string(),
            reply: reply_tx,
        };

        self.tx
            .send(request)
            .await
            .map_err(|_| "ValidationService not running".to_string())?;

        reply_rx
            .await
            .map_err(|_| "ValidationService dropped request".to_string())?
    }

    /// Get the sender for creating additional handles
    pub fn sender(&self) -> mpsc::Sender<ValidationRequest> {
        self.tx.clone()
    }
}
