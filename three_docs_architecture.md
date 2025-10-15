# Three Document Architecture Review

## Current Implementation (2 Docs)

Right now we have:
- **mainDoc** (blocksuite_doc) - website content
- **secondaryDoc** (either thread_comments_doc OR form_submissions_doc)

Problem: Can't have both comments AND submissions in the same resource.

## Required Architecture (3 Docs)

You need:
1. **Website Doc** (blocksuite_doc)
2. **Comments Doc** (thread_comments_doc)
3. **Submissions Doc** (form_submissions_doc)

## Document Permissions

| Document | Owner | Viewer | Sync Behavior |
|----------|-------|--------|---------------|
| **Website Doc** | ✅ Read/Write | 👁️ Read-only | Owner → Viewers (one-way) |
| **Comments Doc** | ✅ Read/Write | ✅ Read/Write | Bidirectional (collaborative) |
| **Submissions Doc** | ✅ Read/Write | ✏️ Append-only | Viewer → Owner (one-way, append-only) |

## Current Issues

### 1. Only 2 Docs Supported
```typescript
export interface YjsDocuments {
  mainDoc: Y.Doc;              // Website content
  blocks: Y.Map<any>;
  viewport: Y.Map<any>;
  awareness: Awareness;

  secondaryDoc?: Y.Doc;        // EITHER comments OR submissions
  secondaryBlocks?: Y.Map<any>;
}
```

**Problem:** Can't have both comments and submissions.

### 2. No Permission Checks
```typescript
mainDoc.on("updateV2", (update: Uint8Array, origin: any) => {
  if (origin !== "sync" && origin !== "loading") {
    this.config.onUpdate!(update, origin, "main");  // Sent to everyone
  }
});
```

**Problem:** Viewers can modify website doc (should be read-only).

### 3. All Updates Sent to Everyone
```typescript
secondaryDoc.on("updateV2", (update: Uint8Array, origin: any) => {
  if (origin !== "sync" && origin !== "loading") {
    this.config.onUpdate!(update, origin, docTypeKey);  // Sent to everyone
  }
});
```

**Problem:**
- Submissions sent to all viewers (should only go to owner)
- No differentiation between comments and submissions

## Proposed Changes

### 1. Three Document Structure

```typescript
export interface YjsDocuments {
  // Main website content
  mainDoc: Y.Doc;
  blocks: Y.Map<any>;
  viewport: Y.Map<any>;
  awareness: Awareness;

  // Collaborative comments (if resource has thread blocks)
  commentsDoc?: Y.Doc;
  commentsBlocks?: Y.Map<any>;

  // Owner-only submissions (if resource has form blocks)
  submissionsDoc?: Y.Doc;
  submissionsBlocks?: Y.Map<any>;
}
```

### 2. User Role Tracking

```typescript
export interface YjsManagerConfig {
  clientId: number;
  isOwner: boolean;  // NEW: Track if current user is owner
  onUpdate?: (update: Uint8Array, origin: any, docType?: string) => void;
  onAwarenessChange?: (changes: any, origin: string) => void;
}
```

### 3. Permission-Based Update Handling

```typescript
// Website doc - only owner can send updates
mainDoc.on("updateV2", (update: Uint8Array, origin: any) => {
  if (origin !== "sync" && origin !== "loading") {
    if (this.config.isOwner) {
      this.config.onUpdate!(update, origin, "blocksuite_doc");
    } else {
      console.warn("⚠️ Viewer tried to modify website doc (blocked)");
    }
  }
});

// Comments doc - everyone can send updates
commentsDoc.on("updateV2", (update: Uint8Array, origin: any) => {
  if (origin !== "sync" && origin !== "loading") {
    this.config.onUpdate!(update, origin, "thread_comments_doc");
  }
});

// Submissions doc - viewers send, only owner receives
submissionsDoc.on("updateV2", (update: Uint8Array, origin: any) => {
  if (origin !== "sync" && origin !== "loading") {
    // Viewers can submit
    this.config.onUpdate!(update, origin, "form_submissions_doc");
  }
});
```

### 4. Backend/Sovereign Node Logic

The sovereign node needs to route updates based on docType:

```typescript
// Pseudo-code for sovereign node
function handleUpdate(update: Uint8Array, docType: string, senderId: number) {
  if (docType === 'blocksuite_doc') {
    // Only accept from owner
    if (senderId === ownerId) {
      broadcastToAllClients(update, docType);
    }
  } else if (docType === 'thread_comments_doc') {
    // Accept from anyone, send to everyone
    broadcastToAllClients(update, docType);
  } else if (docType === 'form_submissions_doc') {
    // Accept from anyone, send ONLY to owner
    sendToOwner(update, docType);
  }
}
```

## Questions

1. **How do we determine isOwner?**
   - Is there a user role/permission field?
   - Is it based on resource creator?
   - Is it in the resource metadata?

2. **Where does permission enforcement happen?**
   - Client-side (YjsManager) - blocks updates from being sent
   - Server-side (Sovereign node) - rejects unauthorized updates
   - Both?

3. **Can a resource have both threads AND forms?**
   - If yes, we need 3 docs
   - If no, we can keep 2 docs but with better permission logic

4. **Submissions append-only:**
   - Should viewers see their own submissions?
   - Or completely blind (submit and forget)?
   - Should there be a submission ID/acknowledgment?

5. **Read-only enforcement for viewers:**
   - Should we make mainDoc read-only in viewer mode?
   - Use Yjs subdocs or separate doc instances?
   - How do we prevent accidental modifications in UI?
