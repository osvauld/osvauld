# Critical Fixes for Message Delivery

## Date: September 30, 2025

## Problems Identified

From your logs, there were **THREE critical issues** preventing messages from appearing:

1. ❌ **Connection ID format mismatch** - Device IDs (base64) vs NodeIDs (hex)
2. ❌ **Doc type naming** - Backend "main_doc" vs Frontend "chat"
3. ❌ **Wrong event name** - Emitting "document-updates" instead of "live-updates"

## Issue 1: Connection ID Format Mismatch

### Problem
```
WARN Connection not found: 7QCvZ+FO+D6vLGmIGar0yY5/egpexW1C2msKUqREXvs=  // Base64 device ID
```

But the actual connection was stored as:
```
Connection found: ed00af67e14ef83eaf2c698819aaf4c98e7f7a0a5ec56d42da6b0a52a4445efb  // Hex NodeID
```

**Root Cause**: 
- Subscription cache returns device IDs in base64 format (from database)
- P2P connection manager stores connections by NodeID (hex format)
- When broadcasting, we passed base64 IDs directly → connection lookup failed → no message sent!

### Solution
Convert device IDs to NodeIDs before broadcasting:

```rust
// Get device IDs from subscription cache (base64 format)
let device_ids = chat_state.get_subscribers(&resource_id).await?;

// Convert to NodeIDs (hex format)
let mut connection_ids = Vec::new();
for device_id in &device_ids {
    match crypto_utils::derive_node_id_from_public_key(device_id) {
        Ok(node_id_bytes) => {
            let node_id_hex = node_id_bytes
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
            connection_ids.push(node_id_hex);
        }
        Err(e) => {
            error!("Failed to derive node ID: {}", e);
        }
    }
}

// Now broadcast with correct connection IDs
p2p_sender.send_sync_update_to_connections(
    resource_id,
    client_id,
    updates,
    connection_ids,  // ✅ Hex NodeIDs
    doc_type,
)?;
```

**File Changed**: `chat/src-tauri/src/listners/tauri_events.rs` (lines 71-94)

## Issue 2: Doc Type Naming Mismatch

### Problem
Backend was sending `"main_doc"` but frontend coordinator expects `"chat"`:

```json
// Backend sent:
{
  "doc_type": "main_doc"  // ❌ 
}

// Frontend expected:
coordinator.applyRemoteUpdate(updates, senderId, "chat");  // Looking for "chat"
```

### Solution
Map backend doc types to frontend doc types before emitting:

```rust
// Map backend doc_type to frontend doc_type
let frontend_doc_type = match doc_type.as_str() {
    "main_doc" => "chat",           // ✅ Map to what frontend expects
    "image_state" => "image_state", // Keep as is
    _ => &doc_type,
};

// Emit with mapped type
let payload = serde_json::json!({
    "resource_id": resource_id,
    "updates": updates,
    "client_id": client_id.to_string(),
    "doc_type": frontend_doc_type,  // ✅ Now "chat"
});
```

**File Changed**: `chat/src-tauri/src/listners/p2p_handlers/updates.rs` (lines 27-33)

## Issue 3: Wrong Event Name

### Problem
From your logs:
```
Successfully emitted document-updates event  // ❌ Wrong event name
```

But frontend is listening for:
```javascript
listen("live-updates", (event) => { ... });  // Listening for "live-updates"
```

The events never matched, so frontend never received anything!

### Solution
Already fixed in previous commit - we emit `"live-updates"` not `"document-updates"`.

## How These Fixes Work Together

### Message Flow (Before Fixes)
```
1. User A sends message
2. Backend A: Applied to ChatState ✅
3. Backend A: Get device IDs from cache (base64) ✅
4. Backend A: Try to broadcast using base64 IDs ❌ Connection not found!
5. Message never sent to User B ❌
```

### Message Flow (After Fixes)
```
1. User A sends message
2. Backend A: Applied to ChatState ✅
3. Backend A: Get device IDs (base64) from cache ✅
4. Backend A: Convert to NodeIDs (hex) ✅
5. Backend A: Broadcast using NodeIDs ✅
6. Backend B: Receives update ✅
7. Backend B: Maps "main_doc" → "chat" ✅
8. Backend B: Emits "live-updates" (not "document-updates") ✅
9. Frontend B: Receives "live-updates" event ✅
10. Frontend B: Applies to coordinator with doc_type="chat" ✅
11. Message appears in UI! ✅✅✅
```

## Testing

### What to Check

1. **Backend logs should show**:
   ```
   Broadcasting chat update to N connections
   Successfully broadcasted chat update
   ```
   
2. **No more "Connection not found" warnings**

3. **Recipient backend should show**:
   ```
   Received editing event for resource <id>
   Applied X bytes to chat resource
   Successfully emitted live-updates event
   ```

4. **Frontend console should show**:
   ```
   Received live-updates for <chat-id>
   Applied update for chat <chat-id>
   ```

5. **Messages appear in UI within 100ms**

### Quick Test
1. Open chat on both devices
2. Send message from Device A
3. Message should appear on Device B immediately
4. Check console logs match above

## Files Modified

| File | Change | Lines |
|------|--------|-------|
| `tauri_events.rs` | Device ID → NodeID conversion | 71-94 |
| `updates.rs` | Doc type mapping | 27-33, 45 |
| `updates.rs` | Emit live-updates (already fixed) | 52 |

## Summary

These three fixes address the **complete message delivery pipeline**:

1. **Connection lookup**: Now finds the right connection
2. **Doc type mapping**: Frontend coordinator applies updates correctly  
3. **Event emission**: Frontend receives the events

All three were breaking the flow. With all three fixed, messages should now:
- ✅ Broadcast successfully
- ✅ Be received by backend
- ✅ Be emitted to frontend
- ✅ Be applied to Y.Doc
- ✅ Appear in UI

## Related Documents

- **Frontend filter fix**: `FRONTEND_FILTER_FIX.md`
- **Frontend updates fix**: `FRONTEND_UPDATES_FIX.md`
- **Architecture**: `ARCHITECTURE_QUICK_REFERENCE.md`

