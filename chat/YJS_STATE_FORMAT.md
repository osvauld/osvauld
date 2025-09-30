# YJS State Format Reference

## Valid vs Invalid YJS State

### ❌ INVALID: Empty Arrays
```typescript
{
  chat: [],           // NOT valid YJS state
  image_state: []     // NOT valid YJS state
}
```

**Error**: `Failed to decode v2 update: EndOfBuffer`

### ✅ VALID: Encoded YJS Documents
```typescript
{
  chat: [0, 0, 1, 0, ...],           // Valid YJS binary state
  image_state: [0, 0, 1, 0, ...]     // Valid YJS binary state  
}
```

## Creating Empty YJS State

### Method 1: Encode Empty Documents (Recommended)
```typescript
import * as Y from 'yjs';

function createEmptyYjsState() {
  const doc = new Y.Doc();
  doc.getMap('messages'); // Initialize the shared type
  
  const state = Y.encodeStateAsUpdate(doc);
  return Array.from(state);
}

// Result: [0, 0, 1, 0, 5, ...] (valid binary format)
```

### Method 2: From Existing Coordinator
```typescript
const coordinator = new ChatCoordinator({...});
const saved = coordinator.saveChat();

// Result: { chat: [...], image_state: [...] }
```

## YJS Binary Format

### Structure
```
┌─────────────────────────────────────────────┐
│ YJS Update Binary Format                    │
├─────────────────────────────────────────────┤
│ Header                                      │
│ - Format version                            │
│ - Client ID                                 │
├─────────────────────────────────────────────┤
│ Structs                                     │
│ - Items (inserts, deletes)                  │
│ - Parent references                         │
├─────────────────────────────────────────────┤
│ Delete Set                                  │
│ - Deleted items                             │
└─────────────────────────────────────────────┘
```

### Empty Document Size
An empty YJS document is ~10-20 bytes (not 0!):
```typescript
const doc = new Y.Doc();
doc.getMap('test');
const state = Y.encodeStateAsUpdate(doc);
console.log(state.length); // ~12 bytes
```

## Common YJS Operations

### Encoding
```typescript
// Get current state as update
const update = Y.encodeStateAsUpdate(doc);
// Returns: Uint8Array

// Get state vector (for sync)
const stateVector = Y.encodeStateVector(doc);
// Returns: Uint8Array

// Get diff for peer
const diff = Y.encodeStateAsUpdate(doc, peerStateVector);
// Returns: Uint8Array (only what peer is missing)
```

### Decoding
```typescript
// Apply update to document
Y.applyUpdate(doc, update);

// Apply update with origin
Y.applyUpdate(doc, update, 'remote');
```

### Error Conditions

#### EndOfBuffer
```
Failed to decode v2 update: EndOfBuffer(position)
```
**Causes**:
- Empty array `[]` passed to `applyUpdate`
- Corrupted binary data
- Truncated update
- Wrong encoding format

**Solution**: Always use `Y.encodeStateAsUpdate()`, never manual arrays

#### ClientID Conflict
```
Applying update with same client ID
```
**Causes**:
- Same client ID used on different devices
- Client ID not unique per device

**Solution**: Use device-based client ID generation

## Chat Resource Format

### In Database (Encrypted)
```json
{
  "chat": [0, 0, 1, 0, 7, 111, 98, 106, 101, ...],
  "image_state": [0, 0, 1, 0, 5, 105, 109, ...]
}
```

### After Decryption (Frontend)
```typescript
{
  chat: number[],        // Array of bytes
  image_state: number[]  // Array of bytes
}
```

### In Memory (Coordinator)
```typescript
chatDoc: Y.Doc {
  share: Map {
    'messages': Y.Map {
      'msg-123': { id, authorId, content, ... },
      'msg-124': { id, authorId, content, ... }
    }
  }
}

imageDoc: Y.Doc {
  share: Map {
    'images': Y.Map {
      'img-456': { id, url, size, ... }
    }
  }
}
```

## Sync Protocol

### Initial Sync
```typescript
// Step 1: Send state vector
const myStateVector = Y.encodeStateVector(doc);
send({ stateVector: Array.from(myStateVector) });

// Step 2: Receive diff
receive(({ updates }) => {
  Y.applyUpdate(doc, new Uint8Array(updates));
});

// Step 3: Send my diff
const peerStateVector = new Uint8Array(received.stateVector);
const myDiff = Y.encodeStateAsUpdate(doc, peerStateVector);
send({ updates: Array.from(myDiff) });
```

### Live Updates
```typescript
// On local change
doc.on('update', (update: Uint8Array) => {
  send({ 
    updates: Array.from(update),
    docType: 'chat'
  });
});

// On remote change
receive(({ updates, docType }) => {
  if (docType === 'chat') {
    Y.applyUpdate(chatDoc, new Uint8Array(updates), 'remote');
  }
});
```

## Debugging YJS State

### Check if Valid
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
```

### Decode and Inspect
```typescript
function inspectYjsState(state: number[]) {
  const doc = new Y.Doc();
  Y.applyUpdate(doc, new Uint8Array(state));
  
  const messages = doc.getMap('messages');
  console.log("Message count:", messages.size);
  
  messages.forEach((value, key) => {
    console.log(`Message ${key}:`, value);
  });
}
```

### Compare States
```typescript
function compareStates(state1: number[], state2: number[]): boolean {
  if (state1.length !== state2.length) return false;
  return state1.every((byte, i) => byte === state2[i]);
}
```

## Best Practices

### ✅ DO
- Always use `Y.encodeStateAsUpdate()` to create state
- Initialize shared types before encoding
- Use proper client IDs (unique per device)
- Handle `'remote'` origin to avoid loops
- Clean up documents with `doc.destroy()`

### ❌ DON'T
- Never use empty arrays `[]` as YJS state
- Never manually construct YJS binary format
- Never share client IDs between devices
- Never apply updates with same origin multiple times
- Never forget to cleanup old documents

## Testing YJS State

```typescript
import * as Y from 'yjs';

describe('YJS State', () => {
  it('creates valid empty state', () => {
    const doc = new Y.Doc();
    doc.getMap('test');
    const state = Y.encodeStateAsUpdate(doc);
    
    expect(state.length).toBeGreaterThan(0);
    expect(Array.isArray(Array.from(state))).toBe(true);
  });
  
  it('can decode encoded state', () => {
    const doc1 = new Y.Doc();
    const map1 = doc1.getMap('test');
    map1.set('key', 'value');
    
    const state = Y.encodeStateAsUpdate(doc1);
    
    const doc2 = new Y.Doc();
    Y.applyUpdate(doc2, state);
    const map2 = doc2.getMap('test');
    
    expect(map2.get('key')).toBe('value');
  });
  
  it('rejects empty array as state', () => {
    const doc = new Y.Doc();
    
    expect(() => {
      Y.applyUpdate(doc, new Uint8Array([]));
    }).toThrow();
  });
});
```

## Resources

- [YJS Documentation](https://docs.yjs.dev/)
- [YJS Protocol Specification](https://github.com/yjs/yjs/blob/main/PROTOCOL.md)
- [Y-CRDT Paper](https://www.researchgate.net/publication/310212186_Near_Real-Time_Peer-to-Peer_Shared_Editing_on_Extensible_Data_Types)
