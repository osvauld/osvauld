# Block System Documentation

## Overview
The block system is a flexible, hierarchical component system that supports various content types, tree structures, forms, navigation, and collaborative features.

---

## Block Types

### 1. **Container Blocks** (Tree Structure)

#### `screen-container`
**Purpose:** Top-level screens with tree hierarchy
**Key Features:**
- Can contain child blocks (tree structure)
- Entry point marking for navigation
- Named screens for organization

**Properties:**
```typescript
{
  name: string;           // Screen name
  isEntryPoint: boolean;  // Mark as entry screen
  parentId: string;       // For hierarchy (usually null for screens)
  visible: boolean | string; // CEL expression support
  css: string;            // Custom styling
}
```

#### `section-container`
**Purpose:** Sub-sections within screens
**Key Features:**
- Can contain child blocks
- Modal support
- Tree structure organization

**Properties:**
```typescript
{
  name: string;      // Container name
  parentId: string;  // Parent block ID (required for tree)
  isModal: boolean;  // Display as modal overlay
  visible: boolean | string; // CEL expression support
  css: string;       // Custom styling + modal positioning
}
```

---

### 2. **Content Blocks**

#### `heading`
**Purpose:** Heading text
**Properties:**
```typescript
{
  content: string;     // Heading text (supports CEL)
  css: string;         // Custom styling (supports CEL)
  visible: boolean | string;
  parentId: string;    // Parent container
  forEach: string;     // Loop over array (optional)
  forEachAs: string;   // Loop variable name (default: 'item')
}
```

#### `text`
**Purpose:** Plain text paragraphs
**Properties:** Same as `heading`

#### `markdown-text`
**Purpose:** Rich markdown content
**Properties:**
```typescript
{
  content: string;     // Markdown text (supports CEL)
  css: string;         // Custom styling (supports CEL)
  visible: boolean | string;
  parentId: string;
}
```

#### `image`
**Purpose:** Image display
**Properties:**
```typescript
{
  content: string;     // Image URL/src (supports CEL)
  alt: string;         // Alt text
  src: string;         // Alternative src property
  css: string;         // Custom styling (supports CEL)
  visible: boolean | string;
  parentId: string;
}
```

#### `html`
**Purpose:** Custom HTML content
**Properties:**
```typescript
{
  content: string;     // HTML content (@html directive)
  css: string;         // Custom styling
  visible: boolean | string;
  parentId: string;
}
```

---

### 3. **Interactive Blocks**

#### `nav-button`
**Purpose:** Navigation and state management
**Key Features:**
- Navigate between screens
- Update template state (setState)
- Form submission (multi-mode)
- CEL expression evaluation

**Properties:**
```typescript
{
  content: string;           // Button label
  action: 'navigate' | 'setState' | 'submit';  // Action type
  css: string;               // Custom styling (supports CEL)
  visible: boolean | string;
  parentId: string;

  // Navigation
  targetContainerId: string; // Target screen/container ID
  targetScreenId: string;    // Alternative target property

  // State Management
  stateKey: string;          // Single state key to update
  stateValue: any;           // Value (supports CEL)
  stateUpdates: Record<string, any>; // Bulk state updates (supports CEL)

  // Form Integration (3 modes)
  formId: string;            // Associated form ID
  submit: boolean;           // TRUE = submit form
  fieldName: string;         // Field to cache (Mode 3A)
  value: any;                // Field value to cache (Mode 3A, supports CEL)

  // Loop support
  forEach: string;           // Array to loop over
  forEachAs: string;         // Loop variable name
}
```

**Action Modes:**
1. **navigate** - Just navigate to target
2. **setState** - Update template state, then optionally navigate
3. **Form modes:**
   - **Mode 1:** Navigate (implicit caching)
   - **Mode 2:** Submit form (fieldName + submit:true)
   - **Mode 3A:** Cache specific field value
   - **Mode 3B:** Implicit caching on navigation

---

### 4. **Form Blocks**

#### `form` (metadata)
**Purpose:** Form configuration (invisible)
**Properties:**
```typescript
{
  name: string;        // Form name
  eventName: string;   // Event name for submissions
  css: string;         // N/A (metadata only)
}
```

#### `form-field-*`
**Purpose:** Form input fields
**Types:** `form-field-text`, `form-field-email`, `form-field-number`, `form-field-textarea`, `form-field-checkbox`

**Properties:**
```typescript
{
  formId: string;          // Parent form ID
  fieldName: string;       // Field identifier
  label: string;           // Field label
  placeholder: string;     // Input placeholder
  required: boolean;       // Required validation
  defaultValue: any;       // Default value
  defaultChecked: boolean; // For checkbox
  stateKey: string;        // Bind to template state
  name: string;            // Alternative to stateKey
  css: string;             // Custom styling
  visible: boolean | string;
  parentId: string;
}
```

#### `form-submit`
**Purpose:** Submit button (deprecated - use nav-button with submit:true)

---

### 5. **Collaborative Blocks**

#### `thread`
**Purpose:** Discussion threads with comments
**Key Features:**
- Main post (markdown support)
- Comments system (collaborative YDoc)
- User identification
- Timestamp tracking

**Properties:**
```typescript
{
  content: string;       // Main post content
  name: string;          // Thread title
  description: string;   // Thread description
  mode: 'markdown' | 'text';  // Content mode
  title: string;         // Alternative to name
  css: string;           // Custom styling
  visible: boolean | string;
  parentId: string;
}
```

**Comment Structure:**
```typescript
{
  id: string;
  author: string;
  userId: string;
  content: string;
  timestamp: number;
}
```

---

### 6. **Logic Blocks**

#### `branching-question`
**Purpose:** Conditional branching logic
**Key Features:**
- Conditional navigation
- Value-based routing

**Properties:**
```typescript
{
  content: string;            // Question text
  branchingQuestionId: string; // Reference ID
  branchValue: any;           // Branch condition value
  css: string;
  visible: boolean | string;
  parentId: string;
}
```

---

## Universal Block Properties

All blocks support these core properties:

```typescript
{
  id: string;             // Unique block identifier
  type: string;           // Block type
  x: number;              // Canvas X position (editor mode)
  y: number;              // Canvas Y position (editor mode)
  width: number;          // Block width (editor mode)
  height: number;         // Block height (editor mode)
  zIndex: number;         // Stacking order
  content: string;        // Primary content (type-specific)
  styles: Record<string, any>;  // Legacy styles (use css instead)

  // Optional Universal Properties
  name?: string;                 // Display name
  css?: string;                  // Custom CSS (supports CEL)
  visible?: boolean | string;    // Visibility (supports CEL)
  parentId?: string;             // Tree hierarchy parent
  order?: number;                // Sibling order

  // Loop Support (forEach)
  forEach?: string;              // Array name in templateState
  forEachAs?: string;            // Variable name (default: 'item')

  // Manual Connections (flow diagrams)
  manualConnections?: Array<{
    id: string;
    target: string;
    color?: string;
    arrowSize?: number;
    style?: string;
  }>;
}
```

---

## CEL Expression Support

### What is CEL?
CEL (Common Expression Language) allows dynamic expressions in block properties.

### CEL Syntax
```
{{ expression }}
```

### Supported in:
- `content` - All content blocks
- `css` - All blocks
- `visible` - All blocks
- `stateValue` - nav-button state updates
- `stateUpdates` - nav-button bulk updates
- `value` - nav-button form values
- All properties when in `forEach` loop context

### Loop Context Variables:
```typescript
{
  item: any;        // Current loop item (also available as forEachAs name)
  index: number;    // Current index
  first: boolean;   // Is first item
  last: boolean;    // Is last item
}
```

### Examples:
```typescript
// Simple state access
content: "Hello {{ user.name }}"

// Conditional visibility
visible: "{{ cart.items.length > 0 }}"

// Loop context
content: "Item #{{ index + 1 }}: {{ item.title }}"
css: "color: {{ first ? 'red' : 'black' }}"

// State updates
stateValue: "{{ currentValue + 1 }}"
```

---

## Template State System

### Global Reactive State
The `templateState` system provides:
- Reactive state signals
- CEL evaluation context
- Form field binding
- Cross-block communication

### State Operations:
```typescript
// Read state
templateState.getValue(key)

// Set single value
setState(key, value)

// Bulk update
updateState({ key1: value1, key2: value2 })

// Watch for changes (automatic in CEL)
templateState.getVersion()
```

---

## Tree Hierarchy System

### Structure:
- **Screen containers** (top-level)
  - **Section containers** (mid-level)
    - **Content blocks** (leaves)
    - **Form fields** (leaves)
    - **Threads** (leaves)
    - **Nav buttons** (leaves)

### Properties:
- `parentId` - Links to parent block
- `order` - Sibling ordering
- `isEntryPoint` - Screen entry marker
- `name` - Display name in tree

### Tree Features:
- Collapse/expand nodes
- Visual guide lines
- Orphaned blocks detection
- Drag-and-drop (planned)

---

## Form System Architecture

### Form Modes:

**Mode 1: Single-Screen Form**
```
[FormField] [FormField] [NavButton(submit:true)]
```

**Mode 2: Multi-Screen Form (implicit caching)**
```
Screen 1: [FormField] [FormField] [NavButton(navigate)]
Screen 2: [FormField] [FormField] [NavButton(submit:true)]
```

**Mode 3A: Branching Forms (explicit field caching)**
```
Question: [NavButton(fieldName, value)] [NavButton(fieldName, value)]
↓
Screen A or Screen B
```

**Mode 3B: Conditional Forms**
```
[NavButton with visible="{{ state.condition }}"
           and stateUpdates]
```

### Form Validation:
- Required field checking
- Email format validation
- Checkbox required validation
- Visual error states

### Form Submission Flow:
1. Collect visible fields
2. Merge with cached values (multi-screen)
3. Validate required fields
4. Submit to `submissionsStore` (YDoc)
5. Sync to backend
6. Clear cache and fields
7. Navigate to target (optional)

---

## Collaborative System (Yjs)

### Documents:
1. **blocksuiteDoc** - Template structure (blocks, viewport)
2. **contentDoc** - Publisher content (template state)
3. **userContentDoc** - Per-user state
4. **commentsDoc** - Thread comments (collaborative)
5. **submissionsDoc** - Form submissions

### Stores:
- **TemplateStructureStore** - Manages blocks hierarchy
- **CollaborativeStore** - Manages comments/threads
- **SubmissionsStore** - Manages form submissions

### Sync:
- Real-time updates via Yjs
- Awareness for cursors/presence
- Document-specific routing
- Efficient delta updates

---

## Best Practices

### Naming:
- Use descriptive `name` properties
- Use semantic `fieldName` for forms
- Mark entry screens with `isEntryPoint`

### Hierarchy:
- Always set `parentId` for tree organization
- Use `order` for intentional sibling ordering
- Keep tree depth reasonable (3-4 levels max)

### CEL Expressions:
- Keep expressions simple and readable
- Use `stateUpdates` for bulk state changes
- Leverage loop context variables (`item`, `index`, `first`, `last`)

### Forms:
- Always set `formId` on all form components
- Use `fieldName` consistently
- Mark `required` fields explicitly
- Use `stateKey` for state-bound fields

### Performance:
- Use `visible` to conditionally render expensive blocks
- Leverage `forEach` for lists instead of duplicating blocks
- Keep modal overlays in higher `zIndex`

---

## Future Enhancements

- Rich text editor (considering Loro-CRDT)
- Real-time presence indicators
- Advanced tree operations (drag-drop reordering)
- Conditional rendering engine
- Animation system
- Template marketplace
