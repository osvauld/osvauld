# Sthalam - Osvauld BlockSuite Website Builder

## Tech Stack
- **Frontend**: Svelte 5 (runes) - `.svelte` files only
- **Backend**: Rust with Tauri
- **Editor**: BlockSuite-inspired infinite canvas
- **P2P**: libp2p for networking
- **State**: Yjs for collaborative editing

## Quick Start
```bash
cd sthalam/src-tauri
cargo tauri dev
```

---

## Current Status (2025-10-14)

### ✅ Completed Features

**Core Website Builder:**
- Infinite canvas with drag/drop blocks
- Block types: Screen Container, Section Container, Heading, Text, Image, HTML/CSS
- Yjs-based data synchronization
- Builder mode (full editing) + Viewer mode (readonly display)
- P2P connection with UCAN tokens
- Publishing and connecting with tokens

**Form System:**
- Form blocks: Form metadata, Text/Email/Password/Number inputs, Textarea, Checkbox, Submit Button
- Form grouping via `formId` - invisible metadata blocks link fields and submit buttons
- Event-based submission with `eventName` for backend identification
- Interactive forms in viewer mode with JSON output

**Responsive Layout System:**
- **Screen containers**: Main responsive page containers (entry point selection with `isEntryPoint`)
- **Section containers**: Layout sections with flex/grid CSS
- **ParentId hierarchy**: Blocks organized by parent-child relationships
- **CSS per block**: Custom CSS styling on all blocks
- **Dual rendering**: Builder mode uses absolute positioning, viewer uses CSS hierarchy

**Full-Screen Viewer Mode:**
- Container-based presentation (each screen fills viewport)
- Direct navigation between screens
- Form submission and navigation handlers
- Screen navigation indicators

**Data Architecture:**
- Unified dataState architecture - single source of truth
- Resource type selection (Website, NoticeBoard, Form)
- Resource-specific document state keys in Rust

**Three Document Architecture (NEW - 2025-10-15):**
- Three independent Yjs documents: mainDoc, commentsDoc, submissionsDoc
- docType-based routing for protocol layer permissions
- Multiple threads support: each thread block has own `${blockId}_comments`
- Multiple forms support: each form has own `${formId}_submissions`
- Reddit-style collapsible comments with Markdown support
- Form validation and submission to submissionsDoc

---

## CURRENT ARCHITECTURE: Three Independent Yjs Documents (2025-10-15)

### Three Separate Y.Doc Instances

The system uses **three independent Yjs documents** for different purposes:

```typescript
mainDoc (blocksuite_doc)
  └── Y.Map("blocks")       → Website structure (screens, sections, all blocks)
  └── Y.Map("viewport")     → Canvas viewport (builder mode)

commentsDoc (thread_comments_doc)
  └── Y.Map("blocks")       → Thread comments (collaborative)
      └── "${blockId}_comments" → { items: [{id, author, content, timestamp}] }

submissionsDoc (form_submissions_doc)
  └── Y.Map("blocks")       → Form submissions (append-only)
      └── "${formId}_submissions" → { items: [{id, formId, eventName, data, timestamp}] }
```

### Key Principles

1. **Three Independent Documents**: Each document syncs separately with its own `docType` identifier
2. **Multiple Threads Support**: Each thread block has its own comments stored as `${blockId}_comments`
3. **Multiple Forms Support**: Each form has its own submissions stored as `${formId}_submissions`
4. **Protocol-Layer Permissions**: UI sends all updates, protocol/sovereign node enforces permissions
5. **Automatic Detection**: System detects thread/form blocks and creates appropriate docs

### Document Creation Logic

```typescript
// YjsManager.initialize(resourceType)
if (resourceType === 'noticeboard' || resourceType === 'website') {
  // Create commentsDoc for thread blocks
}

if (resourceType === 'form' || resourceType === 'website') {
  // Create submissionsDoc for form blocks
}

// Website resources can have BOTH docs simultaneously
```

### Update Flow with docType Routing

```
Component updates Yjs doc
  ↓
YjsManager detects 'updateV2' event
  ↓
Calls onUpdate(update, origin, docType)
  ↓
Coordinator → onCollaborationUpdate(update)
  ↓
Backend sends to sovereign node with docType identifier
  ↓
Protocol layer routes based on docType:
  - 'blocksuite_doc' → Owner writes, all viewers receive
  - 'thread_comments_doc' → Anyone writes, all viewers receive
  - 'form_submissions_doc' → Viewers write, only owner receives
```

### Data Storage Example

Website with both threads and forms saves as:

```json
{
  "blocksuite_doc": [/* Yjs updates for screens/blocks */],
  "thread_comments_doc": [/* Yjs updates for all comments across all threads */],
  "form_submissions_doc": [/* Yjs updates for all submissions across all forms */],
  "last_modified": 1697123456789
}
```

### Permission Matrix (Enforced at Protocol Layer)

| Document | Owner | Viewer | Sync Behavior |
|----------|-------|--------|---------------|
| **blocksuite_doc** | ✅ Read/Write | 👁️ Read-only | Owner → Viewers |
| **thread_comments_doc** | ✅ Read/Write | ✅ Read/Write | Bidirectional (collaborative) |
| **form_submissions_doc** | ✅ Read (all) | ✏️ Write (own only) | Viewers → Owner |

**Note:** Permission enforcement happens at the protocol/sovereign node layer, NOT in the UI.

### Thread Block System

Thread blocks can be placed anywhere in a website:

```typescript
// ThreadBlock stores data in mainDoc
{
  type: 'thread',
  blockId: 'block-123',
  name: 'Discussion Thread',
  description: 'Talk about this topic',
  content: '# Main Post Content\n\nLong blog post...',
  mode: 'markdown' // or 'html'
}

// Comments stored in commentsDoc
{
  'block-123_comments': {
    items: [
      { id: 'c1', author: 'Alice', content: '<p>Great post!</p>', timestamp: 1697123456 },
      { id: 'c2', author: 'Bob', content: '<p>Thanks!</p>', timestamp: 1697123500 }
    ]
  }
}
```

Multiple thread blocks each maintain their own independent comment threads.

### Form Submission System

Forms can be placed anywhere in a website:

```typescript
// Form blocks in mainDoc (invisible metadata)
{
  type: 'form',
  id: 'form-456',
  eventName: 'contact_form_submission'
}

// Form fields linked by formId
{ type: 'form-field-text', formId: 'form-456', fieldName: 'name', required: true }
{ type: 'form-field-email', formId: 'form-456', fieldName: 'email', required: true }

// Submissions stored in submissionsDoc
{
  'form-456_submissions': {
    items: [
      {
        id: 's1',
        formId: 'form-456',
        eventName: 'contact_form_submission',
        data: { name: 'Alice', email: 'alice@example.com' },
        timestamp: 1697123456
      }
    ]
  }
}
```

Multiple forms each maintain their own independent submission queues.

---

## Implementation Status

### ✅ Completed (2025-10-15)

**Three Document Architecture:**
- ✅ YjsManager creates 3 independent docs (mainDoc, commentsDoc, submissionsDoc)
- ✅ Automatic detection of thread/form blocks in blocksuite_doc
- ✅ Document routing based on docType parameter
- ✅ Loading and saving all 3 documents

**Thread Blocks:**
- ✅ ThreadBlock.svelte component with Reddit-style collapsible comments
- ✅ Markdown/HTML support with DOMPurify sanitization
- ✅ Real-time Yjs observation for collaborative commenting
- ✅ Multiple threads per document (each with own `${blockId}_comments`)

**Form System:**
- ✅ FormField.svelte for all field types
- ✅ FormSubmitButton.svelte with validation
- ✅ Submissions saved to submissionsDoc
- ✅ Multiple forms per document (each with own `${formId}_submissions`)

**Component Integration:**
- ✅ FullScreenViewer passes commentsDoc and submissionsDoc separately
- ✅ ViewerMode provides both docs to viewer
- ✅ Proper TypeScript types throughout

### 🔄 Protocol Layer TODO

The protocol/sovereign node needs to route updates based on `docType`:

```rust
// Pseudo-code
fn handle_yjs_update(update: Vec<u8>, sender_id: u64, doc_type: &str) {
    match doc_type {
        "blocksuite_doc" => {
            if sender_id == owner_id {
                merge_and_broadcast_to_all(update, doc_type);
            }
        }
        "thread_comments_doc" => {
            merge_and_broadcast_to_all(update, doc_type);
        }
        "form_submissions_doc" => {
            merge_and_send_to_owner_only(update, doc_type);
        }
        _ => {}
    }
}
```

---

## Key Files

### Frontend (TypeScript/Svelte)
```
sthalam/frontend/desktop/src/
├── lib/
│   ├── yjsManager.ts - Yjs document management (3 docs: main, comments, submissions)
│   ├── blocksuiteCoordinator.ts - Doc initialization and sync with docType routing
│   ├── WebsiteBuilder.svelte - Main builder interface
│   ├── BlockPalette.svelte - Block types palette
│   ├── PropertiesPanel.svelte - Block configuration
│   ├── Canvas.svelte - Infinite canvas with blocks
│   ├── Block.svelte - Individual block rendering
│   ├── FullScreenViewer.svelte - Viewer mode renderer with doc routing
│   └── blocks/
│       ├── ThreadBlock.svelte - Thread with collapsible comments
│       ├── FormField.svelte - Universal form field component
│       ├── FormSubmitButton.svelte - Form submission handler
│       ├── NavButton.svelte - Screen navigation
│       └── BranchingQuestion.svelte - Yes/No branching logic
├── components/
│   └── ViewerMode.svelte - Viewer mode container
├── utils/
│   └── blocksuiteUtils.ts - Doc creation utilities
└── state/
    └── data.svelte.ts - Global application state
```

### Backend (Rust)
```
services/src/
├── node_service.rs - UCAN token generation
├── resource_service.rs - Resource CRUD and validation

network/src/p2p/
├── resource_sync.rs - P2P sync protocol
└── website_handler.rs - Website-specific handlers

core/src/models/
└── p2p.rs - P2P message types
```

---

## Architecture Comparison

| Aspect | Planned (Single Doc) | Actual Implementation (Three Docs) |
|--------|---------------------|-------------------------------------|
| Document count | 1 Y.Doc with 3 Y.Maps | 3 independent Y.Doc instances |
| Data structure | `blocks`, `threads`, `submissions` Y.Maps | Each doc has own `blocks` Y.Map |
| Sync complexity | Single doc, collection-level routing | Three docs, docType-based routing |
| Permissions | Per-collection UCAN tokens | Per-docType at protocol layer |
| Update model | Pull from sovereign node | Push with docType identifier |
| Thread support | Single `threads` collection | Multiple `${blockId}_comments` entries |
| Form support | Single `submissions` collection | Multiple `${formId}_submissions` entries |
| Scalability | New collections in same doc | New documents as needed |

**Why Three Docs:** Simpler to implement, each document syncs independently, protocol layer has clear routing logic based on docType.

---

## Next Session Starting Point

### Frontend Status: ✅ Complete

The three-document architecture is fully implemented in the UI:
- mainDoc (blocksuite_doc) - website structure
- commentsDoc (thread_comments_doc) - collaborative comments
- submissionsDoc (form_submissions_doc) - form submissions

**Key accomplishment:** Multiple threads and multiple forms are fully supported. Each thread block and each form maintains independent data.

### Backend Status: 🔄 Needs Implementation

The protocol/sovereign node layer needs to implement `docType`-based routing:

**Files to modify:**
1. `services/src/node_service.rs` - Update Yjs sync handlers
2. `network/src/p2p/resource_sync.rs` - Add docType routing logic
3. `core/src/models/p2p.rs` - Add docType field to sync messages

**Implementation tasks:**
1. Extract `docType` from Yjs update messages
2. Route `blocksuite_doc` updates: owner only → broadcast to all
3. Route `thread_comments_doc` updates: anyone → broadcast to all
4. Route `form_submissions_doc` updates: viewers → send to owner only
5. Test multi-client sync with different permissions

---

**Status:** Frontend architecture complete with three independent Yjs documents. Backend protocol layer needs to implement permission-based routing using the `docType` identifier.
