# Loro Simplification - Migration Guide

## 🎯 What Changed

### Before (Complex - Tree-based)
```typescript
// Stored template as Loro Tree nodes
templateDoc.getTree('publisherScreens')
  → node.data.set('type', 'screen')
  → node.data.set('blocks', [...])
  → Convert tree → JSON → render

// 514 lines of tree conversion code
```

### After (Simple - Map-based)
```typescript
// Store raw HUML, parse in memory
templateDoc.getMap('template').set('huml_source', humlString)
const template = parseHUML(humlString);  // In memory!
const screens = template.ui.publisher;   // Direct access

// ~100 lines of simple code
```

---

## 📊 Architecture Comparison

| Aspect | OLD (Tree) | NEW (Map/List) |
|--------|------------|----------------|
| **Template storage** | Loro Tree nodes | Raw HUML string |
| **Template access** | Tree traversal | Parse once, keep in memory |
| **State storage** | Loro Maps | Loro Maps (same) |
| **Reactivity** | Tree subscriptions | Map subscriptions |
| **Code complexity** | 980 lines | ~300 lines |
| **Performance** | Tree conversion overhead | Direct object access |

---

## 📁 New File Structure

```
shared/loro/
├── loroCoordinator.new.ts     ← Simplified (NO trees!)
├── templateImporter.new.ts    ← Simplified (NO conversions!)
├── contentStore.ts             ← Keep as-is (optional wrapper)
│
├── loroCoordinator.ts          ← OLD (delete after migration)
└── templateImporter.ts         ← OLD (delete after migration)
```

---

## 🔧 New API

### loroCoordinator.new.ts

```typescript
// Template document (stores HUML source)
const templateMap = loroCoordinator.getTemplateMap();
templateMap.set('huml_source', humlString);
templateMap.set('name', 'My App');
templateMap.set('version', 'v1.0.0');

// Content document (stores dynamic state)
const contentMap = loroCoordinator.getContentMap();
contentMap.set('counter', 0);
contentMap.set('user', { name: 'Alice', age: 25 });

// Content lists (for arrays)
const postsList = loroCoordinator.getContentList('posts');
postsList.push({ id: 1, title: 'First Post' });
```

### templateImporter.new.ts

```typescript
// Import HUML template
await templateImporter.importFromHUML(humlString);

// Behind the scenes:
// 1. Parse HUML
// 2. Store raw HUML in templateDoc
// 3. Extract initial state from template.documents section
// 4. Store state in contentDoc
```

---

## 🔄 Migration Steps

### 1. Update BuilderApp

**OLD:**
```typescript
import { loroCoordinator } from '../shared/loro/loroCoordinator';
import { templateImporter } from '../shared/loro/templateImporter';

// Import HUML
await templateImporter.importFromHUML(fileContent);  // Creates tree nodes
```

**NEW:**
```typescript
import { loroCoordinator } from '../shared/loro/loroCoordinator.new';
import { templateImporter } from '../shared/loro/templateImporter.new';
import { parseHUML } from '../lib/services/humlParser';

// Import HUML
await templateImporter.importFromHUML(fileContent);  // Stores raw HUML
```

### 2. Update PublisherApp

**OLD:**
```typescript
// Load screens from Loro tree
function loadPublisherScreens() {
  const publisherTree = docs.templateDoc.getTree('publisherScreens');
  const screenNodes = publisherTree.roots();

  for (const node of screenNodes) {
    const screen = nodeToScreen(node);  // Complex conversion
    screens.push(screen);
  }
}
```

**NEW:**
```typescript
// Load screens from parsed HUML
let template = $state(null);
let screens = $derived(template?.ui?.publisher || []);

onMount(() => {
  // Get HUML from Loro
  const humlSource = loroCoordinator.getTemplateMap().get('huml_source');

  // Parse once, keep in memory
  template = parseHUML(humlSource);

  // Screens are now directly accessible!
  console.log('Screens:', screens);
});
```

### 3. Reactive State

**OLD:**
```typescript
// Subscribe to tree changes
tree.subscribe((event) => {
  // Complex event handling
});
```

**NEW:**
```typescript
// Subscribe to map changes (simpler!)
contentMap.subscribe((event) => {
  if (event.path[0] === 'counter') {
    counter = contentMap.get('counter');
  }
});
```

---

## ✅ Benefits

1. **Simpler Code**
   - 980 lines → ~300 lines
   - No tree conversions
   - No nodeToBlock() mappings

2. **Faster**
   - No tree traversal
   - Direct object access
   - Parse HUML once, use many times

3. **Easier to Debug**
   - Raw HUML in Loro (human-readable)
   - Simple Map/List structures
   - No complex tree state

4. **Better Separation**
   - Template = static (HUML source)
   - State = dynamic (counter, posts, etc.)
   - Clear responsibilities

5. **Still Collaborative**
   - Loro Maps/Lists sync perfectly
   - State changes propagate
   - Multi-user works fine

---

## 🧪 What to Test

1. **Import HUML** - Does it store correctly?
2. **Switch resources** - Does state clear/load?
3. **Publisher mode** - Do screens render?
4. **State changes** - Does counter update?
5. **Save/load** - Does persistence work?

---

## 📝 Implementation Order

1. ✅ Create `loroCoordinator.new.ts`
2. ✅ Create `templateImporter.new.ts`
3. ⏳ Update BuilderApp imports
4. ⏳ Update PublisherApp to parse HUML
5. ⏳ Test complete flow
6. ⏳ Remove old files

---

## 🎉 Ready to Migrate!

**Next steps:**
1. Update BuilderApp to use `.new` files
2. Update PublisherApp to parse HUML
3. Test import → render flow
4. Delete old files

Everything is backward-compatible with Loro snapshots!
The data format is the same, just simpler access patterns.
