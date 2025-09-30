# Chat Architecture Quick Reference

## Component Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                         FRONTEND (Svelte)                        │
│  ┌────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │ ChatWorkspace  │  │ ChatCoordinator │  │  Y.Map (msgs)   │  │
│  └────────┬───────┘  └────────┬────────┘  └────────┬────────┘  │
│           │                   │                     │           │
│           └───────────────────┴─────────────────────┘           │
│                              │                                   │
│                   emit "sync-update"                            │
└──────────────────────────────┼──────────────────────────────────┘
                               │
                               ▼
┌──────────────────────────────────────────────────────────────────┐
│                    BACKEND (Rust/Tauri)                          │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                     EventManager                           │ │
│  │  ┌──────────────────┐     ┌──────────────────┐           │ │
│  │  │  ChatState       │     │ CurrentNoteState │           │ │
│  │  │  (Multi-resource)│     │  (Compatibility) │           │ │
│  │  └────────┬─────────┘     └──────────────────┘           │ │
│  │           │                                                │ │
│  │  ┌────────▼──────────────────────────────────────────┐   │ │
│  │  │  Listeners:                                       │   │ │
│  │  │  • setup_chat_sync_update_listener()             │   │ │
│  │  │  • setup_note_change_listener()                  │   │ │
│  │  │  • handle_editing_event()                        │   │ │
│  │  └───────────────────────────────────────────────────┘   │ │
│  └────────────────────────────────────────────────────────────┘ │
│                              │                                   │
│                              ▼                                   │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                      P2P Service                           │ │
│  │  • Connection Management                                   │ │
│  │  • Message Broadcasting                                    │ │
│  │  • State Vector Exchange                                   │ │
│  └────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

## Key Components

### ChatState

**Location**: `chat/src-tauri/src/chat_state.rs`

**Purpose**: Manages multiple simultaneous chat resource buffers.

**Key Data Structures**:
```rust
pub struct ChatState {
    // resource_id -> (main_doc, image_doc)
    buffers: Arc<RwLock<HashMap<String, ResourceBuffers>>>,
    
    // resource_id -> (device_ids, timestamp)
    subscription_cache: Arc<RwLock<HashMap<String, SubscriptionCache>>>,
    
    // UI context only
    current_chat_id: Arc<RwLock<Option<String>>>,
    
    // Database access
    repo_ctx: Arc<RepositoryContext>,
    
    // Current user/device
    current_user_id: Arc<RwLock<Option<String>>>,
    current_device_id: Arc<RwLock<Option<String>>>,
}
```

**Critical Methods**:
| Method | Purpose | When Called |
|--------|---------|-------------|
| `load_chat_resource()` | Load decrypted resource into buffers | On chat open |
| `apply_update()` | Apply Yjs update to buffer | On message send/receive |
| `get_subscribers()` | Get device IDs (cached) | On message broadcast |
| `get_state_vectors()` | Generate state vectors | During reconciliation |
| `generate_updates_for_peer()` | Create diff for peer | On update request |

---

### EventManager

**Location**: `chat/src-tauri/src/listners/mod.rs`

**Purpose**: Coordinates Tauri events and P2P messages.

**Key Fields**:
```rust
pub struct EventManager {
    app_handle: AppHandle,
    p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
    p2p_sender: P2PSender,
    current_note_state: CurrentNoteState,  // Compatibility
    chat_state: ChatState,                 // New!
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<Mutex<CryptoUtils>>,
}
```

---

### Event Listeners

**Location**: `chat/src-tauri/src/listners/tauri_events.rs`

#### 1. Chat Sync Update Listener

**Function**: `setup_chat_sync_update_listener()`

**Triggered By**: Frontend emits "sync-update"

**Flow**:
```
1. Receive sync-update event
2. Parse payload (resource_id, updates, client_id, doc_type)
3. Apply to ChatState buffer
4. Get subscribers (with caching)
5. Broadcast to active subscribers
```

**Key Code**:
```rust
// Apply update
chat_state.apply_update(&resource_id, update_bytes, &doc_type).await;

// Get subscribers
let subscribers = chat_state.get_subscribers(&resource_id).await?;

// Broadcast
p2p_sender.send_sync_update_to_connections(
    resource_id, client_id, update_bytes, subscribers, doc_type
)?;
```

#### 2. Note Change Listener

**Function**: `setup_note_change_listener()`

**Triggered By**: Frontend emits "note-change" (when opening chat)

**Flow**:
```
1. Receive note-change event
2. Set current chat in ChatState
3. Set user context
4. Load decrypted resource
5. Load into ChatState buffers
6. Trigger reconciliation
```

**Key Code**:
```rust
// Set context
chat_state.set_current_chat(Some(chat_id.clone())).await;
chat_state.set_current_user(user_id, device_id).await;

// Load resource
chat_state.load_chat_resource(chat_id, main_doc, image_doc).await;

// Reconcile
reconcile_chat_on_open(chat_state, chat_id, p2p_sender).await;
```

#### 3. Reconciliation

**Function**: `reconcile_chat_on_open()`

**Triggered By**: `setup_note_change_listener()` when chat opened

**Flow**:
```
1. Get subscribers
2. Get local state vectors
3. Send live edit requests (establish connections)
4. For each subscriber:
   a. Send document check with state vectors
   b. Peer responds with their state vectors
   c. Exchange missing updates
   d. Apply updates on both sides
5. Chat fully synced
```

**Key Code**:
```rust
// Get subscribers
let subscribers = chat_state.get_subscribers(&chat_id).await?;

// Get state vectors
let state_vectors = chat_state.get_state_vectors(&chat_id).await?;

// Establish connections
p2p_sender.send_live_edit_requests(subscribers.clone())?;

// Initiate reconciliation with each subscriber
for device_id in subscribers {
    p2p_sender.send_live_edit_document_check_response(
        device_id, chat_id.clone(), true, state_vectors.clone()
    )?;
}
```

---

### P2P Handlers

**Location**: `chat/src-tauri/src/listners/p2p_handlers/`

#### updates.rs

**`handle_editing_event()`**:
```rust
// OLD:
if !is_current_note() { return; }
current_note_state.apply_update()

// NEW:
chat_state.apply_update(&resource_id, updates, &doc_type).await;
// Always apply, no current check!
```

**`handle_awareness_event()`**:
```rust
// OLD:
if !is_current_note() { return; }

// NEW:
// No check - always emit to frontend
// Frontend decides visibility
```

#### live_edit.rs

**`handle_document_check()`**:
```rust
// Try to get state vectors from ChatState
let state_vectors = chat_state.get_state_vectors(&resource_id).await?;
let is_match = !state_vectors.is_empty();
```

**`handle_document_update_request()`**:
```rust
// Generate updates for peer
let peer_updates = chat_state
    .generate_updates_for_peer(&resource_id, &state_vectors)
    .await?;
```

**`handle_document_process_update()`**:
```rust
// Apply peer updates and generate diff
let peer_updates = chat_state
    .apply_updates_and_generate_diff(&resource_id, &remote_updates)
    .await?;
```

---

## Message Flow Diagrams

### Sending a Message (Both Online)

```
Frontend A          Backend A           Backend B           Frontend B
    │                   │                   │                   │
    │ 1. Type message   │                   │                   │
    │ 2. Y.Map update   │                   │                   │
    ├──sync-update───►  │                   │                   │
    │                   │ 3. Apply to       │                   │
    │                   │    ChatState      │                   │
    │                   │ 4. Get subs       │                   │
    │                   │    (cached)       │                   │
    │                   ├──DocumentUpdate─►│                   │
    │                   │                   │ 5. Apply to       │
    │                   │                   │    ChatState      │
    │                   │                   ├──live-updates──► │
    │                   │                   │                   │ 6. Display
    │                   │                   │                   │
```

### Sending a Message (Recipient Offline)

```
Frontend A          Backend A           Backend B (Offline)
    │                   │                   
    │ 1. Type message   │                   
    │ 2. Y.Map update   │                   
    ├──sync-update───►  │                   
    │                   │ 3. Apply to       
    │                   │    ChatState      
    │                   │ 4. Get subs       
    │                   │    (cached)       
    │                   │ 5. Broadcast      
    │                   │    (no active     
    │                   │     connections)  
    │                   │                   
    │                   │ Message stored    
    │                   │ in sender's       
    │                   │ ChatState buffer  
```

### Opening a Chat (Reconciliation)

```
Frontend B          Backend B           Backend A           Frontend A
    │                   │                   │                   │
    │ 1. Open chat      │                   │                   │
    ├──note-change───►  │                   │                   │
    │                   │ 2. Load resource  │                   │
    │                   │ 3. Get state vec  │                   │
    │                   ├──DocumentCheck──► │                   │
    │                   │   (with SV)       │                   │
    │                   │                   │ 4. Compare SVs    │
    │                   │                   │ 5. Generate diff  │
    │                   │ ◄─UpdateExchange──┤                   │
    │                   │ 6. Apply updates  │                   │
    │                   │ 7. Generate diff  │                   │
    │                   ├──UpdateResponse─►│                   │
    │                   │                   │ 8. Apply updates  │
    │ ◄─live-updates────┤                   │                   │
    │ 9. Display all    │                   │                   │
    │    messages       │                   │                   │
```

---

## Subscription Cache

### Cache Entry Structure

```rust
struct SubscriptionCache {
    device_ids: Vec<String>,
    last_updated: std::time::Instant,
}
```

### Cache Lifecycle

```
Message Send
    │
    ▼
Is resource_id in cache?
    │
    ├─YES─► Cache age < 5 min?
    │           │
    │           ├─YES─► Use cached device_ids
    │           │
    │           └─NO──► Query DB, update cache
    │
    └─NO───► Query DB, create cache entry
```

### Cache Invalidation

**Manual**:
```rust
chat_state.invalidate_subscription_cache(&resource_id);
```

**When to Invalidate**:
- Resource shared with new user
- User removed from resource
- Permissions changed

---

## State Vector Reconciliation

### State Vector Format

```json
{
  "main_doc": {
    "updates": [],
    "state_vector": [0, 1, 2, 3, ...]
  },
  "image_state": {
    "updates": [],
    "state_vector": [0, 1, 2, ...]
  }
}
```

### Reconciliation Process

1. **Get Local State Vector**:
   ```rust
   let local_sv = chat_state.get_state_vectors(&resource_id).await?;
   ```

2. **Send to Peer**:
   ```rust
   p2p_sender.send_live_edit_document_check_response(
       connection_id, resource_id, true, local_sv
   )?;
   ```

3. **Peer Generates Diff**:
   ```rust
   let updates = chat_state.generate_updates_for_peer(
       &resource_id, &peer_state_vectors
   ).await?;
   ```

4. **Exchange Updates**:
   - Both sides send missing updates
   - Both sides apply received updates
   - State vectors now equal

5. **Result**:
   - Both sides have identical document state
   - All messages synced

---

## Configuration & Constants

### Cache Settings

```rust
// In chat_state.rs
const CACHE_VALIDITY_SECONDS: u64 = 300;  // 5 minutes
```

### Reconciliation Settings

```rust
// In tauri_events.rs
const CONNECTION_ESTABLISH_DELAY_MS: u64 = 100;  // Wait for connections
```

### Performance Targets

| Metric | Target | Location |
|--------|--------|----------|
| Message delivery | < 100ms | Real-time broadcast |
| Cache lookup | < 5ms | In-memory HashMap |
| State vector gen | < 50ms | Per resource |
| Reconciliation | < 2s | Full chat history |

---

## Debugging Commands

### Enable Debug Logging

```bash
# Backend
RUST_LOG=info cargo run

# Specific module
RUST_LOG=chat=debug,info cargo run
```

### Key Log Patterns

```bash
# Subscription caching
grep "Using cached subscribers"
grep "Fetching subscribers from database"
grep "Cached N subscribers"

# Message broadcasting
grep "Broadcasting chat update"
grep "Received editing event"

# Reconciliation
grep "Starting chat reconciliation"
grep "Sent reconciliation request"
grep "Processing update response"

# State vectors
grep "Get state vectors"
grep "Generate updates for peer"
```

### Frontend Console

```javascript
// Check ChatState in Rust backend (via logs)
// In frontend, check Y.Doc
console.log(chatDoc.toJSON());

// Check if updates applied
chatDoc.on('update', (update) => {
  console.log('Update received:', update);
});
```

---

## Common Patterns

### Adding a New Event Listener

```rust
// In tauri_events.rs
fn setup_my_listener(&self) {
    let chat_state = self.chat_state.clone();
    let p2p_sender = self.p2p_sender.clone();
    
    self.app_handle.listen("my-event", move |event| {
        let chat_state = chat_state.clone();
        let p2p_sender = p2p_sender.clone();
        
        tokio::spawn(async move {
            // Handle event
        });
    });
}
```

### Accessing Chat Buffer

```rust
// Apply update
chat_state.apply_update(&resource_id, updates, "main_doc").await;

// Get state vectors
let sv = chat_state.get_state_vectors(&resource_id).await?;

// Generate diff
let diff = chat_state.generate_updates_for_peer(
    &resource_id, &peer_sv
).await?;
```

### Broadcasting to Subscribers

```rust
// Get subscribers
let subs = chat_state.get_subscribers(&resource_id).await?;

// Broadcast
p2p_sender.send_sync_update_to_connections(
    resource_id, client_id, updates, subs, doc_type
)?;
```

---

## Migration Notes

### From Document-Centric to Chat

**Before**:
```rust
if is_current_note(&resource_id) {
    current_note_state.apply_update(updates);
    broadcast_to_active_connections();
}
```

**After**:
```rust
// Always apply, always broadcast
chat_state.apply_update(&resource_id, updates).await;
let subs = chat_state.get_subscribers(&resource_id).await?;
p2p_sender.send_sync_update_to_connections(..., subs, ...)?;
```

### Key Differences

| Aspect | Document-Centric | Chat |
|--------|------------------|------|
| Active tracking | Single current document | Multiple simultaneous |
| Update filtering | `is_current_note()` check | Always process |
| Broadcasting | Active connections only | All subscribers |
| Reconciliation | On document switch | On chat open |
| Connection state | Active/Inactive distinction | Treat all equally |

---

## Quick Command Reference

```bash
# Build
cargo build --release

# Run with logs
RUST_LOG=info cargo run

# Test specific module
cargo test chat_state

# Check lints
cargo clippy

# Format code
cargo fmt

# Watch mode
cargo watch -x run
```

---

## References

- **Implementation Summary**: `CHAT_REFACTORING_IMPLEMENTATION.md`
- **Testing Guide**: `TESTING_GUIDE.md`
- **Original Design**: See user-provided implementation plan
- **Yjs Docs**: https://docs.yjs.dev/
- **Tauri Docs**: https://tauri.app/

