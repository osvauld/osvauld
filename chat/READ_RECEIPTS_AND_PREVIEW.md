# Read Receipts and Chat Preview System

## Overview
Complete rewrite of the preview generation system for chat (since we're not using ProseMirror anymore) and implementation of read receipts functionality.

## Changes Made

### 1. Backend: New Chat Preview Generator

**File**: `/chat/src-tauri/src/chat_preview_generator.rs`

Complete rewrite from ProseMirror-based preview to chat-specific preview:

#### Features:
- **Extract messages from Y.Map** (not ProseMirror XML)
- **Generate chat preview data** including:
  - Last message content
  - Last message timestamp
  - Participant names (excluding current user)
  - Participant IDs
  - Unread message count

#### Key Structures:

```rust
pub struct ChatMessage {
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    pub content: String,
    pub timestamp: i64,
    pub message_type: String,
    pub image_id: Option<String>,
    pub read_by: Option<Vec<String>>, // NEW: User IDs who have read this
}

pub struct ChatPreviewData {
    pub last_message: String,
    pub last_message_time: i64,
    pub participants: Vec<String>,      // All participant names
    pub participant_ids: Vec<String>,   // All participant IDs
    pub unread_count: i32,              // NEW: Unread message count
}
```

#### Main Functions:

1. **`generate_chat_preview()`**
   - Extracts all messages from Y.Map
   - Finds the last message by timestamp
   - Collects unique participants (excluding current user)
   - Calculates unread count (messages not in readBy list for current user)

2. **`extract_messages()`**
   - Reads from Y.Map structure
   - Parses message objects including readBy field
   - Returns sorted messages by timestamp

3. **`mark_message_read()`**
   - Updates a message's readBy array
   - Adds user ID if not already present
   - Returns updated YJS state

### 2. Frontend: ChatCoordinator Updates

**File**: `/chat/frontend/desktop/components/chat/chatCoordinator.ts`

#### Added Read Receipt Support:

**Interface Update:**
```typescript
export interface ChatMessage {
  id: string;
  authorId: string;
  authorName: string;
  content: string;
  timestamp: number;
  type: 'text' | 'image';
  imageId?: string;
  readBy?: string[];  // NEW: Array of user IDs who have read this
}
```

#### New Methods:

1. **`addMessage()`** - Updated
   - Auto-adds author to `readBy` when creating message
   - Author has read their own message by default

2. **`markMessageAsRead(messageId)`** - NEW
   - Marks a specific message as read by current user
   - Adds current user ID to readBy array

3. **`markAllMessagesAsRead()`** - NEW
   - Marks all messages from other users as read
   - Called when opening a chat

4. **`getUnreadCount()`** - NEW
   - Returns count of unread messages for current user
   - Filters messages not in readBy list

### 3. Frontend: Data State Updates

**File**: `/chat/frontend/desktop/state/data.svelte.ts`

#### Auto-mark messages as read:
```typescript
async switchChat(chatId: string | null) {
  // ... load chat ...
  
  // NEW: Mark all messages as read when opening the chat
  const coordinator = this.getChatCoordinator(chatId);
  if (coordinator) {
    coordinator.markAllMessagesAsRead();
  }
}
```

### 4. Frontend: ChatListView Updates

**File**: `/chat/frontend/desktop/components/chat/ChatListView.svelte`

#### New Function:
```typescript
const getUnreadCount = (chatId: string): number => {
  const coordinator = dataState.getChatCoordinator(chatId);
  if (coordinator) {
    return coordinator.getUnreadCount();
  }
  
  // Fallback to stored unread count
  const chat = dataState.getChatById(chatId);
  return chat?.unreadCount || 0;
};
```

#### UI Update:
- Shows unread count badge dynamically
- Updates in real-time as messages are read/received
- Uses lavender badge like WhatsApp

## How It Works

### Read Receipts Flow:

1. **Sending a Message:**
   ```
   User types message → addMessage() → 
   Message created with readBy: [authorId] →
   YJS update sent to peers
   ```

2. **Opening a Chat:**
   ```
   User clicks chat → switchChat() →
   Chat loaded → markAllMessagesAsRead() →
   All unread messages marked as read →
   YJS update sent to peers
   ```

3. **Displaying Unread Count:**
   ```
   Chat list renders → getUnreadCount() →
   Coordinator counts messages where:
     - authorId !== currentUserId
     - readBy doesn't include currentUserId
   → Badge shows count
   ```

### Preview Generation Flow:

1. **Backend generates preview:**
   ```
   Chat state → extract_messages() →
   Find last message →
   Collect participants (excluding current user) →
   Count unread messages →
   Return ChatPreviewData
   ```

2. **Frontend displays preview:**
   ```
   ChatListView →
   getChatParticipantNames() → "abe3, john" →
   getLastMessagePreview() → "Hey there!" →
   getUnreadCount() → 3 →
   Display in chat list
   ```

## Key Features

### ✅ Read Receipts
- Author automatically marked as read on message creation
- All messages marked as read when opening chat
- Real-time unread count calculation
- Per-message read tracking

### ✅ Chat Preview
- Shows last message content (not HTML preview)
- Shows participant names (excluding current user)
- Shows unread count dynamically
- Fallback to stored data if coordinator not loaded

### ✅ WhatsApp-like Behavior
- Unread badges in lavender
- Auto-read on chat open
- Real-time updates
- Participant list (comma-separated)

## Data Flow

### YJS Structure:
```javascript
{
  "messages": Y.Map {
    "msg-id-1": {
      id: "msg-id-1",
      authorId: "user-123",
      authorName: "John",
      content: "Hello!",
      timestamp: 1234567890,
      type: "text",
      readBy: ["user-123", "user-456"] // Read by John and abe3
    },
    "msg-id-2": {
      id: "msg-id-2",
      authorId: "user-456",
      authorName: "abe3",
      content: "Hi!",
      timestamp: 1234567891,
      type: "text",
      readBy: ["user-456"] // Read by abe3 only (unread for John)
    }
  }
}
```

### Chat Preview Data:
```json
{
  "lastMessage": "Hi!",
  "lastMessageTime": 1234567891,
  "participants": ["John"],  // Excludes current user (abe3)
  "participantIds": ["user-123"],
  "unreadCount": 1  // One message John hasn't read
}
```

## UI Updates

### Chat List Preview:
```
┌────────────────────────────┐
│ ● John                   3 │  ← Unread count badge
│   Hi there!       10:30    │  ← Last message + time
└────────────────────────────┘
```

### Group Chat Preview:
```
┌────────────────────────────┐
│ ● John, Sarah, Mike      5 │  ← Multiple participants
│   See you tomorrow! 9:15   │  ← Last message + time
└────────────────────────────┘
```

## Backend Integration Points

To integrate with the backend preview generator:

1. **Add to `src/lib.rs`:**
```rust
mod chat_preview_generator;
pub use chat_preview_generator::*;
```

2. **Use in resource handlers:**
```rust
use crate::chat_preview_generator::generate_chat_preview;

let preview_data = generate_chat_preview(&resource.data, &current_user_id).await?;

// Use preview_data.last_message, preview_data.participants, etc.
```

3. **Update ResourcePreview struct** to include chat-specific fields:
```rust
pub struct ChatResourcePreview {
    pub id: String,
    pub last_message: String,
    pub last_message_time: i64,
    pub participants: Vec<String>,
    pub unread_count: i32,
    // ... other fields
}
```

## Testing Checklist

### Read Receipts:
- [ ] Send a message - verify readBy contains sender
- [ ] Open a chat - verify all messages marked as read
- [ ] Check unread count - verify correct count shown
- [ ] Receive message - verify unread count increases
- [ ] Open chat with unread messages - verify count resets to 0

### Preview Generation:
- [ ] Verify last message shows in chat list
- [ ] Verify participant names (excluding self) show correctly
- [ ] Verify unread count badge appears when > 0
- [ ] Test with no messages - verify "No messages yet"
- [ ] Test with group chat - verify comma-separated participants

## Future Enhancements

1. **Read receipt indicators in messages**
   - Show checkmarks (single/double) like WhatsApp
   - Blue checkmarks for read messages

2. **Typing indicators**
   - Show "John is typing..." in chat list
   - Real-time updates via awareness

3. **Message delivery status**
   - Sent (checkmark)
   - Delivered (double checkmark)
   - Read (blue double checkmark)

4. **Batch read marking**
   - Mark messages as read on scroll
   - Viewport-based read tracking

5. **Read by list**
   - Show who has read each message (group chats)
   - Tooltip or modal with reader names

## Notes

- **ProseMirror completely removed** from chat preview generation
- Read receipts stored in YJS, synced across all peers
- Unread counts calculated client-side for performance
- Backend provides initial preview data
- Frontend keeps it updated dynamically

---

**Status**: ✅ Frontend complete, Backend needs integration
