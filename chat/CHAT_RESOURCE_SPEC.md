# Chat Resource Specification

## Resource Type Definition

### Enum Variant
```rust
ResourceType::Chat
```

### YJS Document Fields

#### 1. Primary Document: `chat`
**Purpose**: Store chat messages as a CRDT

**Recommended Structure** (Frontend):
```typescript
// Using Y.Array for message list
const chatDoc = new Y.Doc();
const messages = chatDoc.getArray('messages');

interface ChatMessage {
  id: string;
  authorId: string;
  authorName: string;
  content: string;
  timestamp: number;
  type: 'text' | 'image' | 'file';
}

// Add message
messages.push([{
  id: uuid(),
  authorId: currentUser.id,
  authorName: currentUser.username,
  content: "Hello!",
  timestamp: Date.now(),
  type: 'text'
}]);
```

#### 2. Secondary Document: `image_state`
**Purpose**: Store image/file metadata and references

**Recommended Structure** (Frontend):
```typescript
// Using Y.Map for image metadata
const imageDoc = new Y.Doc();
const images = imageDoc.getMap('images');

interface ImageMetadata {
  id: string;
  messageId: string;
  url: string;
  thumbnailUrl?: string;
  size: number;
  mimeType: string;
  uploadedBy: string;
  uploadedAt: number;
}

// Add image
images.set(imageId, {
  id: imageId,
  messageId: messageId,
  url: blobUrl,
  size: file.size,
  mimeType: file.type,
  uploadedBy: currentUser.id,
  uploadedAt: Date.now()
});
```

## Backend Storage Format

### In Database (Encrypted)
```json
{
  "chat": [1, 2, 3, 4, 5, ...],        // YJS binary state (Uint8Array)
  "image_state": [6, 7, 8, 9, ...]     // YJS binary state (Uint8Array)
}
```

### Resource Metadata
```rust
Resource {
    id: String,
    resource_type: ResourceType::Chat,
    data: String,                    // JSON with chat + image_state
    folder_id: String,
    signature: String,
    created_by: String,
    // ... other fields
}
```

## Sync Protocol

### Document State Keys
```rust
ResourceType::Chat.document_state_keys() => ["chat", "image_state"]
ResourceType::Chat.primary_state_key() => Some("chat")
```

### LiveEdit Message Format
```json
{
  "resource_id": "abc-123",
  "updates": [1, 2, 3, ...],
  "client_id": "12345",
  "doc_type": "chat"  // or "image_state"
}
```

### State Vector Exchange
```json
{
  "chat": {
    "updates": [],                    // Empty on first sync
    "state_vector": [1, 2, 3, ...]   // Current state
  },
  "image_state": {
    "updates": [],
    "state_vector": [4, 5, 6, ...]
  }
}
```

## Frontend Integration

### Creating a Chat Resource
```typescript
const emptyChatContent = {
  chat: [],
  image_state: []
};

const chat = await sendMessage("addCredential", {
  resourcePayload: JSON.stringify(emptyChatContent),
  resourceType: "chat",
  folderId: defaultFolder.id
});
```

### Setting Up YJS Docs
```typescript
class ChatCoordinator {
  private chatDoc: Y.Doc;
  private imageDoc: Y.Doc;
  
  constructor() {
    this.chatDoc = new Y.Doc();
    this.imageDoc = new Y.Doc();
    
    // Subscribe to changes
    this.chatDoc.on('update', (update: Uint8Array) => {
      this.sendUpdate(update, 'chat');
    });
    
    this.imageDoc.on('update', (update: Uint8Array) => {
      this.sendUpdate(update, 'image_state');
    });
  }
  
  applyRemoteUpdate(update: Uint8Array, docType: 'chat' | 'image_state') {
    if (docType === 'chat') {
      Y.applyUpdate(this.chatDoc, update);
    } else {
      Y.applyUpdate(this.imageDoc, update);
    }
  }
}
```

### Handling Incoming Messages
```typescript
// Listen for live updates
listen('live-updates', (event) => {
  const { resource_id, updates, client_id, doc_type } = event.payload;
  
  if (resource_id === currentChatId && client_id !== myClientId) {
    const updateArray = new Uint8Array(updates);
    chatCoordinator.applyRemoteUpdate(updateArray, doc_type);
  }
});
```

## Sharing Flow

### Complete Flow (Implemented in `addChat`)

```typescript
async addChat(participantId: string, participantName: string) {
  // 1. Get default folder
  const defaultFolder = await this.getDefaultFolder();
  
  // 2. Create chat resource with empty YJS documents
  const chatContent = {
    chat: [],           // Y.Array for messages
    image_state: []     // Y.Map for images
  };
  
  const chat = await sendMessage("addCredential", {
    resourcePayload: JSON.stringify(chatContent),
    resourceType: "chat",
    folderId: defaultFolder.id
  });
  
  // 3. Share with selected user
  await this.shareChatWithUser(chat.id, participantId, participantName);
  
  // 4. Set as current chat
  this.setCurrentChat(chat);
  this.setCurrentChatId(chat.id);
}

async shareChatWithUser(chatId: string, userId: string, username: string) {
  const permissions = [
    [`livnote:resource:${chatId}`, "crud/read"],
    [`livnote:resource:${chatId}`, "crud/update"],
    [`livnote:resource:${chatId}`, "ucan/share"]
  ];

  await sendMessage("shareResource", {
    userId: userId,
    resourceId: chatId,
    permissions: permissions
  });
}
```

### Backend Sharing Process
1. **Encrypt resource key** for recipient's public key
2. **Create share record** with UCAN token
3. **Generate vector clocks** for recipient's devices
4. **Save to database** in transaction
5. **Sync via P2P** when both users online

## Message Operations

### Send Text Message
```typescript
const messages = chatDoc.getArray('messages');
messages.push([{
  id: uuid(),
  authorId: userId,
  authorName: username,
  content: messageText,
  timestamp: Date.now(),
  type: 'text'
}]);
// Update automatically sent via YJS observer
```

### Send Image
```typescript
// 1. Upload to Iroh blobs
const blobUrl = await uploadToIroh(imageFile);

// 2. Add to image_state
const images = imageDoc.getMap('images');
images.set(imageId, {
  id: imageId,
  messageId: messageId,
  url: blobUrl,
  size: imageFile.size,
  mimeType: imageFile.type,
  uploadedBy: userId,
  uploadedAt: Date.now()
});

// 3. Add message reference
const messages = chatDoc.getArray('messages');
messages.push([{
  id: messageId,
  authorId: userId,
  authorName: username,
  content: imageId,  // Reference to image
  timestamp: Date.now(),
  type: 'image'
}]);
```

### Delete Message
```typescript
const messages = chatDoc.getArray('messages');
const index = messages.toArray().findIndex(m => m.id === messageId);
if (index !== -1) {
  messages.delete(index, 1);
}
```

## Best Practices

1. **Always use transactions for multiple operations**
   ```typescript
   chatDoc.transact(() => {
     messages.push([newMessage]);
     // other operations
   });
   ```

2. **Handle offline gracefully**
   - YJS automatically reconciles when back online
   - Use vector clocks for conflict resolution

3. **Optimize image_state**
   - Store only metadata, not actual images
   - Use Iroh blobs for actual file storage
   - Clean up old images periodically

4. **Message ordering**
   - Use timestamp for display order
   - YJS guarantees causal consistency
   - Handle out-of-order receives gracefully

5. **Security**
   - All data encrypted at rest
   - UCAN tokens for access control
   - Share records tracked in database
