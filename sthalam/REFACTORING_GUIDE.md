# Sthalam Refactoring Guide

**Last Updated:** 2025-10-24
**Purpose:** Document refactoring opportunities and guide future improvements for the Sthalam app codebase.

---

## Table of Contents

1. [Overview](#overview)
2. [High Priority Refactoring](#high-priority-refactoring)
3. [Medium Priority Refactoring](#medium-priority-refactoring)
4. [Low Priority Refactoring](#low-priority-refactoring)
5. [Current Architecture](#current-architecture)
6. [Code Metrics](#code-metrics)
7. [Refactoring Guidelines](#refactoring-guidelines)

---

## Overview

This guide documents refactoring opportunities identified in the Sthalam app, focusing on components in `/frontend/desktop/src/components` and `/frontend/desktop/src/lib` folders.

### Key Statistics
- **Total Components:** 24 (18 in `/components`, 6 in `/lib`)
- **Largest File:** `PropertiesPanel.svelte` (52 KB, ~1,850 lines)
- **Total Source Code:** ~350 KB across components and lib
- **Utility Files:** 4 in `/utils`
- **State Management:** 5 stores + 2 state files

---

## High Priority Refactoring

### 1. Split PropertiesPanel.svelte ⚠️ CRITICAL

**File:** `/frontend/desktop/src/lib/PropertiesPanel.svelte`
**Size:** 52 KB (~1,850 lines)
**Issue:** Single Responsibility Violation - handles properties for 15+ block types

#### Problems:
- Massive conditional chains for each block type
- Hard to maintain, test, and extend
- 550+ lines of CSS
- 11 props passed in
- No type safety for block-specific properties

#### Recommended Approach:

```
lib/
├── PropertiesPanel.svelte (main orchestrator)
└── properties/
    ├── TextPropertiesPanel.svelte
    ├── FormFieldPropertiesPanel.svelte
    ├── NavButtonPropertiesPanel.svelte
    ├── ThreadPropertiesPanel.svelte
    ├── ImagePropertiesPanel.svelte
    ├── VideoPropertiesPanel.svelte
    ├── ButtonPropertiesPanel.svelte
    ├── DividerPropertiesPanel.svelte
    └── shared/
        ├── ColorPicker.svelte
        ├── SpacingControls.svelte
        ├── TextStyleControls.svelte
        └── BorderControls.svelte
```

#### Implementation Steps:
1. Create `properties/` folder structure
2. Extract shared controls first (ColorPicker, SpacingControls, etc.)
3. Extract one block type at a time (start with simplest)
4. Create block type registry/factory pattern:
   ```typescript
   const propertyPanels = {
     'text': TextPropertiesPanel,
     'form-field': FormFieldPropertiesPanel,
     'nav-button': NavButtonPropertiesPanel,
     // ...
   };
   ```
5. Update main PropertiesPanel to use dynamic component loading
6. Add proper TypeScript interfaces for each block type

#### Benefits:
- Each component < 200 lines
- Easier to test individual panels
- Better code organization
- Type-safe block properties
- Easier to add new block types

---

### 2. Refactor helper.ts handlerMap Anti-pattern

**File:** `/frontend/desktop/src/utils/helper.ts`
**Size:** 6.5 KB
**Issue:** 60+ actions in giant object with `@ts-ignore`, no type safety

#### Current Problems:
```typescript
// Lines 11-61: Giant object mapping
const handlerMap: Record<string, () => Promise<any>> = {
  getAccounts: invoke.getAccounts,
  // ... 60+ more
};

// @ts-ignore - BAD!
export async function sendMessage(action: string, args?: any): Promise<any> {
  return handlerMap[action](args);
}
```

#### Recommended Approach:

**Option 1: Typed Command Classes**
```typescript
// utils/commands/base.ts
export interface Command<TArgs = void, TResult = void> {
  execute(args: TArgs): Promise<TResult>;
}

// utils/commands/userCommands.ts
export class GetAccountsCommand implements Command<void, Account[]> {
  async execute(): Promise<Account[]> {
    return invoke.getAccounts();
  }
}

// utils/commands/index.ts
export const commands = {
  getAccounts: new GetAccountsCommand(),
  // ...
} as const;
```

**Option 2: Organized by Domain**
```typescript
// utils/api/user.ts
export const userApi = {
  getAccounts: () => invoke.getAccounts(),
  getUserDetails: () => invoke.getUserDetails(),
  // ...
};

// utils/api/resource.ts
export const resourceApi = {
  createResource: (args) => invoke.createResource(args),
  updateResource: (args) => invoke.updateResource(args),
  // ...
};

// utils/api/index.ts
export const api = {
  user: userApi,
  resource: resourceApi,
  website: websiteApi,
  // ...
};
```

#### Implementation Steps:
1. Categorize all 60+ actions by domain (user, resource, website, etc.)
2. Create typed interfaces for each command's args and result
3. Create domain-specific API modules
4. Update all imports throughout codebase
5. Remove handlerMap and @ts-ignore

#### Benefits:
- Full type safety
- Better IDE autocomplete
- Easier to find and update commands
- Clear separation by domain
- No more magic strings

---

### 3. Extract Duplicate Initialization Logic

**Files:** `WebsiteBuilder.svelte` and `ViewerMode.svelte`
**Issue:** Identical resource initialization and BlocksuiteStore subscription code

#### Duplicate Code Pattern:
Both files have:
- Resource change effects (lines 52-170)
- BlocksuiteStore subscription patterns
- Document initialization logic
- Cleanup/unsubscribe logic

#### Recommended Approach:

Create custom Svelte composable:

```typescript
// lib/composables/useResourceInitialization.ts
export function useResourceInitialization(
  resourceId: () => string | null,
  onResourceChange?: (resource: Resource) => void
) {
  let blocksuiteStore = $state<BlocksuiteStore | null>(null);
  let unsubscribe = $state<(() => void) | null>(null);

  $effect(() => {
    if (!resourceId()) return;

    // Initialize BlocksuiteStore
    // Subscribe to changes
    // Handle cleanup

    return () => {
      unsubscribe?.();
      blocksuiteStore?.destroy();
    };
  });

  return {
    get blocksuiteStore() { return blocksuiteStore; },
    // ... other derived state
  };
}
```

#### Implementation Steps:
1. Create `lib/composables/` folder
2. Extract common initialization logic
3. Create `useResourceInitialization.ts`
4. Update WebsiteBuilder to use composable
5. Update ViewerMode to use composable
6. Test both modes thoroughly

#### Benefits:
- DRY (Don't Repeat Yourself)
- Single source of truth for initialization
- Easier to fix bugs in one place
- Consistent behavior across modes

---

### 4. Create Error Handling System

**Issue:** Inconsistent error handling throughout app

#### Current State:
- Some components have try/catch
- Others only console.error
- No user-facing error messages
- No centralized error boundary

#### Recommended Approach:

```typescript
// lib/error/errorHandler.ts
export class AppError extends Error {
  constructor(
    message: string,
    public code: string,
    public severity: 'error' | 'warning' | 'info'
  ) {
    super(message);
  }
}

export function handleError(error: unknown, context: string) {
  const appError = toAppError(error, context);

  // Log to console
  console.error(`[${context}]`, appError);

  // Show user notification
  uiState.showNotification({
    type: appError.severity,
    message: appError.message
  });

  // Optionally report to error tracking service
  // reportError(appError);
}
```

#### Implementation Steps:
1. Create error handling utilities
2. Add error notification UI component
3. Update all try/catch blocks to use handleError
4. Add error boundaries for critical components
5. Define error codes and user-friendly messages

---

## Medium Priority Refactoring

### 5. Separate Canvas Concerns

**File:** `/frontend/desktop/src/lib/Canvas.svelte`
**Size:** 9.4 KB
**Issue:** Mixes panning, zooming, and rendering logic

#### Recommended Approach:

Extract pan/zoom into composable:

```typescript
// lib/composables/usePanZoom.ts
export function usePanZoom(initialPan = { x: 0, y: 0 }, initialZoom = 1) {
  let pan = $state(initialPan);
  let zoom = $state(initialZoom);
  let isDragging = $state(false);

  const handleMouseDown = (e: MouseEvent) => {
    // Pan logic
  };

  const handleMouseMove = (e: MouseEvent) => {
    // Pan logic
  };

  const handleWheel = (e: WheelEvent) => {
    // Zoom logic
  };

  return {
    pan,
    zoom,
    isDragging,
    handleMouseDown,
    handleMouseMove,
    handleWheel,
    transformPoint: (x: number, y: number) => ({
      x: x * zoom + pan.x,
      y: y * zoom + pan.y
    })
  };
}
```

#### Benefits:
- Reusable pan/zoom logic
- Easier to test
- Canvas focused on rendering only

---

### 6. Create Block Type Registry for BlockRenderer

**File:** `/frontend/desktop/src/lib/blocks/BlockRenderer.svelte`
**Issue:** Long if/else chain for 12+ block types (lines 29-81)

#### Current Code:
```svelte
{#if block.type === 'text'}
  <TextBlock ... />
{:else if block.type === 'form-field'}
  <FormField ... />
{:else if block.type === 'nav-button'}
  <NavButton ... />
<!-- ... 10+ more -->
{/if}
```

#### Recommended Approach:

```typescript
// lib/blocks/registry.ts
import type { ComponentType } from 'svelte';

export const blockRegistry: Record<string, ComponentType> = {
  'text': TextBlock,
  'form-field': FormField,
  'nav-button': NavButton,
  'thread': ThreadBlock,
  'markdown': MarkdownText,
  // ...
};

export function getBlockComponent(type: string): ComponentType | null {
  return blockRegistry[type] || null;
}
```

```svelte
<!-- BlockRenderer.svelte -->
<script>
  const BlockComponent = getBlockComponent(block.type);
</script>

{#if BlockComponent}
  <svelte:component this={BlockComponent} {block} {...props} />
{:else}
  <div>Unknown block type: {block.type}</div>
{/if}
```

#### Benefits:
- Easier to add new block types
- No need to modify BlockRenderer
- Cleaner, more maintainable code

---

### 7. Add Block Type Constants/Enums

**Issue:** Magic strings throughout codebase

#### Current Problems:
```typescript
if (block.type === 'form-field-') // typo-prone
if (block.type === 'nav-button')
```

#### Recommended Approach:

```typescript
// types/blockTypes.ts
export const BlockType = {
  TEXT: 'text',
  FORM_FIELD: 'form-field',
  NAV_BUTTON: 'nav-button',
  THREAD: 'thread',
  IMAGE: 'image',
  VIDEO: 'video',
  BUTTON: 'button',
  DIVIDER: 'divider',
  MARKDOWN: 'markdown',
  CONTAINER: 'container',
  SPACER: 'spacer',
  DOWNLOAD: 'download',
} as const;

export type BlockTypeValue = typeof BlockType[keyof typeof BlockType];

// Discriminated union for type safety
export type Block =
  | TextBlock
  | FormFieldBlock
  | NavButtonBlock
  // ...

export interface TextBlock {
  type: typeof BlockType.TEXT;
  content: string;
  styles: TextStyles;
  // ...
}
```

#### Benefits:
- Type safety
- IDE autocomplete
- Catch typos at compile time
- Single source of truth

---

### 8. Consolidate Modal Pattern

**Issue:** 5 modals with similar structure

#### Current Modals:
- AddSovereignNodeModal.svelte
- AddWebsiteConnectionModal.svelte
- CreateResourceModal.svelte
- PublishWebsiteModal.svelte
- TemplateImportModal.svelte

#### Recommended Approach:

Create shared Modal wrapper:

```svelte
<!-- components/shared/Modal.svelte -->
<script lang="ts">
  let {
    show = $bindable(false),
    title,
    onClose,
    children,
    size = 'medium'
  } = $props();
</script>

{#if show}
  <div class="modal-overlay" on:click={onClose}>
    <div class="modal-content modal-{size}" on:click|stopPropagation>
      <div class="modal-header">
        <h2>{title}</h2>
        <button on:click={onClose}>×</button>
      </div>
      <div class="modal-body">
        {@render children()}
      </div>
    </div>
  </div>
{/if}
```

Usage:
```svelte
<Modal bind:show={showModal} title="Add Website" onClose={handleClose}>
  {#snippet children()}
    <!-- Modal content here -->
  {/snippet}
</Modal>
```

---

## Low Priority Refactoring

### 9. Type Safety Improvements

#### Add Discriminated Union Types:
```typescript
// types/blocks.ts
export type Block =
  | { type: 'text'; content: string; styles: TextStyles }
  | { type: 'form-field'; fieldType: string; label: string }
  | { type: 'nav-button'; label: string; targetId: string }
  // ...
```

#### Replace `any` Types:
- Canvas.svelte line 18
- BlockRenderer.svelte line 14
- Various helper functions

#### Add JSDoc Comments:
```typescript
/**
 * Creates an empty BlockSuite document with default structure
 * @param websiteId - The ID of the website this document belongs to
 * @returns Initialized Yjs document
 */
export function createEmptyBlocksuiteDoc(websiteId: string): Y.Doc {
  // ...
}
```

---

### 10. Extract Constants

**Issue:** Hard-coded values throughout

#### Create Theme Constants:
```typescript
// lib/theme/constants.ts
export const COLORS = {
  PRIMARY: '#3b82f6',
  SECONDARY: '#64748b',
  SUCCESS: '#10b981',
  ERROR: '#ef4444',
  // ...
} as const;

export const SPACING = {
  XS: '0.25rem',
  SM: '0.5rem',
  MD: '1rem',
  LG: '1.5rem',
  XL: '2rem',
} as const;

export const SIZES = {
  CANVAS_MIN_ZOOM: 0.1,
  CANVAS_MAX_ZOOM: 3,
  SIDEBAR_WIDTH: 250,
  // ...
} as const;
```

---

### 11. Code Cleanup

#### Remove Console Logs:
```typescript
// Create debug utility
const DEBUG = import.meta.env.DEV;

export function debug(...args: any[]) {
  if (DEBUG) console.log('[DEBUG]', ...args);
}
```

#### Remove Unused Code:
- Commented-out code blocks
- Unused imports
- Dead code paths

#### Standardize Formatting:
- Use Prettier consistently
- Follow ESLint rules

---

## Current Architecture

### State Management Pattern

**Global State** (`/src/state/`):
- `dataState.svelte.ts` - Resources, websites, BlockSuite coordinator
- `uiState.svelte.ts` - Modals, panels, modes

Both use Svelte 5 runes: `$state`, `$derived`, `$effect`

### Yjs Document Architecture

**Multi-Document Pattern:**
- `blocksuite_doc` - Website content/blocks
- `thread_comments_doc` - Collaborative thread comments
- `form_submissions_doc` - Form submissions

**Manager-Coordinator Pattern:**
- `YjsManager` - Low-level Yjs lifecycle
- `BlocksuiteCoordinator` - High-level orchestration

### Store Pattern

- `BlocksuiteStore` - Blocks state
- `SubmissionsStore` - Form submissions
- `ThreadCommentsStore` - Thread comments

All follow subscription pattern for reactivity.

### Component Hierarchy

```
App
├── NavigationPanel
│   ├── WebsiteFolder
│   │   └── ResourceItem
├── ModeSwitcher
└── ViewerMode | WebsiteBuilder
    ├── Canvas (builder)
    │   ├── BlockPalette
    │   └── PropertiesPanel
    └── FullScreenViewer (viewer)
        ├── BlockRenderer (recursive)
        └── SubmissionsViewer
```

---

## Code Metrics

### File Size Distribution

| Category | Largest Files | Size |
|----------|---------------|------|
| **Components** | PublishWebsiteModal | 14 KB |
| | ViewerMode | 13 KB |
| | SubmissionsViewer | 11 KB |
| **Lib** | PropertiesPanel | 52 KB ⚠️ |
| | WebsiteBuilder | 21 KB |
| | blocksuiteCoordinator | 16 KB |
| **Utils** | templateImporter | 11 KB |
| | helper | 6.5 KB |

### Component Count

- **Modal Components:** 5
- **Navigation/UI:** 5
- **Feature Components:** 5
- **List/Display:** 3
- **Lib Components:** 6
- **Block Renderers:** 5
- **Total:** 29 components

---

## Refactoring Guidelines

### General Principles

1. **Single Responsibility Principle**
   - Each component should have one clear purpose
   - Target: Components under 300 lines

2. **DRY (Don't Repeat Yourself)**
   - Extract common patterns into composables
   - Create shared components for repeated UI

3. **Type Safety**
   - Use TypeScript interfaces and types
   - Avoid `any` types
   - Use discriminated unions for variants

4. **Separation of Concerns**
   - Keep business logic separate from UI
   - Extract complex logic into utilities
   - Use composables for reusable logic

5. **Testing**
   - Each refactored component should be testable
   - Smaller components = easier testing

### Refactoring Process

1. **Before Refactoring:**
   - Document current behavior
   - Write tests (if possible)
   - Create feature branch

2. **During Refactoring:**
   - Make small, incremental changes
   - Test after each change
   - Commit frequently

3. **After Refactoring:**
   - Verify all functionality works
   - Update documentation
   - Review code quality

### Testing Checklist

- [ ] All existing features work
- [ ] No console errors
- [ ] TypeScript compiles without errors
- [ ] No performance regressions
- [ ] Code is more maintainable

---

## Recommended Refactoring Order

### Phase 1: Foundation (Week 1-2)
1. ✅ Create refactoring guide (this document)
2. Add block type constants/enums
3. Create error handling system
4. Extract shared constants

### Phase 2: Critical Refactoring (Week 3-4)
5. Split PropertiesPanel into smaller components
6. Refactor helper.ts handlerMap

### Phase 3: Code Organization (Week 5-6)
7. Extract duplicate initialization logic
8. Separate Canvas concerns
9. Create block type registry

### Phase 4: Polish (Week 7-8)
10. Consolidate modal pattern
11. Type safety improvements
12. Code cleanup

---

## Success Metrics

### Code Quality
- [ ] No files over 500 lines
- [ ] PropertiesPanel split into 6+ files
- [ ] All `@ts-ignore` removed
- [ ] 90%+ TypeScript strict mode compliance

### Maintainability
- [ ] New block type can be added in < 1 hour
- [ ] Duplicate code reduced by 50%+
- [ ] Developer onboarding time reduced

### Performance
- [ ] No performance regressions
- [ ] Component render times unchanged or improved

---

## Notes

- This is a living document - update as refactoring progresses
- Mark items as complete with ✅
- Add new issues as discovered
- Review quarterly for updates

---

**Next Review Date:** 2025-11-24
