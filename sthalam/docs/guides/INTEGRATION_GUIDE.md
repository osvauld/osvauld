# Integration Guide - Using New HUML Parser & CEL Evaluator

## Problem
You're seeing this error:
```
Failed to load resource: the server responded with a status of 404 (Not Found) (huml_eval.js, line 0)
```

## Why?
Old code is trying to load `huml_eval.js` which doesn't exist anymore.

## Solution
Use the new WASM-based services we just built!

---

## 1. Load the Services (Once at App Startup)

```typescript
import { loadCELEvaluator } from './lib/services/celEvaluator';
import { loadHUMLParser } from './lib/services/humlParser';

// In your main App.svelte or entry point:
async function initializeApp() {
  await Promise.all([
    loadCELEvaluator(),    // Loads cel_eval.js
    loadHUMLParser()       // Loads huml_parser.js
  ]);
  console.log('✅ Services loaded');
}

onMount(initializeApp);
```

---

## 2. Parse HUML Template

```typescript
import { parseHUML } from './lib/services/humlParser';

// Load HUML file (from file upload, fetch, etc.)
const humlString = await loadHUMLFile();

// Parse it
const template = parseHUML(humlString);

console.log(template);
// {
//   name: "Simple Test Template",
//   documents: { ... },
//   computed: { ... },
//   ui: { ... }
// }
```

---

## 3. Evaluate CEL Expressions

```typescript
import { evaluateCEL, interpolateCEL } from './lib/services/celEvaluator';

const context = {
  user: { name: 'Alice', age: 25 },
  counter: 5
};

// Evaluate expression
const result = evaluateCEL('user.name + " is " + string(user.age)', context);
// Result: "Alice is 25"

// Interpolate template string
const greeting = interpolateCEL('Hello, {{ user.name }}!', context);
// Result: "Hello, Alice!"
```

---

## 4. Render UI with BlockRenderer

```svelte
<script lang="ts">
import { onMount } from 'svelte';
import { loadCELEvaluator } from './lib/services/celEvaluator';
import { loadHUMLParser, parseHUML } from './lib/services/humlParser';
import BlockRenderer from './renderer/BlockRenderer.svelte';

let template: any = $state(null);
let context = $state({
  user: { name: 'Alice', age: 25 },
  counter: 0
});

onMount(async () => {
  // 1. Load services
  await Promise.all([loadCELEvaluator(), loadHUMLParser()]);

  // 2. Load and parse HUML
  const response = await fetch('simple_test.huml');
  const humlString = await response.text();
  template = parseHUML(humlString);

  console.log('Template loaded:', template);
});

function handleAction(action: string, params?: any) {
  console.log('Action:', action, params);
}

function handleStateChange(field: string, value: any) {
  console.log('State change:', field, '=', value);
  // Update context
  const keys = field.split('.');
  let obj = context;
  for (let i = 0; i < keys.length - 1; i++) {
    obj = obj[keys[i]];
  }
  obj[keys[keys.length - 1]] = value;
  context = { ...context }; // Trigger reactivity
}
</script>

{#if template}
  {#each template.ui.viewer as screen}
    <BlockRenderer
      block={screen}
      {context}
      onAction={handleAction}
      onStateChange={handleStateChange}
    />
  {/each}
{:else}
  <div>Loading template...</div>
{/if}
```

---

## 5. Complete Example with Tauri File Loading

```svelte
<script lang="ts">
import { onMount } from 'svelte';
import { open } from '@tauri-apps/plugin-dialog';
import { readTextFile } from '@tauri-apps/plugin-fs';
import { loadCELEvaluator } from './lib/services/celEvaluator';
import { loadHUMLParser, parseHUML } from './lib/services/humlParser';
import BlockRenderer from './renderer/BlockRenderer.svelte';

let template: any = $state(null);
let context = $state({
  user: { name: 'Alice', age: 25 },
  counter: 0
});
let error: string | null = $state(null);

onMount(async () => {
  try {
    // Load WASM services
    await Promise.all([
      loadCELEvaluator(),
      loadHUMLParser()
    ]);
    console.log('✅ Services loaded');
  } catch (err) {
    error = `Failed to load services: ${err}`;
    console.error(error);
  }
});

async function loadTemplate() {
  try {
    // Open file dialog
    const file = await open({
      multiple: false,
      filters: [{ name: 'HUML', extensions: ['huml'] }]
    });

    if (!file) return;

    // Read file
    const humlString = await readTextFile(file);

    // Parse HUML
    template = parseHUML(humlString);
    error = null;

    console.log('✅ Template loaded:', template.name);
  } catch (err) {
    error = `Failed to load template: ${err}`;
    console.error(error);
  }
}

function handleAction(action: string, params?: any) {
  if (action === 'setState') {
    handleStateChange(params.field, params.value);
  }
  // Handle other actions...
}

function handleStateChange(field: string, value: any) {
  // Parse field path and update context
  const keys = field.split('.');
  const newContext = { ...context };
  let obj: any = newContext;

  for (let i = 0; i < keys.length - 1; i++) {
    if (!obj[keys[i]]) obj[keys[i]] = {};
    obj = obj[keys[i]];
  }

  obj[keys[keys.length - 1]] = value;
  context = newContext;
}
</script>

<div class="app">
  {#if error}
    <div class="error">{error}</div>
  {/if}

  <button onclick={loadTemplate}>Load HUML Template</button>

  {#if template}
    <h2>{template.name}</h2>
    {#each template.ui.viewer as screen}
      <BlockRenderer
        block={screen}
        {context}
        onAction={handleAction}
        onStateChange={handleStateChange}
      />
    {/each}
  {/if}
</div>

<style>
  .error {
    background: #fee2e2;
    color: #ef4444;
    padding: 1rem;
    border-radius: 4px;
    margin-bottom: 1rem;
  }
</style>
```

---

## 6. Update Your Existing Files

### Replace Old Imports

**OLD:**
```typescript
import { evaluateExpression } from '../shared/humlEvaluator';
```

**NEW:**
```typescript
import { evaluateCEL } from '../lib/services/celEvaluator';
```

### Files to Update

1. `src/shared/blocks/BlockRenderer.svelte`
2. `src/publisher/PublisherApp.svelte`
3. `src/shared/blocks/CanvasPattern.svelte`

---

## 7. Quick Test

Test the new setup:

```typescript
import { loadCELEvaluator, evaluateCEL } from './lib/services/celEvaluator';
import { loadHUMLParser, parseHUML } from './lib/services/humlParser';

async function test() {
  // Load services
  await Promise.all([loadCELEvaluator(), loadHUMLParser()]);

  // Test CEL evaluator
  const result = evaluateCEL('5 + 3', {});
  console.log('5 + 3 =', result); // 8

  // Test HUML parser
  const huml = `
    name: "Test"
    items::
      - id: 1
        title: "First"
      - id: 2
        title: "Second"
  `;

  const parsed = parseHUML(huml);
  console.log('Parsed:', parsed);
  // { name: "Test", items: [{ id: 1, title: "First" }, ...] }
}
```

---

## Summary

✅ Use `src/lib/services/celEvaluator.ts` instead of old evaluator
✅ Use `src/lib/services/humlParser.ts` to parse HUML files
✅ Load both services at app startup
✅ Use `simple_test.huml` to test

**No more 404 errors!** The new WASM modules are in `public/`:
- `public/cel_eval.js` (CEL evaluator)
- `public/huml_parser.js` (HUML parser)
