# Active Chat Fix - Backend Not Tracking Current Chat

## Problem

Backend logs showed:
```
[INFO] No active note set for resource: 3d1d56aa-d925-4c7f-a8af-14ad3286d5c5
[INFO] Ignoring sync update for non-active note: 3d1d56aa-d925-4c7f-a8af-14ad3286d5c5
```

**Root Cause**: Frontend was emitting `"chat-change"` but backend was listening for `"note-change"`.

## Backend Listener

**File**: `chat/src-tauri/src/listners/tauri_events.rs`

```rust
// Line 215
fn setup_note_change_listener(&self) {
    // ...
    self.app_handle.listen("note-change", move |event| {  // ← Listens for "note-change"
        let note_state = current_note_state.clone();
        let new_note_id = Self::parse_note_id(event.payload());
        info!("Received note-change event with note_id: {:?}", new_note_id);
        
        tokio::spawn(async move {
            // Set current note, manage connections, etc.
            note_state.set_current_note(new_note_id.clone()).await;
            // ...
        });
    });
}
```

The backend:
1. Listens for `"note-change"` event
2. Updates `CurrentNoteState` with the active resource ID
3. Uses this to determine which connections to notify
4. Ignores sync updates for non-active resources

## Frontend Emissions (Before Fix)

**File**: `chat/frontend/desktop/state/data.svelte.ts`

```typescript
// ❌ WRONG: Emitting "chat-change"
async switchChat(chatId: string | null) {
    emit("chat-change", chatId).catch(error => {  // ← Backend doesn't listen to this!
        console.error("Error updating current chat:", error);
    });
    // ...
}

async addChat(participantId: string, participantName: string) {
    // ... create chat ...
    emit("chat-change", chat.id).catch(error => {  // ← Backend doesn't listen to this!
        console.error("Error updating current chat:", error);
    });
    // ...
}
```

**Result**: Backend never knew which chat was active → ignored all sync updates.

## The Fix

Changed frontend to emit `"note-change"` (same event backend expects):

```typescript
// ✅ CORRECT: Emit "note-change"
async switchChat(chatId: string | null) {
    // Emit "note-change" to notify backend of active chat (backend uses same listener)
    emit("note-change", chatId).catch(error => {
        uiState.clearAllLoadingStates();
        console.error("Error updating current chat:", error);
    });
    
    if (chatId) {
        uiState.setNoteFetching(true);
        uiState.setEditorLoading(false);
        uiState.toggleNoteViewLayout(true);
        
        const chat = await sendMessage("getCredential", { resourceId: chatId });
        this.setCurrentChat(chat);
        this.setCurrentChatId(chatId);
        
        // Load chat content into coordinator
        if (chat.data) {
            await this.loadChat(chatId, chat.data);
        }
        
        uiState.setNoteFetching(false);
        StoreService.setCurrentNoteId(chatId);
    } else {
        dataState.clearCurrentChat();
    }
}

async addChat(participantId: string, participantName: string) {
    // ... create chat ...
    
    // Emit "note-change" to notify backend of active chat
    emit("note-change", chat.id).catch(error => {
        console.error("Error updating current chat:", error);
    });
    
    // ...
}
```

## Why This Works

### Backend Is Generic
The backend doesn't distinguish between "notes" and "chats" - they're all **resources**:
- `CurrentNoteState` tracks the active **resource ID**
- `"note-change"` event works for any resource type
- Chat resources are just resources with `type: "chat"`

### Frontend-Backend Contract
```
Frontend                Backend
--------                -------
emit("note-change", id) → listen("note-change", ...)
                          ↓
                       CurrentNoteState.set_current_note(id)
                          ↓
                       Track active connections for this resource
                          ↓
                       Process sync-update events for this resource
```

### Livnote Comparison
Livnote does the same thing:

**File**: `livnote/frontend/desktop/state/data.svelte.ts`
```typescript
async switchNote(noteId: string | null) {
    emit("note-change", noteId).catch(error => {  // ✅ Emits "note-change"
        uiState.clearAllLoadingStates();
        console.error("Error updating current note:", error);
    });
    // ...
}
```

## Testing

### Before Fix
```
1. Create or switch to a chat
2. Send a message
3. Backend logs:
   [INFO] No active note set for resource: <chat-id>
   [INFO] Ignoring sync update for non-active note: <chat-id>
4. Message doesn't sync to other devices ❌
```

### After Fix
```
1. Create or switch to a chat
2. Backend logs:
   [INFO] Received note-change event with note_id: Some("<chat-id>")
   [DEBUG] Setting current note to: <chat-id>
3. Send a message
4. Backend logs:
   [DEBUG] Applying v2 update, size: X bytes
   [INFO] Broadcasting sync update to Y connections
5. Message syncs successfully ✅
```

## Event Flow

### On Chat Switch
```
User clicks chat
    ↓
dataState.switchChat(chatId)
    ↓
emit("note-change", chatId)  ← Frontend sends event
    ↓
Backend: listen("note-change")  ← Backend receives
    ↓
CurrentNoteState.set_current_note(chatId)
    ↓
Backend tracks active connections for this chat
```

### On Message Send
```
User types message
    ↓
dataState.sendChatMessage(content)
    ↓
coordinator.addMessage(content)
    ↓
YJS doc.on('updateV2') fires
    ↓
emit("sync-update", { resource_id: chatId, updates: [...] })
    ↓
Backend: listen("sync-update")
    ↓
Backend checks: is chatId == currentNoteState.get_current_note()? ✅
    ↓
Backend broadcasts update to all active connections for this chat
    ↓
Remote devices receive and apply update
```

## Files Changed

### Frontend
- `/home/abe/osvauld/chat/frontend/desktop/state/data.svelte.ts`
  - `switchChat()`: Changed `emit("chat-change")` → `emit("note-change")`
  - `addChat()`: Changed `emit("chat-change")` → `emit("note-change")`

### Backend
- No changes needed! Backend already had the correct listener.

## Summary

| Aspect | Before (Broken) | After (Fixed) |
|--------|-----------------|---------------|
| Frontend event | `emit("chat-change")` | `emit("note-change")` ✅ |
| Backend listener | `listen("note-change")` | `listen("note-change")` ✅ |
| Backend state | Not set (no match) ❌ | Set correctly ✅ |
| Sync updates | Ignored ❌ | Processed ✅ |
| P2P broadcast | Skipped ❌ | Sent ✅ |
| Message sync | Fails ❌ | Works ✅ |

**Result**: Chat messages now sync correctly to connected peers! 🎉

## Related Issues

This fix resolves:
1. ✅ "No active note set" log messages
2. ✅ "Ignoring sync update for non-active note" log messages
3. ✅ Messages not syncing to other devices
4. ✅ P2P connections not being notified of updates

## Prevention

### Checklist for New Features
- [ ] Check backend event listeners in `listners/tauri_events.rs`
- [ ] Use exact event names that backend expects
- [ ] Don't create new events if existing ones work
- [ ] Reference working examples (Livnote) for patterns
- [ ] Test P2P sync after emitting events

### Code Review Questions
- Does this emit an event the backend listens for?
- Is the payload format what backend expects?
- Have we verified this works like the equivalent in Livnote?
