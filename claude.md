# Sthalam - Osvauld BlockSuite Editor

## Quick Start

```bash
# Run Sthalam in development mode
cd sthalam/src-tauri
cargo tauri dev
```

## Current Focus

**Goal**: Show a basic BlockSuite document on the UI

**App**: Sthalam - A collaborative website/document editor built on Osvauld using BlockSuite instead of ProseMirror

---

## Core Architecture

### Resource Model (from core/src/models/resource.rs)

Every document in Osvauld is a **Resource**:

```rust
pub struct Resource {
    pub id: String,
    pub resource_type: ResourceType,  // "Website" for Sthalam
    pub data: String,                 // JSON string of document content
    pub folder_id: String,
    pub created_folder_id: String,
    pub signature: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub favourite: bool,
    pub created_by: String,
    pub last_accessed: i64,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
}
```

### Resource Type: Website

For Sthalam, `ResourceType::Website` uses:
- **Primary state key**: `"blocksuite_doc"`
- **Secondary state**: `"image_state"` (optional, for images)

```rust
// From resource.rs line 280
ResourceType::Website => vec!["blocksuite_doc", "image_state"]
```

### Data Storage Format

The `data` field is a **JSON string** containing:

```typescript
// Stored in Resource.data
{
  "blocksuite_doc": [1, 2, 3, ...],  // Yjs state as number array
  "image_state": [4, 5, 6, ...]      // Optional: images
}
```

**Key Point**: Store ONLY `blocksuite_doc` and optionally `image_state`. No email, username, or other fields needed.

---

## Data Flow

### 1. Loading a Document

```typescript
// Frontend: BlockSuiteEditor.svelte (lines 38-61)
const response = await invoke('handle_get_resource', {
  input: { resource_id: resourceId }
});

// Response structure (from Rust handlers)
interface ResourceResponse {
  id: string;
  data: string;        // JSON string!
  favourite: boolean;
  last_accessed: number;
  folder_id: string;
}

// Parse the data
const resourceData = JSON.parse(response.data);
// resourceData = { blocksuite_doc: [1,2,3,...], image_state: [...] }

// Load into BlockSuite
if (resourceData.blocksuite_doc && resourceData.blocksuite_doc.length > 0) {
  const stateData = new Uint8Array(resourceData.blocksuite_doc);
  Y.applyUpdate(doc.spaceDoc, stateData);
}
```

### 2. Saving a Document

```typescript
// Frontend: BlockSuiteEditor.svelte (lines 111-135)
async function saveContent() {
  const doc = collection.getDoc(resourceId);

  // Get Yjs state from BlockSuite's internal doc
  const state = Y.encodeStateAsUpdate(doc.spaceDoc);

  // Convert to array
  const yjsArray = Array.from(state);

  // Wrap in object
  const dataToSave = JSON.stringify({
    blocksuite_doc: yjsArray
  });

  // Save via Tauri
  await invoke('handle_update_resource', {
    input: {
      id: resourceId,
      data: dataToSave  // JSON string
    }
  });
}
```

---

## Backend Handlers (Rust)

### Current Handlers (sthalam/src-tauri/src/handlers/website_handler.rs)

1. **`handle_load_website_state`** (lines 118-144)
   - Loads resource from database
   - Returns raw Yjs state bytes
   - Uses WebsiteState for coordination

2. **`handle_update_website_state`** (lines 96-115)
   - Applies updates to WebsiteState
   - Emits events for real-time sync

3. **`handle_generate_share_token`** (lines 34-93)
   - Creates UCAN tokens for sharing

### Reference: Livnote Handlers

For complete resource CRUD, refer to `livnote/src-tauri/src/handlers/resource_handler.rs`:

**Key handler pattern** (lines 22-100):
```rust
#[tauri::command]
pub async fn handle_add_resource(
    input: AddResourceInput,
    user_state: State<'_, UserState>,
    app_handle: AppHandle,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;

    let resource_added = create_resource(
        input.resource_payload,  // JSON string of content
        input.resource_type,     // "website"
        input.folder_id,
        &user,
        &device.id,
        &"sthalam".to_string(),  // App name
        repo_ctx.inner().clone(),
        &crypto_utils,
    ).await?;

    // Generate preview
    let resource_preview = ResourcePreview {
        id: resource_added.id.clone(),
        title: "Untitled",  // Extract from content
        preview: "",
        folder_id: resource_added.folder_id.clone(),
        favourite: false,
        last_accessed: resource_added.last_accessed,
        last_modified: resource_added.last_accessed,
    };

    // Return response
    let response = ResourceResponse {
        id: resource_added.id.clone(),
        data: resource_added.data,  // JSON string
        favourite: resource_added.favourite,
        last_accessed: resource_added.last_accessed,
        folder_id: resource_added.folder_id.clone(),
    };

    // Emit event for UI update
    app_handle
        .emit("resource-added", resource_preview)
        .map_err(|e| e.to_string())?;

    Ok(CryptoResponse::Resource(response))
}
```

---

## Frontend State Management

### Reference: Livnote Data State (livnote/frontend/desktop/state/data.svelte.ts)

Key patterns for Sthalam:

```typescript
// State structure (lines 8-45)
class DataState {
  websites = $state<WebsitePreview[]>([]);
  currentWebsiteId = $state<string | null>(null);
  currentWebsiteData = $state<Website | null>(null);

  async loadWebsite(websiteId: string) {
    const response = await invoke('handle_get_resource', {
      input: { resource_id: websiteId }
    });

    // response is ResourceResponse { id, data, ... }
    this.currentWebsiteData = {
      id: response.id,
      data: JSON.parse(response.data),  // Parse JSON string!
      folder_id: response.folder_id,
    };
  }

  async saveWebsite(websiteId: string) {
    const state = getYjsStateFromBlockSuite();
    const dataToSave = JSON.stringify({
      blocksuite_doc: Array.from(state)
    });

    await invoke('handle_update_resource', {
      input: {
        id: websiteId,
        data: dataToSave
      }
    });
  }
}
```

---

## BlockSuite Integration

### Setup (sthalam/frontend/desktop/components/editor/BlockSuiteEditor.svelte)

```typescript
import { AffineEditorContainer } from '@blocksuite/presets';
import { DocCollection, Schema } from '@blocksuite/store';
import { AffineSchemas } from '@blocksuite/blocks';
import * as Y from 'yjs';

// 1. Create schema
const schema = new Schema();
schema.register(AffineSchemas);

// 2. Create collection (workspace)
const collection = new DocCollection({ schema });
collection.meta.initialize();

// 3. Create doc
const doc = collection.createDoc({ id: resourceId });

// 4. Initialize with default content
doc.load(() => {
  const pageBlockId = doc.addBlock('affine:page', {
    title: new doc.Text('Untitled')
  });
  doc.addBlock('affine:surface', {}, pageBlockId);
  const noteId = doc.addBlock('affine:note', {}, pageBlockId);
  doc.addBlock('affine:paragraph', {}, noteId);
});

// 5. Load existing state (if any)
const stateData = new Uint8Array(yjsArray);
Y.applyUpdate(doc.spaceDoc, stateData);

// 6. Create editor
const editor = new AffineEditorContainer();
editor.doc = doc;

// 7. Mount to DOM
editorContainer.appendChild(editor);

// 8. Listen for changes
doc.spaceDoc.on('update', (update: Uint8Array) => {
  // Debounce and save
  debouncedSave();
});
```

---

## Response Types

### From Rust to TypeScript

**IMPORTANT**: `CryptoResponse` enum has `#[serde(untagged)]` - variants serialize WITHOUT the variant name wrapper!

Based on Livnote patterns, define these types:

```typescript
// Response from handle_get_resource (CryptoResponse::SelectedResourceResponse is untagged)
interface ResourceResponse {
  id: string;
  data: any;           // Already parsed object (serde_json::Value), NOT a JSON string!
  favourite: boolean;
  last_accessed: number;
  folder_id: string;
}

// Parsed data structure
interface WebsiteContent {
  blocksuite_doc: number[];  // Yjs state array
  image_state?: number[];    // Optional images
}

// Complete website object
interface Website {
  id: string;
  data: WebsiteContent;  // Parsed JSON
  folder_id: string;
  favourite: boolean;
  last_accessed: number;
}

// Preview for list view
interface WebsitePreview {
  id: string;
  title: string;
  preview: string;
  folder_id: string;
  favourite: boolean;
  last_accessed: number;
  last_modified: number;
}
```

---

## Key Differences: Livnote vs Sthalam

| Aspect | Livnote (ProseMirror) | Sthalam (BlockSuite) |
|--------|----------------------|---------------------|
| Editor | ProseMirror | BlockSuite |
| Resource Type | `Notes` | `Website` |
| State Keys | `main_doc`, `image_state`, `comment_state` | `blocksuite_doc`, `image_state` |
| Coordinator | `NotesCoordinator` (complex) | Simpler or direct usage |
| Data in Resource.data | `{ main_doc: [...], image_state: [...], comment_state: [...] }` | `{ blocksuite_doc: [...], image_state: [...] }` |

---

## Common Patterns

### Pattern 1: Creating Empty Content

```typescript
// sthalam/frontend/desktop/utils/documentUtils.ts
export function createEmptyWebsiteContent(): any {
  const schema = new Schema().register(AffineSchemas);
  const collection = new DocCollection({ schema });
  collection.meta.initialize();

  const doc = collection.createDoc({ id: 'temp-doc' });
  doc.load(() => {
    const pageBlockId = doc.addBlock('affine:page', {});
    doc.addBlock('affine:surface', {}, pageBlockId);
    const noteId = doc.addBlock('affine:note', {}, pageBlockId);
    doc.addBlock('affine:paragraph', {}, noteId);
  });

  const yjsArray = Array.from(Y.encodeStateAsUpdate(doc.spaceDoc));

  return { blocksuite_doc: yjsArray };
}
```

### Pattern 2: Debounced Auto-save

```typescript
let saveTimeout: ReturnType<typeof setTimeout> | null = null;

function handleDocUpdate(update: Uint8Array, origin: any) {
  if (saveTimeout) clearTimeout(saveTimeout);
  saveTimeout = setTimeout(() => {
    saveContent();
  }, 1000);  // Wait 1 second after last edit
}

doc.spaceDoc.on('update', handleDocUpdate);
```

### Pattern 3: Cleanup

```typescript
onDestroy(() => {
  // Save final state
  if (!readOnly) {
    saveContent();
  }

  // Remove listeners
  doc.spaceDoc.off('update', handleDocUpdate);

  // Cleanup editor
  if (editor && editorContainer) {
    editorContainer.removeChild(editor);
    editor = null;
  }

  collection = null;
});
```

---

## Debugging Checklist

### Issue: Resource data is empty or undefined

**Check**:
1. Is `response.data` a string? → Parse it: `JSON.parse(response.data)`
2. Is it coming from backend as `ResourceResponse`?
3. Log the raw response: `console.log('Raw response:', response)`

### Issue: Yjs state not loading

**Check**:
1. Array conversion: `new Uint8Array(array)` not just `array`
2. Array is numbers: `[1, 2, 3, ...]` not `["1", "2", ...]`
3. Apply before editor creation or after doc.load()
4. Check for errors in console

### Issue: Changes not saving

**Check**:
1. `data` parameter is JSON string: `JSON.stringify({ blocksuite_doc: [...] })`
2. Update handler is receiving correct format
3. Backend is persisting changes
4. No errors in Rust logs

---

## Current File Structure

```
sthalam/
├── frontend/desktop/
│   ├── components/
│   │   ├── editor/
│   │   │   ├── BlockSuiteEditor.svelte      ← Main editor component
│   │   │   ├── EditorContainer.svelte       ← Container wrapper
│   │   │   ├── WebsiteEditor.svelte         ← Alternative editor
│   │   │   ├── websitesCoordinator.ts       ← Coordinator (optional)
│   │   │   └── editor.ts
│   │   ├── layout/
│   │   │   ├── ViewerLayout.svelte
│   │   │   └── EditorLayout.svelte
│   │   ├── resources/
│   │   │   └── ResourceList.svelte
│   │   └── viewer/
│   │       └── FolderNavigator.svelte
│   ├── state/
│   │   └── website.svelte.ts                ← State management
│   ├── utils/
│   │   └── documentUtils.ts                 ← Yjs utilities
│   ├── types/
│   │   └── notes.types.ts                   ← Type definitions
│   ├── App.svelte
│   └── package.json
└── src-tauri/
    ├── src/
    │   ├── handlers/
    │   │   └── website_handler.rs           ← Tauri commands
    │   ├── website_state.rs                 ← State management
    │   └── lib.rs
    └── Cargo.toml
```

---

## Next Steps for Basic UI

1. **Create handlers** (if missing):
   - `handle_get_resource` - Load website
   - `handle_update_resource` - Save website
   - `handle_add_resource` - Create new website
   - `handle_list_resources` - List all websites

2. **Create state management**:
   - Reference `livnote/frontend/desktop/state/data.svelte.ts`
   - Adapt for websites instead of notes
   - Use same event patterns

3. **Update UI**:
   - Ensure `BlockSuiteEditor.svelte` is properly mounted
   - Pass `resourceId` as prop
   - Handle loading states

4. **Test flow**:
   ```
   App loads → List websites → Click website → Load blocksuite_doc → Show in editor
   ```

---

## Reference Commands

```bash
# Development
cd sthalam/src-tauri
cargo tauri dev

# Check Rust
cargo check

# View logs
# Logs appear in terminal running cargo tauri dev

# Frontend only (if needed)
cd sthalam/frontend/desktop
pnpm dev
```

---

## Important Notes

1. **`response.data` is already an object**: `serde_json::Value` means it's parsed, **NOT** a JSON string! Don't call `JSON.parse()` on it!
2. **`CryptoResponse` is untagged**: The enum uses `#[serde(untagged)]` so response is direct object, not wrapped in variant name
3. **Yjs arrays are numbers**: `[1, 2, 3, ...]` not strings
4. **Convert before applying**: `new Uint8Array(array)`
5. **Save as JSON string**: `JSON.stringify({ blocksuite_doc: [...] })`
6. **Resource type is "Website"**: Use `ResourceType::Website` in Rust
7. **Primary key is "blocksuite_doc"**: Not "blocksuit_doc" (note the 'e') - **FIXED typo in all files**
8. **Reference Livnote**: Same patterns for CRUD, just different editor

## Recent Fixes

### 2025-10-07: Fixed typo in field name
- **Issue**: `blocksuit_doc` was used instead of `blocksuite_doc` (missing 'e')
- **Fixed in**:
  - `websitesCoordinator.ts` (interface and all usages)
  - `documentUtils.ts`
  - `BlockSuiteEditor.svelte`
  - `WebsiteEditor.svelte`
- **Also fixed**: Made `editorContainer` reactive in `BlockSuiteEditor.svelte` using `$state`

### 2025-10-07: Fixed response handling in WebsiteEditor
- **Issue**: `response.Resource?.data` was incorrect - CryptoResponse is `#[serde(untagged)]` so no variant wrapper
- **Issue**: Calling `JSON.parse()` on already-parsed data - `ResourceResponse.data` is `serde_json::Value` (already an object)
- **Fixed**:
  - Changed `response.Resource?.data` → `response?.data`
  - Removed `JSON.parse()` - data is already an object
  - Updated parameter: `resourceId` → `resource_id` in invoke call

### 2025-10-07: Fixed architecture - Follow Livnote's state management pattern
- **Issue**: Component-based data fetching caused timing issues with DOM element binding
- **Root cause**: `$effect` runs before `element` is bound, and prop-based approach doesn't match Livnote's pattern
- **Solution**: Refactored to follow Livnote's `switchNote` pattern:
  1. **State Layer** (`website.svelte.ts`): `selectResource()` now fetches data (async)
  2. **Component Layer** (`WebsiteEditor.svelte`): Uses `$effect` to watch `currentResourceId` from state
  3. **Data Flow**: Click resource → `selectResource()` fetches via invoke → updates state → `$effect` triggers → editor loads
- **Changes**:
  - Made `selectResource()` async and fetch data
  - Removed `resourceId` prop from `WebsiteEditor`
  - Editor now gets resource ID and data from `websiteState.currentResource()`
  - Added `setTimeout` in $effect to wait for DOM element binding
- **Key learning**: Follow Livnote's pattern - state layer handles data, components react to state

---

## Quick Reference: Key APIs

### Tauri Commands (TypeScript → Rust)
```typescript
// Load resource
await invoke('handle_get_resource', {
  input: { resource_id: string }
}) → ResourceResponse

// Save resource
await invoke('handle_update_resource', {
  input: { id: string, data: string }
}) → CryptoResponse

// Create resource
await invoke('handle_add_resource', {
  input: AddResourceInput
}) → CryptoResponse
```

### BlockSuite APIs
```typescript
// Encode state
const state = Y.encodeStateAsUpdate(doc.spaceDoc)  // Uint8Array

// Apply state
Y.applyUpdate(doc.spaceDoc, state)  // state: Uint8Array

// Listen to changes
doc.spaceDoc.on('update', (update: Uint8Array, origin: any) => {})
```

---

*Focus: Get BlockSuite document rendering on screen with proper data loading and saving*

---

# Website Builder MVP Plan

## Overview
Building a website builder in Sthalam using Yjs for data synchronization. The builder features an infinite canvas where users can drag, drop, resize, and configure blocks to create websites.

## Current State (2025-10-08)
- ✅ Basic Yjs block system implemented in `sthalam/frontend/desktop/src/lib/BlockSuiteEditor.svelte`
- ✅ Inline editing working
- ✅ TypeScript checking enabled with vite-plugin-checker
- ⚠️ Need to pivot from linear block editor to infinite canvas builder

## MVP Scope
Focus on **builder mode only** - viewing/publishing to Kunki comes later.

### Core Features for MVP
1. **Infinite Canvas**
   - Pan around the canvas (drag background to move)
   - Blocks positioned with absolute x/y coordinates
   - No zoom initially (1:1 scale)

2. **Block Management**
   - Drag blocks to reposition them anywhere on canvas
   - Resize blocks by dragging corner/edge handles
   - Z-index layering (bring forward/send backward)
   - Delete blocks

3. **Block Types** (Start Simple)
   - Text block (paragraph)
   - Heading block (H1, H2, H3)
   - Container/Box block (for layout)
   - More types can be added incrementally

4. **Properties Panel**
   - Right sidebar showing selected block's properties
   - Edit text content
   - Configure styles (colors, fonts, spacing, borders)
   - Adjust dimensions (width, height)
   - Z-index controls (layer up/down)

5. **Block Palette/Toolbar**
   - Left sidebar or top toolbar with available block types
   - Click to add new block to canvas center

## Technical Architecture

### Data Model (Yjs)
```typescript
// Canvas document structure in Y.Doc
{
  canvas: {
    blocks: Y.Map<string, Block> where Block = {
      id: string,           // unique block ID
      type: string,         // 'text' | 'heading' | 'container' | ...
      x: number,            // absolute position in pixels
      y: number,            // absolute position in pixels
      width: number,        // in pixels
      height: number,       // in pixels
      zIndex: number,       // layering
      content: string,      // text content for text/heading blocks
      styles: {             // CSS-like properties
        backgroundColor?: string,
        color?: string,
        fontSize?: string,
        fontWeight?: string,
        padding?: string,
        border?: string,
        borderRadius?: string,
        // ... more as needed
      }
    },
    viewport: Y.Map {
      x: number,            // canvas pan offset x
      y: number,            // canvas pan offset y
      zoom: number          // for future (start with 1.0)
    },
    selectedBlockId: string | null  // currently selected block
  }
}
```

### Component Structure
```
src/lib/
├── WebsiteBuilder.svelte          # Main builder component
├── Canvas.svelte                  # Infinite canvas with pan
├── Block.svelte                   # Individual draggable/resizable block
├── BlockPalette.svelte            # Left sidebar with block types
├── PropertiesPanel.svelte         # Right sidebar for selected block
└── blocks/
    ├── TextBlock.svelte
    ├── HeadingBlock.svelte
    └── ContainerBlock.svelte
```

## Implementation Steps

### Phase 1: Canvas Foundation
1. Create infinite canvas component with pan capability
2. Render blocks at absolute x/y positions
3. Implement canvas viewport state in Yjs

### Phase 2: Block Interaction
4. Make blocks draggable (update x/y in Yjs)
5. Add resize handles (8 handles: corners + edges)
6. Implement z-index layering (bring forward/backward)

### Phase 3: Block Types & Content
7. Create reusable Block wrapper component
8. Implement TextBlock, HeadingBlock, ContainerBlock
9. Make block content editable inline

### Phase 4: UI Panels
10. Create BlockPalette with available block types
11. Create PropertiesPanel that updates Yjs on changes
12. Connect panels to selected block state

### Phase 5: Styling System
13. Implement style properties in properties panel
14. Apply styles to blocks dynamically
15. Add preset styles/themes (optional for MVP)

## Key Libraries/Tools
- **Yjs**: Already integrated, for data model
- **Svelte 5**: Using runes for reactivity
- **CSS transforms**: For dragging/resizing interactions
- **contenteditable**: For inline text editing

## Design Decisions
1. **Block positioning**: Use absolute pixel coordinates
2. **Layering**: Yes, z-index support needed
3. **Resizable blocks**: Yes, for website builder
4. **Properties panel**: Yes
5. **Grid snap**: Not in MVP, can add later
6. **Zoom**: Not in MVP, start at 1:1 scale
7. **Undo/Redo**: Yjs supports this natively, can add later
8. **Multi-select**: Not in MVP
9. **Copy/Paste**: Not in MVP

## Non-MVP Features (Future)
- Grid snap and alignment guides
- Zoom in/out
- Multiple pages/documents (but supported in architecture)
- Publishing to Kunki (sovereign node)
- Token-based view mode for other clients
- Collaboration (multiple users editing simultaneously)
- More block types (Image, Button, Form elements, etc.)
- Custom blocks
- Responsive breakpoints
- Export to HTML/CSS
- Templates

## Architecture Notes

### Kunki (Sovereign Node)
- User publishes websites to their Kunki node
- Kunki stores the website and serves it in view mode only
- Others can access with tokens
- No editing on Kunki - it's read-only

### Sthalam
- Desktop app for creating, editing, and publishing websites
- Builder mode: Full editing capabilities
- Can also view published websites from other users (with token)

### Data Flow (Future)
1. Create website in Sthalam (builder mode)
2. Publish to Kunki → sends Yjs state
3. Share token with others
4. Others use token to fetch and view website in their Sthalam

## Notes
- Focus on builder mode only for now
- Data sync with Kunki comes after MVP is working locally
- Keep it simple - can iterate and add features incrementally
- Support all types of blocks and custom blocks if necessary
- Pages are supported in architecture (independent documents)
