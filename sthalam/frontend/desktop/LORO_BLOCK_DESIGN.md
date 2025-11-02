# Loro Block Design - Type System

## Loro Tree Structure

**How Loro works:**
```typescript
const tree = doc.getTree('blocks');
const node = tree.createNode();

// Each node has:
node.id                    // Unique ID (Loro manages)
node.parent                // Parent node reference (Loro manages)
node.children()           // Array of child nodes (Loro manages)
node.data                 // LoroMap for custom properties (WE manage)
```

**Key Insight:** Loro manages tree structure, we manage block properties in `node.data`

---

## Block Properties Mapping

```typescript
// Block properties stored in node.data (LoroMap)
node.data.set('type', 'container');
node.data.set('name', 'My Container');
node.data.set('css', 'background: blue;');
node.data.set('visible', true);
node.data.set('stateKey', 'user.email');

// Read back
const type = node.data.get('type');
const name = node.data.get('name');
```

---

## What's NOT in Block

**Removed (Loro manages):**
- ❌ `id` - Loro provides node.id
- ❌ `parentId` - Loro provides node.parent
- ❌ `children` - Loro provides node.children()

**Removed (Editor concern):**
- ❌ `x` - Editor layout only
- ❌ `y` - Editor layout only
- ❌ `width` - Editor layout only
- ❌ `height` - Editor layout only
- ❌ `zIndex` - Editor layout only

**Removed (Unused):**
- ❌ `manualConnections` - Not needed
- ❌ `styles` - Use `css` instead

---

## Clean Block Interface

```typescript
/**
 * Block properties stored in Loro tree node.data
 * Each property maps to: node.data.set(key, value)
 */
export interface Block {
  // Core (required)
  type: 'container' | 'text' | 'image' | 'input' | 'button' | 'collaborative';

  // Universal (all blocks)
  name?: string;                     // Display name
  visible?: boolean | string;        // Visibility (supports CEL)
  css?: string;                      // Custom CSS (supports CEL)
  order?: number;                    // Sibling order (for sorting)

  // Fine-grained reactivity
  stateKey?: string;                 // Primary state dependency
  stateKeys?: string[];              // Multiple dependencies

  // Loop support (forEach)
  forEach?: string;                  // Array name to loop over
  forEachAs?: string;                // Variable name (default: 'item')

  // Type-specific properties

  // container
  isEntryPoint?: boolean;            // Mark as entry screen
  isModal?: boolean;                 // Modal overlay

  // text
  content?: string;                  // Text content (supports CEL)
  mode?: 'plain' | 'markdown' | 'html';  // Rendering mode
  level?: 1 | 2 | 3 | 4 | 5 | 6;    // Heading level

  // image
  src?: string;                      // Image URL (supports CEL)
  alt?: string;                      // Alt text

  // input
  inputType?: 'text' | 'email' | 'number' | 'textarea' | 'checkbox' | 'select' | 'radio';
  formId?: string;                   // Parent form ID
  fieldName?: string;                // Field identifier
  label?: string;                    // Field label
  placeholder?: string;              // Input placeholder
  required?: boolean;                // Required validation
  defaultValue?: any;                // Default value
  options?: Array<{label: string, value: any}>;  // For select/radio

  // button
  action?: 'navigate' | 'setState' | 'submit';
  targetContainerId?: string;        // Navigation target
  stateValue?: any;                  // setState value (supports CEL)
  stateUpdates?: Record<string, any>;  // Bulk state updates
  submit?: boolean;                  // Form submit button
  fieldName?: string;                // Field to cache (for forms)
  value?: any;                       // Cached value (supports CEL)
  eventName?: string;                // Event name for submissions

  // collaborative
  title?: string;                    // Thread title
  description?: string;              // Thread description
}
```

---

## Usage Patterns

### Creating Blocks
```typescript
const tree = doc.getTree('blocks');

// Create container
const container = tree.createNode();
container.data.set('type', 'container');
container.data.set('name', 'Main Screen');
container.data.set('isEntryPoint', true);

// Create child text
const text = tree.createNode(container.id);
text.data.set('type', 'text');
text.data.set('content', 'Hello {{ user.name }}');
text.data.set('stateKey', 'user.name');

// Create input
const input = tree.createNode(container.id);
input.data.set('type', 'input');
input.data.set('inputType', 'email');
input.data.set('formId', 'signup');
input.data.set('fieldName', 'email');
input.data.set('label', 'Email Address');
input.data.set('stateKey', 'form.email');
```

### Reading Blocks
```typescript
// Get all root blocks
const roots = tree.roots();

// Get children (O(1) - instant!)
const children = node.children();

// Get block properties
const type = node.data.get('type');
const name = node.data.get('name');
const visible = node.data.get('visible');

// Helper to convert node to Block object
function nodeToBlock(node: LoroTreeNode): Block & { id: string } {
  const block: any = { id: node.id };

  // Copy all properties from node.data
  node.data.forEach((value, key) => {
    block[key] = value;
  });

  return block;
}
```

### Updating Blocks
```typescript
// Update single property
node.data.set('content', 'Updated content');

// Update multiple properties
node.data.set('visible', false);
node.data.set('css', 'display: none;');
```

### Tree Operations
```typescript
// Move node (with entire subtree)
node.moveTo(newParent, index);

// Delete node (with entire subtree)
node.delete();

// Get parent
const parent = node.parent;

// Check if has children
const hasChildren = node.children()?.length > 0;
```

---

## Helper Types

```typescript
// Node with block data
export interface BlockNode {
  id: string;
  parent?: BlockNode;
  children(): BlockNode[];
  data: LoroMap;
  moveTo(parent: BlockNode, index: number): void;
  delete(): void;
}

// Block with metadata (for components)
export interface BlockWithMeta extends Block {
  id: string;                        // From node.id
  hasChildren: boolean;              // Computed
  childCount: number;                // Computed
}

// Editor layout (separate from Block)
export interface EditorLayout {
  blockId: string;
  x: number;
  y: number;
  width: number;
  height: number;
}
```

---

## Store Pattern

```typescript
export class TemplateStructureStore {
  private tree: LoroTree | null = null;

  setTree(tree: LoroTree): void {
    this.tree = tree;

    // Subscribe to changes
    tree.subscribe(() => {
      this.notifyObservers();
    });
  }

  // Get block by node ID
  getBlock(nodeId: string): Block | null {
    if (!this.tree) return null;

    const node = this.tree.getNodeById(nodeId);
    if (!node) return null;

    return nodeToBlock(node);
  }

  // Get children (O(1)!)
  getChildren(nodeId: string): Block[] {
    if (!this.tree) return [];

    const node = this.tree.getNodeById(nodeId);
    if (!node) return [];

    const children = node.children();
    return children?.map(nodeToBlock) || [];
  }

  // Get all root blocks
  getRoots(): Block[] {
    if (!this.tree) return [];

    const roots = this.tree.roots();
    return roots.map(nodeToBlock);
  }

  // Update block
  updateBlock(nodeId: string, updates: Partial<Block>): void {
    if (!this.tree) return;

    const node = this.tree.getNodeById(nodeId);
    if (!node) return;

    // Update properties in node.data
    Object.entries(updates).forEach(([key, value]) => {
      node.data.set(key, value);
    });
  }

  // Create block
  createBlock(parentId: string | null, block: Block): string {
    if (!this.tree) throw new Error('Tree not initialized');

    // Create node
    const node = parentId
      ? this.tree.createNode(parentId)
      : this.tree.createNode();

    // Set properties
    Object.entries(block).forEach(([key, value]) => {
      node.data.set(key, value);
    });

    return node.id;
  }

  // Delete block (with subtree)
  deleteBlock(nodeId: string): void {
    if (!this.tree) return;

    const node = this.tree.getNodeById(nodeId);
    if (!node) return;

    node.delete();  // Loro handles cascade delete
  }

  // Move block
  moveBlock(nodeId: string, newParentId: string, index: number): void {
    if (!this.tree) return;

    const node = this.tree.getNodeById(nodeId);
    const newParent = this.tree.getNodeById(newParentId);
    if (!node || !newParent) return;

    node.moveTo(newParent, index);
  }
}
```

---

## Benefits of This Design

1. **No Redundancy**
   - Loro manages: id, parent, children
   - Block manages: content properties
   - No duplication!

2. **Type Safe**
   - Single Block interface
   - TypeScript knows all properties
   - Easy to update

3. **Performance**
   - O(1) children access (no scanning!)
   - Native tree operations
   - Efficient updates

4. **Clean API**
   ```typescript
   // Before (Yjs):
   const children = Array.from(blocks.values())
     .filter(b => b.parentId === id);  // O(n) scan

   // After (Loro):
   const children = node.children();   // O(1) instant
   ```

5. **HUML Compatible**
   ```huml
   block: container
     name: Main Screen
     isEntryPoint: true

     block: text
       content: Hello {{ user.name }}
       stateKey: user.name

     block: input
       inputType: email
       formId: signup
       fieldName: email
   ```

---

## Migration Notes

**From Yjs flat map:**
```typescript
// Old: Flat map with parentId
blocks: Y.Map = {
  'id1': { id: 'id1', type: 'container', parentId: null },
  'id2': { id: 'id2', type: 'text', parentId: 'id1' }
}
```

**To Loro tree:**
```typescript
// New: Tree with node.data
tree.createNode() → id1
  node.data = { type: 'container' }

  tree.createNode(id1) → id2
    node.data = { type: 'text' }
```

**Conversion function:**
```typescript
function convertYjsToLoro(yjsBlocks: Y.Map, loroTree: LoroTree) {
  // Build tree structure
  const nodeMap = new Map<string, LoroTreeNode>();

  // First pass: create all nodes
  yjsBlocks.forEach((block, id) => {
    if (!block.parentId) {
      const node = loroTree.createNode();
      nodeMap.set(id, node);
    }
  });

  // Second pass: create children recursively
  function createChildren(oldParentId: string, newParentNode: LoroTreeNode) {
    yjsBlocks.forEach((block, id) => {
      if (block.parentId === oldParentId) {
        const node = loroTree.createNode(newParentNode.id);
        nodeMap.set(id, node);

        // Copy properties (excluding id, parentId)
        Object.entries(block).forEach(([key, value]) => {
          if (key !== 'id' && key !== 'parentId' && key !== 'x' && key !== 'y' && key !== 'width' && key !== 'height' && key !== 'zIndex') {
            node.data.set(key, value);
          }
        });

        // Recurse
        createChildren(id, node);
      }
    });
  }

  // Start from roots
  nodeMap.forEach((node, id) => {
    const block = yjsBlocks.get(id);

    // Copy properties
    Object.entries(block).forEach(([key, value]) => {
      if (key !== 'id' && key !== 'parentId' && key !== 'x' && key !== 'y' && key !== 'width' && key !== 'height' && key !== 'zIndex') {
        node.data.set(key, value);
      }
    });

    createChildren(id, node);
  });
}
```
