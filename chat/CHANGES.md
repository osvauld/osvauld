# Chat Application Changes

## Backend Changes

### 1. Core Resource Type Updates (`core/src/models/resource.rs`)

**Added Chat Resource Type:**
```rust
pub enum ResourceType {
    Notes,
    Chat,    // NEW
    Default,
}
```

**Chat Resource Configuration:**
- **Primary YJS State Key**: `chat` - Main chat messages and content
- **Secondary State Key**: `image_state` - For image attachments and media
- **CRDT Support**: Enabled (full real-time sync capability)

**Document State Keys:**
```rust
ResourceType::Chat => vec!["chat", "image_state"]
```

This means chat resources will have two separate YJS documents:
1. `chat`: Primary document for chat messages (Y.Array of messages)
2. `image_state`: For managing shared images/attachments

### 2. Listeners (src-tauri/src/listners/)

**No changes needed** - The existing P2P listeners are already generic and work with all resource types:

- `live_edit.rs`: Handles real-time editing events for any CRDT-enabled resource
- `updates.rs`: Processes document updates, awareness, and resource changes
- All handlers check `resource_type.has_crdt()` and `resource_type.document_state_keys()`

## Frontend Changes

### Components Removed (Unnecessary for Chat)

1. **`components/layout/NotesWorkspace.svelte`** ✗
   - Replaced by ChatWorkspace.svelte
   
2. **`components/modals/CollaboratorSelector.svelte`** ✗
   - Not needed - chat uses direct user selection
   
3. **`components/modals/ShareNote.svelte`** ✗
   - Chat sharing handled differently
   
4. **`components/modals/ShareFolder.svelte`** ✗
   - Simplified folder model for chat
   
5. **`components/ui/FolderManager.svelte`** ✗
   - Chats use default folder only

### Components Kept (Essential for Chat)

1. **`components/chat/`**
   - `ChatListView.svelte` - List of chat conversations
   - `ChatPreview.svelte` - Preview of chat messages
   - `ChatWorkspace.svelte` - Main chat interface

2. **`components/modals/`**
   - `UserSelectionModal.svelte` - Select user to chat with
   - `ConnectUserModal.svelte` - P2P connection
   - `DeleteConfirmationModal.svelte` - Confirm deletions
   - `Toast.svelte` - Notifications

3. **`components/ui/`**
   - `ChatListPanel.svelte` - Search and add chat
   - `AddUserForm.svelte` - Add known users

4. **`components/connection/`**
   - All kept for P2P setup

5. **`components/layout/`**
   - `HeaderSection.svelte` - App header
   - `NavigationPanel.svelte` - Side navigation
   - `ProfileView.svelte` - User profile

## How Chat Resources Work

### Resource Creation & Sharing Flow
```typescript
// 1. Create chat resource
const chat = await sendMessage("addCredential", {
  resourcePayload: JSON.stringify({
    chat: [],              // Empty Y.Array for messages
    image_state: []        // Empty Y.Map for images
  }),
  resourceType: "chat",
  folderId: defaultFolder.id
});

// 2. Immediately share with selected user
await sendMessage("shareResource", {
  userId: participantId,
  resourceId: chat.id,
  permissions: [
    [`livnote:resource:${chat.id}`, "crud/read"],
    [`livnote:resource:${chat.id}`, "crud/update"],
    [`livnote:resource:${chat.id}`, "ucan/share"]
  ]
});

// 3. Resource automatically syncs via P2P when both users online
```

### Live Sync Flow
1. User types message → Frontend applies to Y.Array
2. Y.Array emits update → Sent via LiveEdit channel
3. Backend routes update based on doc_type ("chat" or "image_state")
4. Peer receives update → Applies to their Y.Array
5. Messages merge automatically via CRDT

### Data Structure
```json
{
  "chat": [1, 2, 3, ...],        // YJS binary state
  "image_state": [4, 5, 6, ...]  // YJS binary state
}
```

Each is an independent YJS document that syncs separately but efficiently.

## Migration from Livnote

The chat app reuses Livnote's:
- ✅ Authentication system
- ✅ P2P networking layer
- ✅ CRDT sync infrastructure
- ✅ Resource encryption
- ✅ User management

New for Chat:
- 🆕 Chat-specific resource type
- 🆕 Message-oriented UI
- 🆕 Simplified folder model
- 🆕 Direct user-to-user resources

## Testing Checklist

- [ ] Create new chat resource
- [ ] Share with another user
- [ ] Send messages (CRDT sync)
- [ ] Send images (image_state sync)
- [ ] Verify both YJS docs sync independently
- [ ] Test offline/online reconciliation
- [ ] Verify encryption/decryption
- [ ] Test multi-device chat
