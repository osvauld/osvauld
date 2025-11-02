# HUML Collaborative Block Enhancement - Implementation Plan

**Date:** 2025-01-01
**Version:** 1.0

---

## Current State Analysis

### Architecture Overview

We have a **block-based UI templating system** powered by:
- **HUML** - Declarative template language (like Notion blocks, but NOT BlockSuite)
- **Yjs** - CRDT-based collaboration (real-time sync)
- **CEL** - Expression language for dynamic content
- **Svelte** - Component rendering

### Document Architecture (5 Yjs Documents)

```
┌─────────────────────────────────────────────────────────┐
│ 1. blocksuite_doc / thread_doc / form_doc               │
│    └─ HUML block structure (hierarchy, properties)      │
│    └─ Stored in: BlocksuiteStore                        │
│    └─ Publisher writes, viewers read                    │
├─────────────────────────────────────────────────────────┤
│ 2. content_doc                                          │
│    └─ Publisher's shared content (products, articles)   │
│    └─ Publisher: Read/Write, Viewers: Read-only         │
│    └─ Observable by templateState.svelte                │
├─────────────────────────────────────────────────────────┤
│ 3. user_content_doc                                     │
│    └─ Per-user private data (cart, orders, prefs)      │
│    └─ Isolated per user (Publisher cannot see)         │
│    └─ Observable by templateState.svelte                │
├─────────────────────────────────────────────────────────┤
│ 4. form_submissions_doc                                 │
│    └─ Form submissions (append-only)                    │
│    └─ Viewers: Write-only, Publisher: Read-only        │
│    └─ Stored in: SubmissionsStore                       │
├─────────────────────────────────────────────────────────┤
│ 5. thread_comments_doc                                  │
│    └─ Collaborative data (comments, reviews, ratings)   │
│    └─ Everyone: Read/Write (public collaboration)      │
│    └─ Stored in: ThreadCommentsStore                    │
└─────────────────────────────────────────────────────────┘
```

### 5 Data Interaction Patterns

| Pattern | Document | Viewer Read | Viewer Write | Publisher Read | Publisher Write | Other Viewers | Use Cases |
|---------|----------|-------------|--------------|----------------|-----------------|---------------|-----------|
| **1. Viewer-Only** | `user_content::` | ✅ Own | ✅ Own | ❌ | ❌ | ❌ Isolated | Cart, orders, favorites, preferences |
| **2. Viewer Append** | `form_submissions` | ❌ | ✅ Append | ✅ All | ✅ (rare) | ❌ | Contact forms, surveys, registrations |
| **3. Publisher Broadcast** | `content::` | ✅ All | ❌ | ✅ All | ✅ Update | ✅ Read-only | Products, articles, pricing, inventory |
| **4. Public Collaboration** | `thread_comments_doc` | ✅ All | ✅ Add/Edit | ✅ All | ✅ Add/Edit | ✅ Read/Write | Reviews, Q&A, group chat, discussions |
| **5. Private Collaboration** | `private_thread_doc_{userId}` | ✅ Own | ✅ Own | ✅ This viewer | ✅ This viewer | ❌ | Support tickets, private inquiries |

**Note:** Pattern 5 (Private Collaboration) is **NOT YET IMPLEMENTED** - planned for future.

---

## Current Limitations

### Thread Block (Pattern 4 - Public Collaboration)

**Fixed Schema:**
```typescript
interface Comment {
  id: string;
  author: string;
  userId?: string;
  content: string;
  timestamp: number;
}
```

**Problems:**
- ❌ Cannot add custom fields (rating, upvotes, likes, productId, etc.)
- ❌ No CEL-driven rendering (hardcoded Svelte template)
- ❌ No sorting/filtering (comments shown as-is)
- ❌ No nesting/threading (flat list only)
- ❌ No aggregates (can't compute avgRating, totalReviews, sum)
- ❌ No conditional rendering (can't filter by rating, etc.)
- ❌ Single use case (comments only, not reviews/ratings/chat)

**Current Rendering:**
```svelte
<!-- ThreadBlock.svelte - Hardcoded layout -->
<div class="comment">
  <strong>{comment.author}</strong>
  <span>{comment.timestamp}</span>
  <div>{@html comment.content}</div>
</div>
```

### Naming Issues

Current names reference **"BlockSuite"** (a framework we're NOT using):

❌ `BlocksuiteCoordinator` → Should be `TemplateCoordinator`
❌ `BlocksuiteStore` → Should be `TemplateStructureStore`
❌ `blocksuiteStore.ts` → Should be `templateStructureStore.ts`
❌ Events: `blocksuite-ready`, `blocksuite-store-ready`

---

## Goals

### Goal 1: Remove "BlockSuite" Naming

Rename everything to accurately reflect what we have:
- We're NOT using BlockSuite framework
- We HAVE a custom block-based templating system
- More accurate: "Template", "Structure", "Collaborative"

### Goal 2: Enhance Thread Block for CEL-Driven Rendering

Make the thread/collaborative block **fully flexible** with:

1. **Custom Fields** - Store arbitrary data (ratings, upvotes, productId, etc.)
2. **CEL-Driven Rendering** - Template-defined display (like forEach)
3. **Aggregates** - Compute stats (avgRating, total, sum, etc.)
4. **Sorting/Filtering** - CEL expressions to sort/filter items
5. **Actions** - Support upvote, downvote, like, increment operations
6. **Multiple Renderings** - Reviews, Q&A, group chat, Reddit threads

### Goal 3: Support Multiple Use Cases

**Current:** Only basic comments
**Target:** Reviews, ratings, Q&A, group chat, upvote systems, threaded discussions

---

## Proposed Architecture

### Enhanced HUML Schema

```huml
- ::
  type: "thread"
  name: "Product Reviews"
  collectionId: "reviews-prod-1"  # Unique collection ID

  # Custom input fields (dynamic form)
  inputFields::
    - ::
      type: "form-field-number"
      name: "rating"
      label: "Rating (1-5)"
      min: 1
      max: 5
      required: true
    - ::
      type: "form-field-text"
      name: "title"
      label: "Review Title"
      placeholder: "Summarize your experience"
    - ::
      type: "form-field-textarea"
      name: "content"
      label: "Your Review"
      mode: "markdown"
    - ::
      type: "form-field-checkbox"
      name: "wouldRecommend"
      label: "Would you recommend this product?"

  # Aggregates (computed from collection data)
  aggregates::
    avgRating: "{{items.map(i, i.rating).sum() / items.size()}}"
    totalReviews: "{{items.size()}}"
    fiveStarCount: "{{items.filter(i, i.rating == 5).size()}}"
    recommendPercent: "{{(items.filter(i, i.wouldRecommend).size() / items.size()) * 100}}"

  # Sorting/filtering
  sortBy: "{{-item.timestamp}}"  # Most recent first
  filterBy: "{{item.rating >= 3}}"  # Only 3+ stars

  # Header template (optional - shown above list)
  headerTemplate::
    blocks::
      - ::
        type: "section-container"
        name: "Review Summary"
        css: "background: #f8f9fa; padding: 20px; border-radius: 12px; margin-bottom: 20px;"
        blocks::
          - ::
            type: "heading"
            content: "Customer Reviews"
          - ::
            type: "text"
            content: "⭐ {{avgRating | toFixed(1)}} / 5.0 ({{totalReviews}} reviews)"
            css: "font-size: 20px; font-weight: 600;"
          - ::
            type: "text"
            content: "{{recommendPercent | toFixed(0)}}% would recommend"
            css: "color: #666;"

  # Item renderer (CEL-driven - like forEach)
  itemRenderer::
    blocks::
      - ::
        type: "section-container"
        name: "Review Card"
        css: "background: white; padding: 20px; border-radius: 12px; margin: 15px 0; border-left: 4px solid {{item.rating >= 4 ? '#4caf50' : '#ff9800'}};"
        blocks::
          - ::
            type: "section-container"
            name: "Review Header"
            css: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px;"
            blocks::
              - ::
                type: "text"
                content: "{{'⭐'.repeat(item.rating)}}"
                css: "font-size: 20px; color: #ff9800;"
              - ::
                type: "text"
                content: "{{new Date(item.timestamp).toLocaleDateString()}}"
                css: "font-size: 14px; color: #999;"

          - ::
            type: "heading"
            content: "{{item.title}}"
            css: "font-size: 20px; margin: 10px 0;"

          - ::
            type: "markdown-text"
            mode: "markdown"
            content: "{{item.content}}"

          - ::
            type: "text"
            content: "✓ Recommends this product"
            visible: "{{item.wouldRecommend}}"
            css: "color: #4caf50; font-weight: 600; margin-top: 10px;"

          - ::
            type: "section-container"
            name: "Review Footer"
            css: "display: flex; gap: 15px; margin-top: 15px; align-items: center;"
            blocks::
              - ::
                type: "text"
                content: "By {{item.author}}"
                css: "font-size: 14px; color: #667eea; font-weight: 600;"

              - ::
                type: "nav-button"
                content: "👍 Helpful ({{item.upvotes || 0}})"
                action: "collaborativeAction"
                actionType: "incrementField"
                collectionId: "reviews-prod-1"
                itemId: "{{item.id}}"
                field: "upvotes"
                amount: 1
                css: "background: #f0f0f0; padding: 5px 12px; border-radius: 6px; font-size: 14px;"

  # Footer template (optional - shown below list)
  footerTemplate::
    blocks::
      - ::
        type: "text"
        content: "Showing {{items.size()}} of {{totalReviews}} reviews"
        css: "text-align: center; color: #999; margin-top: 20px;"
```

### Enhanced Data Store

**Current:**
```typescript
// ThreadCommentsStore - Fixed schema
interface Comment {
  id: string;
  author: string;
  userId?: string;
  content: string;
  timestamp: number;
}
```

**Proposed:**
```typescript
// CollaborativeStore - Flexible schema
interface CollaborativeItem {
  id: string;
  author: string;
  userId?: string;
  timestamp: number;
  [key: string]: any;  // Allow arbitrary fields
}

class CollaborativeStore {
  // READ operations
  getItems(collectionId: string): CollaborativeItem[]
  getItem(collectionId: string, itemId: string): CollaborativeItem | null

  // WRITE operations
  addItem(collectionId: string, item: Omit<CollaborativeItem, 'id' | 'timestamp'>): void
  updateItem(collectionId: string, itemId: string, updates: Partial<CollaborativeItem>): void
  deleteItem(collectionId: string, itemId: string): void

  // ACTIONS
  incrementField(collectionId: string, itemId: string, field: string, amount: number): void
  // e.g., incrementField('reviews-prod-1', 'review-123', 'upvotes', 1)

  // COMPUTED
  aggregate(collectionId: string, aggregateFn: (items: CollaborativeItem[]) => any): any

  // Subscribe to changes
  subscribe(callback: () => void): () => void
}
```

### Enhanced Component

**Current:** `ThreadBlock.svelte` (hardcoded rendering)
**Proposed:** `CollaborativeBlock.svelte` (CEL-driven rendering)

**Capabilities:**
1. Parse `inputFields::` → Render dynamic form
2. Parse `headerTemplate::` → Render header with aggregates
3. Parse `itemRenderer::` → Render each item using BlockRenderer
4. Parse `footerTemplate::` → Render footer
5. Compute `aggregates::` → Expose to CEL context
6. Apply `sortBy` and `filterBy` → CEL expressions
7. Handle `collaborativeAction` → Execute actions (upvote, etc.)

**Loop Context for Items:**
```javascript
{
  item: currentItem,        // Current item data
  index: 0,                 // Index in list
  first: true,              // Is first item
  last: false,              // Is last item

  // Aggregates also available
  avgRating: 4.5,
  totalReviews: 42,
  // ... all computed aggregates
}
```

---

## Implementation Phases

### Phase 1: Rename Architecture ✅ (Preparation)

**Goal:** Remove misleading "BlockSuite" naming

**Files to Rename:**
```
src/lib/blocksuiteCoordinator.ts  → src/lib/templateCoordinator.ts
src/lib/blocksuiteStore.ts        → src/lib/templateStructureStore.ts
src/lib/threadCommentsStore.ts    → src/lib/collaborativeStore.ts
```

**Class Renames:**
```typescript
BlocksuiteCoordinator → TemplateCoordinator
BlocksuiteStore       → TemplateStructureStore
ThreadCommentsStore   → CollaborativeStore
```

**Event Renames:**
```typescript
'blocksuite-ready'              → 'template-ready'
'blocksuite-store-ready'        → 'template-structure-ready'
'thread-comments-store-ready'   → 'collaborative-store-ready'
```

**Interface Renames:**
```typescript
Comment            → CollaborativeItem
BlocksuiteConfig   → TemplateCoordinatorConfig
```

**Affected Files:**
- All imports of these stores
- All event listeners
- Component references (ThreadBlock.svelte, etc.)
- Type definitions

**Estimated Effort:** 2-3 hours (find/replace + testing)

---

### Phase 2: Enhance CollaborativeStore ✅ (Data Layer)

**Goal:** Support arbitrary fields and actions

**Changes to `collaborativeStore.ts`:**

1. **Update Interface:**
```typescript
interface CollaborativeItem {
  id: string;
  author: string;
  userId?: string;
  timestamp: number;
  [key: string]: any;  // NEW: Allow custom fields
}
```

2. **Add Action Methods:**
```typescript
class CollaborativeStore {
  // Existing
  addItem(collectionId: string, item: Omit<CollaborativeItem, 'id' | 'timestamp'>): void

  // NEW: Update operations
  updateItem(collectionId: string, itemId: string, updates: Partial<CollaborativeItem>): void {
    // Uses Yjs transactions to update specific fields
  }

  // NEW: Increment (for upvotes, likes, counts)
  incrementField(collectionId: string, itemId: string, field: string, amount: number): void {
    // Atomic increment using Yjs
    // item.upvotes = (item.upvotes || 0) + amount
  }

  // NEW: Delete (optional)
  deleteItem(collectionId: string, itemId: string): void {
    // Remove item from Y.Array
  }
}
```

3. **Ensure Yjs CRDT Safety:**
   - Use `Y.Array` operations (already done)
   - Atomic increments for counters
   - Proper transactions

**Testing:**
- Add item with custom fields (rating, title, wouldRecommend)
- Increment upvotes field
- Update item fields
- Verify CRDT merging works correctly

**Estimated Effort:** 3-4 hours

---

### Phase 3: Create CollaborativeBlock Component ✅ (Rendering Layer)

**Goal:** CEL-driven rendering like forEach

**New Component:** `src/lib/blocks/CollaborativeBlock.svelte`

**Structure:**
```svelte
<script lang="ts">
  import BlockRenderer from './BlockRenderer.svelte';
  import { evaluateValue, hasCEL } from '../../utils/celEvaluator';
  import { templateState } from '../templateState.svelte';

  interface Props {
    blockId: string;
    blockData: any;  // HUML block definition
    allBlocks: Map<string, any>;
  }

  let { blockId, blockData, allBlocks }: Props = $props();

  // 1. Get items from CollaborativeStore
  let items = $state<CollaborativeItem[]>([]);

  // 2. Compute aggregates (expose to CEL)
  let aggregates = $derived.by(() => {
    // Evaluate each aggregate CEL expression
    const aggs: Record<string, any> = {};
    if (blockData.aggregates) {
      for (const [key, expr] of Object.entries(blockData.aggregates)) {
        aggs[key] = evaluateValue(expr, { items });
      }
    }
    return aggs;
  });

  // 3. Sort and filter items
  let processedItems = $derived.by(() => {
    let result = [...items];

    // Filter
    if (blockData.filterBy) {
      result = result.filter((item, index) => {
        return evaluateValue(blockData.filterBy, { item, index });
      });
    }

    // Sort
    if (blockData.sortBy) {
      result.sort((a, b) => {
        const valA = evaluateValue(blockData.sortBy, { item: a });
        const valB = evaluateValue(blockData.sortBy, { item: b });
        return valA - valB;
      });
    }

    return result;
  });

  // 4. Handle form submission
  function submitItem(data: Record<string, any>) {
    collaborativeStore.addItem(blockData.collectionId, data);
  }

  // 5. Handle actions (upvote, etc.)
  function handleAction(action: any) {
    if (action.actionType === 'incrementField') {
      collaborativeStore.incrementField(
        action.collectionId,
        action.itemId,
        action.field,
        action.amount
      );
    }
  }
</script>

<!-- Header Template (optional) -->
{#if blockData.headerTemplate}
  {#each blockData.headerTemplate.blocks as headerBlock}
    <BlockRenderer
      block={headerBlock}
      loopContext={aggregates}
      {allBlocks}
    />
  {/each}
{/if}

<!-- Items List (CEL-driven rendering) -->
{#each processedItems as item, index}
  {#each blockData.itemRenderer.blocks as rendererBlock}
    <BlockRenderer
      block={rendererBlock}
      loopContext={{ item, index, first: index === 0, last: index === processedItems.length - 1, ...aggregates }}
      {allBlocks}
      onAction={handleAction}
    />
  {/each}
{/each}

<!-- Footer Template (optional) -->
{#if blockData.footerTemplate}
  {#each blockData.footerTemplate.blocks as footerBlock}
    <BlockRenderer
      block={footerBlock}
      loopContext={{ items: processedItems, ...aggregates }}
      {allBlocks}
    />
  {/each}
{/if}

<!-- Input Form (dynamic fields) -->
<form onsubmit={handleSubmit}>
  {#each blockData.inputFields || [] as field}
    <FormField blockData={field} />
  {/each}
  <button type="submit">Submit</button>
</form>
```

**Key Features:**
- Renders `headerTemplate` with aggregates in context
- Renders each item using `itemRenderer` (like forEach)
- Renders `footerTemplate` with aggregates
- Generates dynamic form from `inputFields`
- Handles `collaborativeAction` from buttons

**Testing:**
- Render product reviews with ratings
- Show aggregates (avgRating, totalReviews)
- Filter items (only 4+ stars)
- Sort items (most recent first)
- Upvote a review
- Add a new review

**Estimated Effort:** 6-8 hours

---

### Phase 4: Update BlockRenderer for Actions ✅

**Goal:** Support `collaborativeAction` from nav-button

**Changes to `BlockRenderer.svelte`:**

1. **Add action handler prop:**
```typescript
interface Props {
  block: any;
  children: any[];
  allBlocks: Map<string, any>;
  loopContext?: Record<string, any>;
  onAction?: (action: any) => void;  // NEW
}
```

2. **Pass to NavButton:**
```svelte
{:else if block.type === 'nav-button'}
  <NavButton
    blockData={evaluatedBlock}
    {loopContext}
    onAction={onAction}  <!-- NEW -->
  />
{/if}
```

**Changes to `NavButton.svelte`:**

1. **Handle collaborative actions:**
```typescript
function handleClick() {
  if (blockData.action === 'collaborativeAction') {
    // Evaluate CEL expressions in action data
    const action = {
      actionType: blockData.actionType,
      collectionId: blockData.collectionId,
      itemId: evaluateValue(blockData.itemId, loopContext),  // Resolve {{item.id}}
      field: blockData.field,
      amount: blockData.amount
    };
    onAction?.(action);
  } else if (blockData.action === 'setState') {
    // Existing state update logic
  }
}
```

**Testing:**
- Click upvote button
- Verify field increments
- Verify Yjs sync works
- Test with multiple viewers

**Estimated Effort:** 2-3 hours

---

### Phase 5: Update Template Guide ✅ (Documentation)

**Goal:** Document new collaborative block syntax

**Add to `HUML_TEMPLATE_GUIDE_V2.md`:**

New section: **"## 🤝 Collaborative Blocks (Thread)"**

Include:
- Data interaction patterns (5 types)
- Full syntax reference
- `inputFields::` definition
- `aggregates::` definition
- `sortBy` / `filterBy` CEL expressions
- `headerTemplate::` / `itemRenderer::` / `footerTemplate::`
- `collaborativeAction` syntax
- Multiple examples:
  - Product reviews with ratings
  - Q&A with upvotes
  - Group chat
  - Reddit-style threads

**Estimated Effort:** 3-4 hours

---

### Phase 6: Build Ecommerce Example ✅ (Integration Test)

**Goal:** Demonstrate all 5 interaction patterns working together

**Create:** `ecommerce_store.huml`

**Features:**
1. **Products** (content:: - Pattern 3: Publisher Broadcast)
2. **Shopping Cart** (user_content:: - Pattern 1: Viewer-Only)
3. **Past Orders** (user_content:: - Pattern 1: Viewer-Only)
4. **Checkout Form** (form_submissions - Pattern 2: Viewer Append)
5. **Product Reviews** (collaborative - Pattern 4: Public Collaboration)
   - Star ratings
   - Written reviews
   - "Would recommend" checkbox
   - Upvote reviews
   - Average rating display
   - Filter by rating

**Screens:**
- Home (product grid with add to cart)
- Product Detail (reviews with ratings)
- Cart (checkout)
- Orders (past orders list)
- Reviews (filtered/sorted display)

**Estimated Effort:** 4-6 hours

---

## Testing Strategy

### Unit Tests

**CollaborativeStore:**
- Add item with custom fields
- Increment field (upvotes)
- Update item
- Delete item
- Yjs CRDT merging

**CollaborativeBlock:**
- Parse inputFields
- Render headerTemplate with aggregates
- Render itemRenderer with loop context
- Apply sortBy/filterBy
- Handle actions

### Integration Tests

**Ecommerce Store:**
- Add product to cart → user_content updates
- Submit order → form_submissions appends
- Add review → collaborative doc updates
- Upvote review → field increments
- Multiple viewers → see same reviews
- Average rating → computed correctly

### Manual Tests

- Open 2 browser tabs (simulate 2 viewers)
- Add review in tab 1 → appears in tab 2
- Upvote in tab 2 → counter updates in tab 1
- Verify CRDT conflict resolution

---

## Future Work (Not in Current Plan)

### Pattern 5: Private Collaboration (Viewer ↔ Publisher)

**Use Cases:** Customer support, private inquiries, coaching

**Design:**
```huml
- ::
  type: "thread"
  name: "Support Chat"
  mode: "private"  # NEW
  collectionId: "support-{{userId}}"  # Per-user collection

  # Only this viewer and publisher can access
```

**Implementation:**
- New document type: `private_thread_doc_{userId}`
- Access control in CollaborativeStore
- Publisher UI to view all private threads
- Viewer sees only their own thread

**Estimated Effort:** 8-12 hours (future)

---

## Success Criteria

✅ **Phase 1 Complete:** No references to "BlockSuite" in codebase
✅ **Phase 2 Complete:** CollaborativeStore supports custom fields and actions
✅ **Phase 3 Complete:** CollaborativeBlock renders CEL-driven templates
✅ **Phase 4 Complete:** Actions work from nav-buttons
✅ **Phase 5 Complete:** Documentation updated
✅ **Phase 6 Complete:** Ecommerce store template working with all features

**Final Demo:**
- Ecommerce store with products, cart, orders, reviews
- Reviews show star ratings and upvotes
- Average rating computed and displayed
- Multiple viewers can interact in real-time
- All 4 interaction patterns demonstrated

---

## Risk Mitigation

**Risk:** Breaking existing templates
**Mitigation:** Backward compatibility - old thread blocks still work with default rendering

**Risk:** Performance with large collections
**Mitigation:** Implement pagination/virtualization if needed

**Risk:** CEL expression complexity
**Mitigation:** Provide simple examples in docs, error messages

**Risk:** Yjs CRDT conflicts
**Mitigation:** Use atomic operations (Y.Array.push, increment), test merging

---

## Timeline Estimate

| Phase | Effort | Dependencies |
|-------|--------|--------------|
| Phase 1: Rename | 2-3 hours | None |
| Phase 2: Store | 3-4 hours | Phase 1 |
| Phase 3: Component | 6-8 hours | Phase 2 |
| Phase 4: Actions | 2-3 hours | Phase 3 |
| Phase 5: Docs | 3-4 hours | Phase 4 |
| Phase 6: Example | 4-6 hours | Phase 5 |
| **Total** | **20-28 hours** | Sequential |

**Suggested Schedule:** 1-2 weeks (if working part-time)

---

## Questions for Discussion

1. Should we support nested threading (Reddit-style) in Phase 3, or defer to future?
2. Should we add pagination to collaborative blocks, or rely on filterBy?
3. Should aggregates be cached, or recomputed on every render?
4. Should we add rate limiting to prevent spam (upvote throttling)?
5. Do we need moderation features (delete, hide, report)?

---

## Conclusion

This plan transforms the thread block from a **single-purpose comment system** into a **flexible collaborative data framework** that can handle:

- Product reviews with ratings
- Q&A with upvotes
- Group chat
- Discussion forums
- Reddit-style threads
- Any custom collaborative use case

All powered by **CEL expressions** and **Yjs CRDTs**, maintaining real-time sync and conflict-free merging.

The architecture already has **5 distinct interaction patterns** (4 implemented, 1 future), enabling complex applications like ecommerce, blogs, dashboards, and education platforms.
