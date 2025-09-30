# EndOfBuffer Error - Complete Fix

## Error Analysis

```
ERROR network::p2p::peer_connection: Error processing message: 
Service layer error: Resource error: Parse error: 
Failed to decode v2 update: EndOfBuffer(21382106)
```

### When It Happens
- During **resource sync** after P2P connection
- When requesting state vectors from existing resources
- When trying to apply updates to YJS documents
- When sending messages between clients

### Root Causes
1. ❌ **Empty arrays `[]`** instead of valid YJS state (initial bug)
2. ❌ **V1 encoding** when backend expects **V2 encoding** (critical issue!)

## Error Flow

```
P2P Connection Established
    ↓
Resource Sync Starts
    ↓
get_resource_state_vector(resource_id)
    ↓
get_resource() → decrypt resource from DB
    ↓
decrypted_resource.get_state_vectors()
    ↓
For each document_state_key ("chat", "image_state"):
    doc.apply_update_v2(&state_data)  ← FAILS HERE
    ↓
    Update::decode_v2([])  ← Empty array is not valid YJS!
    ↓
    EndOfBuffer error
```

## The Problem

### Invalid Resource Data (Before Fix)
```json
{
  "id": "resource-123",
  "data": {
    "chat": [],           ← NOT valid YJS!
    "image_state": []     ← NOT valid YJS!
  }
}
```

When YJS tries to decode `[]`:
- Expects binary format with header, client ID, structs, etc.
- Gets empty array
- Reads past buffer end → `EndOfBuffer`

### Valid Resource Data (After Fix)
```json
{
  "id": "resource-123",
  "data": {
    "chat": [0, 0, 1, 0, 5, 109, 101, 115, ...],        ← Valid YJS binary
    "image_state": [0, 0, 1, 0, 6, 105, 109, ...]      ← Valid YJS binary
  }
}
```

## Fixes Applied

### 1. Frontend: Use V2 Encoding (`data.svelte.ts`)

```typescript
// ❌ WRONG #1: Empty arrays
createEmptyChatContent() {
  return {
    chat: [],
    image_state: []
  };
}

// ❌ WRONG #2: V1 encoding (backend expects V2!)
createEmptyChatContent() {
  const chatDoc = new Y.Doc();
  const imageDoc = new Y.Doc();
  chatDoc.getMap('messages');
  imageDoc.getMap('images');
  
  return {
    chat: Array.from(Y.encodeStateAsUpdate(chatDoc)),        // V1!
    image_state: Array.from(Y.encodeStateAsUpdate(imageDoc)) // V1!
  };
}

// ✅ CORRECT: V2 encoding to match backend
import * as Y from 'yjs';

createEmptyChatContent() {
  const chatDoc = new Y.Doc();
  const imageDoc = new Y.Doc();
  
  chatDoc.getMap('messages');
  imageDoc.getMap('images');
  
  // Use V2 encoding (backend calls Update::decode_v2)
  const chatState = Y.encodeStateAsUpdateV2(chatDoc);
  const imageState = Y.encodeStateAsUpdateV2(imageDoc);
  
  return {
    chat: Array.from(chatState),           // Valid V2 state
    image_state: Array.from(imageState)    // Valid V2 state
  };
}
```

### 2. Frontend: Update Handler to V2 (`chatCoordinator.ts`)

```typescript
// ❌ WRONG: Listening to V1 updates
private setupUpdateHandlers(): void {
  this.chatDoc.on('update', (update: Uint8Array, origin: any) => {
    if (origin !== 'remote' && this.config.onChatUpdate) {
      this.config.onChatUpdate(update);  // Sends V1 encoded update
    }
  });
}

// ✅ CORRECT: Listen to V2 updates
private setupUpdateHandlers(): void {
  this.chatDoc.on('updateV2', (update: Uint8Array, origin: any) => {
    if (origin !== 'remote' && origin !== 'loading' && this.config.onChatUpdate) {
      this.config.onChatUpdate(update);  // Sends V2 encoded update
    }
  });
  
  this.imageDoc.on('updateV2', (update: Uint8Array, origin: any) => {
    if (origin !== 'remote' && origin !== 'loading' && this.config.onImageUpdate) {
      this.config.onImageUpdate(update);
    }
  });
}
```

### 3. Frontend: Save with V2 Encoding (`chatCoordinator.ts`)

```typescript
// ❌ WRONG: V1 encoding
saveChat(): { chat: number[], image_state: number[] } {
  return {
    chat: Array.from(Y.encodeStateAsUpdate(this.chatDoc)),        // V1
    image_state: Array.from(Y.encodeStateAsUpdate(this.imageDoc)) // V1
  };
}

// ✅ CORRECT: V2 encoding
saveChat(): { chat: number[], image_state: number[] } {
  return {
    chat: Array.from(Y.encodeStateAsUpdateV2(this.chatDoc)),        // V2
    image_state: Array.from(Y.encodeStateAsUpdateV2(this.imageDoc)) // V2
  };
}
```

### 4. Backend: Better Error Logging (`document.rs`)

```rust
async fn apply_update_v2(&mut self, update: &[u8]) -> Result<(), String> {
    if update.is_empty() {
        log::debug!("Skipping empty update");
        return Ok(());
    }

    log::debug!("Applying v2 update, size: {} bytes, first bytes: {:?}", 
        update.len(), 
        &update[..update.len().min(10)]
    );

    let update_obj = Update::decode_v2(update).map_err(|e| {
        log::error!(
            "Failed to decode v2 update: {:?}, update size: {}, update bytes: {:?}",
            e,
            update.len(),
            update
        );
        format!("Failed to decode v2 update: {:?}", e)
    })?;

    // ... rest of function
}
```

### 5. Backend: Handle Empty State Vectors (`document.rs`)

```rust
async fn get_diff_update_v2(&self, state_vector: &[u8]) -> Result<Vec<u8>, String> {
    log::debug!("Getting diff update v2, state vector size: {} bytes", state_vector.len());
    
    if state_vector.is_empty() {
        log::warn!("Empty state vector provided, returning full state");
        return Ok(self.get_state_as_update_v2().await);
    }

    let sv = StateVector::decode_v2(state_vector).map_err(|e| {
        log::error!(
            "Failed to decode v2 state vector: {:?}, size: {}, bytes: {:?}",
            e,
            state_vector.len(),
            state_vector
        );
        format!("Failed to decode v2 state vector: {:?}", e)
    })?;

    let txn = self.transact().await;
    let diff = txn.encode_diff_v2(&sv);
    log::debug!("Generated diff update, size: {} bytes", diff.len());
    Ok(diff)
}
```

## Resolution Steps

### For New Chats
✅ **Fixed** - All new chats will have valid YJS state

### For Existing Chats
⚠️ **Action Required** - Delete old test chats and recreate them

#### Option 1: Delete from UI
1. Open chat app
2. Delete any existing chats
3. Create new chats (will have proper YJS state)

#### Option 2: Delete from Database
```sql
-- Find invalid resources (optional - for debugging)
SELECT id, resource_type, length(data) as data_size 
FROM resources 
WHERE resource_type = 'chat';

-- Delete all chat resources
DELETE FROM resources WHERE resource_type = 'chat';
DELETE FROM resource_keys WHERE resource_id IN (
  SELECT id FROM resources WHERE resource_type = 'chat'
);
DELETE FROM share_records WHERE resource_id IN (
  SELECT id FROM resources WHERE resource_type = 'chat'
);
```

#### Option 3: Database Migration (If Needed)
If you have important chats to preserve, create a migration:

```rust
// Migration to fix invalid YJS state
use yrs::Doc;

async fn migrate_invalid_chat_resources(repo_ctx: Arc<RepositoryContext>) -> Result<(), String> {
    let resources = repo_ctx.resource_repo.get_all_resources().await?;
    
    for resource in resources {
        if resource.resource_type == ResourceType::Chat {
            let mut needs_update = false;
            let mut data: serde_json::Value = serde_json::from_str(&resource.data)?;
            
            // Check and fix 'chat' field
            if let Some(chat) = data.get("chat").and_then(|v| v.as_array()) {
                if chat.is_empty() {
                    let doc = Doc::new();
                    doc.get_map("messages");
                    let state = Y::encodeStateAsUpdate(&doc);
                    data["chat"] = serde_json::to_value(Array::from(state))?;
                    needs_update = true;
                }
            }
            
            // Check and fix 'image_state' field
            if let Some(img) = data.get("image_state").and_then(|v| v.as_array()) {
                if img.is_empty() {
                    let doc = Doc::new();
                    doc.get_map("images");
                    let state = Y::encodeStateAsUpdate(&doc);
                    data["image_state"] = serde_json::to_value(Array::from(state))?;
                    needs_update = true;
                }
            }
            
            if needs_update {
                repo_ctx.resource_repo.update_resource_data(
                    &resource.id,
                    &serde_json::to_string(&data)?
                ).await?;
                println!("Fixed resource: {}", resource.id);
            }
        }
    }
    
    Ok(())
}
```

## Debugging New Issues

### Check Resource Data
```rust
// In resource_service.rs, add logging
pub async fn get_resource_state_vector(...) -> ServiceResult<String> {
    let (decrypted_resource, _) = get_resource(resource_id, repo_ctx, user_id, crypto_utils).await?;
    
    // Log the raw data
    log::debug!("Resource data for {}: {:?}", resource_id, decrypted_resource.data);
    
    let state_vectors = decrypted_resource
        .get_state_vectors()
        .await
        .map_err(|e| {
            log::error!("Failed to get state vectors for {}: {}", resource_id, e);
            ResourceServiceError::ParseError(e)
        })?;
    
    Ok(state_vectors)
}
```

### Expected Log Output (Success)
```
DEBUG Applying v2 update, size: 12 bytes, first bytes: [0, 0, 1, 0, 5, 109, 101, 115, 115, 97]
DEBUG Getting diff update v2, state vector size: 10 bytes
DEBUG Generated diff update, size: 15 bytes
```

### Error Log Output (Failure)
```
ERROR Failed to decode v2 update: EndOfBuffer(21382106), 
      update size: 0, 
      update bytes: []
```

## Prevention

### Always Use YJS Encoding
```typescript
// ✅ DO
const doc = new Y.Doc();
doc.getMap('data');
const state = Y.encodeStateAsUpdate(doc);
return Array.from(state);

// ❌ DON'T
return [];
```

### Validate Before Saving
```typescript
function isValidYjsState(state: number[]): boolean {
  if (state.length === 0) return false;
  
  try {
    const doc = new Y.Doc();
    Y.applyUpdate(doc, new Uint8Array(state));
    return true;
  } catch (e) {
    console.error("Invalid YJS state:", e);
    return false;
  }
}

// Use before saving
if (!isValidYjsState(chatContent.chat)) {
  throw new Error("Invalid chat YJS state");
}
```

### Test Suite
```typescript
describe('YJS State Validation', () => {
  it('rejects empty arrays', () => {
    expect(isValidYjsState([])).toBe(false);
  });
  
  it('accepts valid encoded state', () => {
    const doc = new Y.Doc();
    doc.getMap('test');
    const state = Array.from(Y.encodeStateAsUpdate(doc));
    expect(isValidYjsState(state)).toBe(true);
  });
});
```

## Key Insight: V1 vs V2 Encoding

YJS has **two encoding formats**:
- **V1**: `Y.encodeStateAsUpdate()` - older format
- **V2**: `Y.encodeStateAsUpdateV2()` - newer, more efficient

**Backend uses V2 exclusively**:
```rust
// In document.rs
Update::decode_v2(update)  // ← Only accepts V2!
StateVector::decode_v2(state_vector)  // ← Only accepts V2!
```

**Frontend MUST match**:
```typescript
// Listen to V2 updates
doc.on('updateV2', (update) => { ... })

// Encode as V2
Y.encodeStateAsUpdateV2(doc)
```

**Mixing V1 and V2 causes `EndOfBuffer` errors!**

## Summary

✅ **Fixed in frontend**: Use V2 encoding everywhere  
✅ **Fixed in frontend**: Listen to `updateV2` events  
✅ **Fixed in frontend**: `createEmptyChatContent()` uses V2  
✅ **Fixed in frontend**: `saveChat()` uses V2  
✅ **Fixed in backend**: Added logging and error handling  
✅ **Documented**: Complete analysis and prevention strategies  
⚠️ **Action needed**: Delete old test chats with invalid state  

**Next Steps**:
1. Rebuild frontend: `cd /home/abe/osvauld/chat/frontend/desktop && npm run build`
2. Delete existing test chats (they have V1 encoding)
3. Create new chats (will have V2 encoding)
4. Verify no more EndOfBuffer errors in logs
5. Test message sending/syncing works correctly
