//! Validation service for kunki node runtime
//!
//! **Context**: Runs validation.lua using LuaRuntime with presence_lib access
//! **Pattern**: Scribe sends ValidationRequest → kunki validates → reply with result

use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info};

use butler::{Butler, JsonOp, ScribeMessage, ValidationHandle, ValidationRequest};
use lua_runtime::{LuaCommand, LuaRuntimeConfig, LuaRuntime, ActorScribeHandle, ValidationContext};

/// Validation service - spawns LuaRuntime per validation request
///
/// **Context**: Runs in kunki node runtime, spawned when Butler is created
/// **Pattern**: Single async task processes validation requests sequentially
pub struct ValidationService {
    butler: Arc<Butler>,
}

impl ValidationService {
    /// Create validation service
    pub fn new(butler: Arc<Butler>) -> Self {
        Self { butler }
    }

    /// Spawn validation service and return handle
    ///
    /// **Context**: Called during kunki startup
    /// **Returns**: ValidationHandle that Scribe uses to send requests
    pub fn spawn(self) -> ValidationHandle {
        let (tx, mut rx) = mpsc::channel::<ValidationRequest>(32);

        let butler = self.butler.clone();

        tokio::spawn(async move {
            info!("ValidationService started");

            while let Some(request) = rx.recv().await {
                let result = Self::handle_request(&butler, request).await;
                drop(result);
            }

            info!("ValidationService stopped");
        });

        ValidationHandle::new(tx)
    }

    /// Handle a single validation request
    async fn handle_request(
        butler: &Arc<Butler>,
        request: ValidationRequest,
    ) -> Result<(), String> {
        let ValidationRequest {
            page_id,
            layer_name,
            ops,
            from_did,
            role,
            reply,
        } = request;

        debug!(
            page_id = %page_id,
            layer_name = %layer_name,
            from_did = %from_did,
            role = %role,
            op_count = ops.len(),
            "Validation request received"
        );

        let result = Self::validate(butler, &page_id, &layer_name, &ops, &from_did, &role).await;

        let _ = reply.send(result);

        // Close page after validation to prevent memory leak
        if let Err(e) = butler.close_page(&page_id).await {
            debug!(page_id = %page_id, error = %e, "Failed to close page after validation (may already be closed)");
        }

        Ok(())
    }

    /// Validate operations by spawning a LuaRuntime with validation.lua
    ///
    /// **Context**: Opens page, loads validation code, spawns LuaRuntime, sends Validate command
    /// **Returns**: (passed, error_message)
    async fn validate(
        butler: &Arc<Butler>,
        page_id: &str,
        layer_name: &str,
        ops: &[JsonOp],
        from_did: &str,
        role: &str,
    ) -> Result<(bool, Option<String>), String> {
        // Open page to get scribe reference
        let scribe_ref = butler.open_page(page_id).await
            .map_err(|e| format!("Failed to open page: {}", e))?;

        // Get identity info
        let identity = butler.get_identity().await
            .map_err(|e| format!("Failed to get identity: {}", e))?;

        let identity_data = butler.identity_data()
            .ok()
            .flatten()
            .ok_or_else(|| "Identity data not found".to_string())?;

        // Load validation.lua
        let validation_code = Self::get_validation_code(butler, &scribe_ref, page_id).await?;

        let validation_code = match validation_code {
            Some(code) => code,
            None => {
                debug!(page_id = %page_id, "No validation.lua found, allowing ops");
                return Ok((true, None));
            }
        };

        // Spawn LuaRuntime with validation code
        let (thread, cmd_tx) = LuaRuntime::spawn(LuaRuntimeConfig {
            page_id: page_id.to_string(),
            app_name: "validation".to_string(),
            scribe: ActorScribeHandle::new(scribe_ref),
            user_did: identity.did().to_string(),
            user_name: identity_data.username.clone(),
            user_role: "node".to_string(),
            lua_code: validation_code,
            ui_enabled: false,
            ui_tx: None,
            query_tx: None,
            navigate_tx: None,
            clock: std::sync::Arc::new(domains::RealClock),
        })?;

        // Build ValidationContext from JsonOps
        let ctx = ValidationContext {
            layer_name: layer_name.to_string(),
            ops: ops.iter()
                .map(|op| serde_json::to_value(op)
                    .map(|v| serde_json::from_value(v).unwrap())
                )
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to serialize ops: {}", e))?,
            from_did: from_did.to_string(),
            role: role.to_string(),
            page_id: page_id.to_string(),
        };

        // Send Validate command and await result
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        cmd_tx.send(LuaCommand::Validate { ctx, response_tx }).await
            .map_err(|_| "Failed to send Validate command")?;

        let validation_result = response_rx.await
            .map_err(|_| "Validation response channel dropped")?;

        // Shutdown the runtime
        let _ = cmd_tx.send(LuaCommand::Shutdown).await;
        let _ = thread.join();

        // Convert ValidationResult to (bool, Option<String>)
        match validation_result {
            Ok(result) => Ok((result.valid, result.error)),
            Err(e) => Err(e),
        }
    }

    /// Get validation.lua code for a page
    ///
    /// **Context**: Reads app files from Scribe to find shared/validation.lua
    /// **Returns**: Some(code) if validation.lua exists, None otherwise
    async fn get_validation_code(
        butler: &Arc<Butler>,
        scribe_ref: &ractor::ActorRef<ScribeMessage>,
        page_id: &str,
    ) -> Result<Option<String>, String> {
        let apps = butler.apps().list(page_id)
            .map_err(|e| format!("Failed to list apps: {}", e))?;

        if apps.is_empty() {
            return Ok(None);
        }

        for app_name in apps {
            let (tx, rx) = tokio::sync::oneshot::channel();
            scribe_ref.cast(ScribeMessage::GetAppFiles {
                app_name: app_name.clone(),
                reply: tx,
            }).map_err(|e| format!("Failed to request app files: {}", e))?;

            let files = rx.await
                .map_err(|_| "Failed to receive app files")?
                .map_err(|e| format!("Failed to get app files: {}", e))?;

            if let Some(validation_code) = files.get("shared/validation.lua") {
                info!(page_id = %page_id, app_name = %app_name, "Found validation.lua");
                return Ok(Some(validation_code.clone()));
            }
        }

        Ok(None)
    }
}
