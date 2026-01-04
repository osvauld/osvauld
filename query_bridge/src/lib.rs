//! QueryBridge - Data Access Layer for HUML Renderer
//!
//! **Purpose**: Provides query-based data access to Scribe with:
//! - Result caching (avoid re-querying unchanged data)
//! - Filter-based querying (CEL being replaced by Rune)
//! - Subscription management
//! - Dependency tracking integration
//!
//! **Architecture**:
//! ```text
//! HUML Renderer
//!      │
//!      ▼
//! QueryBridge
//!   ├── Filter evaluation (stubbed - Rune TBD)
//!   ├── QueryCache (cached results)
//!   └── Scribe subscription
//!      │
//!      ▼
//!   Scribe (Loro CRDT)
//! ```
//!
//! **Flow**:
//! 1. Renderer calls `query("messages")` → QueryBridge checks cache
//! 2. If cache miss → send Query to Scribe → cache result
//! 3. QueryBridge subscribes to Scribe for updates
//! 4. When QueryDelta arrives → update cache → notify renderer

use std::collections::HashMap;
use std::sync::Arc;

use butler::models::{QueryDelta, QueryResult, QuerySpec};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

// =============================================================================
// Stub CelEvaluator - CEL is being replaced by Rune runtime
// =============================================================================

/// Stub evaluator - CEL is being replaced by Rune runtime
///
/// All evaluation returns true (filter is a no-op until Rune is integrated).
#[derive(Debug, Clone)]
pub struct CelEvaluator;

impl CelEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Stub evaluate - always returns true
    pub fn evaluate(&self, _expr: &str, _context: &Value) -> std::result::Result<Value, String> {
        // TODO: Replace with native filtering in Scribe
        Ok(Value::Bool(true))
    }
}

impl Default for CelEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Error, Debug)]
pub enum QueryBridgeError {
    #[error("Query not found: {0}")]
    QueryNotFound(String),

    #[error("Scribe communication error: {0}")]
    ScribeCommunication(String),

    #[error("CEL evaluation error: {0}")]
    CelError(String),

    #[error("Channel error: {0}")]
    Channel(String),
}

pub type Result<T> = std::result::Result<T, QueryBridgeError>;

/// Cached query result with version tracking
struct CachedQuery {
    /// The query specification
    spec: QuerySpec,
    /// Cached result
    result: QueryResult,
    /// Dirty flag (needs re-query on next access)
    dirty: bool,
}

/// Request to send to Scribe
pub enum ScribeRequest {
    /// Execute a query
    Query {
        spec: QuerySpec,
        reply: tokio::sync::oneshot::Sender<butler::error::Result<QueryResult>>,
    },
    /// Subscribe to query updates
    Subscribe {
        spec: QuerySpec,
        delta_tx: mpsc::Sender<QueryDelta>,
    },
    /// Unsubscribe from query updates
    Unsubscribe { query_id: String },
    /// Update layer from JSON (for CEL commit())
    UpdateFromJson {
        layer_name: String,
        path: String,
        value: Value,
    },
}

/// QueryBridge - manages data queries and subscriptions
///
/// **Usage from HUML Renderer**:
/// ```ignore
/// let bridge = QueryBridge::new(cel_evaluator, scribe_tx);
///
/// // Register query (usually done once during template setup)
/// bridge.register_query(QuerySpec { query_id: "messages", ... }).await?;
///
/// // Get cached result (fast, no Scribe call if cached)
/// let result = bridge.get_query("messages")?;
///
/// // Poll for changes (call in render loop)
/// for query_id in bridge.poll_changes() {
///     // Re-fetch and re-render affected UI
/// }
/// ```
pub struct QueryBridge {
    /// CEL evaluator for filtering
    cel: Arc<CelEvaluator>,

    /// Cached query results
    cache: HashMap<String, CachedQuery>,

    /// Channel to send requests to Scribe
    scribe_tx: mpsc::Sender<ScribeRequest>,

    /// Channel to receive deltas from Scribe
    delta_rx: mpsc::Receiver<QueryDelta>,

    /// Query IDs that have changed since last poll
    changed_queries: Vec<String>,
}

impl QueryBridge {
    /// Create a new QueryBridge
    ///
    /// **Arguments**:
    /// - `scribe_tx`: Channel to send requests to Scribe
    /// - `delta_rx`: Channel to receive QueryDeltas from Scribe
    pub fn new(
        scribe_tx: mpsc::Sender<ScribeRequest>,
        delta_rx: mpsc::Receiver<QueryDelta>,
    ) -> Self {
        let cel = CelEvaluator::new();

        Self {
            cel: Arc::new(cel),
            cache: HashMap::new(),
            scribe_tx,
            delta_rx,
            changed_queries: Vec::new(),
        }
    }

    /// Create with an existing CEL evaluator
    pub fn with_cel(
        cel: Arc<CelEvaluator>,
        scribe_tx: mpsc::Sender<ScribeRequest>,
        delta_rx: mpsc::Receiver<QueryDelta>,
    ) -> Self {
        Self {
            cel,
            cache: HashMap::new(),
            scribe_tx,
            delta_rx,
            changed_queries: Vec::new(),
        }
    }

    /// Register and execute a query
    ///
    /// Sends query to Scribe, caches result, subscribes to updates.
    pub async fn register_query(&mut self, spec: QuerySpec) -> Result<QueryResult> {
        let query_id = spec.query_id.clone();
        info!(query_id = %query_id, "Registering query");

        // Create channel for receiving delta updates
        let (delta_tx, mut query_delta_rx) = mpsc::channel::<QueryDelta>(32);

        // Send subscribe request to Scribe
        self.scribe_tx
            .send(ScribeRequest::Subscribe {
                spec: spec.clone(),
                delta_tx,
            })
            .await
            .map_err(|e| QueryBridgeError::Channel(e.to_string()))?;

        // Wait for initial result (sent as Reset delta)
        let initial_delta = query_delta_rx.recv().await
            .ok_or_else(|| QueryBridgeError::ScribeCommunication("No initial result".to_string()))?;

        let result = match initial_delta.change {
            butler::models::QueryChange::Reset { result } => result,
            _ => {
                return Err(QueryBridgeError::ScribeCommunication(
                    "Expected Reset delta for initial result".to_string(),
                ));
            }
        };

        // Cache the result
        self.cache.insert(
            query_id.clone(),
            CachedQuery {
                spec,
                result: result.clone(),
                dirty: false,
            },
        );

        // Store the delta receiver for this query
        // NOTE: In a real implementation, we'd spawn a task to forward deltas
        // For MVP, we poll in poll_changes()
        debug!(query_id = %query_id, items = result.items.len(), "Query registered and cached");

        Ok(result)
    }

    /// Get cached query result
    ///
    /// Returns cached result immediately. Does not re-query Scribe.
    /// Call `poll_changes()` to check for updates.
    pub fn get_query(&self, query_id: &str) -> Result<&QueryResult> {
        self.cache
            .get(query_id)
            .map(|cached| &cached.result)
            .ok_or_else(|| QueryBridgeError::QueryNotFound(query_id.to_string()))
    }

    /// Get a single item by index from a query result
    pub fn get_item(&self, query_id: &str, index: usize) -> Result<Option<&Value>> {
        let result = self.get_query(query_id)?;
        Ok(result.items.get(index))
    }

    /// Poll for changes from Scribe
    ///
    /// Processes any pending QueryDeltas and returns IDs of changed queries.
    /// Call this periodically (e.g., at start of render frame).
    pub fn poll_changes(&mut self) -> Vec<String> {
        // Drain pending deltas
        while let Ok(delta) = self.delta_rx.try_recv() {
            self.apply_delta(delta);
        }

        // Return and clear changed queries
        std::mem::take(&mut self.changed_queries)
    }

    /// Apply a query delta to the cache
    fn apply_delta(&mut self, delta: QueryDelta) {
        let query_id = delta.query_id.clone();

        if let Some(cached) = self.cache.get_mut(&query_id) {
            match delta.change {
                butler::models::QueryChange::Reset { result } => {
                    cached.result = result;
                    cached.dirty = false;
                }
                butler::models::QueryChange::Insert { index, item, total_count } => {
                    cached.result.items.insert(index, item);
                    cached.result.total_count = total_count;
                }
                butler::models::QueryChange::Update { index, item } => {
                    if index < cached.result.items.len() {
                        cached.result.items[index] = item;
                    }
                }
                butler::models::QueryChange::Remove { index, total_count } => {
                    if index < cached.result.items.len() {
                        cached.result.items.remove(index);
                    }
                    cached.result.total_count = total_count;
                }
                butler::models::QueryChange::Reorder { from_index, to_index } => {
                    if from_index < cached.result.items.len() {
                        let item = cached.result.items.remove(from_index);
                        let to = if to_index > from_index { to_index - 1 } else { to_index };
                        cached.result.items.insert(to.min(cached.result.items.len()), item);
                    }
                }
                butler::models::QueryChange::Batch { changes } => {
                    for change in changes {
                        self.apply_delta(QueryDelta {
                            query_id: query_id.clone(),
                            change,
                            version: delta.version,
                        });
                    }
                    return; // Don't add to changed_queries multiple times
                }
                butler::models::QueryChange::CountChanged { total_count, has_more } => {
                    cached.result.total_count = total_count;
                    cached.result.has_more = has_more;
                }
            }

            cached.result.version = delta.version;
            self.changed_queries.push(query_id);
        }
    }

    /// Unsubscribe from a query
    pub async fn unsubscribe(&mut self, query_id: &str) -> Result<()> {
        self.cache.remove(query_id);
        self.scribe_tx
            .send(ScribeRequest::Unsubscribe {
                query_id: query_id.to_string(),
            })
            .await
            .map_err(|e| QueryBridgeError::Channel(e.to_string()))?;

        debug!(query_id = %query_id, "Query unsubscribed");
        Ok(())
    }

    /// Filter items using a CEL expression
    ///
    /// Filters cached items client-side. Useful for secondary filtering
    /// without re-querying Scribe.
    ///
    /// **Note**: Each item is evaluated with `item` bound in the context.
    pub fn filter_cached(
        &self,
        query_id: &str,
        filter_expr: &str,
    ) -> Result<Vec<Value>> {
        let result = self.get_query(query_id)?;

        let filtered: Vec<Value> = result
            .items
            .iter()
            .filter(|item| {
                let context = serde_json::json!({ "item": item });
                match self.cel.evaluate(filter_expr, &context) {
                    Ok(Value::Bool(true)) => true,
                    _ => false,
                }
            })
            .cloned()
            .collect();

        Ok(filtered)
    }

    /// Get the CEL evaluator (for template rendering)
    pub fn cel(&self) -> &CelEvaluator {
        &self.cel
    }

    /// Check if a query is registered
    pub fn has_query(&self, query_id: &str) -> bool {
        self.cache.contains_key(query_id)
    }

    /// Get all registered query IDs
    pub fn query_ids(&self) -> Vec<&str> {
        self.cache.keys().map(String::as_str).collect()
    }
}

/// Synchronous wrapper for QueryBridge
///
/// For use in immediate-mode contexts where async isn't convenient.
/// Internally uses blocking operations.
pub struct SyncQueryBridge {
    inner: QueryBridge,
    runtime: tokio::runtime::Handle,
}

impl SyncQueryBridge {
    pub fn new(bridge: QueryBridge, runtime: tokio::runtime::Handle) -> Self {
        Self {
            inner: bridge,
            runtime,
        }
    }

    pub fn register_query(&mut self, spec: QuerySpec) -> Result<QueryResult> {
        self.runtime.block_on(self.inner.register_query(spec))
    }

    pub fn get_query(&self, query_id: &str) -> Result<&QueryResult> {
        self.inner.get_query(query_id)
    }

    pub fn poll_changes(&mut self) -> Vec<String> {
        self.inner.poll_changes()
    }

    pub fn cel(&self) -> &CelEvaluator {
        self.inner.cel()
    }
}

// ============================================================================
// QueryBridge Actor - Spawning and Message Types
// ============================================================================

/// Messages sent from HumlRenderer to QueryBridge actor
///
/// **Pattern**: Same as peer_actor uses for Scribe communication - mpsc channels
/// instead of direct actor calls, keeping UI thread responsive.
#[derive(Debug, Clone)]
pub enum QueryBridgeMsg {
    /// Register and subscribe to a query
    RegisterQuery {
        query_id: String,
        layer: String,
        path: String,
    },
    /// Commit a value to Scribe (from CEL commit())
    Commit {
        /// Field name (maps to query/layer)
        field: String,
        /// Value to commit
        value: Value,
    },
}

/// Handle for async QueryBridge operations
///
/// **Context**: HumlRenderer runs on the UI thread (sync). QueryBridge
/// communicates with Scribe (async). This handle provides mpsc channels
/// following the same pattern as peer_actor.
#[derive(Debug, Clone)]
pub struct QueryBridgeHandle {
    /// Channel to send mutations to QueryBridge actor
    pub cmd_tx: mpsc::Sender<QueryBridgeMsg>,
}

impl QueryBridgeHandle {
    /// Create a new handle with the command channel
    pub fn new(cmd_tx: mpsc::Sender<QueryBridgeMsg>) -> Self {
        Self { cmd_tx }
    }

    /// Send a commit message (non-blocking, best effort)
    pub fn send_commit(&self, field: String, value: Value) {
        let _ = self.cmd_tx.try_send(QueryBridgeMsg::Commit { field, value });
    }

    /// Register a query
    pub fn register_query(&self, query_id: String, layer: String, path: String) {
        let _ = self.cmd_tx.try_send(QueryBridgeMsg::RegisterQuery {
            query_id,
            layer,
            path,
        });
    }
}

/// Delta notification for the renderer
///
/// **Context**: When Scribe data changes, the QueryBridge forwards deltas
/// to the renderer via this channel.
#[derive(Debug, Clone)]
pub struct RendererDelta {
    /// Query ID that changed
    pub query_id: String,
    /// New items (for simplicity, always full replacement for now)
    pub items: Vec<Value>,
}

/// Spawn the QueryBridge actor
///
/// **Context**: Creates an actor that bridges between HumlRenderer (sync, UI thread)
/// and Scribe (async, ractor actor). Uses mpsc channels following the same pattern
/// as peer_actor's broadcast subscription.
///
/// **Arguments**:
/// - `scribe_tx`: Channel to send ScribeRequests (Query, Subscribe, etc.)
/// - `scribe_delta_rx`: Channel to receive QueryDeltas from Scribe subscriptions
///
/// **Returns**: (QueryBridgeHandle, mpsc::Receiver<RendererDelta>)
/// - Handle for renderer to send commands
/// - Receiver for renderer to get delta notifications
pub fn spawn_query_bridge(
    scribe_tx: mpsc::Sender<ScribeRequest>,
    mut scribe_delta_rx: mpsc::Receiver<QueryDelta>,
) -> (QueryBridgeHandle, mpsc::Receiver<RendererDelta>) {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<QueryBridgeMsg>(64);
    let (delta_tx, delta_rx) = mpsc::channel::<RendererDelta>(64);

    // Spawn the bridge actor
    tokio::spawn(async move {
        // Track registered queries for routing commits
        let mut query_layers: HashMap<String, (String, String)> = HashMap::new();

        loop {
            tokio::select! {
                // Handle commands from renderer
                Some(msg) = cmd_rx.recv() => {
                    match msg {
                        QueryBridgeMsg::RegisterQuery { query_id, layer, path } => {
                            info!(query_id = %query_id, layer = %layer, path = %path, "Registering query");

                            // Store layer mapping for commit routing
                            query_layers.insert(query_id.clone(), (layer.clone(), path.clone()));

                            // TODO: Send Subscribe request to Scribe
                            // For now, just log - actual subscription will be added when
                            // we integrate with Scribe's SubscribeQuery message
                            debug!(query_id = %query_id, "Query registered (subscription TODO)");
                        }
                        QueryBridgeMsg::Commit { field, value } => {
                            info!(field = %field, "Commit received");

                            // Find the layer/path for this field
                            if let Some((layer, path)) = query_layers.get(&field) {
                                // Send UpdateFromJson to Scribe
                                let request = ScribeRequest::UpdateFromJson {
                                    layer_name: layer.clone(),
                                    path: path.clone(),
                                    value: value.clone(),
                                };

                                if let Err(e) = scribe_tx.send(request).await {
                                    warn!(field = %field, error = %e, "Failed to send commit to Scribe");
                                } else {
                                    debug!(
                                        field = %field,
                                        layer = %layer,
                                        path = %path,
                                        "Sent commit to Scribe"
                                    );
                                }

                                // Note: Scribe will send QueryDelta back via scribe_delta_rx
                                // which will update the renderer
                            } else {
                                debug!(field = %field, "No layer mapping for commit, treating as local-only");
                            }
                        }
                    }
                }

                // Handle deltas from Scribe
                Some(delta) = scribe_delta_rx.recv() => {
                    info!(query_id = %delta.query_id, "Received delta from Scribe");

                    // Extract items from the delta change
                    let items = match delta.change {
                        butler::models::QueryChange::Reset { result } => result.items,
                        butler::models::QueryChange::Insert { item, .. } => vec![item],
                        butler::models::QueryChange::Update { item, .. } => vec![item],
                        _ => Vec::new(),
                    };

                    // Forward to renderer
                    let _ = delta_tx.send(RendererDelta {
                        query_id: delta.query_id,
                        items,
                    }).await;
                }

                else => break,
            }
        }

        info!("QueryBridge actor shutting down");
    });

    let handle = QueryBridgeHandle::new(cmd_tx);
    (handle, delta_rx)
}

/// Create a simple QueryBridgeHandle without the full actor (for testing/local-only mode)
///
/// **Context**: Creates a handle that logs commands but doesn't actually persist.
/// Useful for testing the renderer without Scribe.
pub fn create_local_only_handle() -> QueryBridgeHandle {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<QueryBridgeMsg>(64);

    // Spawn a simple task that just logs and discards
    tokio::spawn(async move {
        while let Some(msg) = cmd_rx.recv().await {
            match msg {
                QueryBridgeMsg::RegisterQuery { query_id, .. } => {
                    debug!(query_id = %query_id, "Local-only: ignoring register");
                }
                QueryBridgeMsg::Commit { field, .. } => {
                    debug!(field = %field, "Local-only: ignoring commit");
                }
            }
        }
    });

    QueryBridgeHandle::new(cmd_tx)
}

/// Spawn a bridge that converts ScribeRequest to ScribeMessage
///
/// **Context**: QueryBridge sends ScribeRequests over a channel. This bridge
/// receives them and calls the appropriate ScribeMessage on the Scribe actor.
///
/// **Arguments**:
/// - `scribe_ref`: Reference to the Scribe actor
///
/// **Returns**: (scribe_tx, delta_rx)
/// - scribe_tx: Channel to send ScribeRequests (pass to spawn_query_bridge)
/// - delta_rx: Channel to receive QueryDeltas (pass to spawn_query_bridge)
pub fn spawn_scribe_bridge(
    scribe_ref: ractor::ActorRef<butler::scribe::ScribeMessage>,
) -> (mpsc::Sender<ScribeRequest>, mpsc::Receiver<QueryDelta>) {
    use butler::scribe::ScribeMessage;

    let (scribe_tx, mut scribe_rx) = mpsc::channel::<ScribeRequest>(64);
    let (delta_tx, delta_rx) = mpsc::channel::<QueryDelta>(64);

    tokio::spawn(async move {
        while let Some(request) = scribe_rx.recv().await {
            match request {
                ScribeRequest::Query { spec, reply } => {
                    // Create a oneshot channel for Scribe's reply
                    let (scribe_reply_tx, scribe_reply_rx) = tokio::sync::oneshot::channel();

                    // Send query request to Scribe
                    if let Err(e) = scribe_ref.cast(ScribeMessage::Query {
                        spec,
                        reply: scribe_reply_tx,
                    }) {
                        warn!(error = %e, "Failed to send Query to Scribe");
                        let _ = reply.send(Err(butler::error::ButlerError::Storage(e.to_string())));
                        continue;
                    }

                    // Wait for Scribe's reply and forward it
                    match scribe_reply_rx.await {
                        Ok(result) => {
                            let _ = reply.send(result);
                        }
                        Err(e) => {
                            warn!(error = %e, "Scribe Query reply channel closed");
                            let _ = reply.send(Err(butler::error::ButlerError::Storage(e.to_string())));
                        }
                    }
                }

                ScribeRequest::Subscribe { spec, delta_tx: sub_delta_tx } => {
                    // Subscribe to query - Scribe will send deltas to sub_delta_tx
                    // We also need to forward to our delta_tx for the renderer
                    let query_id = spec.query_id.clone();

                    // Create a forwarding channel
                    let (forward_tx, mut forward_rx) = mpsc::channel::<QueryDelta>(64);

                    // Send subscribe to Scribe
                    scribe_ref.cast(ScribeMessage::SubscribeQuery {
                        spec,
                        delta_tx: forward_tx,
                    }).ok();

                    // Spawn a task to forward deltas to both subscribers
                    let delta_tx_clone = delta_tx.clone();
                    tokio::spawn(async move {
                        while let Some(delta) = forward_rx.recv().await {
                            // Send to the original subscriber
                            let _ = sub_delta_tx.send(delta.clone()).await;
                            // Also send to the renderer delta channel
                            let _ = delta_tx_clone.send(delta).await;
                        }
                        debug!(query_id = %query_id, "Query subscription ended");
                    });
                }

                ScribeRequest::Unsubscribe { query_id } => {
                    scribe_ref.cast(ScribeMessage::UnsubscribeQuery { query_id }).ok();
                }

                ScribeRequest::UpdateFromJson { layer_name, path, value } => {
                    // Send update to Scribe
                    scribe_ref.cast(ScribeMessage::UpdateFromJson {
                        layer_name,
                        path,
                        value,
                        reply: None, // Fire and forget for now
                    }).ok();
                }
            }
        }

        info!("Scribe bridge shutting down");
    });

    (scribe_tx, delta_rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require Scribe to be running
    // These are basic unit tests for the data structures

    #[test]
    fn test_query_spec_serialization() {
        use butler::models::SortOrder;

        let spec = QuerySpec {
            query_id: "messages".to_string(),
            layer_name: "chat".to_string(),
            path: "messages".to_string(),
            filter: Some("${ item.active }".to_string()),
            sort_by: Some("timestamp".to_string()),
            sort_order: Some(SortOrder::Desc),
            offset: 0,
            limit: Some(50),
        };

        let json = serde_json::to_string(&spec).unwrap();
        let parsed: QuerySpec = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.query_id, "messages");
        assert_eq!(parsed.layer_name, "chat");
    }
}
