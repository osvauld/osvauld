# New BlockRenderer Design: Fresh Start

**Complete redesign plan with rationale**

---

## 🔴 Problems with Current BlockRenderer (496 lines)

### 1. **Tightly Coupled to Specific Use Cases**
```typescript
// Current: Hardcoded canvas/WebGL logic in the renderer
let webglContexts = new Map<string, {...}>();
async function renderCanvas(canvas: HTMLCanvasElement, blockId: string) {
  const gridSize = context.gridSize || 40;  // Hardcoded assumptions
  const pattern = context.selectedPattern || 'waves';  // Specific to one demo
}
```
**Problem**: Canvas logic should be a separate component, not mixed into the renderer.

---

### 2. **Hardcoded Action Logic**
```typescript
// Current: Specific actions hardcoded
if (action === 'deleteComment') {
  params.commentId = evaluatedId;
} else {
  params.postId = evaluatedId;  // Assumes all actions need postId!
}
```
**Problem**: Not extensible. Each new action type requires code changes.

---

### 3. **Missing Core HUML Features**
- ❌ No `screen` block → Can't build multi-screen apps
- ❌ No `modal` block → Can't show dialogs
- ❌ No `container` with layout options → Limited layouts
- ❌ No `if/then/else` → No branching logic
- ❌ No `match/cases` → No pattern matching
- ❌ No `when` → Visibility based on `visible` property only
- ❌ Missing 15+ block types from spec

---

### 4. **No Separation of Concerns**
Everything in one 496-line file:
- Block rendering logic
- Canvas-specific WebGL code
- Form handling
- Action dispatching
- Expression evaluation

**Problem**: Hard to maintain, test, and extend.

---

### 5. **Incomplete Control Flow**
```typescript
// Current: Only supports forEach with loops
{#if block.loop}
  {#each evaluateValue(block.loop.items) as item, idx}
    <!-- render -->
  {/each}
{/if}
```
**Problem**: Missing `if/then/else`, `match/cases`, `when` conditions.

---

### 6. **No Type Safety**
```typescript
interface Props {
  block: any;  // No type safety!
  context: Record<string, any>;  // No type safety!
}
```
**Problem**: Easy to make mistakes, no IDE autocomplete.

---

## ✅ New BlockRenderer: What's Different

### **Architecture: Modular Design**

```
New Structure:
┌────────────────────────────────────────────────────────┐
│  BlockRenderer.svelte (Core Orchestrator - ~200 lines) │
├────────────────────────────────────────────────────────┤
│                                                        │
│  Responsibilities:                                     │
│    - Control flow (when, if/then/else, match, forEach)│
│    - Block type dispatch                              │
│    - CEL expression evaluation                        │
│    - Recursive rendering                              │
│                                                        │
└────────────────────────────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
          ▼               ▼               ▼
┌─────────────────┐ ┌─────────────┐ ┌─────────────┐
│ Layout Blocks   │ │Input Blocks │ │Special Blocks│
├─────────────────┤ ├─────────────┤ ├─────────────┤
│ - Screen        │ │ - Input     │ │ - Canvas    │
│ - Container     │ │ - Textarea  │ │ - Modal     │
│ - Section       │ │ - Checkbox  │ │ - Custom    │
└─────────────────┘ │ - Select    │ └─────────────┘
                    │ - Radio     │
                    └─────────────┘

Shared Services:
┌────────────────────────────────────────────────────────┐
│  services/                                             │
├────────────────────────────────────────────────────────┤
│  - celEvaluator.ts     (CEL expression evaluation)     │
│  - actionDispatcher.ts (Handle all actions)            │
│  - navigationService.ts (Screen navigation)            │
│  - modalService.ts     (Modal state management)        │
│  - stateManager.ts     (Loro CRDT integration)         │
└────────────────────────────────────────────────────────┘
```

---

## 🎯 Key Improvements

### **1. Complete Type Safety**

**Old**:
```typescript
interface Props {
  block: any;  // No type checking
}
```

**New**:
```typescript
// types/huml.ts
export type BlockType =
  | 'screen' | 'container' | 'section'
  | 'text' | 'heading' | 'label' | 'image' | 'video'
  | 'input' | 'textarea' | 'checkbox' | 'select' | 'radio'
  | 'button' | 'link' | 'form'
  | 'canvas' | 'modal';

export interface BaseBlock {
  type: BlockType;
  id?: string;
  css?: string;
  when?: string;  // Conditional visibility
  blocks?: Block[];  // Child blocks
}

export interface ScreenBlock extends BaseBlock {
  type: 'screen';
  name: string;
  blocks: Block[];
}

export interface ContainerBlock extends BaseBlock {
  type: 'container';
  layout?: 'flex' | 'grid' | 'block';
  direction?: 'row' | 'column';
  gap?: string;
  alignItems?: string;
  justifyContent?: string;
  columns?: number;
  forEach?: string;  // Loop iteration
  as?: string;       // Loop variable name
  key?: string;      // Reconciliation key
}

export interface ButtonBlock extends BaseBlock {
  type: 'button';
  content: string;
  action?: string;
  params?: Record<string, any>;
  disabled?: string | boolean;
  onClick?: string;
}

// ... 20+ more typed interfaces

export type Block =
  | ScreenBlock
  | ContainerBlock
  | TextBlock
  | ButtonBlock
  | ... ;

// Props with full type safety
interface Props {
  block: Block;
  context: Context;
  onAction: ActionHandler;
  onStateChange: StateChangeHandler;
}
```

**Why**: Catch errors at compile time, better IDE support, self-documenting code.

---

### **2. Complete Control Flow**

**Old**: Only `forEach` loops
```typescript
{#if block.loop}
  {#each evaluateValue(block.loop.items) as item}
    <!-- render -->
  {/each}
{/if}
```

**New**: Full control flow
```svelte
<script lang="ts">
  // Control flow handler
  function renderBlock(block: Block, context: Context) {
    // 1. Check visibility (when condition)
    if (block.when && !evaluateCEL(block.when, context)) {
      return null;
    }

    // 2. Handle if/then/else
    if ('if' in block) {
      const condition = evaluateCEL(block.if, context);
      return condition ? renderBlocks(block.then) : renderBlocks(block.else);
    }

    // 3. Handle match/cases
    if ('match' in block) {
      const value = evaluateCEL(block.match, context);
      const matchedCase = block.cases.find(c =>
        c.value === value || c.default
      );
      return matchedCase ? renderBlocks(matchedCase.blocks) : null;
    }

    // 4. Handle forEach loops
    if (block.forEach) {
      const items = evaluateCEL(block.forEach, context);
      return items.map(item => renderWithLoopContext(block, item));
    }

    // 5. Render normal block
    return renderBlockByType(block);
  }
</script>

<!-- Template -->
{#if block.when && !evaluateCEL(block.when, context)}
  <!-- Hidden -->
{:else if block.if}
  <!-- If/Then/Else -->
  {@const condition = evaluateCEL(block.if, context)}
  {#if condition}
    {#each block.then as childBlock}
      <svelte:self block={childBlock} {context} {onAction} {onStateChange} />
    {/each}
  {:else if block.else}
    {#each block.else as childBlock}
      <svelte:self block={childBlock} {context} {onAction} {onStateChange} />
    {/each}
  {/if}
{:else if block.match}
  <!-- Match/Cases -->
  {@const value = evaluateCEL(block.match, context)}
  {@const matchedCase = block.cases.find(c => c.value === value || c.default)}
  {#if matchedCase?.blocks}
    {#each matchedCase.blocks as childBlock}
      <svelte:self block={childBlock} {context} {onAction} {onStateChange} />
    {/each}
  {/if}
{:else if block.forEach}
  <!-- ForEach Loop -->
  {@const items = evaluateCEL(block.forEach, context)}
  {@const itemName = block.as || 'item'}
  {#each items as item, index (block.key ? item[block.key] : index)}
    {@const loopContext = {...context, [itemName]: item}}
    <!-- Render block with item context -->
    {#if block.blocks}
      {#each block.blocks as childBlock}
        <svelte:self block={childBlock} context={loopContext} {onAction} {onStateChange} />
      {/each}
    {/if}
  {/each}
{:else}
  <!-- Normal block rendering -->
  {@const BlockComponent = getBlockComponent(block.type)}
  <BlockComponent {block} {context} {onAction} {onStateChange} />
{/if}
```

**Why**: Supports all HUML control flow keywords, enabling complex UIs.

---

### **3. Extensible Action System**

**Old**: Hardcoded action logic
```typescript
if (action === 'deleteComment') {
  params.commentId = evaluatedId;
} else {
  params.postId = evaluatedId;
}
```

**New**: Generic action dispatcher
```typescript
// services/actionDispatcher.ts
export class ActionDispatcher {
  private handlers = new Map<string, ActionHandler>();

  // Register built-in actions
  constructor(
    private navigation: NavigationService,
    private modals: ModalService,
    private state: StateManager
  ) {
    this.registerBuiltInActions();
  }

  private registerBuiltInActions() {
    // Navigate to screen
    this.register('navigate', (params) => {
      this.navigation.navigateTo(params.screen, params);
    });

    // Open modal
    this.register('openModal', (params) => {
      this.modals.open(params.modal, params.data);
    });

    // Close modal
    this.register('closeModal', (params) => {
      this.modals.close(params.modal);
    });

    // Set state
    this.register('setState', (params) => {
      this.state.set(params.field, params.value);
    });
  }

  // Register custom action
  register(name: string, handler: ActionHandler) {
    this.handlers.set(name, handler);
  }

  // Dispatch action
  async dispatch(action: string, params: any, context: Context) {
    // Evaluate params (may contain CEL expressions)
    const evaluatedParams = this.evaluateParams(params, context);

    // Get handler
    const handler = this.handlers.get(action);
    if (!handler) {
      throw new Error(`Unknown action: ${action}`);
    }

    // Execute
    return handler(evaluatedParams, context);
  }

  private evaluateParams(params: any, context: Context): any {
    if (!params) return {};

    const result: any = {};
    for (const [key, value] of Object.entries(params)) {
      if (typeof value === 'string' && value.includes('{{')) {
        result[key] = evaluateCEL(value, context);
      } else {
        result[key] = value;
      }
    }
    return result;
  }
}
```

**Usage in BlockRenderer**:
```typescript
// Simple! Just dispatch
function handleAction(block: ButtonBlock) {
  actionDispatcher.dispatch(block.action, block.params, context);
}
```

**Why**:
- Easy to add new actions
- No code changes in BlockRenderer
- Template authors can define custom actions
- Built-in actions are centralized

---

### **4. Screen & Navigation System**

**Old**: No concept of screens, no navigation

**New**: Full screen/navigation support
```typescript
// services/navigationService.ts
import { writable } from 'svelte/store';

export class NavigationService {
  private currentScreen = writable<string>('home');
  private routeParams = writable<Record<string, any>>({});
  private screens = new Map<string, ScreenBlock>();

  // Register screens from template
  registerScreens(screens: ScreenBlock[]) {
    screens.forEach(screen => {
      this.screens.set(screen.name, screen);
    });
  }

  // Navigate to screen
  navigateTo(screenName: string, params?: Record<string, any>) {
    if (!this.screens.has(screenName)) {
      console.error(`Screen not found: ${screenName}`);
      return;
    }

    this.currentScreen.set(screenName);
    this.routeParams.set(params || {});

    // Update browser history
    const url = params ? `/${screenName}?${new URLSearchParams(params)}` : `/${screenName}`;
    window.history.pushState({}, '', url);
  }

  // Get current screen
  getCurrentScreen() {
    return this.currentScreen;
  }

  // Get route params
  getRouteParams() {
    return this.routeParams;
  }

  // Go back
  goBack() {
    window.history.back();
  }
}
```

**Usage in App**:
```svelte
<script lang="ts">
  import { navigationService } from '$lib/services';

  const currentScreen = navigationService.getCurrentScreen();
  const routeParams = navigationService.getRouteParams();

  // Register screens from template
  navigationService.registerScreens(template.ui.viewer);
</script>

{#each template.ui.viewer as screen}
  {#if $currentScreen === screen.name}
    <BlockRenderer block={screen} context={{...$routeParams}} />
  {/if}
{/each}
```

**Why**: Enables multi-screen SPAs, a core Sthalam feature.

---

### **5. Modal System**

**Old**: No modal support

**New**: Full modal management
```typescript
// services/modalService.ts
import { writable } from 'svelte/store';

interface ModalState {
  name: string;
  visible: boolean;
  data?: any;
}

export class ModalService {
  private modals = writable<Map<string, ModalState>>(new Map());

  // Open modal
  open(name: string, data?: any) {
    this.modals.update(m => {
      m.set(name, { name, visible: true, data });
      return m;
    });
  }

  // Close modal
  close(name: string) {
    this.modals.update(m => {
      const modal = m.get(name);
      if (modal) {
        modal.visible = false;
      }
      return m;
    });
  }

  // Check if modal is visible
  isVisible(name: string) {
    return derived(this.modals, $modals => {
      return $modals.get(name)?.visible || false;
    });
  }

  // Get modal data
  getData(name: string) {
    return derived(this.modals, $modals => {
      return $modals.get(name)?.data;
    });
  }
}
```

**Modal Block Rendering**:
```svelte
{:else if block.type === 'modal'}
  {@const isVisible = modalService.isVisible(block.name)}
  {#if $isVisible}
    <div class="modal-overlay" onclick={() => block.closable && modalService.close(block.name)}>
      <div class="modal modal-{block.size || 'medium'}" onclick={(e) => e.stopPropagation()}>
        {#if block.closable !== false}
          <button class="modal-close" onclick={() => modalService.close(block.name)}>×</button>
        {/if}
        <div class="modal-content">
          {#each block.blocks as childBlock}
            <svelte:self {childBlock} {context} {onAction} {onStateChange} />
          {/each}
        </div>
      </div>
    </div>
  {/if}
{/if}
```

**Why**: Modals are essential for UX (confirmations, forms, details).

---

### **6. Modular Block Components**

**Old**: All blocks in one file (496 lines)

**New**: Separate components
```
components/blocks/
├── layout/
│   ├── ScreenBlock.svelte
│   ├── ContainerBlock.svelte
│   └── SectionBlock.svelte
├── content/
│   ├── TextBlock.svelte
│   ├── HeadingBlock.svelte
│   ├── LabelBlock.svelte
│   ├── ImageBlock.svelte
│   └── VideoBlock.svelte
├── input/
│   ├── InputBlock.svelte
│   ├── TextareaBlock.svelte
│   ├── CheckboxBlock.svelte
│   ├── SelectBlock.svelte
│   └── RadioBlock.svelte
├── action/
│   ├── ButtonBlock.svelte
│   ├── LinkBlock.svelte
│   └── FormBlock.svelte
└── special/
    ├── CanvasBlock.svelte
    └── ModalBlock.svelte
```

**BlockRenderer becomes a dispatcher**:
```svelte
<script lang="ts">
  import * as LayoutBlocks from './blocks/layout';
  import * as ContentBlocks from './blocks/content';
  import * as InputBlocks from './blocks/input';
  import * as ActionBlocks from './blocks/action';
  import * as SpecialBlocks from './blocks/special';

  const blockComponents = {
    // Layout
    screen: LayoutBlocks.ScreenBlock,
    container: LayoutBlocks.ContainerBlock,
    section: LayoutBlocks.SectionBlock,

    // Content
    text: ContentBlocks.TextBlock,
    heading: ContentBlocks.HeadingBlock,
    label: ContentBlocks.LabelBlock,
    image: ContentBlocks.ImageBlock,
    video: ContentBlocks.VideoBlock,

    // Input
    input: InputBlocks.InputBlock,
    textarea: InputBlocks.TextareaBlock,
    checkbox: InputBlocks.CheckboxBlock,
    select: InputBlocks.SelectBlock,
    radio: InputBlocks.RadioBlock,

    // Action
    button: ActionBlocks.ButtonBlock,
    link: ActionBlocks.LinkBlock,
    form: ActionBlocks.FormBlock,

    // Special
    canvas: SpecialBlocks.CanvasBlock,
    modal: SpecialBlocks.ModalBlock,
  };

  function getBlockComponent(type: BlockType) {
    return blockComponents[type] || UnknownBlock;
  }
</script>

<!-- Render using component -->
<svelte:component
  this={getBlockComponent(block.type)}
  {block}
  {context}
  {onAction}
  {onStateChange}
/>
```

**Why**:
- Easy to test individual blocks
- Easy to add new block types
- Cleaner code organization
- Reusable across projects

---

### **7. CEL Integration with OCaml WASM**

**Old**: Uses old JavaScript evaluator
```typescript
import { evaluateExpression } from '../humlEvaluator';
```

**New**: Uses our new OCaml CEL evaluator (compiled to WASM)
```typescript
// lib/celEvaluator.ts
import { initCELWasm } from './cel-wasm';

let celEvaluator: any = null;

export async function initCEL() {
  if (!celEvaluator) {
    celEvaluator = await initCELWasm();
  }
}

export function evaluateCEL(expr: string, context: Record<string, any>): any {
  if (!celEvaluator) {
    throw new Error('CEL evaluator not initialized');
  }

  try {
    // Convert context to JSON
    const contextJson = JSON.stringify(context);

    // Call WASM evaluator
    const resultJson = celEvaluator.evaluate(expr, contextJson);

    // Parse result
    return JSON.parse(resultJson);
  } catch (error) {
    console.error('CEL evaluation error:', expr, error);
    throw error;
  }
}

// Interpolate {{ }} in strings
export function interpolateCEL(text: string, context: Record<string, any>): string {
  return text.replace(/\{\{([^}]+)\}\}/g, (match, expr) => {
    try {
      const result = evaluateCEL(expr.trim(), context);
      return String(result ?? '');
    } catch {
      return match;
    }
  });
}
```

**Why**:
- Uses our new CEL implementation (28 functions, tested)
- 40x faster performance (from WASM_TAURI_KNOWLEDGE.md)
- Standard CEL syntax
- Supports all Sthalam extensions (exists_one, find, join)

---

### **8. Computed Values Integration**

**Old**: No computed values

**New**: Reactive computed values from template
```typescript
// lib/computedValues.ts
import { derived } from 'svelte/store';
import { evaluateCEL } from './celEvaluator';

export function createComputedStore(
  definition: ComputedDefinition,
  dependencies: Record<string, any>
) {
  // Get dependency stores
  const depStores = definition.depends.map(dep => getDependencyStore(dep));

  // Create derived store
  return derived(depStores, ($deps) => {
    // Build context from dependencies
    const context = buildContext($deps);

    // Evaluate CEL expression
    return evaluateCEL(definition.expr, context);
  });
}
```

**Usage**:
```svelte
<script>
  // Template defines:
  // computed.viewer.filteredPosts:
  //   expr: "posts.filter(p => p.author == currentUser)"
  //   depends: [content.posts, viewerState.currentUser]

  const filteredPosts = createComputedStore(
    template.computed.viewer.filteredPosts,
    { posts: $postsStore, currentUser: $userStore }
  );
</script>

<!-- Use in template -->
{#each $filteredPosts as post}
  <PostCard {post} />
{/each}
```

**Why**:
- Automatic reactivity
- Efficient re-computation (only when dependencies change)
- Declarative data flow

---

## 📊 Side-by-Side Comparison

| Feature | Old BlockRenderer | New BlockRenderer |
|---------|------------------|-------------------|
| **Lines of Code** | 496 lines | ~200 core + modular components |
| **Type Safety** | `block: any` | Fully typed `Block` union |
| **Block Types** | 8 types | 20+ types |
| **Control Flow** | `forEach` only | `when`, `if/then/else`, `match`, `forEach` |
| **Navigation** | None | Full screen system |
| **Modals** | None | Full modal system |
| **Actions** | Hardcoded | Extensible dispatcher |
| **CEL Integration** | Old JS evaluator | New OCaml WASM (40x faster) |
| **Computed Values** | None | Full reactive computed |
| **Modularity** | Monolithic | Separate block components |
| **Canvas Handling** | Mixed into renderer | Separate CanvasBlock component |
| **Extensibility** | Hard to extend | Easy to add blocks/actions |

---

## 🚀 Implementation Plan

### **Phase 1: Core Infrastructure** (Week 1)

**Files to Create**:
```
lib/
├── types/
│   └── huml.ts                 ✅ Complete type definitions
├── services/
│   ├── celEvaluator.ts         ✅ WASM CEL evaluator wrapper
│   ├── actionDispatcher.ts     ✅ Generic action system
│   ├── navigationService.ts    ✅ Screen navigation
│   ├── modalService.ts         ✅ Modal management
│   └── stateManager.ts         ✅ Loro CRDT integration
└── utils/
    └── blockHelpers.ts         ✅ Shared utilities
```

**Deliverable**: Working infrastructure, no UI yet

---

### **Phase 2: Core Renderer** (Week 2)

**Files to Create**:
```
components/
└── BlockRenderer2.svelte       ✅ New core renderer
    - Control flow (when, if, match, forEach)
    - Block type dispatch
    - Recursive rendering
    - ~200 lines, clean
```

**Deliverable**: Renderer working with mock blocks

---

### **Phase 3: Essential Blocks** (Week 3)

**Files to Create**:
```
components/blocks/
├── layout/
│   ├── ScreenBlock.svelte      ✅ Critical: enables navigation
│   ├── ContainerBlock.svelte   ✅ Critical: layout system
│   └── SectionBlock.svelte     ✅
├── content/
│   ├── TextBlock.svelte        ✅
│   ├── HeadingBlock.svelte     ✅
│   └── LabelBlock.svelte       ✅
├── action/
│   ├── ButtonBlock.svelte      ✅ Critical: user actions
│   └── LinkBlock.svelte        ✅
└── special/
    └── ModalBlock.svelte       ✅ Critical: dialogs
```

**Deliverable**: Can render basic templates with navigation

---

### **Phase 4: Input Blocks** (Week 4)

**Files to Create**:
```
components/blocks/input/
├── InputBlock.svelte           ✅
├── TextareaBlock.svelte        ✅
├── CheckboxBlock.svelte        ✅
├── SelectBlock.svelte          ✅
└── RadioBlock.svelte           ✅
```

**Deliverable**: Can build forms

---

### **Phase 5: Remaining Blocks** (Week 5)

**Files to Create**:
```
components/blocks/
├── content/
│   ├── ImageBlock.svelte       ✅
│   └── VideoBlock.svelte       ✅
├── action/
│   └── FormBlock.svelte        ✅
└── special/
    └── CanvasBlock.svelte      ✅ (refactor from old)
```

**Deliverable**: 100% HUML spec coverage

---

## 🎯 Key Benefits of Fresh Start

### **1. Future-Proof Architecture**
- Easy to add new block types
- Easy to add new actions
- Easy to extend control flow

### **2. Performance**
- OCaml WASM evaluator (40x faster)
- Efficient computed value caching
- Proper Svelte reactivity

### **3. Developer Experience**
- Full TypeScript types
- Clear separation of concerns
- Easy to understand codebase
- Self-documenting code

### **4. User Experience**
- Multi-screen navigation
- Modal dialogs
- Complex conditional rendering
- Rich form validation

### **5. Maintainability**
- Each block is ~20-50 lines
- Easy to test in isolation
- Clear dependencies
- No hidden coupling

---

## 📝 Migration Strategy

### **Option A: Parallel Development**
1. Create BlockRenderer2.svelte alongside old one
2. Migrate screens one at a time
3. Remove old BlockRenderer when done

### **Option B: Feature Flags**
1. Add feature flag `useNewRenderer`
2. Switch between old/new based on flag
3. Gradual rollout

### **Option C: Fresh Template**
1. Create new test template using all new features
2. Build new renderer for it
3. Migrate old templates later

**Recommendation**: Option A (Parallel) - safest, allows comparison.

---

## ✅ Ready to Build?

**Next Steps**:
1. Create type definitions (`types/huml.ts`)
2. Create services (action, navigation, modal)
3. Build core BlockRenderer2.svelte
4. Implement essential blocks (screen, container, button, text)
5. Test with real template

Would you like me to start implementing? Which phase should we begin with?
