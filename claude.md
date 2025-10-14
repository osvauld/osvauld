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

---

## NEW ARCHITECTURE: Unified Single Document (2025-10-14)

### Single BlockSuite Doc with Three Collections

Previous POC used **separate Yjs documents** (❌ deprecated):
- ❌ `thread_doc` + `thread_comments_doc` (separate Y.Doc instances)
- ❌ `form_doc` + `form_submissions_doc` (separate Y.Doc instances)
- **Why deprecated:** Overly complex, required separate sync logic, difficult to scale

**NEW: Single Y.Doc with three Y.Map collections:**

```typescript
Y.Doc (one document)
├── Y.Map("blocks")       → Website structure (readonly for viewers)
├── Y.Map("threads")      → Thread comments (collaborative, read/write for viewers)
└── Y.Map("submissions")  → Form submissions (write-only for viewers, read-only for owner)
```

### Key Principles

1. **Single Document**: All data in one Y.Doc, not separate documents
2. **Collection-Level Permissions**: UCAN tokens specify permissions per Y.Map collection
3. **Pull Model**: Viewers request updates from sovereign node (not push/broadcast)
4. **Thread Block Type**: New draggable "thread" block that can be placed anywhere in website
5. **Embedded Logic**: UCAN tokens encode which collections can be read/written per role

### Update Flow (Pull Model)

```
Viewer opens doc → Request updates from sovereign node
                 → Sovereign returns:
                    ✓ blocks (website structure)
                    ✓ threads (comments)
                    ✗ submissions (not included for viewers)

Owner opens doc → Request updates from sovereign node
                → Sovereign returns:
                   ✓ blocks (website structure)
                   ✓ threads (comments)
                   ✓ submissions (only owner sees these)
```

### Sovereign Node Merge Logic

```rust
match collection_name {
    "blocks" => {
        if sender_is_owner { merge_and_store() }
        // All viewers can pull when they request
    },
    "threads" => {
        merge_and_store()  // Anyone can write
        // All viewers can pull when they request
    },
    "submissions" => {
        merge_and_store()  // Viewers can write
        // Only owner can pull when they request
    }
}
```

### UCAN Token Structure

```typescript
{
  docId: "website-abc123",
  permissions: {
    "blocks": { read: true, write: false },      // Viewer readonly
    "threads": { read: true, write: true },      // Viewer can comment
    "submissions": { write: true, read: false }  // Viewer can submit (blind write)
  }
}
```

### Permission Matrix

| Collection | Owner | Viewer | Sync Behavior |
|------------|-------|--------|---------------|
| **blocks** | Read/Write | Read-only | Owner changes pulled by viewers |
| **threads** | Read/Write | Read/Write | Collaborative CRDT merge, all see updates |
| **submissions** | Read-only | Write-only | Viewers submit, only owner can read |

---

## Implementation Roadmap

### Phase 1: Frontend Collections (Next)
1. Update `blocksuiteUtils.ts` to create three Y.Map collections in single doc
2. Add "thread" block type to BlockPalette alongside other blocks
3. Update coordinator to handle collection-specific data
4. Initialize all three collections on doc creation

### Phase 2: UCAN Collection Permissions
1. Modify UCAN token structure to include collection-level permissions
2. Update token validation to check collection access
3. Implement sovereign node filtering logic for pull requests

### Phase 3: Pull Model Implementation
1. Update sovereign node to filter collections based on UCAN tokens
2. Implement viewer request/response flow
3. Test owner vs viewer data access
4. Ensure submissions never sent to viewers

### Phase 4: Thread & Form Interaction
1. Build thread comment UI (similar to existing NoticeBoard but as inline block)
2. Build form submission handler (send to `submissions` collection)
3. Test collaborative commenting
4. Test form submissions with blind write

---

## Key Files

### Frontend (TypeScript/Svelte)
```
sthalam/frontend/desktop/src/
├── lib/
│   ├── yjsManager.ts - Yjs document management
│   ├── blocksuiteCoordinator.ts - Doc initialization and sync
│   ├── WebsiteBuilder.svelte - Main builder interface
│   ├── BlockPalette.svelte - Block types palette
│   ├── PropertiesPanel.svelte - Block configuration
│   ├── Canvas.svelte - Infinite canvas with blocks
│   ├── Block.svelte - Individual block rendering
│   ├── FullScreenViewer.svelte - Viewer mode renderer
│   └── NoticeBoardBuilder.svelte - Thread UI (to be adapted)
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

| Aspect | Old (Split Docs) | New (Single Doc) |
|--------|------------------|------------------|
| Document count | 2-4 Y.Doc instances | 1 Y.Doc instance |
| Collections | `blocks` in each doc | `blocks`, `threads`, `submissions` |
| Sync complexity | Multiple doc sync | Single doc sync |
| Permissions | Per-document | Per-collection within doc |
| Update model | Broadcast/push | Pull from sovereign node |
| Thread placement | Separate noticeboard | "thread" block type in website |
| Scalability | Hard to add features | Easy to add new collections |

---

## Next Session Starting Point

1. **Create three collections in single doc:**
   - File: `sthalam/frontend/desktop/src/utils/blocksuiteUtils.ts`
   - Task: Initialize `blocks`, `threads`, `submissions` Y.Map in single Y.Doc

2. **Add thread block type:**
   - File: `sthalam/frontend/desktop/src/lib/BlockPalette.svelte`
   - Task: Add `{ type: "thread", label: "Thread", icon: "💬", ... }`

3. **Update coordinator:**
   - File: `sthalam/frontend/desktop/src/lib/blocksuiteCoordinator.ts`
   - Task: Access all three collections from single doc

---

**Status:** Architecture redesigned for simplicity and scalability. Ready to implement unified document structure with collection-based permissions.
