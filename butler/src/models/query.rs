//! Query Types for HUML Renderer
//!
//! **Context**: HUML templates need query-based data access for large datasets.
//! Instead of fetching full context every frame, queries provide:
//! - Filtering (CEL expressions)
//! - Sorting
//! - Pagination
//! - Incremental updates (QueryDelta)
//!
//! **Architecture**:
//! ```
//! HUML Renderer → QueryBridge → Scribe
//!                      ↑
//!                 QueryDelta (subscribed updates)
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Query specification for data access
///
/// **Example** (HUML template):
/// ```yaml
/// queries:
///   recent_messages:
///     layer: "chat"
///     path: "messages"
///     filter: "${ timestamp > now() - duration('24h') }"
///     sort_by: "timestamp"
///     sort_order: "desc"
///     limit: 50
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySpec {
    /// Unique query identifier (referenced in template)
    pub query_id: String,

    /// Layer name to query from
    pub layer_name: String,

    /// JSON path within the layer (e.g., "messages", "users.active")
    pub path: String,

    /// CEL filter expression (optional)
    /// e.g., "${ item.active && item.role == 'admin' }"
    #[serde(default)]
    pub filter: Option<String>,

    /// Field to sort by (optional)
    #[serde(default)]
    pub sort_by: Option<String>,

    /// Sort order: "asc" or "desc" (default: "asc")
    #[serde(default)]
    pub sort_order: Option<SortOrder>,

    /// Starting offset for pagination
    #[serde(default)]
    pub offset: usize,

    /// Maximum items to return (default: unlimited)
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Sort order for queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

/// Result of executing a query
///
/// Contains the matching items plus metadata for pagination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Query identifier this result is for
    pub query_id: String,

    /// Matching items (as JSON values)
    pub items: Vec<Value>,

    /// Total count of items matching the filter (before pagination)
    /// Used for "showing 1-50 of 1234" UI
    pub total_count: usize,

    /// Whether there are more items after this page
    pub has_more: bool,

    /// Version identifier for this result (for cache invalidation)
    pub version: u64,
}

impl QueryResult {
    /// Create a new query result
    pub fn new(query_id: String, items: Vec<Value>, total_count: usize, has_more: bool) -> Self {
        Self {
            query_id,
            items,
            total_count,
            has_more,
            version: 0,
        }
    }

    /// Create an empty result
    pub fn empty(query_id: String) -> Self {
        Self {
            query_id,
            items: Vec::new(),
            total_count: 0,
            has_more: false,
            version: 0,
        }
    }
}

/// Incremental delta for query result changes
///
/// **Context**: Instead of re-fetching entire query results, Scribe sends
/// deltas when the underlying data changes.
///
/// **Types**:
/// - `Reset`: Full replacement (major schema change or first sync)
/// - `Insert`: New item matching the query
/// - `Update`: Existing item changed (still matches)
/// - `Remove`: Item no longer matches filter or was deleted
/// - `Reorder`: Sort order changed (item moved position)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryDelta {
    /// Query identifier this delta applies to
    pub query_id: String,

    /// The change type
    pub change: QueryChange,

    /// New version after this delta
    pub version: u64,
}

/// Types of changes to a query result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueryChange {
    /// Full reset - replace entire result set
    /// Used for initial sync or major changes
    Reset {
        /// New result set
        result: QueryResult,
    },

    /// New item inserted into result set
    Insert {
        /// Index where item was inserted
        index: usize,
        /// The new item
        item: Value,
        /// New total count
        total_count: usize,
    },

    /// Existing item was updated (still matches filter)
    Update {
        /// Index of the updated item
        index: usize,
        /// The updated item
        item: Value,
    },

    /// Item removed from result set (deleted or no longer matches filter)
    Remove {
        /// Index of the removed item
        index: usize,
        /// New total count
        total_count: usize,
    },

    /// Item moved to a new position (sort order changed)
    Reorder {
        /// Old index
        from_index: usize,
        /// New index
        to_index: usize,
    },

    /// Multiple changes batched together
    Batch {
        /// List of individual changes
        changes: Vec<QueryChange>,
    },

    /// Total count changed (affects pagination UI)
    CountChanged {
        /// New total count
        total_count: usize,
        /// Whether there are more items
        has_more: bool,
    },
}

impl QueryDelta {
    /// Create a reset delta with full result
    pub fn reset(query_id: String, result: QueryResult, version: u64) -> Self {
        Self {
            query_id,
            change: QueryChange::Reset { result },
            version,
        }
    }

    /// Create an insert delta
    pub fn insert(query_id: String, index: usize, item: Value, total_count: usize, version: u64) -> Self {
        Self {
            query_id,
            change: QueryChange::Insert { index, item, total_count },
            version,
        }
    }

    /// Create an update delta
    pub fn update(query_id: String, index: usize, item: Value, version: u64) -> Self {
        Self {
            query_id,
            change: QueryChange::Update { index, item },
            version,
        }
    }

    /// Create a remove delta
    pub fn remove(query_id: String, index: usize, total_count: usize, version: u64) -> Self {
        Self {
            query_id,
            change: QueryChange::Remove { index, total_count },
            version,
        }
    }
}

/// Subscription handle for query updates
///
/// **Context**: UI subscribes to a query, Scribe sends deltas as data changes.
#[derive(Debug, Clone)]
pub struct QuerySubscription {
    /// Query specification
    pub spec: QuerySpec,

    /// Current version (for deduplication)
    pub version: u64,
}

impl QuerySubscription {
    pub fn new(spec: QuerySpec) -> Self {
        Self {
            spec,
            version: 0,
        }
    }
}
