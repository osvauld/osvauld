# Three Document Architecture - Implementation Complete

## Summary

Successfully refactored the Yjs architecture to support **3 independent documents** instead of 2:

1. **mainDoc** (`blocksuite_doc`) - Website content (screens, blocks, layout)
2. **commentsDoc** (`thread_comments_doc`) - Thread comments (collaborative)
3. **submissionsDoc** (`form_submissions_doc`) - Form submissions (append-only)

## Changes Made

### 1. YjsManager (`src/lib/yjsManager.ts`)

**Updated YjsDocuments interface:**
```typescript
export interface YjsDocuments {
  mainDoc: Y.Doc;
  blocks: Y.Map<any>;
  viewport: Y.Map<any>;
  awareness: Awareness;

  // Thread comments document (collaborative - all participants can read/write)
  commentsDoc?: Y.Doc;
  commentsBlocks?: Y.Map<any>;

  // Form submissions document (viewers append, owner receives)
  submissionsDoc?: Y.Doc;
  submissionsBlocks?: Y.Map<any>;
}
```

**Updated initialize() method:**
- Creates `commentsDoc` when `resourceType === 'noticeboard' || resourceType === 'website'`
- Creates `submissionsDoc` when `resourceType === 'form' || resourceType === 'website'`
- Website resources can have BOTH docs simultaneously
- Each doc has its own update listener with proper `docType` routing

**Updated applyUpdate() method:**
- Routes updates to correct document based on `docType` parameter:
  - `'thread_comments_doc'` → `commentsDoc`
  - `'form_submissions_doc'` → `submissionsDoc`
  - `'blocksuite_doc'` or `'main'` → `mainDoc`

**Updated destroy() method:**
- Destroys all 3 documents properly

### 2. BlocksuiteCoordinator (`src/lib/blocksuiteCoordinator.ts`)

**Added tracking flags:**
```typescript
private hasThreadBlocks: boolean = false;
private hasFormBlocks: boolean = false;
```

**Updated detectResourceType():**
- Checks `blocksuite_doc` for presence of thread/form blocks
- Sets both flags independently
- Website can have threads, forms, or both

**Updated loadBlocksuite():**
- Simplified 3-step loading:
  1. Load main doc (`blocksuite_doc`, `thread_doc`, or `form_doc`)
  2. Load comments doc if `hasThreadBlocks && data.thread_comments_doc`
  3. Load submissions doc if `hasFormBlocks && data.form_submissions_doc`

**Updated saveBlocksuite():**
- Always saves mainDoc with appropriate key
- Saves commentsDoc if it exists → `thread_comments_doc`
- Saves submissionsDoc if it exists → `form_submissions_doc`
- Result can have 1, 2, or all 3 documents

### 3. Component Updates

**ThreadBlock.svelte:**
- Changed prop from `secondaryDoc` → `commentsDoc`
- Observes `commentsDoc.getMap('blocks')` for comments
- Writes comments to `commentsDoc.transact(...)`

**FormSubmitButton.svelte:**
- Changed prop from `secondaryDoc` → `submissionsDoc`
- Writes submissions to `submissionsDoc.transact(...)`

**FullScreenViewer.svelte:**
- Changed props from `secondaryDoc` → `commentsDoc, submissionsDoc`
- Passes appropriate doc to each component:
  - ThreadBlock gets `commentsDoc`
  - FormSubmitButton gets `submissionsDoc`

**ViewerMode.svelte:**
- Passes both docs: `commentsDoc={yDocs?.commentsDoc} submissionsDoc={yDocs?.submissionsDoc}`

## Data Flow

### Comments (Collaborative):
```
User posts comment
  ↓
ThreadBlock.submitComment()
  ↓
commentsDoc.transact(() => {
  commentsBlocks.set(`${blockId}_comments`, { items: [...] })
})
  ↓
YjsManager detects 'updateV2' on commentsDoc
  ↓
Calls onUpdate(update, origin, 'thread_comments_doc')
  ↓
Coordinator → onCollaborationUpdate()
  ↓
Protocol layer routes to ALL participants (sovereign node)
  ↓
Other clients receive update
  ↓
Apply to their commentsDoc
  ↓
ThreadBlock auto-updates (Yjs observation)
```

### Form Submissions (Append-Only):
```
User submits form
  ↓
FormSubmitButton.handleSubmit()
  ↓
submissionsDoc.transact(() => {
  submissionsBlocks.set(`${formId}_submissions`, { items: [...] })
})
  ↓
YjsManager detects 'updateV2' on submissionsDoc
  ↓
Calls onUpdate(update, origin, 'form_submissions_doc')
  ↓
Coordinator → onCollaborationUpdate()
  ↓
Protocol layer routes to OWNER ONLY (sovereign node)
  ↓
Owner receives update
  ↓
Viewer does not see other submissions (protocol layer handles this)
```

### Website Content (Owner Writes, Viewers Read):
```
Owner edits in builder mode
  ↓
Updates mainDoc (blocks, viewport)
  ↓
YjsManager detects 'updateV2' on mainDoc
  ↓
Calls onUpdate(update, origin, 'blocksuite_doc')
  ↓
Coordinator → onCollaborationUpdate()
  ↓
Protocol layer routes to ALL viewers (sovereign node)
  ↓
Viewers receive update
  ↓
Apply to their mainDoc (read-only at UI layer)
```

## Document Permissions (Handled at Protocol Layer)

| Document | Owner | Viewer | Sync Direction |
|----------|-------|--------|----------------|
| `blocksuite_doc` (main) | ✅ Read/Write | 👁️ Read-only | Owner → Viewers |
| `thread_comments_doc` (comments) | ✅ Read/Write | ✅ Read/Write | Bidirectional |
| `form_submissions_doc` (submissions) | ✅ Read (all) | ✏️ Write (own only) | Viewers → Owner |

**Note:** Permission enforcement happens at the protocol/sovereign node layer, NOT in the UI. The UI just sends updates to the appropriate documents.

## Benefits

1. **Clean Separation**: Comments and submissions are completely independent
2. **Scalability**: Can have multiple thread blocks and multiple forms in same document
3. **Flexible**: Website resources support any combination of threads/forms
4. **Protocol-Agnostic**: UI doesn't enforce permissions, that's protocol layer's job
5. **Type-Safe**: TypeScript ensures correct docs passed to components

## Storage Example

A website with both threads and forms saves as:
```json
{
  "blocksuite_doc": [/* Yjs updates for screens/blocks */],
  "thread_comments_doc": [/* Yjs updates for all comments */],
  "form_submissions_doc": [/* Yjs updates for all submissions */],
  "last_modified": 1697123456789
}
```

## Testing

To test the implementation:

1. **Create a website with thread blocks**
   - Console should show: `"🔧 Creating commentsDoc for thread blocks"`
   - Post a comment - should save to `commentsDoc`

2. **Create a website with form blocks**
   - Console should show: `"🔧 Creating submissionsDoc for form blocks"`
   - Submit form - should save to `submissionsDoc`

3. **Create a website with BOTH**
   - Console should show both docs being created
   - Both features should work independently

4. **Save and reload**
   - Check console logs for document loading
   - All 3 docs should load and restore correctly

## Protocol Layer TODO

The protocol/sovereign node needs to handle routing based on `docType`:

```typescript
// Pseudo-code for sovereign node
function handleYjsUpdate(update: Uint8Array, senderId: number, docType: string) {
  if (docType === 'blocksuite_doc') {
    // Only accept from owner
    if (senderId !== ownerId) return; // Reject
    broadcastToAllClients(update, docType);
  } else if (docType === 'thread_comments_doc') {
    // Accept from anyone, broadcast to everyone
    broadcastToAllClients(update, docType);
  } else if (docType === 'form_submissions_doc') {
    // Accept from anyone, send ONLY to owner
    sendToOwner(update, docType);
  }
}
```

The `docType` parameter is already being passed through the entire chain:
- Component → `doc.transact()`
- YjsManager → `on('updateV2')` with docType
- Coordinator → `onUpdate(update, origin, docType)`
- Your backend receives the docType and can route accordingly
