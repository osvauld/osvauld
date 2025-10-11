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

## Current Status (2025-10-12)

### ✅ Completed
- Infinite canvas website builder with drag/drop blocks
- Block types: Text, Heading, Container, Image, HTML/CSS
- Yjs-based data synchronization
- Builder mode (full editing)
- Viewer mode (readonly display)
- P2P connection with UCAN tokens
- Basic publishing and connecting with token
- Three separate resource types: Website, NoticeBoard, Form
- Resource type selection modal when creating new pages
- Resource type-specific document state keys in Rust

### 🚧 In Progress
- **Publish update to sovereign node from builder**
- **NoticeBoard and Form builders** (need separate builder components)
- **Accept updates from public notice board**

---

## Resource Types

Each resource type is independent with its own document state keys and builder UI.

### Website
- **State keys**: `blocksuite_doc`
- Canvas-based infinite layout with blocks
- Blocks positioned with x/y coordinates, draggable/resizable
- Block types: Heading, Text, Image, Container, HTML/CSS
- **Builder**: `WebsiteBuilder.svelte` with BlockPalette and PropertiesPanel

### NoticeBoard
- **State keys**: `thread_doc`
- Reddit-style discussion thread using BlockSuite
- Main thread post (editable by owner)
- Cascading replies (all users can reply)
- **Builder**: ⚠️ TODO - Need separate NoticeBoard builder component
- **Status**: Type registered, needs builder implementation

### Form
- **State keys**: `form_doc`
- Form builder using BlockSuite blocks
- Form fields as draggable blocks
- Submission creates subdoc sent to sovereign node
- **Builder**: ⚠️ TODO - Need separate Form builder component
- **Status**: Type registered, needs builder implementation

---

## Data Model

### Resource Structure
```rust
pub struct Resource {
    pub id: String,
    pub resource_type: ResourceType,  // Website | NoticeBoard | Form
    pub data: String,                 // JSON of content
    pub folder_id: String,
    // ... timestamps, metadata
}

pub enum ResourceType {
    Notes,
    Chat,
    Website,      // Canvas-based website builder
    NoticeBoard,  // Discussion thread
    Form,         // Form builder
    Default,
}

impl ResourceType {
    pub fn document_state_keys(&self) -> Vec<&'static str> {
        match self {
            ResourceType::Website => vec!["blocksuite_doc"],
            ResourceType::NoticeBoard => vec!["thread_doc"],
            ResourceType::Form => vec!["form_doc"],
            ResourceType::Notes => vec!["main_doc", "image_state", "comment_state"],
            ResourceType::Chat => vec!["chat", "image_state"],
            ResourceType::Default => vec!["yjs_state"],
        }
    }
}
```

### Canvas Data (Yjs) - For Website Type
```typescript
{
  blocks: Y.Map<string, Block> where Block = {
    id: string,
    type: 'text' | 'heading' | 'container' | 'image' | 'html',
    x: number, y: number,         // position
    width: number, height: number, // dimensions
    zIndex: number,                // layering
    content: string,               // text content or HTML
    styles: { /* CSS properties, or 'css' for html type */ }
  },
  viewport: { x, y, zoom },
  selectedBlockId: string | null
}
```

---

## P2P Architecture

### Connection Types
- **Device**: Device-to-device sync
- **User**: User sharing
- **Website**: Publishing and viewing

### Connection Flow
```
1. Builder publishes → generates connection string with UCAN token
2. Viewer connects with string → handshake with token
3. Node validates UCAN → sends folder + resources
4. Viewer displays in readonly mode
5. Interactive blocks (forms, noticeboards) still functional
```

### Connection String Format
Base64-encoded JSON:
```json
{
  "user_public_key": "base64...",
  "device_public_key": "base64...",
  "username": "NodeOwner",
  "ucan_token": "eyJ...",
  "ucan_pub_key": "base64..."
}
```

---

## Modes

### Builder Mode - Website
- Full editing capabilities
- Left: NavigationPanel with folders and "+" button for new pages
- Resource creation shows modal with:
  - Title input
  - Type selector (Website, Notice Board, Form)
- Left sidebar: BlockPalette (add blocks)
- Right sidebar: PropertiesPanel (edit selected block)
- Center: Canvas with drag/drop/resize
- Available blocks: Heading, Text, Image, Container, HTML/CSS
- **TODO: Add "Publish Update" button to sync changes to sovereign node**

### Builder Mode - NoticeBoard (TODO)
- Thread editor for owner to write main post
- Preview of reply structure
- No BlockPalette needed

### Builder Mode - Form (TODO)
- Form field palette (text, email, textarea, checkbox, etc.)
- Drag/drop form fields onto canvas
- Configure field properties (label, placeholder, required, etc.)

### Viewer Mode
- Readonly display (no editing)
- Left: Sidebar with synced resources
- Center: Display based on resource type
- Interactive elements work (forms submit, notice boards accept comments)
- **TODO: Accept and sync updates from notice board posts**

---

## Key APIs

### Tauri Commands
```typescript
// Load resource
invoke('handle_get_resource', { input: { resource_id } })

// Save resource
invoke('handle_update_resource', { input: { id, data: JSON.stringify({...}) } })

// Connect to website
invoke('handle_connect_to_website', { input: { connectionString } })
```

### Yjs APIs
```typescript
// Encode state
const state = Y.encodeStateAsUpdate(doc)

// Apply state
Y.applyUpdate(doc, state)

// Listen to changes
doc.on('update', (update: Uint8Array) => {})
```

---

## Next Steps

### 1. Build NoticeBoard Builder
- Create `NoticeBoardBuilder.svelte` component
- Main thread editor (BlockSuite for rich text)
- Reply thread preview
- Route to correct builder based on resource type

### 2. Build Form Builder
- Create `FormBuilder.svelte` component
- Form field palette (text input, email, textarea, checkbox, radio, select, etc.)
- Drag/drop interface for field arrangement
- Field property editor (label, placeholder, required, validation)
- Form preview mode

### 3. Routing Logic
- Update App.svelte to route based on `resource_type`
  - `Website` → `WebsiteBuilder.svelte`
  - `NoticeBoard` → `NoticeBoardBuilder.svelte`
  - `Form` → `FormBuilder.svelte`

### 4. Publish Update to Sovereign Node
- Add "Publish Update" button in Builder mode (all types)
- Send ResourceUpdate message to sovereign node when clicked
- Include state vector for efficient sync

### 5. Accept Updates from Notice Board
- Viewer posts comment on notice board
- Send update to sovereign node
- Node broadcasts to all viewers
- Implement bidirectional sync for notice boards

### 6. Form Submission Flow
- Create subdoc for each form submission
- Send to sovereign node
- Owner can view all submissions
- Aggregate submissions in a table/list view

---

## File Structure
```
sthalam/
├── frontend/desktop/src/
│   ├── components/
│   │   ├── NavigationPanel.svelte       # Folder tree & navigation
│   │   ├── WebsiteFolder.svelte         # Folder with resources
│   │   ├── CreateResourceModal.svelte   # Title + type selection modal ✅
│   │   ├── ViewerMode.svelte
│   │   └── AddWebsiteConnectionModal.svelte
│   ├── lib/
│   │   ├── WebsiteBuilder.svelte        # Website builder (current) ✅
│   │   ├── Canvas.svelte                # Infinite canvas
│   │   ├── Block.svelte                 # Draggable/resizable block ✅
│   │   ├── BlockPalette.svelte          # Block type palette ✅
│   │   ├── PropertiesPanel.svelte       # Block property editor
│   │   ├── NoticeBoardBuilder.svelte    # TODO: NoticeBoard builder
│   │   └── FormBuilder.svelte           # TODO: Form builder
│   ├── state/
│   │   ├── data.svelte.ts               # Resource management ✅
│   │   └── ui.svelte.ts
│   └── utils/
│       └── helper.ts
├── core/src/
│   └── models/
│       └── resource.rs                  # ResourceType enum ✅
└── src-tauri/src/
    ├── handlers/
    │   └── website_handler.rs
    └── lib.rs
```

✅ = Updated in this session

---

## Important Notes
- `response.data` from Rust is already parsed (not JSON string)
- Yjs arrays are `number[]`
- Convert with `new Uint8Array(array)` before applying
- Save as `JSON.stringify({ blocksuite_doc: [...] })`
- Resource keys generated on-the-fly for website sharing (not persisted)
- UCAN tokens control folder access permissions
