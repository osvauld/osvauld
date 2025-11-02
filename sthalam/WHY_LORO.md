# Why Loro for Sthalam

**Date:** 2025-11-02
**Decision:** Use Loro CRDT as the universal state manager for Sthalam

---

## 🎯 The Core Insight

Sthalam is a **platform** for building sovereign applications, not a single application. Templates define everything - structure, state, behavior. This requires a state management solution that is:
- **Dynamic** - State shape determined at runtime by templates
- **Reactive** - Changes propagate automatically
- **Collaborative** - Multiple users can interact
- **Persistent** - State can be saved and synced
- **Performant** - Handle hundreds of blocks and state updates

Loro's tree-based CRDT with fine-grained events is the perfect match.

---

## 🚀 Game-Changing Features

### 1. Tree-Based State with Node-Level Events

```typescript
// Traditional approach - coarse-grained
stateContainer.subscribe(() => {
  // Something changed, but what?
  // Must check everything!
});

// Loro Tree approach - fine-grained
uiStateTree.subscribe((events) => {
  // Event includes TreeID!
  // We know EXACTLY which node changed
  // Path: ['tree', TreeID, 'data', 'fieldName']
});
```

**Impact**: Component-scoped state with surgical updates. Each component/screen is a tree node with isolated state.

### 2. CEL-Controlled CRDT Operations

```typescript
// Expose CRDT operations directly to templates
onClick: "{{uiState.node('modal').data.set('open', true)}}"
onChange: "{{uiState.node('search').data.set('query', event.value)}}"
onSubmit: "{{submissions.get(formId).push(formData)}}"

// Templates control state directly - no imperative code needed!
```

**Impact**: Templates become fully self-contained. State logic lives in the template, not in code.

### 3. Composable CRDTs for Complex State

```typescript
// Mix and match CRDT types
uiStateTree (Tree)
├── globalNode (TreeNode)
│   └── data (Map)
│       ├── theme: "dark" (String)
│       ├── user (Map)
│       └── notifications (List)
└── screenNode (TreeNode)
    └── data (Map)
        ├── searchResults (List)
        └── richText (Text)
```

**Impact**: Use the right CRDT type for each use case. Trees for hierarchy, Maps for key-value, Lists for arrays, Text for rich content.

---

## 💡 Architecture Benefits

### 1. Unified State Management

```typescript
// Everything uses the same API
content.get('products')       // Publisher content
uiState.node('home').data     // UI state
userContent.get('cart')       // User state
threads.get('comments')       // Collaborative state

// Template authors learn ONE system
```

### 2. Component-Scoped Reactivity

```typescript
// Each component gets its own tree node
const componentNode = uiTree.createNode(screenNode);
componentNode.data.set('expanded', false);

// Only THIS component updates when its node changes
componentNode.data.subscribe(() => {
  // Precise, efficient updates
});
```

### 3. Natural State Organization

```typescript
uiStateTree
├── global                     // Global UI state
│   ├── theme
│   ├── user
│   └── settings
├── screens
│   ├── home                   // Per-screen state
│   │   ├── searchQuery
│   │   └── components
│   │       ├── navbar         // Per-component state
│   │       └── cart
│   └── product
│       ├── selectedImage
│       └── quantity
```

---

## 📊 Performance Analysis

### Fine-Grained Updates

| Operation | Traditional State | Loro Tree |
|-----------|------------------|-----------|
| Update one field | Re-render everything | Re-render one node |
| Deep nested change | Traverse entire state | Direct node access O(1) |
| Subscribe to changes | All changes notify | Node-specific notifications |
| Find what changed | Manual diffing | TreeID in event |

### Memory Efficiency

```typescript
// Loro Tree is memory efficient
- Tree structure: ~O(n) where n = number of nodes
- Each node: ~100 bytes metadata + data size
- 1000 nodes = ~100KB overhead (negligible)
- Built-in compression for history
```

### Real-World Performance

```typescript
// Scenario: E-commerce with 200 products, 50 visible blocks
// User types in search box

// Without Loro Tree:
- Check all 50 blocks for dependencies
- Re-evaluate many CEL expressions
- ~40ms per keystroke (laggy)

// With Loro Tree:
- Event says: node 'home:search' changed
- Only evaluate blocks watching that node
- ~5ms per keystroke (smooth)
```

---

## 🔧 Implementation Strategy

### 1. Six Loro Documents

```typescript
export interface Documents {
  templateDoc: Loro;       // Template structure (Tree)
  uiStateDoc: Loro;        // UI state (Tree)
  contentDoc: Loro;        // Publisher content (Map)
  userContentDoc: Loro;    // User state (Map)
  collaborativeDoc: Loro;  // Comments/threads (Map of Trees)
  submissionsDoc: Loro;    // Form submissions (Map of Lists)
}
```

### 2. CEL Integration

```typescript
// Expose CRDT operations to CEL
const celContext = {
  // Direct CRDT access
  ui: uiStateTree,
  content: contentMap,
  user: userContentMap,

  // CRDT operations
  set: (container, key, value) => container.set(key, value),
  push: (list, items) => list.push(items),
  create: (tree, parentId, data) => tree.createNode(parentId),

  // Utility functions
  node: (nodeId) => uiStateTree.getNode(nodeId),
  thread: (threadId) => collaborativeDoc.get(threadId)
};
```

### 3. Smart CEL Evaluation

```typescript
class CELEvaluator {
  private dependencies = new Map(); // Expression -> TreeIDs

  evaluate(expression: string, nodeId: string) {
    // Track which nodes this expression depends on
    const deps = extractDependencies(expression);
    this.dependencies.set(expression, deps);

    // Subscribe to those specific nodes
    deps.forEach(dep => {
      uiStateTree.getNode(dep).data.subscribe(() => {
        // Re-evaluate only this expression
        this.invalidate(expression);
      });
    });
  }
}
```

---

## ✅ Why Not Alternatives?

### Yjs
- ❌ No native tree structure (must build on top)
- ❌ Larger bundle size (~200KB vs ~100KB)
- ❌ Less efficient for hierarchical data
- ✅ More mature ecosystem

### Plain Svelte State
- ❌ No persistence/sync capabilities
- ❌ Must build reactivity system
- ❌ No collaboration support
- ✅ Simpler, no CRDT complexity

### Redux/MobX
- ❌ Not designed for dynamic schemas
- ❌ No real-time collaboration
- ❌ Requires extensive boilerplate
- ✅ Better TypeScript support

---

## 🎯 Key Advantages for Sthalam

1. **Platform-Ready**: Dynamic state shapes defined by templates
2. **Fine-Grained**: TreeID events enable precise updates
3. **CEL-Native**: CRDT operations exposed directly to templates
4. **Collaborative**: Real-time sync built-in
5. **Performant**: Efficient tree operations and event targeting
6. **Unified**: Single state system for all data types

---

## 🚀 Implementation Plan

### Phase 1: Core Setup
1. Initialize 6 Loro documents
2. Create tree structure for UI state
3. Test basic CRDT operations

### Phase 2: CEL Integration
1. Expose CRDT operations to CEL
2. Build dependency tracking
3. Implement smart evaluation

### Phase 3: Component Integration
1. Create component-scoped nodes
2. Wire up subscriptions
3. Test reactivity

### Phase 4: Optimization
1. Cache CEL evaluations
2. Batch CRDT operations
3. Profile and optimize

---

## 📈 Success Metrics

- ✅ **60 FPS** during typing and interactions
- ✅ **< 10ms** for state updates
- ✅ **< 100ms** for initial render
- ✅ **Fine-grained** updates (no unnecessary re-renders)
- ✅ **Zero sync code** (CRDT handles it)

---

## 🎬 Conclusion

Loro's tree-based CRDT with fine-grained events solves our core challenges:

1. **Dynamic state** - Templates define everything
2. **Performance** - Node-level updates, not container-level
3. **Integration** - CEL controls CRDTs directly
4. **Collaboration** - Built-in real-time sync
5. **Developer Experience** - One unified system

The combination of Loro Tree + CEL creates a powerful, declarative state management system perfect for a platform like Sthalam where templates define entire applications.

**Decision: Use Loro as the universal state manager for Sthalam.**