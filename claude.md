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
- **Form blocks**: Form Container, Text/Email/Number inputs, Textarea, Checkbox, Submit Button
- Yjs-based data synchronization
- Builder mode (full editing)
- Viewer mode (readonly display)
- **Forms are interactive in viewer mode** - can submit with JSON output
- **Form and Website share same WebsiteBuilder** (block-based architecture)
- P2P connection with UCAN tokens
- Basic publishing and connecting with token
- Three separate resource types: Website, NoticeBoard, Form
- Resource type selection modal when creating new pages
- Resource type-specific document state keys in Rust
- **Unified dataState architecture** - single source of truth for resources + coordinator
- **NoticeBoard UI**: Reddit-style thread builder with rich text/HTML/CSS/markdown support
  - Main thread editor with markdown, HTML/CSS, and preview modes
  - Nested comment threading with collapse/expand
  - Comment input with markdown, HTML/CSS, and preview
  - Rich text rendering with syntax highlighting

### 🚧 In Progress
- **Publish update to sovereign node from builder**
- **Accept updates from public notice board**
- **NoticeBoard viewer mode integration**

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
- Reddit-style discussion thread
- Main thread post (editable by owner) with markdown/HTML/CSS/preview modes
- Nested comment threading with collapse/expand functionality
- Comment replies support markdown, HTML/CSS, and preview
- Each comment has: author, timestamp, content, mode (markdown/html), custom CSS
- **Builder**: `NoticeBoardBuilder.svelte` ✅
- **Components**: `ThreadEditor.svelte`, `CommentSection.svelte`, `CommentInput.svelte`, `Comment.svelte` ✅
- **Status**: ✅ UI implemented - backend integration pending

### Form
- **State keys**: `form_doc` (shares same blocksuite_doc structure as Website)
- Form builder using BlockSuite blocks (same as Website)
- Form fields as draggable blocks with properties (label, placeholder, fieldName, required)
- Form types: `form-container`, `form-field-text`, `form-field-email`, `form-field-number`, `form-field-textarea`, `form-field-checkbox`, `form-submit-button`
- Submit button spatially detects form container and collects field values
- Submission outputs JSON: `{ fieldName: value, ... }`
- **Builder**: `WebsiteBuilder.svelte` (shared with Website type)
- **Status**: ✅ Fully implemented - interactive in viewer mode

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

### Canvas Data (Yjs) - For Website & Form Types
```typescript
{
  blocks: Y.Map<string, Block> where Block = {
    id: string,
    type: 'text' | 'heading' | 'container' | 'image' | 'html' |
          'form-container' | 'form-field-text' | 'form-field-email' |
          'form-field-number' | 'form-field-textarea' | 'form-field-checkbox' |
          'form-submit-button',
    x: number, y: number,         // position
    width: number, height: number, // dimensions
    zIndex: number,                // layering
    content: string,               // text content or HTML or button label
    // Form-specific properties (when type starts with 'form-field-')
    label?: string,                // Field label
    placeholder?: string,          // Input placeholder
    fieldName?: string,            // JSON key for submission
    required?: boolean,            // Is field required
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

### Builder Mode - Website & Form
- Full editing capabilities (both types use same `WebsiteBuilder.svelte`)
- Left: NavigationPanel with folders and "+" button for new pages
- Resource creation shows modal with:
  - Title input
  - Type selector (Website, Notice Board, Form)
- Left sidebar: BlockPalette (add blocks)
- Right sidebar: PropertiesPanel (edit selected block)
- Center: Canvas with drag/drop/resize
- **Website blocks**: Heading, Text, Image, Container, HTML/CSS
- **Form blocks**: Form Container, Text Input, Email Input, Number Input, Textarea, Checkbox, Submit Button
- **Drag controls**: Ctrl+Click (Cmd+Click on Mac) to drag blocks
- **Resize**: Click edges/corners, or Shift+Click handles for incremental resize
- **TODO: Add "Publish Update" button to sync changes to sovereign node**

### Builder Mode - NoticeBoard
- Thread editor with three modes:
  - **Markdown mode**: Write posts in markdown with syntax support
  - **HTML/CSS mode**: Direct HTML and CSS editing in split view
  - **Preview mode**: See rendered output before posting
- Comment section with nested threading (Reddit-style)
- Each comment supports:
  - Markdown or HTML/CSS mode
  - Preview before posting
  - Reply functionality
  - Collapse/expand threads
  - Timestamp and author display
- Auto-save every 10 seconds
- No BlockPalette needed (focused on text content)

### Viewer Mode
- Readonly display (no editing blocks)
- Left: Sidebar with synced resources
- Center: Display based on resource type
- **Interactive elements work**:
  - ✅ Forms: Fields are editable, submit button collects data and logs JSON to console
  - 🚧 Notice boards: Can view threads (commenting TODO)
- Blocks cannot be dragged/resized in viewer mode
- **TODO: Accept and sync updates from notice board posts**
- **TODO: Send form submissions to sovereign node**

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
- Thread-based data model (different from canvas blocks)
- Route to correct builder based on resource type:
  - `Website` → `WebsiteBuilder.svelte` ✅
  - `Form` → `WebsiteBuilder.svelte` ✅
  - `NoticeBoard` → `NoticeBoardBuilder.svelte` 🚧

### 2. Publish Update to Sovereign Node
- Add "Publish Update" button in Builder mode (all types)
- Send ResourceUpdate message to sovereign node when clicked
- Include state vector for efficient sync

### 3. Accept Updates from Notice Board
- Viewer posts comment on notice board
- Send update to sovereign node
- Node broadcasts to all viewers
- Implement bidirectional sync for notice boards

### 4. Form Submission Flow
- Send form JSON to sovereign node (currently only logs to console)
- Create subdoc for each form submission
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
│   │   ├── WebsiteBuilder.svelte        # Website & Form builder ✅
│   │   ├── Canvas.svelte                # Infinite canvas + form submit logic ✅
│   │   ├── Block.svelte                 # Draggable/resizable block + form fields ✅
│   │   ├── BlockPalette.svelte          # Block type palette (includes form blocks) ✅
│   │   ├── PropertiesPanel.svelte       # Block property editor (includes form props) ✅
│   │   ├── blocksuiteCoordinator.ts     # Yjs document coordinator ✅
│   │   ├── yjsManager.ts                # Yjs lifecycle management ✅
│   │   ├── NoticeBoardBuilder.svelte    # NoticeBoard builder ✅
│   │   ├── ThreadEditor.svelte          # Main thread post editor ✅
│   │   ├── CommentSection.svelte        # Comments list and tree ✅
│   │   ├── CommentInput.svelte          # Comment input with modes ✅
│   │   └── Comment.svelte               # Individual comment display ✅
│   ├── state/
│   │   ├── data.svelte.ts               # Unified state: resources + coordinator ✅
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

✅ = Updated in this session (2025-10-12)
🗑️ Removed: FormBuilder.svelte (forms now use WebsiteBuilder)
🗑️ Removed: store.svelte.ts (consolidated into data.svelte.ts)

---

## Important Notes
- `response.data` from Rust is already parsed (not JSON string)
- Yjs arrays are `number[]`
- Convert with `new Uint8Array(array)` before applying
- Save as `JSON.stringify({ blocksuite_doc: [...] })`
- Resource keys generated on-the-fly for website sharing (not persisted)
- UCAN tokens control folder access permissions

---

## Session Summary (2025-10-12) - Form Builder Implementation

### Key Achievements
1. **Unified Architecture**: Consolidated two separate dataState instances into one
   - Moved coordinator management from `store.svelte.ts` into `state/data.svelte.ts`
   - Single source of truth for resources + BlockSuite coordinator
   - Simplified initialization flow in `App.svelte`

2. **Forms as Blocks**: Implemented block-based form builder
   - Both `Website` and `Form` types now use `WebsiteBuilder.svelte`
   - Added 7 new form block types: container, text, email, number, textarea, checkbox, submit button
   - Form fields are draggable/resizable blocks with properties (label, placeholder, fieldName, required)

3. **Interactive Forms in Viewer Mode**:
   - Form fields become editable in viewer mode
   - Submit button spatially detects parent form container
   - Collects field values and outputs JSON: `{ fieldName: value, ... }`
   - Form submission handler in `Canvas.svelte`

4. **Code Cleanup**:
   - Deleted `FormBuilder.svelte` (no longer needed)
   - Deleted `store.svelte.ts` (consolidated into unified state)
   - Removed all collaborator tracking code (not needed yet)
   - Fixed submit button dragging with `pointer-events: none` on disabled state

### Technical Details
- Form blocks use same Yjs document structure as Website blocks
- Spatial detection algorithm finds form container boundaries
- Submit button queries DOM for field values using `data-field-id` attributes
- Ctrl+Click (Cmd+Click) drag interaction works for all blocks including forms

### Next Up
- **NoticeBoard Viewer Mode**: Display threads and allow commenting in viewer mode
- **NoticeBoard Backend Integration**: Connect to Yjs thread_doc and sync comments
- Form submission to sovereign node (currently logs to console)
- Publish updates button for all resource types

---

## Session Summary (2025-10-12 PM) - NoticeBoard UI Implementation

### Key Achievements
1. **NoticeBoard Builder Created**: Complete Reddit-style thread UI
   - `NoticeBoardBuilder.svelte`: Main container with Yjs integration
   - `ThreadEditor.svelte`: Main post editor with markdown/HTML/CSS/preview modes
   - `CommentSection.svelte`: Comment management with nested threading
   - `CommentInput.svelte`: Reusable comment input with mode switching
   - `Comment.svelte`: Individual comment display with collapse/expand

2. **Rich Text Support**: Multiple editing modes
   - **Markdown mode**: Full markdown syntax support using `marked` library
   - **HTML/CSS mode**: Split view for direct HTML and CSS editing
   - **Preview mode**: Live preview of rendered content
   - Custom CSS injection for styled content

3. **Reddit-Style Threading**:
   - Nested comment replies with visual indentation
   - Collapse/expand functionality for comment threads
   - Reply counts and timestamps
   - Visual threading lines showing hierarchy

4. **Technical Implementation**:
   - Installed `marked` package for markdown parsing
   - Yjs integration with `thread_doc` state key
   - Comment tree structure with parent-child relationships
   - Auto-save every 10 seconds
   - Self-importing component pattern for recursive rendering

### Data Structure
```typescript
// Thread content
thread_content: string

// Comments array
comments: Array<{
  id: string,
  content: string,
  mode: 'markdown' | 'html',
  css: string,
  author: string,
  timestamp: string,
  parentId?: string,
  replies: Comment[]
}>
```

### Next Steps
- Integrate NoticeBoard with viewer mode
- Connect comments to P2P sync
- Add comment editing and deletion
- Implement real-time updates for new comments
