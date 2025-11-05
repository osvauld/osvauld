# Block Renderer Specification

**Complete Implementation Guide for Sthalam Block Renderer**

**Status**: Planning Phase
**Target**: Implement all block types from STHALAM_DSL.md

---

## Architecture Overview

```typescript
┌────────────────────────────────────────────────────────────┐
│                    BlockRenderer.svelte                    │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  Props:                                                    │
│    - block: Block (UI block definition)                   │
│    - context: Record<string, any> (CEL evaluation context)│
│    - onAction: (action, params) => void                   │
│    - onStateChange: (key, value) => void                  │
│                                                            │
│  Responsibilities:                                         │
│    1. Evaluate CEL expressions using OCaml/WASM evaluator  │
│    2. Render block based on type                          │
│    3. Handle control flow (when, if/then/else, forEach)   │
│    4. Dispatch actions (navigate, openModal, setState)     │
│    5. Recursive rendering for nested blocks               │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

---

## Block Types to Implement

### 1. Layout Blocks

#### `screen` (**CRITICAL - Top Priority**)
```huml
- type: screen
  name: home
  css: "max-width: 1200px; margin: 0 auto;"
  blocks::
    # ... child blocks
```

**Implementation**:
```svelte
{#if block.type === 'screen'}
  <div
    class="sthalam-screen"
    data-screen={block.name}
    style={evaluateCSS(block.css)}
  >
    {#if block.blocks}
      {#each block.blocks as childBlock}
        <svelte:self {childBlock} {context} {onAction} {onStateChange} />
      {/each}
    {/if}
  </div>
{/if}
```

---

#### `container`
```huml
- type: container
  layout: flex              # flex | grid | block
  direction: column         # row | column (for flex)
  gap: 1rem
  alignItems: center        # flex alignment
  justifyContent: space-between
  columns: 3                # for grid
  css: "padding: 1rem;"
  blocks::
    # ... child blocks
```

**Implementation**:
```svelte
{:else if block.type === 'container'}
  {@const layout = block.layout || 'block'}
  {@const direction = block.direction || 'row'}
  {@const gap = block.gap || '0'}
  {@const cssStyles = buildContainerStyles(layout, direction, gap, block)}

  <div
    class="sthalam-container"
    style={cssStyles}
  >
    {#if block.blocks}
      {#each block.blocks as childBlock}
        <svelte:self {childBlock} {context} {onAction} {onStateChange} />
      {/each}
    {/if}
  </div>
{/if}
```

---

#### `section`
```huml
- type: section
  css: "padding: 2rem; background: white;"
  blocks::
    # ... child blocks
```

**Implementation**:
```svelte
{:else if block.type === 'section'}
  <section style={evaluateCSS(block.css)}>
    {#if block.blocks}
      {#each block.blocks as childBlock}
        <svelte:self {childBlock} {context} {onAction} {onStateChange} />
      {/each}
    {/if}
  </section>
{/if}
```

---

### 2. Content Blocks

#### `text`
```huml
- type: text
  content: "This is a paragraph."
  css: "color: #333;"
```

**Implementation**:
```svelte
{:else if block.type === 'text'}
  <p style={evaluateCSS(block.css)}>
    {@html evaluateExpression(block.content, context)}
  </p>
{/if}
```

---

#### `heading`
```huml
- type: heading
  level: 1                  # 1-6 (default: 1)
  content: "Page Title"
  css: "font-size: 2rem;"
```

**Implementation**:
```svelte
{:else if block.type === 'heading'}
  {@const level = block.level || 1}
  {@const Tag = `h${level}`}

  <svelte:element this={Tag} style={evaluateCSS(block.css)}>
    {@html evaluateExpression(block.content, context)}
  </svelte:element>
{/if}
```

---

#### `label`
```huml
- type: label
  for: email-input
  content: "Email Address"
  css: "font-weight: 500;"
```

---

#### `image`
```huml
- type: image
  src: "{{ post.imageUrl }}"
  alt: "{{ post.title }}"
  css: "max-width: 100%;"
```

---

#### `video`
```huml
- type: video
  src: "{{ post.videoUrl }}"
  controls: true
  css: "width: 100%;"
```

---

### 3. Input Blocks

#### `input`
```huml
- type: input
  name: email
  value: "{{ userEmail }}"
  placeholder: "Enter your email"
  validate: "value.contains('@')"
  error: "Invalid email"
  css: "padding: 0.5rem;"
  onChange: updateEmail
```

---

#### `textarea`
```huml
- type: textarea
  name: body
  value: "{{ draftBody }}"
  placeholder: "Write your post..."
  rows: 10
  css: "width: 100%;"
  onChange: updateBody
```

---

#### `checkbox`
```huml
- type: checkbox
  name: subscribe
  checked: "{{ isSubscribed }}"
  label: "Subscribe to newsletter"
  onChange: toggleSubscribe
```

---

#### `select`
```huml
- type: select
  name: category
  value: "{{ selectedCategory }}"
  options::
    - value: "tech"
      label: "Technology"
    - value: "design"
      label: "Design"
  onChange: updateCategory
```

---

#### `radio`
```huml
- type: radio
  name: view
  value: "{{ currentView }}"
  options::
    - value: "grid"
      label: "Grid View"
    - value: "list"
      label: "List View"
  onChange: updateView
```

---

### 4. Action Blocks

#### `button`
```huml
- type: button
  content: "Publish Post"
  action: publishPost
  disabled: "!canPublish"
  css: "padding: 0.75rem 1.5rem;"
  onClick: handlePublish
```

**Implementation**:
```svelte
{:else if block.type === 'button'}
  {@const isDisabled = block.disabled ? evaluateExpression(block.disabled, context) : false}

  <button
    type={block.submitForm ? 'submit' : 'button'}
    disabled={isDisabled}
    style={evaluateCSS(block.css)}
    onclick={() => handleButtonClick(block)}
  >
    {@html evaluateExpression(block.content, context)}
  </button>
{/if}
```

---

#### `link`
```huml
- type: link
  content: "View Profile"
  href: "/profile/{{ user.id }}"
  action: navigate                    # Optional: use navigate action
  params::                            # Optional: for SPA navigation
    screen: profile
    userId: "{{ user.id }}"
  css: "color: #0066cc;"
```

---

#### `form`
```huml
- type: form
  name: publish-form
  blocks::
    - type: input
      name: title
    - type: button
      content: "Submit"
      type: submit
  onSubmit: handleSubmit
```

---

### 5. Special Blocks

#### `canvas`
```huml
- type: canvas
  width: 400
  height: 400
  render: animatedPattern     # References a render function
  css: "border: 1px solid #ddd;"
```

---

#### `modal` (**CRITICAL - High Priority**)
```huml
- type: modal
  name: confirm-action
  visible: "{{ showModal }}"
  size: medium               # small | medium | large | fullscreen
  closable: true            # Show close button
  blocks::
    # Modal content
```

**Implementation**:
```svelte
{:else if block.type === 'modal'}
  {@const isVisible = block.visible ? evaluateExpression(block.visible, context) : false}
  {@const size = block.size || 'medium'}
  {@const closable = block.closable !== false}

  {#if isVisible}
    <div class="sthalam-modal-overlay" onclick={() => closable && closeModal(block.name)}>
      <div
        class="sthalam-modal sthalam-modal-{size}"
        onclick={(e) => e.stopPropagation()}
      >
        {#if closable}
          <button
            class="sthalam-modal-close"
            onclick={() => closeModal(block.name)}
          >×</button>
        {/if}

        <div class="sthalam-modal-content">
          {#if block.blocks}
            {#each block.blocks as childBlock}
              <svelte:self {childBlock} {context} {onAction} {onStateChange} />
            {/each}
          {/if}
        </div>
      </div>
    </div>
  {/if}
{/if}
```

---

## Control Flow

### `when` (Conditional Visibility)
```huml
- type: text
  content: "Loading..."
  when: isLoading
```

**Implementation**:
```svelte
<!-- Wrap every block with visibility check -->
{@const isVisible = checkVisibility(block, context)}

{#if isVisible}
  <!-- Render block normally -->
{/if}
```

---

### `if/then/else` (Branching)
```huml
- if: "posts.size() > 0"
  then::
    - type: text
      content: "{{ posts.size() }} posts"
  else::
    - type: text
      content: "No posts yet"
```

**Implementation**:
```svelte
{#if block.if}
  {@const condition = evaluateExpression(block.if, context)}

  {#if condition}
    {#if block.then}
      {#each block.then as childBlock}
        <svelte:self {childBlock} {context} {onAction} {onStateChange} />
      {/each}
    {/if}
  {:else}
    {#if block.else}
      {#each block.else as childBlock}
        <svelte:self {childBlock} {context} {onAction} {onStateChange} />
      {/each}
    {/if}
  {/if}
{/if}
```

---

### `match/cases` (Pattern Matching)
```huml
- match: currentView
  cases::
    - value: "grid"
      blocks::
        # Grid layout
    - value: "list"
      blocks::
        # List layout
    - default: true
      blocks::
        # Default layout
```

---

### `forEach/as` (Iteration)
```huml
- type: container
  forEach: posts
  as: post
  key: post.id
  blocks::
    - type: heading
      content: "{{ post.title }}"
```

**Implementation**:
```svelte
{#if block.forEach}
  {@const items = evaluateExpression(block.forEach, context)}
  {@const itemName = block.as || 'item'}

  {#each items as item, index (block.key ? item[block.key] : index)}
    {@const loopContext = {...context, [itemName]: item, [`${itemName}Index`]: index}}

    <!-- Render block with loop context -->
    {#if block.blocks}
      {#each block.blocks as childBlock}
        <svelte:self block={childBlock} context={loopContext} {onAction} {onStateChange} />
      {/each}
    {/if}
  {/each}
{/if}
```

---

## Action System

### Built-in Actions

1. **`navigate`** - Navigate to screen
   ```typescript
   function handleNavigate(params: { screen: string, [key: string]: any }) {
     // Update active screen
     // Store route params in context
   }
   ```

2. **`openModal`** - Open modal dialog
   ```typescript
   function handleOpenModal(params: { modal: string, data?: any }) {
     // Set modal visibility state
     // Store modal data in context
   }
   ```

3. **`closeModal`** - Close modal
   ```typescript
   function handleCloseModal(params: { modal: string }) {
     // Clear modal visibility state
   }
   ```

4. **`setState`** - Update state field
   ```typescript
   function handleSetState(params: { field: string, value: any }) {
     // Update Loro CRDT
     // Trigger reactivity
   }
   ```

### Custom Actions

Template authors define custom actions that are handled by the runtime:
```huml
- type: button
  content: "Publish"
  action: publishPost
  params::
    postId: "{{ post.id }}"
```

**Implementation**:
```typescript
function handleAction(action: string, params: any) {
  // Check built-in actions first
  if (action === 'navigate') return handleNavigate(params);
  if (action === 'openModal') return handleOpenModal(params);
  if (action === 'closeModal') return handleCloseModal(params);
  if (action === 'setState') return handleSetState(params);

  // Otherwise, dispatch to custom action handler
  onAction?.(action, params);
}
```

---

## CEL Integration

All expressions use the CEL evaluator we built:

```typescript
import { evaluateExpression } from '$lib/celEvaluator';

// Evaluate CEL expression with context
function evalCEL(expr: string, context: Record<string, any>): any {
  return evaluateExpression(expr, context);
}

// Evaluate content with {{ }} interpolation
function evaluateContent(content: string, context: Record<string, any>): string {
  return content.replace(/\{\{([^}]+)\}\}/g, (match, expr) => {
    return String(evalCEL(expr.trim(), context));
  });
}
```

---

## Implementation Priority

### **Phase 1: Core Infrastructure** (Week 1)
1. ✅ CEL evaluator integration
2. ⬜ Context management (state + computed values)
3. ⬜ Action dispatcher system
4. ⬜ Basic block rendering structure

### **Phase 2: Critical Features** (Week 2)
5. ⬜ `screen` block + navigation system
6. ⬜ `modal` block + modal actions
7. ⬜ `container` with layout options
8. ⬜ Control flow: `when`, `if/then/else`, `forEach`

### **Phase 3: UI Blocks** (Week 3)
9. ⬜ Content blocks: `text`, `heading`, `label`, `image`, `video`
10. ⬜ Input blocks: `input`, `textarea`, `checkbox`, `select`, `radio`
11. ⬜ Action blocks: `button`, `link`, `form`

### **Phase 4: Advanced Features** (Week 4)
12. ⬜ `match/cases` pattern matching
13. ⬜ `canvas` block
14. ⬜ Input validation
15. ⬜ List reconciliation with `key`
16. ⬜ Pagination with `limit`/`offset`

---

## Testing Strategy

Create test templates for each phase:

1. **phase1_test.huml** - Context and actions
2. **phase2_test.huml** - Navigation and modals
3. **phase3_test.huml** - All UI blocks
4. **phase4_test.huml** - Advanced features

---

## Next Steps

**Ready to implement?** Let me know which phase you'd like to start with!

Recommended: **Phase 1** - Set up the foundation (CEL integration, context, actions) before building blocks.
