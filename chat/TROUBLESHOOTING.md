# Chat Troubleshooting Guide

## Issue: Chat showing empty

### Root Cause
The chat coordinator was not being loaded with chat data when switching to or creating a chat.

### Fixes Applied

#### 1. Load Chat Data on Switch (`switchChat`)
```typescript
async switchChat(chatId: string | null) {
  if (chatId) {
    const chat = await sendMessage("getCredential", { resourceId: chatId });
    this.setCurrentChat(chat);
    this.setCurrentChatId(chatId);
    
    // ✅ NEW: Load chat content into coordinator
    if (chat.data) {
      await this.loadChat(chatId, chat.data);
    }
    
    uiState.setNoteFetching(false);
  }
}
```

#### 2. Load Chat Data on Creation (`addChat`)
```typescript
async addChat(participantId: string, participantName: string) {
  // Create chat
  const chat = await sendMessage("addCredential", {...});
  
  // Share with user
  await this.shareChatWithUser(chat.id, participantId, participantName);
  
  this.setCurrentChat(chat);
  this.setCurrentChatId(chat.id);
  
  // ✅ NEW: Load empty chat into coordinator
  if (chat.data) {
    await this.loadChat(chat.id, chat.data);
  }
}
```

#### 3. Use DataState Method in ChatWorkspace
```typescript
// ❌ BEFORE: Manually managing messages
const sendChatMessage = async () => {
  const message = { id, author, content, timestamp, clientId };
  dataState.currentChatMessages = [...dataState.currentChatMessages, message];
  messageInput = "";
};

// ✅ AFTER: Use coordinator through dataState
const sendChatMessage = async () => {
  const content = messageInput.trim();
  messageInput = ""; // Clear input immediately
  
  // Coordinator handles CRDT and auto-updates currentChatMessages
  await dataState.sendChatMessage(content);
};
```

#### 4. Fix Message Display Fields
```typescript
// ❌ BEFORE: Using old fields
{#each dataState.currentChatMessages as message}
  <div>
    {message.author}  
    {message.content}
  </div>
{/each}

// ✅ AFTER: Using ChatMessage interface fields
{#each dataState.currentChatMessages as message}
  <div>
    {message.authorName}  // Changed from 'author'
    {message.content}
  </div>
{/each}

// Check author using authorId not author
{message.authorId === dataState.userDetails?.userId}
```

#### 5. Fix Backend Method Calls
```typescript
// ❌ BEFORE: Wrong method names
await sendMessage("getAllResources", { folderId: defaultFolder.id });
await sendMessage("emitAllResources", { selectedResourceId, folderId });

// ✅ AFTER: Correct method names
await sendMessage("getCredentialsForFolder", { folderId: defaultFolder.id });
await sendMessage("emitAllResources", selectedChatId);
```

### Data Flow

```
1. User creates/switches to chat
   ↓
2. Fetch chat data from backend
   {
     id: "chat-123",
     data: {
       chat: [1, 2, 3, ...],        // YJS state
       image_state: [4, 5, 6, ...]  // YJS state
     }
   }
   ↓
3. loadChat(chatId, chat.data)
   ↓
4. createChatCoordinator(chatId)
   ↓
5. coordinator.loadChat(chatContent)
   - Applies YJS updates to chatDoc and imageDoc
   ↓
6. coordinator.getMessages()
   - Extracts messages from Y.Map
   ↓
7. currentChatMessages = messages
   - Updates reactive state
   ↓
8. UI re-renders with messages
```

### Message Send Flow

```
1. User types message
   ↓
2. dataState.sendChatMessage(content)
   ↓
3. coordinator.addMessage(content, 'text')
   - Adds to Y.Map with generated ID
   ↓
4. Y.Map.set() triggers observe()
   ↓
5. onMessagesChange callback fires
   ↓
6. currentChatMessages updated automatically
   ↓
7. YJS update event fires
   ↓
8. onChatUpdate callback
   ↓
9. emit("sync-update") to backend
   ↓
10. P2P forwards to peer
   ↓
11. Peer applies update
   ↓
12. Peer's currentChatMessages updates
```

### Debugging Tips

#### Check if Coordinator Exists
```typescript
const coordinator = dataState.getChatCoordinator(chatId);
console.log("Coordinator exists:", !!coordinator);
```

#### Check Messages in Coordinator
```typescript
const messages = coordinator?.getMessages();
console.log("Messages in coordinator:", messages);
```

#### Check Current Chat Messages
```typescript
console.log("Current chat messages:", dataState.currentChatMessages);
console.log("Current chat ID:", dataState.getCurrentChatId());
```

#### Check if Chat Data Loaded
```typescript
// In loadChat method (already added)
console.log("Loading chat:", chatId, "with content:", chatContent);
console.log("Loaded messages:", messages);
```

#### Check Message Sending
```typescript
// In sendChatMessage method (already added)
console.log("Sending message:", message);
console.log("Message added to coordinator:", addedMessage);
```

### Common Issues

#### Issue: `Error processing message: EndOfBuffer` when creating new chat
**Error**: 
```
ERROR network::p2p::peer_connection: Error processing message: 
Service layer error: Resource error: Parse error: 
Failed to decode v2 update: EndOfBuffer(21382106)
```

**Cause**: Empty arrays `[]` are not valid YJS state. Backend tries to decode them and fails.

**Fix**: Create proper YJS documents with encoded state

```typescript
// ❌ WRONG: Empty arrays are not valid YJS
createEmptyChatContent() {
  return {
    chat: [],           // Invalid!
    image_state: []     // Invalid!
  };
}

// ✅ CORRECT: Encode actual YJS documents
createEmptyChatContent() {
  // Create fresh YJS documents
  const chatDoc = new Y.Doc();
  const imageDoc = new Y.Doc();
  
  // Initialize shared types (creates valid YJS state)
  chatDoc.getMap('messages');
  imageDoc.getMap('images');
  
  // Encode as updates
  const chatState = Y.encodeStateAsUpdate(chatDoc);
  const imageState = Y.encodeStateAsUpdate(imageDoc);
  
  return {
    chat: Array.from(chatState),           // Valid YJS state
    image_state: Array.from(imageState)    // Valid YJS state
  };
}
```

**Why This Happens**:
1. Backend receives resource with `chat: []` and `image_state: []`
2. When syncing, backend tries to apply updates using `Y.applyUpdate(doc, [])`
3. YJS expects proper binary format, not empty array
4. Decoding fails with `EndOfBuffer` error

**Solution**: Always encode YJS documents properly, even when empty. An empty YJS document still has a valid binary representation that includes metadata.

### Common Issues

#### Issue: Messages not showing after sending
**Cause**: Observer not set up correctly
**Fix**: Ensure `createChatCoordinator` subscribes to `onMessagesChange`

```typescript
const unobserve = coordinator.onMessagesChange((messages) => {
  if (this.currentChatId === chatId) {
    this.currentChatMessages = messages;
  }
});
this._messageObservers.set(chatId, unobserve);
```

#### Issue: Messages duplicate on page refresh
**Cause**: Not clearing old coordinator before creating new one
**Fix**: Destroy and cleanup before creating

```typescript
if (this.chatCoordinators.has(chatId)) {
  const existing = this.chatCoordinators.get(chatId);
  existing?.destroy();
  
  // Cleanup observer
  const observer = this._messageObservers.get(chatId);
  if (observer) {
    observer();
    this._messageObservers.delete(chatId);
  }
}
```

#### Issue: Empty chat on first load
**Cause**: Chat data not being loaded into coordinator
**Fix**: Call `loadChat` in both `switchChat` and `addChat`

```typescript
if (chat.data) {
  await this.loadChat(chatId, chat.data);
}
```

#### Issue: Messages from wrong user
**Cause**: Using `author` instead of `authorId`
**Fix**: Update comparison

```typescript
// ❌ Wrong
message.author === dataState.userDetails?.username

// ✅ Correct  
message.authorId === dataState.userDetails?.userId
```

### Expected Console Output

When creating a new chat:
```
Creating chat with user: Alice
Loading chat: chat-123 with content: { chat: [], image_state: [] }
Loaded messages: []
Chat with Alice created
```

When sending first message:
```
Sending message: Hello!
Message added to coordinator: {
  id: "user-123-1234567890-abc",
  authorId: "user-123",
  authorName: "Bob",
  content: "Hello!",
  timestamp: 1234567890,
  type: "text"
}
```

When switching to existing chat:
```
Loading chat: chat-123 with content: { chat: [1,2,3...], image_state: [] }
Loaded messages: [
  { id: "...", authorName: "Alice", content: "Hi!", ... },
  { id: "...", authorName: "Bob", content: "Hello!", ... }
]
```

### Verification Checklist

- [ ] Chat loads when clicking on it from list
- [ ] New chat starts empty
- [ ] Messages appear immediately after sending
- [ ] Messages persist after page refresh
- [ ] Messages from other user show on left
- [ ] Your messages show on right
- [ ] Timestamps display correctly
- [ ] Author names display correctly
- [ ] No duplicate messages
- [ ] No console errors
