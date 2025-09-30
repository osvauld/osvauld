# ChatCoordinator Implementation

## Overview

The `ChatCoordinator` is a simplified alternative to Livnote's `NotesCoordinator`. Instead of managing ProseMirror documents, it manages simple chat messages using Y.Map for the chat document and Y.Map for images.

## Architecture

### YJS Documents

**Two separate Y.Doc instances:**

1. **chatDoc** - Contains messages
   - `Y.Map<ChatMessage>` where key = messageId, value = ChatMessage
   
2. **imageDoc** - Contains image metadata
   - `Y.Map<ImageMetadata>` where key = imageId, value = metadata

### Message Structure

```typescript
interface ChatMessage {
  id: string;              // Unique message ID
  authorId: string;        // User ID who sent it
  authorName: string;      // Display name
  content: string;         // Message text or image reference
  timestamp: number;       // Millisecond timestamp
  type: 'text' | 'image';  // Message type
  imageId?: string;        // Reference to imageDoc if type='image'
}
```

### Image Metadata Structure

```typescript
interface ImageMetadata {
  id: string;
  url: string;             // Iroh blob URL
  size: number;
  mimeType: string;
  thumbnailUrl?: string;
  uploadedBy: string;
  uploadedAt: number;
}
```

## Key Methods

### Initialization

```typescript
const coordinator = new ChatCoordinator({
  userInfo: {
    id: clientId,
    userId: "user-uuid",
    name: "Username",
    color: "#FF5630"
  },
  onChatUpdate: (update: Uint8Array) => {
    // Emit to backend for P2P sync
    emit("sync-update", {
      update: Array.from(update),
      resource_id: chatId,
      doc_type: "chat"
    });
  },
  onImageUpdate: (update: Uint8Array) => {
    // Emit image state updates
    emit("sync-update", {
      update: Array.from(update),
      resource_id: chatId,
      doc_type: "image_state"
    });
  }
});
```

### Loading Existing Chat

```typescript
await coordinator.loadChat({
  chat: [1, 2, 3, ...],        // YJS state as number array
  image_state: [4, 5, 6, ...]  // YJS state as number array
});
```

### Sending Messages

```typescript
// Text message
const message = coordinator.addMessage("Hello!", 'text');
// Result: { id, authorId, authorName, content, timestamp, type }

// Image message (after uploading to Iroh)
coordinator.addImage(imageId, {
  url: blobUrl,
  size: fileSize,
  mimeType: "image/png"
});
const imageMsg = coordinator.addMessage(imageId, 'image', imageId);
```

### Receiving Messages

```typescript
// Subscribe to message changes
const unsubscribe = coordinator.onMessagesChange((messages) => {
  // messages is ChatMessage[] sorted by timestamp
  this.currentChatMessages = messages;
});

// Don't forget to cleanup
unsubscribe();
```

### Remote Updates

```typescript
// From LiveEdit channel
coordinator.applyRemoteUpdate(
  updateArray,     // Uint8Array
  senderId,        // number
  'chat'           // 'chat' | 'image_state'
);
```

### Saving

```typescript
// Save current state
const saved = coordinator.saveChat();
// Returns: { chat: number[], image_state: number[] }

// Send to backend
await sendMessage("updateCredential", {
  id: chatId,
  data: JSON.stringify(saved)
});
```

## Integration with Data State

### Creating Coordinator

```typescript
private createChatCoordinator(chatId: string) {
  const coordinator = new ChatCoordinator({
    userInfo: {
      name: this.userDetails.username,
      color: this.generateUserColor(),
      id: this.clientId,
      userId: this.userDetails.userId,
    },
    onChatUpdate: async (update) => {
      await emit("sync-update", {
        update: Array.from(update),
        clientID: this.clientId,
        resource_id: chatId,
        doc_type: "chat",
      });
    },
    onImageUpdate: async (update) => {
      await emit("sync-update", {
        update: Array.from(update),
        clientID: this.clientId,
        resource_id: chatId,
        doc_type: "image_state",
      });
    },
  });
  
  // Subscribe to message changes
  const unobserve = coordinator.onMessagesChange((messages) => {
    if (this.currentChatId === chatId) {
      this.currentChatMessages = messages;
    }
  });
  
  this._messageObservers.set(chatId, unobserve);
  this.chatCoordinators.set(chatId, coordinator);
  
  return coordinator;
}
```

### Sending Messages

```typescript
async sendChatMessage(message: string): Promise<void> {
  const coordinator = this.getChatCoordinator(this.currentChatId);
  
  // Add to Y.Map - triggers update handler
  coordinator.addMessage(message, 'text');
  
  // Save to backend
  await this.saveChat(this.currentChatId);
}
```

### Handling Live Updates

```typescript
async handleLiveUpdates(event: any) {
  const { resource_id, updates, client_id, doc_type } = event.payload;
  
  const coordinator = this.getChatCoordinator(resource_id);
  const updatesArray = new Uint8Array(updates);
  const senderId = parseInt(client_id, 10);
  
  // Apply remote changes
  coordinator?.applyRemoteUpdate(updatesArray, senderId, doc_type);
  
  // Messages automatically update via onMessagesChange subscription
}
```

## Comparison with NotesCoordinator

| Feature | NotesCoordinator | ChatCoordinator |
|---------|------------------|-----------------|
| **Editor** | ProseMirror | None (direct Y.Map) |
| **Primary Doc** | Y.XmlFragment | Y.Map<ChatMessage> |
| **Complexity** | High (plugins, schema, etc.) | Low (just YJS) |
| **Data Structure** | Rich text XML | Simple JSON objects |
| **Use Case** | Rich text editing | Chat messages |
| **File Size** | ~400 lines | ~350 lines |
| **Dependencies** | prosemirror-*, y-prosemirror | Only yjs |

## Sync Flow

```
User types message
      ↓
coordinator.addMessage("Hello")
      ↓
Y.Map.set(messageId, message)
      ↓
chatDoc.on('update') fires
      ↓
onChatUpdate callback
      ↓
emit("sync-update") to backend
      ↓
P2P forwards to peer
      ↓
peer: handleLiveUpdates()
      ↓
coordinator.applyRemoteUpdate()
      ↓
Y.Map updated
      ↓
messages.observe() fires
      ↓
onMessagesChange callback
      ↓
currentChatMessages updated
      ↓
UI re-renders
```

## State Vector Exchange

The coordinator implements the same state vector protocol as NotesCoordinator:

```typescript
// Get state vectors
const vectors = await coordinator.getStateVectors();
// Returns: 
// {
//   chat: { updates: [], state_vector: [1, 2, 3...] },
//   image_state: { updates: [], state_vector: [4, 5, 6...] }
// }

// Generate updates for peer
const updates = await coordinator.generateUpdatesForPeer(peerVectors);

// Apply and generate diff
const diff = await coordinator.applyUpdatesAndGenerateDiff(remoteUpdates);
```

## Message Ordering

Messages are stored in Y.Map with messageId as key. Order is determined by:

1. **Timestamp** - Primary sort key
2. **MessageId** - Secondary (includes timestamp + random)

```typescript
getMessages(): ChatMessage[] {
  const messages: ChatMessage[] = [];
  this.messages.forEach((message) => messages.push(message));
  return messages.sort((a, b) => a.timestamp - b.timestamp);
}
```

YJS guarantees:
- **Causal consistency** - If A caused B, B will be seen after A
- **Eventual consistency** - All peers converge to same state
- **Concurrent edits** - Deterministic merge (last-write-wins by timestamp)

## Cleanup

```typescript
// In data.svelte.ts clearAllState()
this.chatCoordinators.forEach(coordinator => {
  coordinator.destroy();
});
this.chatCoordinators.clear();

// Cleanup observers
this._messageObservers.forEach(unobserve => unobserve());
this._messageObservers.clear();
```

## Future Enhancements

### Typing Indicators (Awareness)

```typescript
// Add to ChatCoordinator
import { awarenessProtocol } from 'y-protocols/awareness';

this.awareness = new awarenessProtocol.Awareness(this.chatDoc);

this.awareness.on('change', ({ added, updated, removed }) => {
  // Emit typing status
  if (config.onAwarenessUpdate) {
    config.onAwarenessUpdate(
      awarenessProtocol.encodeAwarenessUpdate(this.awareness, [...added, ...updated, ...removed])
    );
  }
});
```

### Message Editing

```typescript
editMessage(messageId: string, newContent: string): void {
  const message = this.messages.get(messageId);
  if (message && message.authorId === this.userInfo.userId) {
    this.messages.set(messageId, {
      ...message,
      content: newContent,
      edited: true,
      editedAt: Date.now()
    });
  }
}
```

### Message Reactions

```typescript
addReaction(messageId: string, emoji: string): void {
  const message = this.messages.get(messageId);
  if (message) {
    const reactions = message.reactions || {};
    reactions[this.userInfo.userId] = emoji;
    this.messages.set(messageId, { ...message, reactions });
  }
}
```

## Benefits

1. **Simplicity** - No ProseMirror complexity
2. **Performance** - Lighter weight than rich text editor
3. **Real-time** - Automatic CRDT merging
4. **Scalability** - Y.Map is efficient for large message lists
5. **Flexibility** - Easy to extend with reactions, edits, etc.

## Testing

```typescript
// Unit test example
const coordinator = new ChatCoordinator({
  userInfo: testUser,
  onChatUpdate: vi.fn(),
  onImageUpdate: vi.fn()
});

// Send message
const msg = coordinator.addMessage("Test");
expect(msg.content).toBe("Test");
expect(msg.authorId).toBe(testUser.userId);

// Verify it's in the list
const messages = coordinator.getMessages();
expect(messages).toHaveLength(1);
expect(messages[0].content).toBe("Test");

// Cleanup
coordinator.destroy();
```
