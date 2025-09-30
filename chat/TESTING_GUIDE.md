# Chat Refactoring Testing Guide

## Overview

This guide provides step-by-step instructions for testing the refactored chat application to ensure all features work correctly with the new multi-resource architecture.

## Prerequisites

- Two devices/instances with chat application installed
- Users: User A and User B
- Network connectivity between instances

## Test Scenarios

### 1. Basic Message Exchange (Both Users Online)

**Objective**: Verify real-time message delivery when both users have chat open.

**Steps**:
1. User A opens Chat 1
2. User B opens Chat 1
3. User A sends message "Hello"
4. User B should see message within 100ms
5. User B sends reply "Hi there"
6. User A should see reply within 100ms

**Expected Behavior**:
- ✅ Messages appear immediately
- ✅ No console errors
- ✅ Messages persist after app refresh

**Logs to Check**:
```
[Backend A] Applied X bytes to chat resource: <resource_id>
[Backend A] Broadcasting chat update to N subscribers
[Backend B] Received editing event for resource <resource_id>
[Backend B] Applied X bytes to chat resource: <resource_id>
[Frontend B] Message appears in UI
```

---

### 2. Multiple Simultaneous Chats

**Objective**: Verify independent operation of multiple chats.

**Steps**:
1. User A opens Chat 1, sends "Message in Chat 1"
2. User A switches to Chat 2, sends "Message in Chat 2"
3. User B opens Chat 2
4. Verify User B sees "Message in Chat 2"
5. User B opens Chat 1
6. Verify User B sees "Message in Chat 1"

**Expected Behavior**:
- ✅ Both messages preserved correctly
- ✅ No message mixing between chats
- ✅ Each chat maintains independent state

**Key Check**: Verify in logs that different `resource_id` values are used.

---

### 3. Offline Message Delivery

**Objective**: Verify messages are delivered when recipient comes online.

**Steps**:
1. User B closes chat app completely
2. User A sends message "Offline message"
3. Wait 5 seconds
4. User B opens chat app
5. User B opens chat
6. Verify User B receives "Offline message"

**Expected Behavior**:
- ✅ Message appears when User B opens chat
- ✅ Reconciliation logs show state vector exchange
- ✅ No duplicate messages

**Logs to Check**:
```
[Backend A] No active connections available for broadcast
[Backend B] Starting chat reconciliation for: <resource_id>
[Backend B] Sent reconciliation request to device: <device_id>
[Backend B] Processing update response for chat resource
```

---

### 4. Message When Chat Not Open

**Objective**: Verify messages received even when different chat is open.

**Steps**:
1. User A opens Chat 1
2. User B opens Chat 2 (different chat)
3. User A sends message in Chat 1
4. Verify User B's Chat 1 shows unread indicator
5. User B switches to Chat 1
6. Verify message appears

**Expected Behavior**:
- ✅ Message received in background
- ✅ Unread count incremented
- ✅ Message visible when chat opened

**Key Check**: Backend should NOT check is_current_note before applying updates.

---

### 5. Connection Drop Recovery

**Objective**: Verify reconciliation after network interruption.

**Steps**:
1. Both users in Chat 1
2. Simulate network drop (disable WiFi on User A)
3. User A sends message "During outage" (appears locally only)
4. Re-enable WiFi on User A
5. Verify connection re-establishes
6. Verify User B receives "During outage"

**Expected Behavior**:
- ✅ Message appears for sender immediately (local Yjs)
- ✅ Message queued during outage
- ✅ Message delivered on reconnection
- ✅ No message duplication

**Logs to Check**:
```
[Backend A] Failed to broadcast chat update: <error>
[Backend A] Starting chat reconciliation for: <resource_id>
[Backend B] Received editing event for resource <resource_id>
```

---

### 6. Rapid Message Exchange

**Objective**: Stress test with quick succession of messages.

**Steps**:
1. Both users in Chat 1
2. User A sends 10 messages quickly (< 1 second apart)
3. Verify all messages appear on User B's side
4. Verify correct message order
5. User B replies with 10 quick messages
6. Verify all messages appear on User A's side

**Expected Behavior**:
- ✅ All messages delivered
- ✅ Correct chronological order
- ✅ No performance degradation
- ✅ No UI freezing

---

### 7. Subscription Cache Test

**Objective**: Verify subscription caching works correctly.

**Steps**:
1. User A sends first message in Chat 1
2. Check logs for "Fetching subscribers from database"
3. User A sends second message within 5 minutes
4. Check logs for "Using cached subscribers"
5. Wait 6 minutes
6. User A sends third message
7. Check logs for "Fetching subscribers from database" again

**Expected Behavior**:
- ✅ First message: Database query
- ✅ Second message: Cache hit
- ✅ After 5 min: Cache expired, new query

**Logs to Check**:
```
[Backend A] Fetching subscribers from database for resource: <resource_id>
[Backend A] Cached N subscribers for resource: <resource_id>
[Backend A] Using cached subscribers for resource: <resource_id>
```

---

### 8. App Restart with Pending Messages

**Objective**: Verify message sync after app restart.

**Steps**:
1. User A and User B both in Chat 1
2. User B force-quits app
3. User A sends 3 messages
4. User B restarts app
5. User B opens Chat 1
6. Verify all 3 messages appear

**Expected Behavior**:
- ✅ All messages received on open
- ✅ Reconciliation triggered automatically
- ✅ Correct message order

---

### 9. Three-Way Chat (If Supported)

**Objective**: Verify broadcasting to multiple subscribers.

**Steps**:
1. Create chat shared with User A, User B, and User C
2. User A sends message
3. Verify both User B and User C receive it
4. User B sends message
5. Verify both User A and User C receive it

**Expected Behavior**:
- ✅ Message broadcast to all subscribers
- ✅ No duplicate messages
- ✅ All users stay in sync

**Logs to Check**:
```
[Backend A] Broadcasting chat update to 2 subscribers
```

---

### 10. Edge Case: Empty Chat Open

**Objective**: Verify behavior when opening empty chat.

**Steps**:
1. User A creates new chat with User B
2. User B opens the empty chat
3. Verify no errors in console
4. User A sends first message
5. Verify User B receives it

**Expected Behavior**:
- ✅ No errors when opening empty chat
- ✅ First message delivered correctly
- ✅ Reconciliation handles empty state

---

## Debugging Tools

### Backend Logs to Monitor

```rust
// Enable debug logging
RUST_LOG=info cargo run

// Key patterns to grep for:
grep "Broadcasting chat update"
grep "Starting chat reconciliation"
grep "Using cached subscribers"
grep "Received editing event"
```

### Frontend Console Checks

```javascript
// Check Y.Doc state
console.log(chatDoc.toJSON())

// Check awareness
console.log(awareness.getStates())

// Check connection status
console.log(navigator.onLine)
```

### Database Queries

```sql
-- Check resource sharing
SELECT * FROM resource_shares WHERE resource_id = '<chat_id>';

-- Check devices
SELECT * FROM devices WHERE user_id = '<user_id>';
```

---

## Performance Benchmarks

### Latency Targets

| Scenario | Target | Measurement |
|----------|--------|-------------|
| Message delivery (both online) | < 100ms | Time from send to receive |
| Chat open reconciliation | < 2s | Time to load full history |
| Subscription cache lookup | < 5ms | Cache hit time |
| State vector generation | < 50ms | Per-resource generation |

### Memory Targets

| Component | Target | Measurement |
|-----------|--------|-------------|
| Per-chat buffer | < 1MB | After 100 messages |
| Subscription cache | < 100KB | Per 10 chats |
| Total ChatState | < 10MB | With 10 active chats |

---

## Common Issues and Solutions

### Issue: Messages Not Appearing

**Check**:
1. Is `is_current_note` check removed? (Should be!)
2. Are backend logs showing "Received editing event"?
3. Is frontend listening for "live-updates"?

**Solution**: Verify `handle_editing_event()` applies to ChatState without current check.

### Issue: Offline Messages Not Received

**Check**:
1. Is reconciliation triggered on chat open?
2. Are state vectors being exchanged?
3. Are connections re-established?

**Solution**: Verify `reconcile_chat_on_open()` is called in note-change listener.

### Issue: Duplicate Messages

**Check**:
1. Is message deduplication working in frontend?
2. Are multiple reconciliations happening?

**Solution**: Add client-side deduplication based on message IDs.

### Issue: High Memory Usage

**Check**:
1. How many chat buffers are loaded?
2. Are old chats being evicted?

**Solution**: Implement LRU eviction for inactive chats (future enhancement).

---

## Automated Test Script Template

```bash
#!/bin/bash
# Basic smoke test

echo "Starting chat refactoring tests..."

# Test 1: Basic message exchange
echo "Test 1: Basic message exchange"
# TODO: Implement automated test

# Test 2: Multiple chats
echo "Test 2: Multiple simultaneous chats"
# TODO: Implement automated test

# Test 3: Offline messages
echo "Test 3: Offline message delivery"
# TODO: Implement automated test

echo "All tests completed!"
```

---

## Sign-Off Checklist

Before considering the refactoring complete:

- [ ] All 10 test scenarios pass
- [ ] No console errors in any scenario
- [ ] Performance meets latency targets
- [ ] Memory usage within limits
- [ ] Logs show correct behavior
- [ ] No message loss in any scenario
- [ ] Subscription cache working correctly
- [ ] Reconciliation catching all missed messages

## Reporting Issues

If you find issues during testing:

1. **Capture logs**: Both frontend console and backend logs
2. **Exact steps**: Reproduce the issue reliably
3. **Expected vs actual**: What should happen vs what happened
4. **Screenshots**: If UI-related
5. **Network logs**: If connection-related

File issues with all information in the project issue tracker.

