# Block Types Refactor - Essential Types Analysis

## Current State (Too Many Types)

### Container Types (2)
- `screen-container` - Top-level screens
- `section-container` - Sub-sections
**Problem:** Distinction is arbitrary. Both are just containers.

### Content Types (5)
- `heading` - Headings
- `text` - Paragraphs
- `markdown-text` - Markdown
- `image` - Images
- `html` - Custom HTML
**Problem:** heading/text/markdown-text could be one type

### Form Types (7+)
- `form` - Metadata
- `form-field-text`
- `form-field-email`
- `form-field-number`
- `form-field-textarea`
- `form-field-checkbox`
- `form-submit`
**Problem:** Too granular. Should be generic `input` with `type` property

### Action Types (2)
- `nav-button` - Navigation/actions
- `branching-question` - Logic
**Problem:** branching-question is rarely used, could be handled with conditional nav-buttons

### Collaboration Types (1)
- `thread` - Comments/discussions
**Status:** Good, unique feature

---

## Core Use Cases

1. **Display content** (text, images, markdown)
2. **Organize layout** (containers, hierarchy)
3. **Collect input** (forms, fields)
4. **Trigger actions** (navigation, state updates, submission)
5. **Collaborate** (comments, threads)
6. **Conditional logic** (visibility, branching)

---

## Proposed Minimal Set (6 Types)

### 1. `container`
**Replaces:** screen-container, section-container

```typescript
{
  type: 'container',
  name: string,           // Display name
  parentId: string,       // Tree hierarchy
  isEntryPoint: boolean,  // For top-level screens
  isModal: boolean,       // Modal overlay
  visible: string,        // CEL expression
  css: string
}
```

**Properties determine behavior:**
- `isEntryPoint: true` → Acts as screen-container
- `isModal: true` → Acts as modal
- Default → Acts as section-container

**Benefits:**
- Single container concept
- Less mental overhead
- Properties define behavior

---

### 2. `text`
**Replaces:** heading, text, markdown-text, html

```typescript
{
  type: 'text',
  content: string,        // Text content or HTML
  mode: 'plain' | 'markdown' | 'html',  // Rendering mode
  level: 1 | 2 | 3 | 4 | 5 | 6,  // For headings (optional)
  visible: string,
  css: string
}
```

**Examples:**
```typescript
// Heading
{ type: 'text', content: 'Title', level: 1 }

// Paragraph
{ type: 'text', content: 'Body text', mode: 'plain' }

// Markdown
{ type: 'text', content: '# Title\n**bold**', mode: 'markdown' }

// HTML
{ type: 'text', content: '<div>Custom</div>', mode: 'html' }
```

**Benefits:**
- One type for all text
- Mode property determines rendering
- Simpler to understand

---

### 3. `image`
**Status:** Keep as-is

```typescript
{
  type: 'image',
  src: string,           // Image URL (supports CEL)
  alt: string,           // Alt text
  visible: string,
  css: string
}
```

**Rationale:** Images are distinct from text, keep separate.

---

### 4. `input`
**Replaces:** form-field-text, form-field-email, form-field-number, form-field-textarea, form-field-checkbox

```typescript
{
  type: 'input',
  inputType: 'text' | 'email' | 'number' | 'textarea' | 'checkbox' | 'select' | 'radio',
  formId: string,        // Parent form
  fieldName: string,     // Field identifier
  label: string,         // Field label
  placeholder: string,
  required: boolean,
  defaultValue: any,
  stateKey: string,      // Bind to template state
  options: Array<{label: string, value: any}>,  // For select/radio
  visible: string,
  css: string
}
```

**Benefits:**
- One type for all inputs
- `inputType` determines rendering
- Matches HTML input concept
- Easier to extend (add date, file, etc.)

---

### 5. `button`
**Replaces:** nav-button, form-submit, branching-question

```typescript
{
  type: 'button',
  content: string,           // Button label
  action: 'navigate' | 'setState' | 'submit',
  visible: string,
  css: string,

  // Navigation
  targetContainerId: string,

  // State Management
  stateKey: string,
  stateValue: any,
  stateUpdates: Record<string, any>,

  // Form Integration
  formId: string,
  submit: boolean,
  fieldName: string,
  value: any
}
```

**Benefits:**
- Semantic naming (button, not nav-button)
- `action` property determines behavior
- Handles all button use cases

---

### 6. `thread`
**Status:** Keep as-is

```typescript
{
  type: 'thread',
  content: string,       // Main post
  name: string,          // Thread title
  description: string,
  mode: 'markdown' | 'text',
  visible: string,
  css: string
}
```

**Rationale:** Unique collaborative feature, keep separate.

---

## Eliminated Types

### ❌ `form` (metadata)
**Why remove:** Unnecessary. Forms are just collections of inputs with the same `formId`.

**Migration:**
```typescript
// OLD: Need separate form block
{ type: 'form', id: 'form1', eventName: 'signup' }
{ type: 'form-field-text', formId: 'form1', ... }

// NEW: Inputs share formId, eventName on submit button
{ type: 'input', formId: 'form1', ... }
{ type: 'button', formId: 'form1', submit: true, eventName: 'signup' }
```

### ❌ `branching-question`
**Why remove:** Can be handled with conditional buttons.

**Migration:**
```typescript
// OLD: Separate branching-question block
{ type: 'branching-question', question: 'Continue?', yesTarget: 'A', noTarget: 'B' }

// NEW: Two buttons with labels
{ type: 'button', content: 'Yes', action: 'navigate', targetContainerId: 'A' }
{ type: 'button', content: 'No', action: 'navigate', targetContainerId: 'B' }

// OR: Conditional visibility with state
{ type: 'button', content: 'Yes', visible: '{{ choice == "yes" }}', ... }
```

### ❌ `form-submit`
**Why remove:** Just a button with `submit: true`.

---

## Summary: 6 Essential Types

| Type | Purpose | Replaces |
|------|---------|----------|
| `container` | Layout & hierarchy | screen-container, section-container |
| `text` | All text content | heading, text, markdown-text, html |
| `image` | Images | (unchanged) |
| `input` | Form inputs | form-field-text, form-field-email, form-field-number, form-field-textarea, form-field-checkbox |
| `button` | Actions & navigation | nav-button, form-submit, branching-question |
| `thread` | Collaboration | (unchanged) |

---

## Migration Strategy

### Phase 1: Add New Types (Parallel Support)
```typescript
type BlockType =
  // New types
  | 'container' | 'text' | 'image' | 'input' | 'button' | 'thread'
  // Legacy types (deprecated)
  | 'screen-container' | 'section-container' | 'heading' | 'nav-button' | ...
```

### Phase 2: Update Components
- Add mode/inputType/action support
- Keep backward compatibility

### Phase 3: Migration Script
```typescript
function migrateBlock(oldBlock: Block): Block {
  switch(oldBlock.type) {
    case 'screen-container':
      return { ...oldBlock, type: 'container', isEntryPoint: true };
    case 'section-container':
      return { ...oldBlock, type: 'container' };
    case 'heading':
      return { ...oldBlock, type: 'text', level: 1 };
    case 'nav-button':
      return { ...oldBlock, type: 'button' };
    // ...
  }
}
```

### Phase 4: Remove Legacy Types

---

## Proposed Block Interface

```typescript
interface Block {
  // Core (all blocks)
  id: string;
  type: 'container' | 'text' | 'image' | 'input' | 'button' | 'thread';
  x: number;
  y: number;
  width: number;
  height: number;
  zIndex: number;
  parentId?: string;
  name?: string;
  visible?: string | boolean;  // CEL expression
  css?: string;
  order?: number;

  // Loop support
  forEach?: string;
  forEachAs?: string;

  // Type-specific properties (use discriminated unions for type safety)

  // container
  isEntryPoint?: boolean;
  isModal?: boolean;

  // text
  content?: string;
  mode?: 'plain' | 'markdown' | 'html';
  level?: 1 | 2 | 3 | 4 | 5 | 6;

  // image
  src?: string;
  alt?: string;

  // input
  inputType?: 'text' | 'email' | 'number' | 'textarea' | 'checkbox' | 'select' | 'radio';
  formId?: string;
  fieldName?: string;
  label?: string;
  placeholder?: string;
  required?: boolean;
  defaultValue?: any;
  stateKey?: string;
  options?: Array<{label: string, value: any}>;

  // button
  action?: 'navigate' | 'setState' | 'submit';
  targetContainerId?: string;
  stateKey?: string;
  stateValue?: any;
  stateUpdates?: Record<string, any>;
  submit?: boolean;
  fieldName?: string;
  value?: any;
  eventName?: string;  // For form submissions

  // thread
  description?: string;
  title?: string;

  // Manual connections (editor feature)
  manualConnections?: Array<{
    id: string;
    target: string;
    color?: string;
    arrowSize?: number;
    style?: string;
  }>;

  // Legacy (deprecated)
  styles?: Record<string, any>;
}
```

**Better: Use Discriminated Unions for Type Safety**

```typescript
type Block = ContainerBlock | TextBlock | ImageBlock | InputBlock | ButtonBlock | ThreadBlock;

interface BaseBlock {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  zIndex: number;
  parentId?: string;
  name?: string;
  visible?: string | boolean;
  css?: string;
  order?: number;
  forEach?: string;
  forEachAs?: string;
}

interface ContainerBlock extends BaseBlock {
  type: 'container';
  isEntryPoint?: boolean;
  isModal?: boolean;
}

interface TextBlock extends BaseBlock {
  type: 'text';
  content: string;
  mode?: 'plain' | 'markdown' | 'html';
  level?: 1 | 2 | 3 | 4 | 5 | 6;
}

interface ImageBlock extends BaseBlock {
  type: 'image';
  src: string;
  alt?: string;
}

interface InputBlock extends BaseBlock {
  type: 'input';
  inputType: 'text' | 'email' | 'number' | 'textarea' | 'checkbox' | 'select' | 'radio';
  formId: string;
  fieldName: string;
  label: string;
  placeholder?: string;
  required?: boolean;
  defaultValue?: any;
  stateKey?: string;
  options?: Array<{label: string, value: any}>;
}

interface ButtonBlock extends BaseBlock {
  type: 'button';
  content: string;
  action: 'navigate' | 'setState' | 'submit';
  targetContainerId?: string;
  stateKey?: string;
  stateValue?: any;
  stateUpdates?: Record<string, any>;
  formId?: string;
  submit?: boolean;
  fieldName?: string;
  value?: any;
  eventName?: string;
}

interface ThreadBlock extends BaseBlock {
  type: 'thread';
  content: string;
  title?: string;
  description?: string;
  mode?: 'markdown' | 'text';
}
```

---

## Benefits of This Refactor

### 1. **Simpler Mental Model**
- 6 types vs 15+ types
- Clear categorization
- Easier to learn

### 2. **Better Extensibility**
```typescript
// Easy to add new input types
inputType: 'date' | 'file' | 'color' | ...

// Easy to add new text modes
mode: 'plain' | 'markdown' | 'html' | 'latex' | ...

// Easy to add new actions
action: 'navigate' | 'setState' | 'submit' | 'fetch' | ...
```

### 3. **Closer to Web Primitives**
- `container` → `<div>`
- `text` → `<p>`, `<h1>`
- `image` → `<img>`
- `input` → `<input>`, `<textarea>`
- `button` → `<button>`
- `thread` → (custom)

### 4. **Type Safety**
Discriminated unions give perfect TypeScript type narrowing:
```typescript
if (block.type === 'input') {
  // TypeScript knows: block.inputType exists
  // TypeScript knows: block.content doesn't exist
}
```

### 5. **Easier HUML Templates**
```huml
# Old (too verbose)
block: form-field-text
  formId: signup
  fieldName: email
  label: Email

# New (cleaner)
block: input
  type: email
  formId: signup
  name: email
  label: Email
```

---

## Recommendation

**Go with 6 essential types:**
1. container
2. text
3. image
4. input
5. button
6. thread

**Use discriminated unions for type safety.**

**Migrate gradually with backward compatibility.**

This gives you a **minimal, extensible, type-safe** block system that's easier to understand and maintain.

What do you think? Should we proceed with this refactor?
