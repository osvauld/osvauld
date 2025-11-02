# Loro vs Yjs Analysis for Tree-Based Block System

## Current Yjs Usage

**Files using Yjs (10 total):**
1. `src/lib/yjsManager.ts` - Core document management
2. `src/lib/templateCoordinator.ts` - Uses yjsManager
3. `src/lib/templateStructureStore.ts` - Y.Map observer
4. `src/lib/collaborativeStore.ts` - Y.Map + Y.Array
5. `src/lib/submissionsStore.ts` - Y.Map
6. `src/lib/templateState.svelte.ts` - Y.Map observer
7. `src/lib/blocks/BlockRenderer.svelte` - Type imports
8. `src/lib/blocks/NavButton.svelte` - Type imports
9. `src/lib/FullScreenViewer.svelte` - Type imports
10. `src/utils/blocksuiteUtils.ts` - Type imports

**Yjs Data Structures:**
```typescript
// Current implementation
blocksuiteDoc.getMap("blocks")  // Y.Map<string, Block>
commentsDoc.getMap("blocks")    // Y.Map<string, Comments>
submissionsDoc.getMap("blocks") // Y.Map<string, Submissions>
```

---

## Why Loro is Better for This Use Case

### 1. **Native Tree Support** ⭐⭐⭐

**Yjs (Current):**
```typescript
// Flat map with parentId references
blocks: Y.Map<string, Block> = {
  'block1': { id: 'block1', type: 'container', parentId: null },
  'block2': { id: 'block2', type: 'text', parentId: 'block1' },
  'block3': { id: 'block3', type: 'text', parentId: 'block1' }
}

// Build tree manually every time
function getChildren(blockId: string): Block[] {
  return Array.from(blocks.values())
    .filter(b => b.parentId === blockId);  // O(n) scan
}
```

**Loro:**
```typescript
// Native tree structure
const tree = doc.getTree("blocks");
const root = tree.createNode({ id: 'root', type: 'container' });
const child = root.createNode({ id: 'child1', type: 'text' });

// Get children is O(1)
const children = node.children();  // Built-in!

// Move subtrees
node.moveTo(newParent, index);  // Native operation

// Delete subtrees
node.delete();  // Automatically deletes all descendants
```

**Benefits:**
- ✅ O(1) children access vs O(n) scan
- ✅ Native move/delete operations
- ✅ Tree structure preserved in CRDT
- ✅ No manual tree construction needed

---

### 2. **Better Performance** ⚡

**Loro benchmarks:**
- ~10x faster than Yjs for tree operations
- Written in Rust (compiled to WASM)
- More efficient memory usage

**For your use case:**
```typescript
// Current: O(n) to find all children at each render
function renderContainer(block: Block) {
  const children = getAllBlocks().filter(b => b.parentId === block.id);
  // Scan 1000 blocks every time
}

// Loro: O(1) to get children
function renderContainer(node: LoroNode) {
  const children = node.children();
  // Instant access
}
```

---

### 3. **Rich Text Support** 📝

You mentioned rich text as a reason to consider Loro earlier.

**Yjs:**
```typescript
// Basic text
Y.Text - plain collaborative text
// For rich text need: y-prosemirror or y-quill or y-codemirror
```

**Loro:**
```typescript
// Built-in rich text
LoroText - supports formatting, styles
// Native support for:
- Bold, italic, underline
- Lists (ordered, unordered)
- Headings
- Custom attributes
```

**Your text blocks could use:**
```typescript
{
  type: 'text',
  mode: 'rich',  // Instead of markdown
  content: LoroText  // Native rich text CRDT
}
```

---

### 4. **Time Travel** ⏰

**Loro has built-in time travel:**
```typescript
// Get document at any point in history
const snapshot = doc.checkout(version);

// Undo/redo built-in
doc.undo();
doc.redo();
```

**Yjs requires:**
```typescript
// Manual undo manager
const undoManager = new Y.UndoManager(ymap);
// Limited capabilities
```

---

### 5. **Migration Effort** 🔧

**Files needing changes:**
- ✅ `yjsManager.ts` → `loroManager.ts` (complete rewrite)
- ✅ `templateCoordinator.ts` → Update document handling
- ✅ `templateStructureStore.ts` → LoroTree observer
- ✅ `collaborativeStore.ts` → LoroList observer
- ✅ `submissionsStore.ts` → LoroMap observer
- ✅ `templateState.svelte.ts` → LoroMap observer

**NOT needing changes:**
- ✅ All UI components (they use stores, not Yjs directly)
- ✅ CEL evaluator
- ✅ Block components
- ✅ Template importer (works with plain objects)

**Backend changes:**
- ❌ Update sync protocol (different binary format)
- ❌ Migrate existing documents (Yjs → Loro converter needed)

---

## Comparison Table

| Feature | Yjs | Loro | Winner |
|---------|-----|------|--------|
| Tree Structure | Manual (parentId) | Native (LoroTree) | **Loro** 🏆 |
| Performance | Good | Excellent (~10x) | **Loro** 🏆 |
| Rich Text | Via plugins | Native | **Loro** 🏆 |
| Time Travel | Manual | Built-in | **Loro** 🏆 |
| Maturity | Very mature (2015) | Newer (2022) | **Yjs** |
| Ecosystem | Large | Growing | **Yjs** |
| Awareness | Built-in | Limited | **Yjs** |
| Bundle Size | ~50KB | ~200KB (WASM) | **Yjs** |
| Type Safety | Good | Excellent (Rust) | **Loro** 🏆 |

---

## Code Comparison

### Current (Yjs):
```typescript
// Define flat structure
const blocks = doc.getMap('blocks');

// Add block
blocks.set('block1', {
  id: 'block1',
  type: 'container',
  parentId: null
});

blocks.set('block2', {
  id: 'block2',
  type: 'text',
  parentId: 'block1',
  content: 'Hello'
});

// Get children (manual scan)
function getChildren(blockId: string) {
  const result = [];
  blocks.forEach((block, id) => {
    if (block.parentId === blockId) {
      result.push(block);
    }
  });
  return result;
}

// Move block (manual updates)
function moveBlock(blockId: string, newParentId: string) {
  const block = blocks.get(blockId);
  block.parentId = newParentId;
  blocks.set(blockId, block);
}
```

### With Loro:
```typescript
// Define tree structure
const tree = doc.getTree('blocks');

// Add blocks (native hierarchy)
const root = tree.createNode({
  id: 'block1',
  type: 'container'
});

const child = root.createNode({
  id: 'block2',
  type: 'text',
  content: 'Hello'
});

// Get children (built-in, O(1))
function getChildren(node: LoroNode) {
  return node.children();
}

// Move block (native operation)
function moveBlock(node: LoroNode, newParent: LoroNode) {
  node.moveTo(newParent, index);
}

// Delete subtree (automatic)
function deleteSubtree(node: LoroNode) {
  node.delete();  // Deletes all descendants automatically
}
```

---

## Migration Path

### Phase 1: Parallel Implementation
```typescript
// Add Loro alongside Yjs
import * as Y from 'yjs';
import { Loro, LoroTree } from 'loro-crdt';

// Support both formats temporarily
```

### Phase 2: Create Loro Manager
```typescript
// src/lib/loroManager.ts
export class LoroManager {
  private doc: Loro;
  private tree: LoroTree;

  initialize() {
    this.doc = new Loro();
    this.tree = this.doc.getTree('blocks');
  }

  // Similar interface to yjsManager
}
```

### Phase 3: Update Stores
```typescript
// templateStructureStore.ts
export class TemplateStructureStore {
  private tree: LoroTree | null = null;

  setTree(tree: LoroTree): void {
    this.tree = tree;

    // Subscribe to changes
    tree.subscribe(this.handleTreeChange);
  }

  getBlock(blockId: string): Block | null {
    return this.tree.findNode(blockId);
  }

  getChildren(blockId: string): Block[] {
    const node = this.tree.findNode(blockId);
    return node ? node.children() : [];
  }
}
```

### Phase 4: Backend Migration
```typescript
// Converter script
function convertYjsToLoro(yjsUpdate: Uint8Array): Uint8Array {
  // 1. Decode Yjs document
  const ydoc = new Y.Doc();
  Y.applyUpdate(ydoc, yjsUpdate);
  const blocks = ydoc.getMap('blocks');

  // 2. Create Loro document
  const loroDoc = new Loro();
  const tree = loroDoc.getTree('blocks');

  // 3. Convert flat map to tree
  const rootBlocks = [];
  blocks.forEach((block, id) => {
    if (!block.parentId) {
      rootBlocks.push(block);
    }
  });

  // 4. Build tree recursively
  function buildTree(parentNode: LoroNode | null, parentId: string | null) {
    blocks.forEach((block, id) => {
      if (block.parentId === parentId) {
        const node = parentNode
          ? parentNode.createNode(block)
          : tree.createNode(block);
        buildTree(node, block.id);
      }
    });
  }

  buildTree(null, null);

  // 5. Export Loro format
  return loroDoc.export();
}
```

---

## Recommendation

### ✅ **SWITCH TO LORO** if:
1. ✅ You're doing extensive tree operations (containers, hierarchy)
2. ✅ You need rich text (not just markdown)
3. ✅ Performance is important (large documents)
4. ✅ You want time-travel/undo built-in
5. ✅ You're willing to do migration work (10 files)

### ❌ **STAY WITH YJS** if:
1. ❌ Can't afford backend migration downtime
2. ❌ Need proven ecosystem (plugins, integrations)
3. ❌ Awareness/presence is critical (Loro support is limited)
4. ❌ Bundle size is a concern (Loro is 4x larger)

---

## Your Situation

**You said:**
- "we dont have a lot of yjs dependencies right?" ✅ Only 10 files
- Tree structure for rendering ✅ Loro excels here
- Rich text interest ✅ Loro has native support
- Major refactor anyway ✅ Good time to switch

**My recommendation: SWITCH TO LORO** 🎯

**Timing:**
- Do it NOW during the major refactor
- Before you have more data/users
- While the architecture is fresh

**Benefits:**
1. Native tree structure (no more parentId scanning)
2. Better performance (10x for tree ops)
3. Rich text ready
4. Cleaner code (less manual tree management)
5. Time travel built-in

**Cost:**
- 1-2 weeks migration effort
- Backend protocol update
- Document migration script

---

## Decision Matrix

| Factor | Weight | Yjs | Loro | Winner |
|--------|--------|-----|------|--------|
| Tree Support | High | 2/5 | 5/5 | Loro |
| Your Use Case Fit | High | 3/5 | 5/5 | Loro |
| Migration Effort | Medium | 5/5 | 2/5 | Yjs |
| Maturity | Low | 5/5 | 3/5 | Yjs |
| Performance | High | 3/5 | 5/5 | Loro |
| **Total** | | **18/25** | **20/25** | **Loro** |

**Verdict: LORO wins for your use case** 🏆
