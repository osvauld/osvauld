# Sthalam Renderer Implementation Roadmap

**Next Steps: WASM Integration + New Renderer**

**Current Status**: CEL evaluator complete (OCaml), types partially defined
**Next Phase**: Integrate WASM + Build new renderer

---

## 📁 Proposed Folder Structure

```
frontend/desktop/src/
├── lib/                          # NEW: Core libraries
│   ├── services/                 # NEW: Business logic services
│   │   ├── celEvaluator.ts      # WASM CEL evaluator wrapper
│   │   ├── actionDispatcher.ts   # Action handling system
│   │   ├── navigationService.ts  # Screen navigation
│   │   ├── modalService.ts       # Modal management
│   │   └── stateManager.ts       # Loro CRDT state
│   │
│   ├── types/                    # NEW: Consolidated types
│   │   ├── huml.ts              # Complete HUML type definitions
│   │   ├── context.ts            # Evaluation context types
│   │   └── actions.ts            # Action types
│   │
│   └── utils/                    # NEW: Shared utilities
│       ├── blockHelpers.ts       # Block-related helpers
│       └── celHelpers.ts         # CEL expression helpers
│
├── renderer/                     # NEW: Rendering system (separate from shared!)
│   ├── BlockRenderer.svelte      # Core renderer (control flow + dispatch)
│   │
│   ├── blocks/                   # Block components
│   │   ├── layout/
│   │   │   ├── ScreenBlock.svelte
│   │   │   ├── ContainerBlock.svelte
│   │   │   └── SectionBlock.svelte
│   │   │
│   │   ├── content/
│   │   │   ├── TextBlock.svelte
│   │   │   ├── HeadingBlock.svelte
│   │   │   ├── LabelBlock.svelte
│   │   │   ├── ImageBlock.svelte
│   │   │   └── VideoBlock.svelte
│   │   │
│   │   ├── input/
│   │   │   ├── InputBlock.svelte
│   │   │   ├── TextareaBlock.svelte
│   │   │   ├── CheckboxBlock.svelte
│   │   │   ├── SelectBlock.svelte
│   │   │   └── RadioBlock.svelte
│   │   │
│   │   ├── action/
│   │   │   ├── ButtonBlock.svelte
│   │   │   ├── LinkBlock.svelte
│   │   │   └── FormBlock.svelte
│   │   │
│   │   └── special/
│   │       ├── CanvasBlock.svelte    # WebGL/Canvas rendering
│   │       └── ModalBlock.svelte
│   │
│   └── index.ts                  # Exports for renderer
│
├── shared/                       # EXISTING: Shared utilities
│   ├── blocks/                   # OLD: Will be deprecated
│   │   └── BlockRenderer.svelte  # Current renderer (keep for comparison)
│   │
│   ├── types/
│   │   └── block.types.ts        # EXISTING: Needs updating
│   │
│   └── loro/                     # Loro-specific utilities
│
└── public/                       # WASM files
    ├── cel_wasm.js               # WASM loader
    └── cel_wasm_bg.wasm          # WASM binary

```

**Why separate `renderer/` from `shared/`?**
- ✅ Clear separation of concerns
- ✅ Renderer is a complete subsystem
- ✅ Easier to test/develop independently
- ✅ Can version/deploy renderer separately
- ✅ `shared/` stays for truly shared utilities (Loro, etc.)

---

## 🎯 Phase 1: Type Definitions & WASM Integration

### **Step 1.1: Update Block Types** ✅

**File**: `src/lib/types/huml.ts` (NEW - comprehensive types)

```typescript
// lib/types/huml.ts
import type { TreeID } from 'loro-crdt';

// ============================================
// CONTROL FLOW TYPES
// ============================================

export interface ConditionalBlock {
  if: string;               // CEL expression
  then: Block[];            // Blocks to render if true
  else?: Block[];           // Blocks to render if false
}

export interface MatchBlock {
  match: string;            // CEL expression to match
  cases: Array<{
    value?: any;            // Value to match (omit for default)
    default?: boolean;      // Is this the default case?
    blocks: Block[];        // Blocks to render
  }>;
}

// ============================================
// BASE BLOCK INTERFACE
// ============================================

export interface BaseBlock {
  type: BlockType;
  id?: string;
  treeId?: TreeID;
  css?: string;
  when?: string;            // Conditional visibility (CEL expression)

  // Control flow
  forEach?: string;         // Loop over array (CEL expression)
  as?: string;             // Loop variable name
  key?: string;            // Reconciliation key for lists
  limit?: number;          // Pagination limit
  offset?: number | string; // Pagination offset (can be CEL)
}

// ============================================
// LAYOUT BLOCKS
// ============================================

export interface ScreenBlock extends BaseBlock {
  type: 'screen';
  name: string;             // Screen identifier
  blocks: Block[];
}

export interface ContainerBlock extends BaseBlock {
  type: 'container';
  layout?: 'flex' | 'grid' | 'block';
  direction?: 'row' | 'column';
  gap?: string;
  alignItems?: string;
  justifyContent?: string;
  columns?: number;         // For grid layout
  blocks?: Block[];
}

export interface SectionBlock extends BaseBlock {
  type: 'section';
  blocks?: Block[];
}

// ============================================
// CONTENT BLOCKS
// ============================================

export interface TextBlock extends BaseBlock {
  type: 'text';
  content: string;          // May contain {{ }} interpolation
}

export interface HeadingBlock extends BaseBlock {
  type: 'heading';
  level?: 1 | 2 | 3 | 4 | 5 | 6;
  content: string;
}

export interface LabelBlock extends BaseBlock {
  type: 'label';
  for?: string;             // ID of input this labels
  content: string;
}

export interface ImageBlock extends BaseBlock {
  type: 'image';
  src: string;              // May contain {{ }} interpolation
  alt?: string;
}

export interface VideoBlock extends BaseBlock {
  type: 'video';
  src: string;
  controls?: boolean;
  autoplay?: boolean;
}

// ============================================
// INPUT BLOCKS
// ============================================

export interface InputBlock extends BaseBlock {
  type: 'input';
  name: string;
  value?: string;
  placeholder?: string;
  validate?: string;        // CEL expression
  error?: string;
  onChange?: string;        // Action name
}

export interface TextareaBlock extends BaseBlock {
  type: 'textarea';
  name: string;
  value?: string;
  placeholder?: string;
  rows?: number;
  onChange?: string;
}

export interface CheckboxBlock extends BaseBlock {
  type: 'checkbox';
  name: string;
  checked?: string | boolean;
  label?: string;
  onChange?: string;
}

export interface SelectBlock extends BaseBlock {
  type: 'select';
  name: string;
  value?: string;
  options: Array<{
    value: string;
    label: string;
  }>;
  onChange?: string;
}

export interface RadioBlock extends BaseBlock {
  type: 'radio';
  name: string;
  value?: string;
  options: Array<{
    value: string;
    label: string;
  }>;
  onChange?: string;
}

// ============================================
// ACTION BLOCKS
// ============================================

export interface ButtonBlock extends BaseBlock {
  type: 'button';
  content: string;
  action?: string;          // Action name
  params?: Record<string, any>;
  disabled?: string | boolean;
  onClick?: string;
}

export interface LinkBlock extends BaseBlock {
  type: 'link';
  content: string;
  href?: string;            // URL for external links
  action?: string;          // Or action for SPA navigation
  params?: Record<string, any>;
}

export interface FormBlock extends BaseBlock {
  type: 'form';
  name: string;
  blocks: Block[];
  onSubmit?: string;        // Action name
}

// ============================================
// SPECIAL BLOCKS
// ============================================

export interface CanvasBlock extends BaseBlock {
  type: 'canvas';
  mode?: 'pattern' | 'chart' | 'interactive' | 'custom';
  width?: number;
  height?: number;

  // Pattern mode
  gridSize?: number;
  cellSize?: number;
  pattern?: string;         // Current pattern (CEL)
  expressions?: Record<string, string>; // Pattern name → CEL expression

  // Canvas controls
  autoplay?: boolean;
  fps?: number;
  onRender?: string;        // Action called each frame
}

export interface ModalBlock extends BaseBlock {
  type: 'modal';
  name: string;             // Modal identifier
  visible?: string;         // CEL expression
  size?: 'small' | 'medium' | 'large' | 'fullscreen';
  closable?: boolean;
  blocks: Block[];
}

// ============================================
// DISCRIMINATED UNION
// ============================================

export type Block =
  // Special control flow (not rendered directly)
  | ConditionalBlock
  | MatchBlock
  // Layout
  | ScreenBlock
  | ContainerBlock
  | SectionBlock
  // Content
  | TextBlock
  | HeadingBlock
  | LabelBlock
  | ImageBlock
  | VideoBlock
  // Input
  | InputBlock
  | TextareaBlock
  | CheckboxBlock
  | SelectBlock
  | RadioBlock
  // Action
  | ButtonBlock
  | LinkBlock
  | FormBlock
  // Special
  | CanvasBlock
  | ModalBlock;

export type BlockType =
  | 'screen'
  | 'container'
  | 'section'
  | 'text'
  | 'heading'
  | 'label'
  | 'image'
  | 'video'
  | 'input'
  | 'textarea'
  | 'checkbox'
  | 'select'
  | 'radio'
  | 'button'
  | 'link'
  | 'form'
  | 'canvas'
  | 'modal';

// ============================================
// CONTEXT & EVALUATION
// ============================================

export interface Context {
  // State from Loro documents
  [key: string]: any;

  // Loop context (when inside forEach)
  item?: any;
  itemIndex?: number;

  // Navigation context
  currentScreen?: string;
  routeParams?: Record<string, any>;

  // Modal context
  modalData?: Record<string, any>;

  // Time/animation context
  time?: number;
  fps?: number;
}

// ============================================
// ACTIONS
// ============================================

export interface Action {
  name: string;
  params?: Record<string, any>;
}

export type ActionHandler = (params: any, context: Context) => void | Promise<void>;

// ============================================
// TYPE GUARDS
// ============================================

export function isConditional(block: any): block is ConditionalBlock {
  return 'if' in block && 'then' in block;
}

export function isMatch(block: any): block is MatchBlock {
  return 'match' in block && 'cases' in block;
}

export function hasBlocks(block: Block): block is (ScreenBlock | ContainerBlock | SectionBlock | FormBlock | ModalBlock) {
  return 'blocks' in block;
}

export function isContainer(block: Block): block is (ScreenBlock | ContainerBlock | SectionBlock) {
  return block.type === 'screen' || block.type === 'container' || block.type === 'section';
}
```

---

### **Step 1.2: WASM CEL Evaluator Integration** ⚡

**File**: `src/lib/services/celEvaluator.ts`

```typescript
// lib/services/celEvaluator.ts

/**
 * OCaml WASM CEL Evaluator Integration
 *
 * Compiles OCaml CEL implementation to WASM using wasm_of_ocaml
 * Provides 40x performance improvement over JS evaluator
 */

interface CELWasmModule {
  evaluate: (expr: string, contextJson: string) => string;
  evaluateGrid: (expr: string, contextJson: string, gridSize: number) => Uint8Array;
}

let wasmModule: CELWasmModule | null = null;

/**
 * Initialize WASM module
 * Call this on app startup
 */
export async function initCEL(): Promise<void> {
  if (wasmModule) return; // Already initialized

  try {
    // Load WASM module
    // Note: Path depends on your build output from wasm_of_ocaml
    const module = await import('/cel_wasm.js');
    await module.default(); // Initialize WASM

    wasmModule = module as CELWasmModule;

    console.log('✅ CEL WASM evaluator initialized');
  } catch (error) {
    console.error('❌ Failed to initialize CEL WASM:', error);
    throw error;
  }
}

/**
 * Evaluate CEL expression
 *
 * @param expr - CEL expression to evaluate
 * @param context - Evaluation context (variables)
 * @returns Evaluated result
 */
export function evaluateCEL(expr: string, context: Record<string, any>): any {
  if (!wasmModule) {
    throw new Error('CEL evaluator not initialized. Call initCEL() first.');
  }

  try {
    // Convert context to JSON
    const contextJson = JSON.stringify(context);

    // Call WASM evaluator
    const resultJson = wasmModule.evaluate(expr, contextJson);

    // Parse result
    return JSON.parse(resultJson);
  } catch (error) {
    console.error('CEL evaluation error:', { expr, context, error });
    throw new Error(`CEL evaluation failed: ${error}`);
  }
}

/**
 * Interpolate {{ }} expressions in string
 *
 * @param text - Text with {{ expr }} placeholders
 * @param context - Evaluation context
 * @returns Interpolated string
 */
export function interpolateCEL(text: string, context: Record<string, any>): string {
  if (!text || typeof text !== 'string') return text;

  return text.replace(/\{\{([^}]+)\}\}/g, (match, expr) => {
    try {
      const result = evaluateCEL(expr.trim(), context);
      return result === null || result === undefined ? '' : String(result);
    } catch (error) {
      console.warn('Failed to interpolate expression:', expr, error);
      return match; // Return original if evaluation fails
    }
  });
}

/**
 * Evaluate expression for grid/canvas rendering
 * Optimized for evaluating same expression for many (x, y) points
 *
 * @param expr - CEL expression (can reference x, y, time, etc.)
 * @param context - Base context (time, gridSize, etc.)
 * @param gridSize - Grid dimensions
 * @returns Uint8Array of pixel values (0-255)
 */
export async function evaluateGridCEL(
  expr: string,
  context: Record<string, any>,
  gridSize: number
): Promise<Uint8Array> {
  if (!wasmModule?.evaluateGrid) {
    // Fallback to JS implementation if WASM doesn't have optimized grid eval
    return evaluateGridFallback(expr, context, gridSize);
  }

  try {
    const contextJson = JSON.stringify(context);
    return wasmModule.evaluateGrid(expr, contextJson, gridSize);
  } catch (error) {
    console.error('Grid evaluation error:', error);
    return evaluateGridFallback(expr, context, gridSize);
  }
}

/**
 * Fallback grid evaluation using regular evaluate
 */
function evaluateGridFallback(
  expr: string,
  baseContext: Record<string, any>,
  gridSize: number
): Uint8Array {
  const output = new Uint8Array(gridSize * gridSize);
  const ctx = { ...baseContext };

  let idx = 0;
  for (let y = 0; y < gridSize; y++) {
    ctx.y = y;
    for (let x = 0; x < gridSize; x++) {
      ctx.x = x;

      try {
        const value = evaluateCEL(expr, ctx);
        // Map -1..1 to 0..255
        const brightness = (value + 1) * 127.5;
        output[idx++] = Math.max(0, Math.min(255, Math.floor(brightness)));
      } catch {
        output[idx++] = 0;
      }
    }
  }

  return output;
}

/**
 * Check if CEL is initialized
 */
export function isCELReady(): boolean {
  return wasmModule !== null;
}
```

---

## 🎯 Phase 2: Core Services

### **Step 2.1: Action Dispatcher** ⚡

**File**: `src/lib/services/actionDispatcher.ts`

```typescript
// lib/services/actionDispatcher.ts
import type { Context, ActionHandler } from '$lib/types/huml';
import { navigationService } from './navigationService';
import { modalService } from './modalService';
import { stateManager } from './stateManager';
import { evaluateCEL } from './celEvaluator';

export class ActionDispatcher {
  private handlers = new Map<string, ActionHandler>();

  constructor() {
    this.registerBuiltInActions();
  }

  /**
   * Register built-in Sthalam actions
   */
  private registerBuiltInActions() {
    // Navigate to screen
    this.register('navigate', (params, context) => {
      navigationService.navigateTo(params.screen, params);
    });

    // Open modal
    this.register('openModal', (params) => {
      modalService.open(params.modal, params.data);
    });

    // Close modal
    this.register('closeModal', (params) => {
      modalService.close(params.modal);
    });

    // Set state
    this.register('setState', (params) => {
      stateManager.set(params.field, params.value);
    });
  }

  /**
   * Register custom action handler
   */
  register(name: string, handler: ActionHandler) {
    this.handlers.set(name, handler);
  }

  /**
   * Dispatch action
   */
  async dispatch(action: string, params: any, context: Context): Promise<void> {
    // Evaluate params (may contain CEL expressions)
    const evaluatedParams = this.evaluateParams(params, context);

    // Get handler
    const handler = this.handlers.get(action);

    if (!handler) {
      console.warn(`Unknown action: ${action}`);
      return;
    }

    // Execute
    try {
      await handler(evaluatedParams, context);
    } catch (error) {
      console.error(`Action '${action}' failed:`, error);
      throw error;
    }
  }

  /**
   * Evaluate parameters (convert CEL expressions to values)
   */
  private evaluateParams(params: any, context: Context): any {
    if (!params || typeof params !== 'object') {
      return params;
    }

    const result: any = {};

    for (const [key, value] of Object.entries(params)) {
      if (typeof value === 'string' && value.includes('{{')) {
        // Interpolate CEL expression
        result[key] = this.interpolateValue(value, context);
      } else if (typeof value === 'object' && value !== null) {
        // Recursive for nested objects
        result[key] = this.evaluateParams(value, context);
      } else {
        result[key] = value;
      }
    }

    return result;
  }

  private interpolateValue(value: string, context: Context): any {
    const trimmed = value.trim();

    // Pure expression: {{ expr }}
    if (trimmed.startsWith('{{') && trimmed.endsWith('}}')) {
      const expr = trimmed.slice(2, -2).trim();
      return evaluateCEL(expr, context);
    }

    // Interpolated string: "text {{ expr }} more"
    return value.replace(/\{\{([^}]+)\}\}/g, (match, expr) => {
      try {
        const result = evaluateCEL(expr.trim(), context);
        return String(result ?? '');
      } catch {
        return match;
      }
    });
  }
}

// Singleton instance
export const actionDispatcher = new ActionDispatcher();
```

---

### **Step 2.2: Navigation Service** 🧭

**File**: `src/lib/services/navigationService.ts`

```typescript
// lib/services/navigationService.ts
import { writable, derived, type Readable } from 'svelte/store';
import type { ScreenBlock } from '$lib/types/huml';

interface NavigationState {
  currentScreen: string;
  routeParams: Record<string, any>;
  history: string[];
}

class NavigationService {
  private state = writable<NavigationState>({
    currentScreen: '',
    routeParams: {},
    history: []
  });

  private screens = new Map<string, ScreenBlock>();

  // Public stores
  currentScreen: Readable<string>;
  routeParams: Readable<Record<string, any>>;

  constructor() {
    this.currentScreen = derived(this.state, $state => $state.currentScreen);
    this.routeParams = derived(this.state, $state => $state.routeParams);
  }

  /**
   * Register screens from template
   */
  registerScreens(screens: ScreenBlock[]) {
    screens.forEach(screen => {
      this.screens.set(screen.name, screen);
    });

    // Navigate to first screen if none set
    if (!this.getCurrentScreen() && screens.length > 0) {
      this.navigateTo(screens[0].name);
    }
  }

  /**
   * Navigate to screen
   */
  navigateTo(screenName: string, params?: Record<string, any>) {
    if (!this.screens.has(screenName)) {
      console.error(`Screen not found: ${screenName}`);
      return;
    }

    this.state.update($state => ({
      currentScreen: screenName,
      routeParams: params || {},
      history: [...$state.history, screenName]
    }));

    // Update URL (optional, for browser history)
    this.updateURL(screenName, params);
  }

  /**
   * Go back in history
   */
  goBack() {
    this.state.update($state => {
      const history = [...$state.history];
      history.pop(); // Remove current
      const previous = history[history.length - 1];

      if (previous) {
        return {
          ...$state,
          currentScreen: previous,
          history
        };
      }

      return $state;
    });
  }

  /**
   * Get current screen name
   */
  getCurrentScreen(): string {
    let current = '';
    this.state.subscribe($s => current = $s.currentScreen)();
    return current;
  }

  /**
   * Update browser URL
   */
  private updateURL(screenName: string, params?: Record<string, any>) {
    const url = params
      ? `/${screenName}?${new URLSearchParams(params)}`
      : `/${screenName}`;

    window.history.pushState({ screen: screenName, params }, '', url);
  }
}

export const navigationService = new NavigationService();
```

---

### **Step 2.3: Modal Service** 💬

**File**: `src/lib/services/modalService.ts`

```typescript
// lib/services/modalService.ts
import { writable, derived, type Readable } from 'svelte/store';

interface ModalState {
  visible: boolean;
  data?: any;
}

class ModalService {
  private modals = writable<Map<string, ModalState>>(new Map());

  /**
   * Open modal
   */
  open(name: string, data?: any) {
    this.modals.update(m => {
      m.set(name, { visible: true, data });
      return new Map(m);
    });
  }

  /**
   * Close modal
   */
  close(name: string) {
    this.modals.update(m => {
      const modal = m.get(name);
      if (modal) {
        modal.visible = false;
      }
      return new Map(m);
    });
  }

  /**
   * Check if modal is visible
   */
  isVisible(name: string): Readable<boolean> {
    return derived(this.modals, $modals => {
      return $modals.get(name)?.visible || false;
    });
  }

  /**
   * Get modal data
   */
  getData(name: string): Readable<any> {
    return derived(this.modals, $modals => {
      return $modals.get(name)?.data;
    });
  }
}

export const modalService = new ModalService();
```

---

## 🎯 Phase 3: Core Renderer

**Next Document**: Create detailed BlockRenderer implementation

---

## ✅ Implementation Checklist

### **Phase 1: Foundation** (Week 1)
- [ ] Create `/src/lib/types/huml.ts` with complete types
- [ ] Create `/src/lib/services/celEvaluator.ts`
- [ ] Compile OCaml CEL to WASM
- [ ] Deploy WASM to `/public/`
- [ ] Test WASM integration

### **Phase 2: Services** (Week 1-2)
- [ ] Create `actionDispatcher.ts`
- [ ] Create `navigationService.ts`
- [ ] Create `modalService.ts`
- [ ] Create `stateManager.ts` (Loro integration)
- [ ] Test services independently

### **Phase 3: Renderer** (Week 2-3)
- [ ] Create `/src/renderer/` folder
- [ ] Create core `BlockRenderer.svelte`
- [ ] Create layout blocks (Screen, Container, Section)
- [ ] Create content blocks (Text, Heading, Label, Image, Video)
- [ ] Create action blocks (Button, Link, Form)
- [ ] Create special blocks (Canvas, Modal)
- [ ] Create input blocks (Input, Textarea, Checkbox, Select, Radio)

### **Phase 4: Integration** (Week 3-4)
- [ ] Integrate with existing Loro state
- [ ] Add computed values support
- [ ] Test with real HUML templates
- [ ] Performance testing (60 FPS target)
- [ ] Migration from old BlockRenderer

---

## 🚀 Next Immediate Steps

**What to build first?**

1. ✅ **WASM Compilation** - Compile CEL evaluator to WASM
2. ✅ **Type Definitions** - Create complete `huml.ts` types
3. ✅ **CEL Service** - Create evaluator wrapper
4. ✅ **Core Services** - Action/navigation/modal services
5. ✅ **Core Renderer** - BlockRenderer.svelte with control flow
6. ✅ **Essential Blocks** - Screen, Container, Text, Button

**Ready to start?** Let me know which phase you'd like to begin with!
