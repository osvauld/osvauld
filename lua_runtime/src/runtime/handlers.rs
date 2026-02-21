use mlua::{Function, Table, Value};
use serde_json::Value as JsonValue;
use tracing::{debug, info, warn};

use butler::DynamicLayerMeta;

use crate::commands::UiEventType;
use crate::ui_types::UiMutation;

use super::LuaRuntime;

impl LuaRuntime {
    fn call_cached_handler<A: mlua::IntoLuaMulti>(&self, enabled: bool, name: &str, args: A) {
        if !enabled {
            return;
        }

        if let Err(e) = self.call_handler(name, args) {
            warn!(page_id = %self.page_id, handler = %name, error = %e, "Lua handler error");
        }
    }

    /// Handle layer discovered.
    ///
    /// Context: new layer arriving via sync (e.g., derived/orders_summary first appearance)
    /// We do: process bindings for the discovered layer, then call Lua callback.
    pub(super) fn handle_layer_discovered(
        &self,
        layer_name: &str,
        dynamic_ref: Option<&DynamicLayerMeta>,
    ) -> Result<(), String> {
        if self.ui_enabled {
            let normalized_layer = self.normalize_layer_name_for_lookup(layer_name);
            let has_bindings = {
                let manager = self.binding_manager.lock();
                !manager.get_bindings_for_layer(&normalized_layer).is_empty()
            };

            if has_bindings {
                if let Ok(data) = self.fetch_layer_data(layer_name) {
                    self.process_bindings(layer_name, Some(&data), None);
                }
            }
        }

        if self.handler_cache.on_layer_discovered {
            let mut args = mlua::MultiValue::new();
            args.push_back(Value::String(
                self.lua
                    .create_string(layer_name)
                    .map_err(|e| format!("Lua string error: {}", e))?,
            ));

            match dynamic_ref {
                Some(meta) => {
                    let meta_table = self
                        .lua
                        .create_table()
                        .map_err(|e| format!("Lua table error: {}", e))?;
                    meta_table
                        .set("schema_key", meta.schema_key.clone())
                        .map_err(|e| format!("Lua table set error: {}", e))?;
                    if let Some(creator_did) = &meta.creator_did {
                        meta_table
                            .set("creator_did", creator_did.clone())
                            .map_err(|e| format!("Lua table set error: {}", e))?;
                    } else {
                        meta_table
                            .set("creator_did", Value::Nil)
                            .map_err(|e| format!("Lua table set error: {}", e))?;
                    }

                    let placeholders = self
                        .lua
                        .create_table()
                        .map_err(|e| format!("Lua table error: {}", e))?;
                    for (k, v) in &meta.placeholders {
                        placeholders
                            .set(k.as_str(), v.as_str())
                            .map_err(|e| format!("Lua table set error: {}", e))?;
                    }
                    meta_table
                        .set("placeholders", placeholders)
                        .map_err(|e| format!("Lua table set error: {}", e))?;

                    args.push_back(Value::Table(meta_table));
                }
                None => args.push_back(Value::Nil),
            }

            if let Err(e) = self.call_handler("on_layer_discovered", args) {
                warn!(page_id = %self.page_id, layer = %layer_name, error = %e, "on_layer_discovered error");
            }
        }

        self.trigger_derivation(layer_name);

        Ok(())
    }

    /// Fetch layer data as JSON from Scribe.
    pub(super) fn fetch_layer_data(&self, layer_name: &str) -> Result<JsonValue, String> {
        match self.scribe.get_layer_sthithi(layer_name)? {
            Some(data) => Ok(JsonValue::from(&data)),
            None => Ok(JsonValue::Array(vec![])),
        }
    }

    /// Fire due timers.
    pub(super) fn fire_due_timers(&self) {
        let fired_ids = {
            let mut scheduler = self.scheduler.lock();
            scheduler.fire_due_timers()
        };

        for timer_id in fired_ids {
            self.fire_timer(timer_id);
        }
    }

    /// Fire a single timer callback.
    pub(super) fn fire_timer(&self, timer_id: u64) {
        let timers: Result<Table, _> = self.lua.globals().get("_timers");
        if let Ok(timers) = timers {
            let callback: Result<Function, _> = timers.get(timer_id);
            if let Ok(func) = callback {
                if let Err(e) = func.call::<()>(()) {
                    warn!(timer_id, error = %e, "Timer callback error");
                }
            }

            let is_one_shot = {
                let scheduler = self.scheduler.lock();
                !scheduler.timers.contains_key(&timer_id)
            };
            if is_one_shot {
                let _ = timers.set(timer_id, Value::Nil);
            }
        }
    }

    /// Handle ephemeral message.
    pub(super) fn handle_ephemeral(&self, user_did: &str, payload: &[u8]) {
        if self.handler_cache.on_ephemeral {
            let payload_str = String::from_utf8_lossy(payload);
            if let Err(e) = self.call_handler("on_ephemeral", (user_did, payload_str.to_string())) {
                warn!(page_id = %self.page_id, user = %user_did, error = %e, "on_ephemeral error");
            }
        }
    }

    /// Handle structured ephemeral message.
    pub(super) fn handle_structured_ephemeral(
        &self,
        from_did: &str,
        func_name: &str,
        args: &JsonValue,
    ) {
        debug!(page_id = %self.page_id, from_did = %from_did, func = %func_name, "handle_structured_ephemeral: received");

        if self.handler_cache.on_ephemeral {
            let lua_args = match crate::bindings::convert::json_to_lua(&self.lua, args) {
                Ok(v) => v,
                Err(e) => {
                    warn!(page_id = %self.page_id, error = %e, "Args conversion error");
                    return;
                }
            };

            if let Err(e) = self.call_handler("on_ephemeral", (from_did, func_name, lua_args)) {
                warn!(page_id = %self.page_id, from = %from_did, func = %func_name, error = %e, "on_ephemeral error");
            } else {
                debug!(page_id = %self.page_id, from_did = %from_did, func = %func_name, "on_ephemeral called successfully");
            }
        } else {
            debug!(page_id = %self.page_id, "on_ephemeral function not defined");
        }
    }

    /// Handle peer joined.
    pub(super) fn handle_peer_joined(&self, user_did: &str) {
        if self.handler_cache.on_peer_joined {
            self.call_cached_handler(true, "on_peer_joined", user_did.to_string());
            info!(page_id = %self.page_id, user = %user_did, "on_peer_joined executed");
        }
    }

    /// Handle peer left.
    pub(super) fn handle_peer_left(&self, user_did: &str) {
        if self.handler_cache.on_peer_left {
            self.call_cached_handler(true, "on_peer_left", user_did.to_string());
            info!(page_id = %self.page_id, user = %user_did, "on_peer_left executed");
        }
    }

    /// Handle asset uploaded.
    pub(super) fn handle_asset_uploaded(
        &self,
        hash: &str,
        filename: &str,
        mime_type: &str,
        size: u64,
    ) {
        if self.handler_cache.on_asset_uploaded {
            let table = match self.lua.create_table() {
                Ok(t) => t,
                Err(e) => {
                    warn!(page_id = %self.page_id, error = %e, "Failed to create table");
                    return;
                }
            };

            let _ = table.set("hash", hash);
            let _ = table.set("filename", filename);
            let _ = table.set("mime_type", mime_type);
            let _ = table.set("size", size);

            if let Err(e) = self.call_handler("on_asset_uploaded", table) {
                warn!(page_id = %self.page_id, hash = %hash, error = %e, "on_asset_uploaded error");
            } else {
                info!(page_id = %self.page_id, hash = %hash, filename = %filename, "on_asset_uploaded executed");
            }
        }
    }

    /// Handle UI callback.
    pub(super) fn handle_ui_callback(
        &self,
        name: &str,
        args: Vec<JsonValue>,
    ) -> Result<(), String> {
        if !self.has_function(name) {
            debug!(page_id = %self.page_id, callback = %name, "No handler found");
            return Ok(());
        }

        let lua_args: Vec<Value> = args
            .iter()
            .filter_map(|a| crate::bindings::convert::json_to_lua(&self.lua, a).ok())
            .collect();

        if let Err(e) = self.call_handler(name, mlua::MultiValue::from_iter(lua_args)) {
            warn!(page_id = %self.page_id, callback = %name, error = %e, "UI callback error");
        }

        Ok(())
    }

    /// Handle UI event.
    pub(super) fn handle_ui_event(&self, event: UiEventType) -> Result<(), String> {
        match event {
            UiEventType::KeyPressed { key } => {
                self.call_cached_handler(self.handler_cache.on_key_pressed, "on_key_pressed", key);
            }
            UiEventType::TextChanged { element: _, text } => {
                self.call_cached_handler(self.handler_cache.on_text_input, "on_text_input", text);
            }
        }
        Ok(())
    }

    /// Flush accumulated UI mutations.
    pub(super) fn flush_mutations(&self) {
        if !self.ui_enabled {
            return;
        }

        let mut shared = self.ui_shared.lock();
        let properties = std::mem::take(&mut shared.pending_properties);
        let model_ops = std::mem::take(&mut shared.pending_model_ops);
        drop(shared);

        if properties.is_empty() && model_ops.is_empty() {
            return;
        }

        let mutation = UiMutation {
            app_id: self.page_id.clone(),
            properties,
            model_ops,
        };

        debug!(
            page_id = %self.page_id,
            prop_count = mutation.properties.len(),
            op_count = mutation.model_ops.len(),
            "Flushing UI mutations"
        );

        if let Some(ref tx) = self.ui_tx {
            if let Err(e) = tx.try_send(mutation) {
                warn!(page_id = %self.page_id, error = %e, "Failed to send mutation");
            }
        }
    }
}
