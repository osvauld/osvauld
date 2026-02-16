//! ScribeManager - Scribe actor lifecycle management
//!
//! Manages the lifecycle of Scribe actors (one per page):
//! - Spawns new Scribes on demand
//! - Caches active Scribes for reuse
//! - Implements LRU eviction when capacity is reached
//! - Provides listing and cleanup operations

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use ractor::ActorRef;
use tokio::sync::RwLock;

use crate::{ButlerError, Result, Scribe, ScribeArgs, ScribeMessage};
use tracing::instrument;

/// Default maximum number of open pages
pub(crate) const DEFAULT_MAX_OPEN_PAGES: usize = 50;

/// Scribe entry with last access time for LRU eviction
struct ScribeEntry {
    actor: ActorRef<ScribeMessage>,
    last_accessed: Instant,
}

/// Manages Scribe actor lifecycle
///
/// **Responsibilities**:
/// - Spawn Scribe actors on demand
/// - Cache active Scribes (one per page_id)
/// - LRU eviction when max_open_pages reached
/// - Cleanup on close
pub(crate) struct ScribeManager {
    /// Active Scribe actors (page_id -> ScribeEntry)
    scribes: RwLock<HashMap<String, ScribeEntry>>,
    /// Per-page locks for serializing open calls
    page_locks: RwLock<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Maximum number of open pages
    max_open_pages: usize,
}

impl ScribeManager {
    /// Create a new ScribeManager with default max pages
    pub fn new() -> Self {
        Self {
            scribes: RwLock::new(HashMap::new()),
            page_locks: RwLock::new(HashMap::new()),
            max_open_pages: DEFAULT_MAX_OPEN_PAGES,
        }
    }

    /// Create a new ScribeManager with custom max pages
    pub fn with_max_pages(max_open_pages: usize) -> Self {
        Self {
            scribes: RwLock::new(HashMap::new()),
            page_locks: RwLock::new(HashMap::new()),
            max_open_pages,
        }
    }

    /// Get an existing Scribe actor, updating last_accessed time
    ///
    /// **Context**: Quick check before spawning
    /// **Returns**: ActorRef if Scribe exists and is still active
    #[instrument(skip(self), fields(page_id = %page_id))]
    pub async fn get(&self, page_id: &str) -> Option<ActorRef<ScribeMessage>> {
        let mut scribes = self.scribes.write().await;
        if let Some(entry) = scribes.get_mut(page_id) {
            entry.last_accessed = Instant::now();
            Some(entry.actor.clone())
        } else {
            None
        }
    }

    /// Get or create per-page lock for serializing open calls
    ///
    /// **Context**: Prevents concurrent spawns of the same page
    #[instrument(skip(self), fields(page_id = %page_id))]
    pub async fn get_page_lock(&self, page_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.page_locks.write().await;
        locks
            .entry(page_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// Spawn and register a new Scribe actor
    ///
    /// **Context**: Called after get() returns None and page_lock is acquired
    /// **Precondition**: Caller must hold page_lock and verify Scribe doesn't exist
    ///
    /// # Arguments
    /// * `page_id` - The page ID
    /// * `actor_name` - Unique actor name (e.g., "scribe-{user_hash}-{page_id}")
    /// * `args` - ScribeArgs for actor initialization
    #[instrument(skip(self, args), fields(page_id = %page_id, actor_name = %actor_name))]
    pub async fn spawn(
        &self,
        page_id: &str,
        actor_name: String,
        args: ScribeArgs,
    ) -> Result<ActorRef<ScribeMessage>> {
        // Check for stale actor in registry (happens when evicted actor hasn't fully shut down)
        if let Some(old_actor) = ractor::registry::where_is(actor_name.clone()) {
            log::info!(
                "Found stale Scribe actor in registry: {} - stopping it",
                actor_name
            );
            old_actor.stop(Some("replaced by new open_page call".to_string()));

            // Wait for the old actor to fully unregister (up to 500ms)
            for _ in 0..50 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                if ractor::registry::where_is(actor_name.clone()).is_none() {
                    break;
                }
            }
        }

        // Evict LRU if at capacity
        self.maybe_evict_lru().await;

        // Spawn the Scribe actor
        let (actor, _handle) = ractor::Actor::spawn(Some(actor_name), Scribe::new(), args)
            .await
            .map_err(|e| ButlerError::Database(format!("Failed to spawn Scribe: {:?}", e)))?;

        // Register in cache
        {
            let mut scribes = self.scribes.write().await;
            scribes.insert(
                page_id.to_string(),
                ScribeEntry {
                    actor: actor.clone(),
                    last_accessed: Instant::now(),
                },
            );
        }

        Ok(actor)
    }

    /// Close a page and stop its Scribe actor
    ///
    /// **Context**: Page no longer needed, release resources
    #[instrument(skip(self), fields(page_id = %page_id))]
    pub async fn close(&self, page_id: &str) {
        let entry = {
            let mut scribes = self.scribes.write().await;
            scribes.remove(page_id)
        };

        if let Some(entry) = entry {
            // Send shutdown message - Scribe will flush before stopping
            let _ = entry.actor.cast(ScribeMessage::Shutdown);
        }
    }

    /// Get active Scribe count
    #[instrument(skip_all)]
    pub async fn active_count(&self) -> usize {
        self.scribes.read().await.len()
    }

    /// List active page IDs (for peer subscription)
    ///
    /// **Context**: After handshake, PeerActor needs to know which pages are active
    /// **Returns**: List of page_ids with active Scribes
    #[instrument(skip_all)]
    pub async fn list_active_page_ids(&self) -> Vec<String> {
        self.scribes.read().await.keys().cloned().collect()
    }

    /// Evict least recently used Scribe if at capacity
    #[instrument(skip_all)]
    async fn maybe_evict_lru(&self) {
        let mut scribes = self.scribes.write().await;

        if scribes.len() < self.max_open_pages {
            return;
        }

        // Find LRU entry
        let lru_page_id = scribes
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(id, _)| id.clone());

        if let Some(page_id) = lru_page_id {
            if let Some(entry) = scribes.remove(&page_id) {
                log::info!("Evicting LRU Scribe for page {}", page_id);
                let _ = entry.actor.cast(ScribeMessage::Shutdown);
            }
        }
    }
}
