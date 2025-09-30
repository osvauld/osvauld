# Frontend Updates Fix

## Problem Statement

After the initial refactoring, messages were being:
1. ✅ Sent between peers (backend to backend)
2. ✅ Applied to backend ChatState buffers
3. ❌ **NOT** showing up in the frontend UI
4. ❌ **NOT** updating chat previews in sidebar
5. ❌ **NOT** incrementing unread counts

Even currently open chats were not displaying new incoming messages.

## Root Cause

The refactored `handle_editing_event()` was:
1. Applying updates to ChatState buffers ✅
2. Emitting "live-updates" to frontend ✅
3. **BUT** not generating chat previews for sidebar updates ❌
4. **AND** missing full document state export for preview generation ❌

## Solution Implemented

### 1. Added Document State Export to ChatState

**File**: `chat/src-tauri/src/chat_state.rs`

**New Method**: `export_document_state()`

```rust
/// Export full document state as bytes (for preview generation)
/// Returns the complete state of the main_doc for a resource
pub async fn export_document_state(&self, resource_id: &str) -> Result<Vec<u8>, String> {
    let buffers = self.buffers.read().await;
    
    let resource_buffers = buffers
        .get(resource_id)
        .ok_or_else(|| format!("No buffers found for resource: {}", resource_id))?;

    // Export the full state of the main document
    use yrs::{StateVector, Transact, ReadTxn};
    let txn = resource_buffers.main_doc.transact();
    let empty_state_vector = StateVector::default();
    let full_state = txn.encode_state_as_update_v2(&empty_state_vector);
    
    Ok(full_state.to_vec())
}
```

**Purpose**: Exports the complete Yjs document state as bytes, which can be used by the ChatPreviewGenerator to extract messages and generate preview data.

### 2. Enhanced Message Handling with Preview Generation

**File**: `chat/src-tauri/src/listners/p2p_handlers/updates.rs`

**Updated**: `handle_editing_event()`

**Flow**:
```
1. Apply update to ChatState buffer
2. Emit "live-updates" to frontend (for editor updates)
3. Generate chat preview (NEW!)
4. Emit "chat-preview-update" to frontend (NEW!)
```

**Key Code**:
```rust
pub(crate) async fn handle_editing_event(
    &self,
    resource_id: String,
    client_id: u32,
    updates: Vec<u8>,
    doc_type: String,
) {
    // Apply to chat state buffer
    self.chat_state
        .apply_update(&resource_id, updates.clone(), &doc_type)
        .await;

    // Emit to frontend for editor updates
    let payload = serde_json::json!({
        "resource_id": resource_id,
        "updates": updates,
        "client_id": client_id.to_string(),
        "doc_type": doc_type,
    });
    
    if let Err(e) = self.emit_json("live-updates", payload) {
        error!("Failed to emit live-updates event: {}", e);
    }

    // Generate and emit chat preview (NEW!)
    self.emit_chat_preview_update(&resource_id).await;
}
```

### 3. Added Preview Generation Method

**New Method**: `emit_chat_preview_update()`

**Purpose**: Generates chat preview data and emits to frontend.

**Steps**:
1. Get current user ID
2. Export full document state from ChatState
3. Use ChatPreviewGenerator to parse messages
4. Calculate unread count
5. Emit "chat-preview-update" event

**Emitted Data**:
```json
{
  "resource_id": "chat-123",
  "last_message": "Hello world!",
  "last_message_time": 1696089600000,
  "unread_count": 3,
  "participants": ["Alice", "Bob"],
  "participant_ids": ["user-1", "user-2"]
}
```

### 4. Added Helper Method

**New Method**: `get_chat_state_bytes()`

**Purpose**: Retrieves the full document state bytes for a resource.

```rust
async fn get_chat_state_bytes(&self, resource_id: &str) -> Result<Vec<u8>, String> {
    // Export the full document state from ChatState
    self.chat_state.export_document_state(resource_id).await
}
```

## Frontend Event Handling

The frontend now needs to listen for **two events**:

### Event 1: "live-updates"
**Purpose**: Real-time Yjs updates for the editor

**Payload**:
```json
{
  "resource_id": "chat-123",
  "updates": [1, 2, 3, ...],  // Yjs update bytes
  "client_id": "12345",
  "doc_type": "main_doc"
}
```

**Frontend Action**:
```javascript
// If this chat is currently open
if (currentChatId === payload.resource_id) {
  // Apply update to Y.Doc
  chatDoc.applyUpdateV2(payload.updates);
  // Messages appear in editor
}
```

### Event 2: "chat-preview-update"
**Purpose**: Update sidebar chat list

**Payload**:
```json
{
  "resource_id": "chat-123",
  "last_message": "Hello world!",
  "last_message_time": 1696089600000,
  "unread_count": 3,
  "participants": ["Alice"],
  "participant_ids": ["user-1"]
}
```

**Frontend Action**:
```javascript
// Update chat in sidebar
updateChatInList({
  id: payload.resource_id,
  lastMessage: payload.last_message,
  lastMessageTime: payload.last_message_time,
  unreadCount: payload.unread_count,
  // ... other fields
});
```

## Complete Message Flow

### Scenario: User B receives message from User A

```
┌─────────────┐       ┌─────────────┐       ┌─────────────┐       ┌─────────────┐
│ Frontend A  │       │  Backend A  │       │  Backend B  │       │ Frontend B  │
└──────┬──────┘       └──────┬──────┘       └──────┬──────┘       └──────┬──────┘
       │                     │                      │                     │
       │ 1. Type message     │                      │                     │
       │ 2. Y.Map update     │                      │                     │
       ├──sync-update────────►                      │                     │
       │                     │                      │                     │
       │                     │ 3. Apply to          │                     │
       │                     │    ChatState         │                     │
       │                     │ 4. Broadcast         │                     │
       │                     ├──DocumentUpdate──────►                     │
       │                     │                      │                     │
       │                     │                      │ 5. Apply to         │
       │                     │                      │    ChatState        │
       │                     │                      │                     │
       │                     │                      ├──live-updates───────► 6a. Apply to
       │                     │                      │                     │     editor (if open)
       │                     │                      │                     │
       │                     │                      │ 7. Generate preview │
       │                     │                      │    (parse Y.Map)    │
       │                     │                      │                     │
       │                     │                      ├─chat-preview-update─► 6b. Update sidebar
       │                     │                      │                     │     - Last message
       │                     │                      │                     │     - Unread count
       │                     │                      │                     │     - Timestamp
```

## Benefits

### 1. Dual-Event Architecture

**live-updates**: Fast, low-latency editor updates
- Only parsed by frontend if chat is open
- Direct Y.Doc application
- No heavy processing in backend

**chat-preview-update**: Comprehensive sidebar data
- Pre-processed in backend
- Frontend just updates UI
- Includes calculated unread counts

### 2. Always Up-to-Date

- Sidebar shows latest message immediately
- Unread counts accurate
- Works for both open and closed chats

### 3. Frontend Simplicity

Frontend only needs to:
1. Apply updates if chat is open
2. Update sidebar with pre-calculated data
3. No need to parse Y.Map in frontend

## Frontend Implementation Example

### Svelte Component (ChatList.svelte)

```svelte
<script>
import { listen } from '@tauri-apps/api/event';
import { onMount } from 'svelte';

let chats = [];

onMount(() => {
  // Listen for chat preview updates
  const unlisten = listen('chat-preview-update', (event) => {
    const payload = event.payload;
    
    // Find chat in list
    const chatIndex = chats.findIndex(c => c.id === payload.resource_id);
    
    if (chatIndex >= 0) {
      // Update existing chat
      chats[chatIndex] = {
        ...chats[chatIndex],
        lastMessage: payload.last_message,
        lastMessageTime: payload.last_message_time,
        unreadCount: payload.unread_count,
        participants: payload.participants,
      };
      
      // Re-sort chats by time
      chats = chats.sort((a, b) => b.lastMessageTime - a.lastMessageTime);
    }
  });
  
  return () => {
    unlisten();
  };
});
</script>

{#each chats as chat}
  <div class="chat-item">
    <div class="chat-name">{chat.participants.join(', ')}</div>
    <div class="last-message">{chat.lastMessage}</div>
    {#if chat.unreadCount > 0}
      <div class="unread-badge">{chat.unreadCount}</div>
    {/if}
    <div class="timestamp">{formatTime(chat.lastMessageTime)}</div>
  </div>
{/each}
```

### Svelte Component (ChatWorkspace.svelte)

```svelte
<script>
import { listen } from '@tauri-apps/api/event';
import { onMount } from 'svelte';

let currentChatId = $state(null);
let chatDoc = null; // Yjs Doc

onMount(() => {
  // Listen for live updates
  const unlisten = listen('live-updates', (event) => {
    const payload = event.payload;
    
    // Only apply if this is the currently open chat
    if (payload.resource_id === currentChatId && chatDoc) {
      // Convert update array back to Uint8Array
      const updateBytes = new Uint8Array(payload.updates);
      
      // Apply to Y.Doc
      Y.applyUpdateV2(chatDoc, updateBytes);
      
      // Yjs will automatically update the UI through bindings
    }
  });
  
  return () => {
    unlisten();
  };
});
</script>
```

## Testing the Fix

### Test 1: Message Appears in Open Chat

1. User A and User B both have Chat 1 open
2. User A types "Hello"
3. **Expected**: Message appears in User B's editor within 100ms

**Backend Logs**:
```
[Backend B] Received editing event for resource chat-1
[Backend B] Applied 45 bytes to chat resource: chat-1
[Backend B] Successfully emitted live-updates event for resource: chat-1
[Backend B] Emitted chat preview update for resource: chat-1 (unread: 0)
```

**Frontend Console**:
```
Received live-updates for chat-1
Applied 45 bytes to Y.Doc
```

### Test 2: Sidebar Updates for Closed Chat

1. User B has Chat 2 open
2. User A sends message in Chat 1
3. **Expected**: Chat 1 in sidebar shows new message and unread count

**Backend Logs**:
```
[Backend B] Received editing event for resource chat-1
[Backend B] Applied 45 bytes to chat resource: chat-1
[Backend B] Successfully emitted live-updates event for resource: chat-1
[Backend B] Emitted chat preview update for resource: chat-1 (unread: 1)
```

**Frontend Console**:
```
Received chat-preview-update for chat-1
Updated sidebar: lastMessage="Hello", unreadCount=1
```

### Test 3: Both Open and Closed Chat Updates

1. User B has 3 chats (Chat 1 open, Chat 2 and 3 closed)
2. Messages arrive in all 3 chats
3. **Expected**:
   - Chat 1: Message appears in editor immediately
   - Chat 2: Sidebar updates with new message and unread +1
   - Chat 3: Sidebar updates with new message and unread +1

## Summary of Changes

| File | Change | Purpose |
|------|--------|---------|
| `chat_state.rs` | Added `export_document_state()` | Export full Yjs state for preview |
| `updates.rs` | Enhanced `handle_editing_event()` | Always emit updates + generate preview |
| `updates.rs` | Added `emit_chat_preview_update()` | Generate and emit preview data |
| `updates.rs` | Added `get_chat_state_bytes()` | Helper to get state bytes |

## Performance Considerations

### Preview Generation Cost

- **When**: On every message received
- **Cost**: ~5-10ms (parse Y.Map, calculate unread)
- **Acceptable**: Preview only generated once per message

### Optimization Opportunities

1. **Debouncing**: If many rapid messages, debounce preview generation
2. **Caching**: Cache last message/unread count, only recalculate on change
3. **Incremental**: Instead of parsing full Y.Map, incrementally update preview

These optimizations can be added if performance becomes an issue with large chat histories.

## Next Steps

1. ✅ Backend changes complete
2. 🔄 Frontend needs to implement event listeners:
   - "live-updates" → Apply to Y.Doc
   - "chat-preview-update" → Update sidebar
3. 🔄 Test all scenarios
4. 🔄 Add read receipts (mark as read when chat opened)

## Related Files

- Chat preview generator: `chat/src-tauri/src/chat_preview_generator.rs`
- Main implementation doc: `chat/CHAT_REFACTORING_IMPLEMENTATION.md`
- Testing guide: `chat/TESTING_GUIDE.md`

