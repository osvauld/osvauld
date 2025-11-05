# Sthalam Architecture Guide

**For Developers New to This Paradigm**

**Last Updated**: 2025-11-04

---

## The Big Picture

Sthalam is a **template-based application framework** where users create apps using HUML templates instead of writing code. Think of it like this:

- **Traditional**: User writes JavaScript/React/Svelte code
- **Sthalam**: User writes HUML templates, Sthalam renders them

```
┌─────────────────────────────────────────────────────────────┐
│                      HUML Template                          │
│  (User-created, like WordPress theme or Notion template)   │
└─────────────────────┬───────────────────────────────────────┘
                      │
          ┌───────────▼───────────┐
          │   Sthalam Runtime     │
          │                       │
          │  1. Parse HUML        │◄──── HUML Parser
          │  2. Evaluate CEL      │◄──── CEL Evaluator (✅ DONE!)
          │  3. Render UI         │◄──── Block Renderer
          │  4. Handle Actions    │◄──── Action System
          │  5. Sync State (Loro) │◄──── Loro CRDTs
          └───────────┬───────────┘
                      │
          ┌───────────▼───────────┐
          │    Svelte 5 App       │
          │  (Reactive UI Layer)  │
          └───────────────────────┘
```

---

## Two Separate Systems

### 1. **CEL Evaluator** (Expression Engine) ✅ DONE

**What it does**: Evaluates runtime logic/expressions
**Where we are**: Fully implemented in OCaml, 28 functions, 30 tests passing

**Examples**:
```cel
// Expression evaluation
posts.filter(p => p.author == currentUser)
draftTitle != '' && draftBody != ''
wordCount / 200
```

**Files**:
- `/huml-evaluator-ocaml/cel/` - OCaml CEL implementation
- Will be compiled to WASM for use in browser

---

### 2. **HUML System** (Template Structure) ⚠️ PARTIALLY DONE

**What it does**: Parses templates, renders UI, handles state/actions
**Where we are**: Some blocks implemented, many missing

This is the main focus of your question! Let me break it down:

---

## HUML System Components

### A. **HUML Parser** (Status: ?)

Reads `.huml` files and converts them to JSON/objects.

**Example Input**:
```huml
name: "Blog App"

documents::
  appState:
    message:
      type: string
      initial: "Hello!"

ui::
  viewer::
    - type: screen
      blocks::
        - type: text
          content: "{{ message }}"
```

**Example Output** (JSON):
```json
{
  "name": "Blog App",
  "documents": {
    "appState": {
      "message": {
        "type": "string",
        "initial": "Hello!"
      }
    }
  },
  "ui": {
    "viewer": [
      {
        "type": "screen",
        "blocks": [
          {
            "type": "text",
            "content": "{{ message }}"
          }
        ]
      }
    ]
  }
}
```

**Question**: Do we have a HUML parser? Where is it?

---

### B. **Block Renderer** (Status: PARTIALLY IMPLEMENTED)

Renders UI blocks as Svelte components.

**File**: `/src/shared/blocks/BlockRenderer.svelte`

#### **✅ Currently Implemented Blocks**:

| Block Type | Status | Example |
|------------|--------|---------|
| `section-container` | ✅ | Container with child blocks |
| `heading` | ✅ | H1 heading |
| `text` | ✅ | Paragraph text |
| `form` | ✅ | Form wrapper |
| `form-field-textarea` | ✅ | Multi-line input |
| `form-field-text` | ✅ | Single-line input |
| `nav-button` | ✅ | Button with action |
| `canvas-pattern` | ✅ | WebGL canvas |

#### **❌ Missing Blocks (From STHALAM_DSL.md)**:

**Layout Blocks**:
- `screen` - Top-level container (**critical for navigation!**)
- `container` - Generic container with layout options
- `section` - Semantic section

**Content Blocks**:
- `label` - Form label
- `image` - Image display
- `video` - Video player
- `link` - Navigation link

**Input Blocks**:
- `input` - General text input (we have form-field-text, but not standalone)
- `textarea` - Multi-line input (we have form-field-textarea, but not standalone)
- `checkbox` - Boolean checkbox
- `select` - Dropdown menu
- `radio` - Radio button group

**Action Blocks**:
- `button` - General button (we have nav-button, need to check if it's the same)

**Special Blocks**:
- `canvas` - General canvas (we have canvas-pattern, need to check if it's the same)
- `modal` - Modal dialog (**critical!**)

---

### C. **Control Flow** (Status: UNKNOWN)

Control flow keywords that affect rendering:

| Keyword | Purpose | Status | Example |
|---------|---------|--------|---------|
| `when` | Conditional show/hide | ❓ | `when: isLoading` |
| `if/then/else` | Branching | ❓ | `if: "posts.size() > 0"` |
| `match/cases` | Pattern matching | ❓ | `match: currentView` |
| `forEach/as` | Iteration | ✅? | `forEach: posts` |
| `key` | List reconciliation | ❓ | `key: post.id` |
| `limit/offset` | Pagination | ❓ | `limit: 10` |

**Question**: Are these implemented in BlockRenderer? I saw `forEach` in the code, but not the others.

---

### D. **Action System** (Status: UNKNOWN)

Handles user interactions and state updates.

#### **Built-in Actions (From STHALAM_DSL.md)**:

| Action | Purpose | Example | Status |
|--------|---------|---------|--------|
| `navigate` | Navigate to screen | `action: navigate, params: {screen: posts}` | ❓ |
| `openModal` | Open modal dialog | `action: openModal, params: {modal: confirm-delete}` | ❓ |
| `closeModal` | Close modal | `action: closeModal` | ❓ |
| `setState` | Update state field | `action: setState, params: {field: view, value: grid}` | ❓ |

**Custom Actions**:
- Template authors can define custom actions like `publishPost`, `deletePost`
- These need to be handled by the runtime

**Question**: Where are actions handled? Is there an action dispatcher?

---

### E. **State Management** (Status: USES LORO)

**Documents** define the state schema:

```huml
documents::
  appState:
    message:
      type: string
      initial: "Hello!"

    counter:
      type: number
      initial: 0
```

**Computed Values** define derived state:

```huml
computed::
  viewer:
    doubledCounter:
      type: number
      expr: "counter * 2"
      depends::
        - appState.counter
```

**State is stored in Loro CRDTs** for real-time collaboration.

**Question**: How are documents and computed values initialized? Where's the state initialization code?

---

### F. **Navigation System** (Status: CRITICAL - UNKNOWN)

Sthalam apps are **SPAs with multiple screens**:

```huml
ui::
  viewer::
    - type: screen
      name: home
      blocks::
        - type: text
          content: "Home Screen"
        - type: button
          content: "Go to Posts"
          action: navigate
          params::
            screen: posts

    - type: screen
      name: posts
      blocks::
        - type: text
          content: "Posts Screen"
```

**Critical Questions**:
1. How are screens defined and registered?
2. How does `navigate` action work?
3. Is there a router/navigation state?
4. How are route parameters passed?

---

## What We Need to Build/Document

### **High Priority** (Blocking Features)

1. **`screen` block type** - Without this, we can't have multiple screens!
2. **Navigation system** - `navigate` action, route params, active screen tracking
3. **Modal system** - `modal` block type, `openModal`/`closeModal` actions
4. **Control flow** - Ensure `when`, `if/then/else`, `match/cases` all work

### **Medium Priority** (Core Functionality)

5. **Missing block types**:
   - `container` (with layout options: flex/grid)
   - `label`, `image`, `video`
   - `input`, `textarea`, `checkbox`, `select`, `radio` (standalone, not just in forms)
   - `link`

6. **Action system improvements**:
   - Document how actions are dispatched
   - Implement built-in actions: `setState`, `navigate`, `openModal`, `closeModal`
   - Custom action registration/handling

7. **State initialization**:
   - Parse `documents::` section
   - Initialize Loro CRDTs with correct schema
   - Parse `computed::` section
   - Set up reactive dependencies

### **Low Priority** (Nice to Have)

8. **Advanced control flow**:
   - `key` for list reconciliation
   - `limit`/`offset` for pagination

9. **Validation**:
   - Input validation with `validate` expressions
   - Error messages

---

## Recommended Action Plan

### **Step 1: Document What Exists** 📝

Create a document that lists:
- ✅ What block types are implemented (and where)
- ✅ What control flow works (with examples)
- ✅ What actions are handled (and how)
- ✅ How state is managed (initialization, updates, reactivity)

**I can help you audit the codebase to create this!**

### **Step 2: Priority Implementation** 🚀

Based on Step 1, implement in this order:

1. **Screen + Navigation** (required for multi-screen apps)
2. **Modals** (commonly needed UI pattern)
3. **Missing block types** (basic UI building blocks)
4. **Control flow** (if/when/forEach completeness)

### **Step 3: Test with Real Template** ✅

Create a complete `.huml` template (like the blog example in STHALAM_DSL.md) and verify:
- All block types render correctly
- Navigation works
- Modals open/close
- Actions dispatch correctly
- State updates reactively

---

## How to Think About This

Coming from traditional web development, here's the mental model:

### **Traditional React/Svelte**:
```jsx
// User writes this code
function BlogPost({ post }) {
  return (
    <div>
      <h1>{post.title}</h1>
      <p>{post.body}</p>
      <button onClick={() => deletePost(post.id)}>Delete</button>
    </div>
  );
}
```

### **Sthalam HUML**:
```huml
# User writes this template
- type: container
  blocks::
    - type: heading
      content: "{{ post.title }}"

    - type: text
      content: "{{ post.body }}"

    - type: button
      content: "Delete"
      action: deletePost
      params::
        postId: "{{ post.id }}"
```

**Your job** as the Sthalam developer is to:
1. Parse the HUML
2. Evaluate the CEL expressions (✅ DONE!)
3. Render the blocks as Svelte components
4. Handle the actions
5. Manage the state

---

## Next Steps

**Would you like me to**:

1. **Audit the existing codebase** to document what's already implemented?
2. **Create a comprehensive keyword reference** mapping all HUML keywords to their implementation status?
3. **Design the missing components** (screen system, navigation, modals)?
4. **Help implement the highest priority features** (screens + navigation)?

Let me know what would be most helpful! 🚀
