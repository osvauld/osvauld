# Listeners Architecture Documentation

**Date**: 2025-11-07
**Status**: Pre-removal documentation - This architecture will be removed and reimplemented
**Purpose**: Document the current P2P ↔ Listeners communication pattern for future reference

---

## Overview

The listeners system provides bidirectional real-time communication between the Tauri frontend, P2P network layer, and the Yjs CRDT document state. It enables live collaborative editing by maintaining a live buffer of the current note's state and synchronizing updates across connected peers.

---

## Architecture Components

### 1. EventManager (Central Coordinator)

**Location**: `sthalam/src-tauri/src/listners/mod.rs`

**Responsibilities**:
- Manages bidirectional event flow between frontend and P2P network
- Maintains reference to CurrentNoteState (the live editing buffer)
- Spawns two main async tasks:
  - Tauri event listeners (frontend → P2P)
  - P2P event receiver (P2P → frontend)

**Key Fields**:
```rust
pub struct EventManager {
    app_handle: AppHandle,                     // Tauri app handle for emitting events to frontend
    p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,  // Receives events FROM P2P network
    p2p_sender: P2PSender,                     // Sends messages TO P2P network
    current_note_state: CurrentNoteState,      // Live Yjs document buffer
    repo_ctx: Arc<RepositoryContext>,          // Database access
    crypto_utils: Arc<RwLock<CryptoUtils>>,   // Encryption/decryption
}
```

---

## Communication Flow

### Channel Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                           INITIALIZATION                             │
│                         (in lib.rs setup)                            │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
                    P2PService::new() creates 4 channels
                                    │
        ┌───────────────────────────┼───────────────────────────┐
        │                           │                           │
        ▼                           ▼                           ▼
  p2p_service              p2p_receiver               incoming_receiver
  (Arc<P2PService>)        (mpsc channel)             (mpsc channel)
        │                           │                           │
        │                           │                           │
        │                   ┌───────┴───────┐                   │
        │                   │               │                   │
        ▼                   ▼               ▼                   ▼
  Managed State      EventManager    P2PSender         P2PService::
  (Tauri)            (listners)      (clone given to   start_processing_
                                     EventManager)     incoming_events()
```

### Three Communication Paths

#### Path 1: Frontend → P2P Network (Outgoing Updates)

```
┌─────────────┐        ┌──────────────────┐        ┌─────────────┐
│   Frontend  │  emit  │  Tauri Listener  │  call  │  P2PSender  │
│   (Svelte)  │───────→│  (EventManager)  │───────→│  (network)  │
└─────────────┘        └──────────────────┘        └─────────────┘
                               │
                               │ also updates
                               ▼
                     ┌────────────────────┐
                     │ CurrentNoteState   │
                     │ (Yjs buffer)       │
                     └────────────────────┘
```

**Events**:
- `sync-update` - User's CRDT updates
- `awareness-update` - Cursor position, selections
- `note-change` - User switches notes
- `resource-update-complete` - User saves note
- `request-folder-token` - Request access to folder

**Example Flow**: User types in editor
1. Frontend emits `sync-update` event with Yjs update bytes
2. EventManager's `setup_update_listener()` catches event
3. Updates applied to `CurrentNoteState` buffer
4. Broadcasts to all active P2P connections via `P2PSender`

#### Path 2: P2P Network → Frontend (Incoming Events)

```
┌─────────────┐   recv   ┌──────────────────┐   emit   ┌─────────────┐
│ P2P Network │─────────→│  EventManager    │─────────→│  Frontend   │
│ (incoming)  │  P2PEvent│  .listen_for_    │  Tauri   │  (Svelte)   │
└─────────────┘          │   p2p_events()   │          └─────────────┘
                         └──────────────────┘
```

**Events** (P2PEvent enum):
- System events: `Connected`, `Disconnected`, `HandshakeFailed`, `SyncComplete`
- Live edit negotiation: `LiveEditConnected`, `DocumentCheck`, `DocumentMismatch`
- Real-time updates: `EditingEvent`, `AwarenessEvent`, `UpdatesEvent`
- Update exchange: `UpdateRequest`, `ProcessUpdate`, `ProcessUpdateResponse`
- Resource sync: `ResourceAdded`, `FoldersAdded`, `FolderTokenReceived`

**Example Flow**: Peer sends edit update
1. P2P network receives message, converts to `P2PEvent::EditingEvent`
2. EventManager receives from `p2p_receiver` channel
3. Applies update to `CurrentNoteState` buffer
4. Emits to frontend via Tauri event system

#### Path 3: P2P Network Internal (Incoming Message Handler)

```
┌──────────────────────┐   incoming_receiver   ┌─────────────────────┐
│   P2P Connection     │──────────────────────→│  P2PService::       │
│   (peer messages)    │   (IncomingEvent)     │  start_processing_  │
└──────────────────────┘                       │  incoming_events()  │
                                               └─────────────────────┘
                                                         │
                                               Processes protocol msgs
                                               (handshakes, sync, etc)
                                                         │
                                                         ▼
                                               Emits P2PEvent via
                                               p2p_sender channel
```

---

## CurrentNoteState (Live Edit Buffer)

**Location**: `sthalam/src-tauri/src/current_note_state.rs`

**Purpose**: Maintains in-memory Yjs documents for the currently opened note to enable real-time collaborative editing without constant database reads/writes.

### Structure

```rust
struct Buffers {
    note_id: Option<String>,                   // Current note ID
    main_doc: Doc,                             // Yjs Doc for main content
    image_doc: Doc,                            // Yjs Doc for images
    comment_doc: Doc,                          // Yjs Doc for comments
    shared_users: Vec<String>,                 // Device IDs of users with access
    active_connections: HashSet<String>,       // Peers actively editing
    inactive_connections: HashSet<String>,     // Peers viewing but not editing
}
```

### Key Methods

**Document State Management**:
- `set_current_note()` - Load note from DB into buffer
- `apply_update()` - Apply incoming Yjs update to buffer
- `get_state_vectors()` - Get current state for sync
- `generate_updates_for_peer()` - Generate diff based on peer's state
- `apply_updates_and_generate_diff()` - Bidirectional sync
- `apply_peer_updates()` - One-way sync from peer

**Connection Management**:
- `add_active_connection()` - Peer is live editing
- `remove_active_connection()` - Peer stopped editing
- `add_inactive_connection()` - Peer is viewing only
- `get_active_connections()` - List of live editors
- `set_shared_users()` - Update access list

---

## Live Edit Negotiation Flow

### 1. Connection Establishment

```
Peer A                        Peer B
   │                             │
   │  LiveEditConnected event    │
   ├────────────────────────────→│
   │                             │
   │  DocumentCheck              │
   │  (resource_id)              │
   ├────────────────────────────→│
   │                             │
   │                          checks if
   │                          viewing same
   │                          resource
   │                             │
   │  DocumentCheckResponse      │
   │  (is_match, state_vectors)  │
   │←────────────────────────────┤
   │                             │
```

### 2. Document Mismatch

If peers are viewing different resources:
```
Peer A                        Peer B
   │                             │
   │  DocumentCheckResponse      │
   │  (is_match=false)           │
   │←────────────────────────────┤
   │                             │
   │  DocumentMismatch event     │
   │                             │
   ├──→ add_inactive_connection  │
   │    (will sync on note change)│
   │                             │
```

### 3. Document Match - Synchronization

If peers viewing same resource:
```
Peer A                        Peer B
   │                             │
   │  DocumentCheckResponse      │
   │  (is_match=true, state_vectors)│
   │←────────────────────────────┤
   │                             │
   │  UpdateRequest              │
   │  (state_vectors, resource_id)│
   ├────────────────────────────→│
   │                             │
   │                          generates
   │                          diff updates
   │                             │
   │  ProcessUpdate              │
   │  (updates, client_id)       │
   │←────────────────────────────┤
   │                             │
   │  applies updates            │
   │  generates response         │
   │                             │
   │  ProcessUpdateResponse      │
   │  (updates, client_id)       │
   ├────────────────────────────→│
   │                             │
   ├──→ add_active_connection    │
   │                             │
```

### 4. Real-time Editing

Once synchronized, bidirectional updates flow:
```
Peer A                        Peer B
   │                             │
   │  types in editor            │
   │                             │
   │  EditingEvent               │
   │  (updates, doc_type)        │
   ├────────────────────────────→│
   │                             │
   │                          applies to
   │                          CurrentNoteState
   │                             │
   │                          emits to
   │                          frontend
   │                             │
   │                          types in editor│
   │                             │
   │  EditingEvent               │
   │  (updates, doc_type)        │
   │←────────────────────────────┤
   │                             │
```

---

## Event Handler Methods

### Tauri Event Listeners (Frontend → P2P)

**File**: `sthalam/src-tauri/src/listners/tauri_events.rs`

| Event Name | Handler Method | Description |
|-----------|----------------|-------------|
| `sync-update` | `setup_update_listener()` | User's CRDT updates, applies to buffer and broadcasts |
| `awareness-update` | `setup_update_listener()` | Cursor/selection updates, broadcasts only |
| `note-change` | `setup_note_change_listener()` | Switches notes, manages connections, loads new buffer |
| `resource-update-complete` | `setup_resource_update_complete_listener()` | (Deprecated) Note save complete |
| `request-folder-token` | `setup_request_folder_token_listener()` | Request folder access token |

### P2P Event Handlers (P2P → Frontend)

**File**: `sthalam/src-tauri/src/listners/p2p_reciever.rs`

**System Events**:
- `handle_connected_event()` - Connection established
- `handle_disconnected_event()` - Connection lost
- `handle_handshake_failed_event()` - Authentication failed
- `handle_sync_complete_event()` - Initial sync done
- `handle_error_event()` - P2P error occurred

**Live Edit Handlers** (`p2p_handlers/live_edit.rs`):
- `handle_live_edit_connected()` - Initiate live edit negotiation
- `handle_document_check()` - Verify both viewing same resource
- `handle_document_missmatch()` - Add to inactive connections
- `handle_document_update_request()` - Generate diff for peer
- `handle_document_process_update()` - Apply peer's updates
- `handle_document_process_update_response()` - Apply response, activate connection
- `handle_document_changed_event()` - Peer switched notes

**Update Handlers** (`p2p_handlers/updates.rs`):
- `handle_editing_event()` - Apply real-time CRDT updates
- `handle_awareness_event()` - Update peer awareness (cursor, selection)
- `handle_update_event()` - Process batch updates

**Resource Handlers** (`p2p_handlers/system.rs` + `p2p_handlers/mod.rs`):
- `handle_resource_added()` - New resource shared with user
- `handle_folders_added()` - New folders added
- `handle_folder_token_received()` - Folder access token received

---

## P2P Sender Methods

**File**: `sthalam/src-tauri/src/listners/p2p_sender.rs`

Methods that send messages TO the P2P network:

```rust
impl EventManager {
    // Live edit negotiation
    fn send_live_edit_document_check(connection_id, resource_id)
    fn send_live_edit_document_check_response(connection_id, resource_id, is_match, state_vectors)
    fn send_live_edit_update_exchange(connection_id, resource_id, peer_updates)
    fn send_live_edit_update_exchange_response(connection_id, resource_id, peer_updates)
}
```

All delegate to `P2PSender` which handles actual network message construction.

---

## Reconciliation Timer System

**File**: `sthalam/src-tauri/src/listners/p2p_handlers/live_edit.rs`

**Purpose**: Periodically sync with inactive connections to keep them up-to-date even when not actively editing.

```rust
pub async fn start_reconciliation_timer(&self) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(30));
        loop {
            interval.tick().await;

            let inactive_connections = current_note_state.get_inactive_connections().await;
            if !inactive_connections.is_empty() {
                // Send state vector request to sync inactive peers
                p2p_sender.send_state_vector_request(inactive_connections, resource_id);
            }
        }
    });
}
```

**Runs every 30 seconds** to ensure inactive viewers stay synchronized.

---

## Key Data Flow Examples

### Example 1: User Opens a Note

```
Frontend                EventManager              CurrentNoteState         Database
   │                         │                           │                    │
   │  emit "note-change"     │                           │                    │
   ├────────────────────────→│                           │                    │
   │                         │                           │                    │
   │                         │  get_resource_by_id()     │                    │
   │                         ├───────────────────────────┼───────────────────→│
   │                         │                           │                    │
   │                         │  ← decrypted resource     │                    │
   │                         │←──────────────────────────┼────────────────────┤
   │                         │                           │                    │
   │                         │  set_current_note()       │                    │
   │                         ├──────────────────────────→│                    │
   │                         │  (loads Yjs docs)         │                    │
   │                         │                           │                    │
   │                         │  get_shared_users()       │                    │
   │                         ├───────────────────────────┼───────────────────→│
   │                         │                           │                    │
   │                         │  ← shared users           │                    │
   │                         │←──────────────────────────┼────────────────────┤
   │                         │                           │                    │
   │                         │  set_shared_users()       │                    │
   │                         ├──────────────────────────→│                    │
   │                         │                           │                    │
   │  emit "shared-users"    │                           │                    │
   │←────────────────────────┤                           │                    │
   │                         │                           │                    │
```

### Example 2: User Types in Editor (Live Edit Active)

```
Frontend                EventManager              CurrentNoteState         P2P Network
   │                         │                           │                    │
   │  emit "sync-update"     │                           │                    │
   ├────────────────────────→│                           │                    │
   │                         │                           │                    │
   │                         │  apply_update()           │                    │
   │                         ├──────────────────────────→│                    │
   │                         │  (updates Yjs buffer)     │                    │
   │                         │                           │                    │
   │                         │  get_active_connections() │                    │
   │                         ├──────────────────────────→│                    │
   │                         │                           │                    │
   │                         │  ← [conn1, conn2]         │                    │
   │                         │←──────────────────────────┤                    │
   │                         │                           │                    │
   │                         │  send_sync_update_to_connections()             │
   │                         ├───────────────────────────┼───────────────────→│
   │                         │                           │                    │
   │                         │                           │  ← EditingEvent    │
   │                         │←──────────────────────────┼────────────────────┤
   │  (from P2P receiver)    │                           │                    │
   │                         │                           │                    │
   │                         │  apply_update()           │                    │
   │                         ├──────────────────────────→│                    │
   │                         │                           │                    │
   │  emit to frontend       │                           │                    │
   │←────────────────────────┤                           │                    │
   │                         │                           │                    │
```

### Example 3: Peer Requests to Join Live Edit

```
Peer B                  EventManager A            CurrentNoteState A       P2P Network
   │                         │                           │                    │
   │                         │  LiveEditConnected        │                    │
   │                         │←──────────────────────────┼────────────────────┤
   │                         │  (from p2p_receiver)      │                    │
   │                         │                           │                    │
   │                         │  get_current_note()       │                    │
   │                         ├──────────────────────────→│                    │
   │                         │                           │                    │
   │                         │  ← Some(note_id)          │                    │
   │                         │←──────────────────────────┤                    │
   │                         │                           │                    │
   │                         │  send_live_edit_document_check()               │
   │                         ├───────────────────────────┼───────────────────→│
   │                         │  (connection_id, note_id) │                    │
   │                         │                           │  ─ DocumentCheck ─→│
   │                         │                           │                    │  Peer B
   │                         │                           │                    │
   │                         │  ← DocumentCheck          │                    │
   │                         │←──────────────────────────┼────────────────────┤
   │                         │  (from p2p_receiver)      │                    │
   │                         │                           │                    │
   │                         │  get_current_note()       │                    │
   │                         ├──────────────────────────→│                    │
   │                         │                           │                    │
   │                         │  ← Some(note_id)          │                    │
   │                         │  (matches!)               │                    │
   │                         │                           │                    │
   │                         │  get_state_vectors()      │                    │
   │                         ├──────────────────────────→│                    │
   │                         │                           │                    │
   │                         │  ← state_vectors          │                    │
   │                         │←──────────────────────────┤                    │
   │                         │                           │                    │
   │                         │  send_live_edit_document_check_response()      │
   │                         ├───────────────────────────┼───────────────────→│
   │                         │  (is_match=true, vectors) │                    │
   │                         │                           │  ─ DocumentCheck ─→│
   │                         │                           │     Response       │  Peer B
   │                         │                           │                    │
   │  [Peer B now syncs]     │                           │                    │
   │                         │                           │                    │
   │                         │  ← UpdateRequest          │                    │
   │                         │←──────────────────────────┼────────────────────┤
   │                         │  (Peer B's state_vectors) │                    │
   │                         │                           │                    │
   │                         │  generate_updates_for_peer()                   │
   │                         ├──────────────────────────→│                    │
   │                         │                           │                    │
   │                         │  ← updates JSON           │                    │
   │                         │←──────────────────────────┤                    │
   │                         │                           │                    │
   │                         │  send_live_edit_update_exchange()              │
   │                         ├───────────────────────────┼───────────────────→│
   │                         │                           │  ─ ProcessUpdate ─→│
   │                         │                           │                    │  Peer B
   │                         │                           │                    │
   │                         │  ← ProcessUpdateResponse  │                    │
   │                         │←──────────────────────────┼────────────────────┤
   │                         │                           │                    │
   │                         │  apply_peer_updates()     │                    │
   │                         ├──────────────────────────→│                    │
   │                         │                           │                    │
   │                         │  add_active_connection()  │                    │
   │                         ├──────────────────────────→│                    │
   │                         │  (connection_id)          │                    │
   │                         │                           │                    │
   │  [Now both peers have   │                           │                    │
   │   synchronized state    │                           │                    │
   │   and live editing      │                           │                    │
   │   begins]               │                           │                    │
   │                         │                           │                    │
```

---

## Why This Will Be Removed

The current architecture is tightly coupled to:
1. **Yrs CRDT library** - We're migrating to Loro
2. **Live Edit via P2P** - Complex state management
3. **CurrentNoteState buffer** - In-memory Yjs docs

### New Loro Architecture Will:
1. **Remove live editing initially** - Simplify to sync-on-demand
2. **Use Loro snapshots** - Store complete state, not incremental updates
3. **Unified sync protocol** - Same flow for owner/node/viewer
4. **UCAN-based permissions** - No connection state tracking needed
5. **No in-memory buffer** - Decrypt, sync, re-encrypt on-demand

---

## References

**Related Network Files**:
- `network/src/p2p/incoming_handler.rs` - Processes incoming P2P messages
- `network/src/p2p/p2p_service.rs` - P2P service initialization
- `network/src/p2p/emitter.rs` - Emits P2PEvent to listeners

**Related Protocol Files**:
- `core/src/models/p2p.rs` - P2PEvent and Message enums

**Migration Plan**:
- See `HANDOFF.md` - Phase 3: Network Protocol Migration
- See `MIGRATION_PLAN.md` - Overall migration strategy

---

## Future Implementation Notes

When reimplementing sync after Loro migration:

1. **Keep the channel pattern** - It's clean and works well
2. **Simplify event types** - Remove LiveEdit events
3. **Remove CurrentNoteState** - Use on-demand decryption instead
4. **Keep EventManager pattern** - Good separation of concerns
5. **Consider reconciliation timer** - Useful for keeping viewers synced
6. **Reuse connection management** - Active/inactive pattern is sound

**Do NOT reimplement**:
- Complex Yjs state vector logic
- Live edit negotiation protocol
- In-memory document buffers
- Awareness updates (cursor sharing)

---

**End of Documentation**
