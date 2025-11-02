# Fresh Start Architecture - Sthalam with Loro CRDT

**Version:** 1.0
**Date:** 2025-11-01
**Purpose:** Complete architectural specification for fresh implementation

---

## 🎯 Overview

Sthalam is a sovereign web publishing platform enabling:
- **Publishers** create interactive content (websites, forms, blogs)
- **Viewers** browse and interact without platforms
- **P2P distribution** via sovereign nodes
- **LLM-first** template creation (main interface)

### Three UI Modes

```
┌─────────────┐     ┌──────────────┐     ┌────────────┐
│   Builder   │     │  Publisher   │     │   Viewer   │
│   (Edit     │────▶│  (Add        │────▶│  (Browse   │
│  Template)  │     │   Content)   │     │   & Use)   │
└─────────────┘     └──────────────┘     └────────────┘
```

---

## 📦 Loro Document Structure

### Five Loro Documents

```typescript
1. templateDoc (LoroTree)    - Template structure (blocks hierarchy)
2. contentDoc (LoroMap)      - Publisher content (shared: products, posts)
3. userContentDoc (LoroMap)  - Per-user state (cart, favorites)
4. commentsDoc (LoroMap)     - Comments/threads
5. submissionsDoc (LoroMap)  - Form submissions
```

### Document Details

#### 1. **templateDoc** - Loro Tree
```typescript
const tree = templateDoc.getTree('blocks');

// Structure: Tree of blocks
root (screen-container)
  ├─ section (section-container)
  │   ├─ heading (text)
  │   ├─ description (text)
  │   └─ button (button)
  └─ section (section-container)
      └─ product-grid (container with forEach)

// Each node.data contains block properties
node.data.set('type', 'container');
node.data.set('name', 'Hero Section');
node.data.set('visible', '{{ user.isLoggedIn }}');
node.data.set('css', 'background: #fff; padding: 20px;');
```

#### 2. **contentDoc** - Loro Map
```typescript
const content = contentDoc.getMap('content');

// Publisher content (shared across all users)
content.set('products', LoroList[
  {id: '1', name: 'Laptop', price: 999, image: 'url'},
  {id: '2', name: 'Mouse', price: 29, image: 'url'}
]);

content.set('blogPosts', LoroList[
  {id: 'post1', title: 'Hello', content: LoroText, author: 'Alice'}
]);

content.set('settings', LoroMap{
  theme: 'dark',
  currency: 'USD'
});
```

#### 3. **userContentDoc** - Loro Map
```typescript
const userContent = userContentDoc.getMap('user_content');

// Per-user isolated state
userContent.set('cartTotal', 0);
userContent.set('cartItems', LoroList[]);
userContent.set('favorites', LoroList['prod1', 'prod5']);
userContent.set('preferences', LoroMap{darkMode: true});
```

#### 4. **commentsDoc** - Loro Map
```typescript
const threads = commentsDoc.getMap('threads');

// Thread-based comments
threads.set('thread-block-id', LoroList[
  {
    id: 'comment1',
    author: 'Alice',
    userId: 'did:alice',
    content: 'Great post!',
    timestamp: 1234567890
  }
]);
```

#### 5. **submissionsDoc** - Loro Map
```typescript
const submissions = submissionsDoc.getMap('submissions');

// Organized by formId
submissions.set('contact-form', LoroList[
  {
    id: 'sub1',
    timestamp: 1234567890,
    viewer_id: 'did:viewer',
    fields: {
      name: {value: 'Bob', metadata: {parse_as: 'contact_name'}},
      email: {value: 'bob@test.com', metadata: {parse_as: 'email'}}
    }
  }
]);
```

---

## 🏗️ Block Types (6 Essential Types)

### Type System

```typescript
type BlockType =
  | 'container'        // Screens, sections, layouts
  | 'text'            // All text content (plain/markdown/html/rich)
  | 'image'           // Images
  | 'input'           // All form inputs
  | 'button'          // Actions (navigate/setState/submit)
  | 'collaborative';  // Comments/discussions
```

### Block Interface

```typescript
interface Block {
  // Core (stored in node.data)
  type: BlockType;

  // Universal
  name?: string;                     // Display name
  visible?: boolean | string;        // Visibility (supports CEL)
  css?: string;                      // Custom CSS (supports CEL)
  order?: number;                    // Sibling order

  // Fine-grained reactivity
  stateKey?: string;                 // Primary state dependency
  stateKeys?: string[];              // Multiple dependencies

  // Loop support
  forEach?: string;                  // Array name (e.g., "products")
  forEachAs?: string;                // Variable name (default: 'item')

  // Container-specific
  isEntryPoint?: boolean;            // Entry screen marker
  isModal?: boolean;                 // Modal overlay

  // Text-specific
  content?: string | LoroText;       // Text content (supports CEL)
  mode?: 'plain' | 'markdown' | 'html' | 'rich';  // Rendering mode
  level?: 1 | 2 | 3 | 4 | 5 | 6;    // Heading level

  // Image-specific
  src?: string;                      // Image URL (supports CEL)
  alt?: string;                      // Alt text

  // Input-specific
  inputType?: 'text' | 'email' | 'number' | 'textarea' | 'checkbox' | 'select' | 'radio';
  formId?: string;                   // Parent form ID
  fieldName?: string;                // Field identifier
  label?: string;                    // Field label
  placeholder?: string;              // Placeholder text
  required?: boolean;                // Required validation
  defaultValue?: any;                // Default value
  options?: Array<{label: string, value: any}>;  // For select/radio

  // Button-specific
  action?: 'navigate' | 'setState' | 'submit';
  targetContainerId?: string;        // Navigation target
  stateValue?: any;                  // setState value (supports CEL)
  stateUpdates?: Record<string, any>;  // Bulk state updates
  submit?: boolean;                  // Form submit button
  eventName?: string;                // Event name for submissions

  // Collaborative-specific
  title?: string;                    // Thread title
  description?: string;              // Thread description
}
```

### What's NOT in Block

**Removed (Loro manages):**
- ❌ `id` - Loro provides `node.id`
- ❌ `parentId` - Loro provides `node.parent`
- ❌ `children` - Loro provides `node.children()`

**Removed (Editor concern):**
- ❌ `x`, `y`, `width`, `height`, `zIndex` - Moved to separate `EditorLayout` map

**Removed (Unused):**
- ❌ `manualConnections` - Not needed
- ❌ `styles` - Use `css` instead

---

## 🎨 Rendering Architecture

### Viewer Rendering Flow

```typescript
// 1. Load template structure (Loro tree)
const tree = templateDoc.getTree('blocks');
const roots = tree.roots();  // Get root screens

// 2. Load content data (Loro map)
const content = contentDoc.getMap('content');
const products = content.get('products');  // LoroList

// 3. Load user state (Loro map)
const userContent = userContentDoc.getMap('user_content');
const cartTotal = userContent.get('cartTotal');

// 4. Render using template + data
function renderBlock(node: LoroNode) {
  const block = nodeToBlock(node);  // Convert node to Block

  // Handle forEach loops
  if (block.forEach) {
    const array = getStateValue(block.forEach);  // e.g., products
    return array.map((item, index) => {
      return renderBlock(node, {item, index, first, last});
    });
  }

  // Render block based on type
  switch (block.type) {
    case 'container':
      return renderContainer(node);
    case 'text':
      return renderText(block);
    case 'button':
      return renderButton(block);
    // ...
  }
}

function renderContainer(node: LoroNode) {
  const children = node.children();  // O(1) - instant!
  return (
    <div css={evaluateCEL(block.css)}>
      {children.map(child => renderBlock(child))}
    </div>
  );
}
```

### Key Insight: Template + Content Separation

```
Template (tree):     "Show product grid with forEach"
Content (map):       [Product1, Product2, Product3]
Viewer:              forEach products → render each using template
```

**Example:**
```typescript
// Template defines structure
{
  type: 'container',
  forEach: 'products',  // Loop over content.products
  blocks: [
    {type: 'image', src: '{{item.image}}'},
    {type: 'text', content: '{{item.name}}'},
    {type: 'text', content: '${{item.price}}'}
  ]
}

// Content provides data
content.products = [
  {name: 'Laptop', price: 999, image: 'url1'},
  {name: 'Mouse', price: 29, image: 'url2'}
]

// Viewer renders
→ Product card 1 (Laptop, $999, url1)
→ Product card 2 (Mouse, $29, url2)
```

---

## 🔧 Three UI Modes Implementation

### 1. Builder UI (`src/builder/`)

**Purpose:** Create/edit HUML templates
**Users:** Template creators (mostly LLMs)
**Key Features:**
- Tree view of blocks (no canvas)
- Properties panel for editing
- HUML import/export
- Live preview

**Components:**
```
src/builder/
  ├─ TreeView.svelte           # Hierarchical block tree
  ├─ PropertiesPanel.svelte    # Edit block properties
  ├─ BlockPalette.svelte       # Add new blocks
  ├─ PreviewPane.svelte        # Live preview
  └─ stores/
      └─ templateStore.ts      # Manages templateDoc (Loro tree)
```

**Core Operations:**
```typescript
// Add block
const parent = tree.getNodeById(parentId);
const newNode = tree.createNode(parent?.id);
newNode.data.set('type', 'text');
newNode.data.set('content', 'Hello');

// Update block
node.data.set('css', 'background: blue;');

// Delete block (with subtree)
node.delete();

// Move block
node.moveTo(newParent, index);
```

---

### 2. Publisher UI (`src/publisher/`)

**Purpose:** Add content to existing template
**Users:** Content publishers (store owners, bloggers)
**Key Features:**
- "Add Product" button
- "Create Post" form
- "Upload Images" interface
- **Defined by HUML template!**

**How it works:**
```huml
# Template defines what can be published
publisherActions::
  - ::
    type: "add-product"
    label: "Add New Product"
    icon: "shopping-cart"
    fields::
      - :: name: "productName", type: "text", required: true
      - :: name: "price", type: "number", required: true
      - :: name: "image", type: "image", required: true
      - :: name: "description", type: "richtext"

  - ::
    type: "add-blog-post"
    label: "New Blog Post"
    fields::
      - :: name: "title", type: "text"
      - :: name: "content", type: "richtext"
      - :: name: "author", type: "text"
```

**Publisher UI renders these actions:**
```typescript
// Read publisherActions from template
const actions = templateDoc.getMap('meta').get('publisherActions');

// Render UI dynamically
actions.forEach(action => {
  if (action.type === 'add-product') {
    renderAddProductForm(action.fields);
  }
});

// When publisher adds product
function addProduct(data) {
  const products = contentDoc.getMap('content').get('products');
  products.push([{
    id: crypto.randomUUID(),
    name: data.productName,
    price: data.price,
    image: data.image,
    description: data.description  // LoroText for rich content
  }]);
}
```

**Components:**
```
src/publisher/
  ├─ ActionButtons.svelte      # Dynamically rendered actions
  ├─ AddProductForm.svelte     # Add product UI
  ├─ AddPostForm.svelte        # Add blog post UI
  ├─ ManageContent.svelte      # View/edit existing content
  └─ stores/
      └─ contentStore.ts       # Manages contentDoc (Loro map)
```

---

### 3. Viewer UI (`src/viewer/`)

**Purpose:** Browse and interact
**Users:** End users
**Key Features:**
- Multi-screen navigation
- Fill forms
- Submit data
- Comment threads
- See products/posts

**Components:**
```
src/viewer/
  ├─ ScreenRenderer.svelte     # Render screens
  ├─ BlockRenderer.svelte      # Render individual blocks
  ├─ Navigation.svelte         # Screen navigation
  ├─ blocks/
  │   ├─ Container.svelte
  │   ├─ Text.svelte
  │   ├─ Image.svelte
  │   ├─ Input.svelte
  │   ├─ Button.svelte
  │   └─ Collaborative.svelte
  └─ stores/
      ├─ viewerState.ts        # Manages state, content, userContent
      └─ navigationStore.ts    # Screen navigation state
```

**Rendering:**
```svelte
<!-- ScreenRenderer.svelte -->
<script>
  import { templateStore, contentStore, userContentStore } from './stores';
  import BlockRenderer from './BlockRenderer.svelte';

  const currentScreen = $derived(
    tree.roots().find(node =>
      node.data.get('isEntryPoint') === true
    )
  );
</script>

{#if currentScreen}
  <BlockRenderer node={currentScreen} />
{/if}

<!-- BlockRenderer.svelte -->
<script lang="ts">
  let { node, loopContext } = $props();

  const block = $derived(nodeToBlock(node));
  const children = $derived(node.children() || []);

  // Evaluate CEL expressions
  const visible = $derived(evaluateCEL(block.visible, loopContext));
  const css = $derived(evaluateCEL(block.css, loopContext));
</script>

{#if visible}
  {#if block.forEach}
    {#each getArrayData(block.forEach) as item, index}
      <BlockRenderer
        node={node}
        loopContext={{item, index, first: index === 0, last: index === length-1}}
      />
    {/each}
  {:else if block.type === 'container'}
    <div style={css}>
      {#each children as child}
        <BlockRenderer node={child} loopContext={loopContext} />
      {/each}
    </div>
  {:else if block.type === 'text'}
    <Text {block} {loopContext} />
  {:else if block.type === 'button'}
    <Button {block} {loopContext} />
  {/if}
{/if}
```

---

## 🎯 Essential Features

### 1. **forEach Loops** ✅

```huml
content::
  products::
    - :: name: "Laptop", price: 999
    - :: name: "Mouse", price: 29

screens::
  - ::
    blocks::
      - ::
        type: "section-container"
        forEach: "products"
        blocks::
          - :: type: "text", content: "{{item.name}}"
          - :: type: "text", content: "${{item.price}}"
```

**Implementation:**
```typescript
if (block.forEach) {
  const array = getStateValue(block.forEach);  // From content/state/userContent
  return array.map((item, index) => {
    const context = {item, index, first: index === 0, last: index === array.length - 1};
    return renderBlock(node, context);
  });
}
```

---

### 2. **Forms & Submissions** ✅

**Pattern 1: Real-time reactive**
```huml
state::
  email: ""

blocks::
  - ::
    type: "form-field-email"
    stateKey: "email"  # Updates state on keystroke
    label: "Email"
```

**Pattern 2: Traditional submission**
```huml
blocks::
  - ::
    type: "form"
    id: "contact"
    eventName: "contact_submission"

  - ::
    type: "form-field-text"
    formId: "contact"
    fieldName: "name"

  - ::
    type: "nav-button"
    formId: "contact"
    submit: true
```

**Implementation:**
```typescript
function submitForm(formId: string, data: Record<string, any>) {
  const submissions = submissionsDoc.getMap('submissions');
  const formSubmissions = submissions.get(formId) || new LoroList();

  formSubmissions.push([{
    id: crypto.randomUUID(),
    timestamp: Date.now(),
    viewer_id: currentUser.did,
    fields: data
  }]);

  if (!submissions.has(formId)) {
    submissions.set(formId, formSubmissions);
  }
}
```

---

### 3. **Comments/Threads** ✅

```huml
blocks::
  - ::
    type: "collaborative"
    name: "Discussion"
    title: "Comments"
    description: "Share your thoughts"
    mode: "markdown"
```

**Implementation:**
```typescript
function addComment(threadId: string, comment: {author: string, content: string}) {
  const threads = commentsDoc.getMap('threads');
  const threadComments = threads.get(threadId) || new LoroList();

  threadComments.push([{
    id: crypto.randomUUID(),
    author: comment.author,
    userId: currentUser.did,
    content: comment.content,
    timestamp: Date.now()
  }]);

  if (!threads.has(threadId)) {
    threads.set(threadId, threadComments);
  }
}
```

---

### 4. **CEL Expressions** ✅

```huml
# Conditional visibility
visible: "{{cart.items.length > 0}}"

# Dynamic content
content: "Welcome, {{firstName + ' ' + lastName}}"

# Computed values
content: "Total: ${{cart.items | sum('price') | toFixed(2)}}"

# Conditional CSS
css: "background: {{theme == 'dark' ? '#000' : '#fff'}}"

# Loop context
content: "{{index + 1}}. {{item.name}}"
```

**Implementation:**
```typescript
import { evaluate } from '@marcbachmann/cel-js';

function evaluateCEL(expression: string, context: Record<string, any>) {
  if (!expression || !expression.includes('{{')) {
    return expression;
  }

  // Extract {{ ... }} patterns
  return expression.replace(/\{\{(.+?)\}\}/g, (_, expr) => {
    return evaluate(expr, context);
  });
}
```

---

### 5. **Multi-Screen Navigation** ✅

```huml
screens::
  - ::
    id: "home"
    isEntryPoint: true
    blocks::
      - ::
        type: "nav-button"
        content: "Go to About"
        action: "navigate"
        targetContainerId: "about"

  - ::
    id: "about"
    blocks::
      - :: type: "text", content: "About page"
```

**Implementation:**
```typescript
const navigationStore = {
  currentScreenId: $state('home'),

  navigate(screenId: string) {
    this.currentScreenId = screenId;
  }
};

function handleButtonClick(block: Block) {
  if (block.action === 'navigate' && block.targetContainerId) {
    navigationStore.navigate(block.targetContainerId);
  }
}
```

---

### 6. **State Management** ✅

**Three state layers:**
```huml
# In-memory reactive state
state::
  currentStep: "welcome"
  searchQuery: ""

# Publisher content (shared across users)
content::
  products:: []
  settings:: {}

# Per-user state (isolated)
user_content::
  cartTotal: 0
  favorites:: []
```

**Priority:** `state` < `content` < `user_content` (rightmost wins)

**Implementation:**
```typescript
class StateManager {
  private state = $state({});           // In-memory
  private content: LoroMap;              // From contentDoc
  private userContent: LoroMap;          // From userContentDoc

  getValue(key: string) {
    // Priority: user_content > content > state
    if (this.userContent.has(key)) return this.userContent.get(key);
    if (this.content.has(key)) return this.content.get(key);
    return this.state[key];
  }

  setValue(key: string, value: any) {
    // Determine which layer to update
    if (this.userContent.has(key)) {
      this.userContent.set(key, value);
    } else if (this.content.has(key)) {
      this.content.set(key, value);  // Publisher action
    } else {
      this.state[key] = value;  // In-memory
    }
  }
}
```

---

## 🚀 Migration from Yjs to Loro

### Document Mapping

```typescript
// OLD (Yjs)
blocksuite_doc → Y.Map<string, Block>
content_doc → Y.Map
user_content_doc → Y.Map
thread_comments_doc → Y.Map
form_submissions_doc → Y.Map

// NEW (Loro)
templateDoc → LoroTree (blocks hierarchy)
contentDoc → LoroMap (content)
userContentDoc → LoroMap (user_content)
commentsDoc → LoroMap (threads)
submissionsDoc → LoroMap (submissions)
```

### Conversion Strategy

```typescript
function convertYjsToLoro(yjsDoc: Y.Doc, loroDoc: Loro) {
  const yjsBlocks = yjsDoc.getMap('blocks');
  const loroTree = loroDoc.getTree('blocks');

  // Build tree from flat map with parentId
  const nodeMap = new Map<string, LoroNode>();

  // First pass: create root nodes
  yjsBlocks.forEach((block, id) => {
    if (!block.parentId) {
      const node = loroTree.createNode();
      copyBlockProperties(node, block);
      nodeMap.set(id, node);
    }
  });

  // Second pass: create children recursively
  function createChildren(oldParentId: string, newParentNode: LoroNode) {
    yjsBlocks.forEach((block, id) => {
      if (block.parentId === oldParentId) {
        const node = loroTree.createNode(newParentNode.id);
        copyBlockProperties(node, block);
        nodeMap.set(id, node);
        createChildren(id, node);
      }
    });
  }

  nodeMap.forEach((node, id) => {
    const block = yjsBlocks.get(id);
    createChildren(id, node);
  });
}

function copyBlockProperties(node: LoroNode, block: Block) {
  // Copy all properties except id, parentId, x, y, width, height, zIndex
  const excludeKeys = ['id', 'parentId', 'x', 'y', 'width', 'height', 'zIndex', 'manualConnections', 'styles'];

  Object.entries(block).forEach(([key, value]) => {
    if (!excludeKeys.includes(key)) {
      node.data.set(key, value);
    }
  });
}
```

---

## 📁 New Folder Structure

```
src/
├─ builder/                    # Builder UI
│   ├─ TreeView.svelte
│   ├─ PropertiesPanel.svelte
│   ├─ BlockPalette.svelte
│   ├─ PreviewPane.svelte
│   └─ stores/
│       └─ templateStore.ts
│
├─ publisher/                  # Publisher UI
│   ├─ ActionButtons.svelte
│   ├─ AddProductForm.svelte
│   ├─ AddPostForm.svelte
│   ├─ ManageContent.svelte
│   └─ stores/
│       └─ contentStore.ts
│
├─ viewer/                     # Viewer UI
│   ├─ ScreenRenderer.svelte
│   ├─ BlockRenderer.svelte
│   ├─ Navigation.svelte
│   ├─ blocks/
│   │   ├─ Container.svelte
│   │   ├─ Text.svelte
│   │   ├─ Image.svelte
│   │   ├─ Input.svelte
│   │   ├─ Button.svelte
│   │   └─ Collaborative.svelte
│   └─ stores/
│       ├─ viewerState.ts
│       └─ navigationStore.ts
│
├─ shared/                     # Shared utilities
│   ├─ loro/
│   │   ├─ loroManager.ts
│   │   ├─ templateStructureStore.ts
│   │   ├─ contentStore.ts
│   │   ├─ collaborativeStore.ts
│   │   └─ submissionsStore.ts
│   ├─ cel/
│   │   └─ celEvaluator.ts
│   ├─ huml/
│   │   ├─ parser.ts
│   │   └─ importer.ts
│   └─ types/
│       ├─ block.types.ts
│       ├─ loro.types.ts
│       └─ huml.types.ts
│
└─ types/                      # Global types
    └─ index.ts
```

---

## 🎨 Rich Text Support

### Loro Rich Text

```typescript
// Create rich text
const post = contentDoc.getMap('content').get('blogPosts')[0];
const content = post.content;  // LoroText

// Rich text operations
content.insert(0, 'Hello ');
content.insert(6, 'World', {bold: true});
content.insert(11, '!');

// Render rich text
function renderRichText(loroText: LoroText) {
  const chunks = loroText.toDelta();
  return chunks.map(chunk => {
    if (chunk.attributes?.bold) {
      return <strong>{chunk.insert}</strong>;
    }
    return chunk.insert;
  });
}
```

**Block with rich text:**
```typescript
{
  type: 'text',
  mode: 'rich',
  content: LoroText  // Not a string, actual LoroText instance
}
```

---

## 🔑 Key Decisions

### 1. **Rendering Approach**

**Decision:** Template + Content Map (Option B)

**Why:**
- Template defines structure (Loro tree)
- Content provides data (Loro map)
- Viewer uses forEach to loop over content
- Clean separation of structure and data
- Publisher UI only edits content map (simple)

---

### 2. **Block Storage**

**Decision:** Minimal block properties in tree

**Why:**
- Loro manages tree structure (id, parent, children)
- Only store content properties in node.data
- No x, y, width, height (editor layout separate)
- Clean data model

---

### 3. **State Management**

**Decision:** Three-layer state (state, content, user_content)

**Why:**
- `state` - In-memory reactive (fast)
- `content` - Publisher data (shared, synced)
- `user_content` - Per-user (isolated, synced)
- Priority system (rightmost wins)
- Matches HUML spec

---

### 4. **Publisher UI Definition**

**Decision:** Defined in HUML template

**Why:**
- Template specifies what can be published
- Dynamic Publisher UI generation
- Type-safe publisher actions
- LLM can generate publisher UI spec

---

## 🚀 Implementation Priority

### Phase 1: Core Foundation
1. Loro integration (loroManager.ts)
2. Block types (block.types.ts)
3. Template structure store
4. Content store
5. CEL evaluator

### Phase 2: Viewer UI
1. Screen renderer
2. Block renderer
3. Block components (6 types)
4. Navigation
5. Forms & submissions
6. Comments

### Phase 3: Builder UI
1. Tree view
2. Properties panel
3. Block palette
4. Preview pane
5. HUML import/export

### Phase 4: Publisher UI
1. Publisher actions parser
2. Dynamic form generation
3. Content management
4. Rich text editor (LoroText)

### Phase 5: Polish
1. Error handling
2. Loading states
3. Optimizations
4. Testing
5. Documentation

---

## 🎯 Success Criteria

✅ All 6 block types working
✅ forEach loops rendering correctly
✅ Forms submitting to Loro
✅ Comments syncing in real-time
✅ CEL expressions evaluating
✅ Multi-screen navigation working
✅ State management functional
✅ Publisher UI generating from template
✅ Loro tree operations (create/update/delete/move)
✅ Rich text editing with LoroText

---

## 📖 Next Steps

1. **Review this document** - Confirm architecture
2. **Start Phase 1** - Loro integration
3. **Build incrementally** - Test each component
4. **Delete old code** - After migration complete

---

**Ready to build! 🚀**
