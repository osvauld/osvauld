# HUML Template Guide (Accurate - 2025-01-12)

**Status:** ✅ Reflects actual implementation
**Version:** Sthalam v0.1.0
**Last Validated:** 2025-01-12
**Last Updated:** Added collaborativeState documentation and forEach clarifications

---

## Overview

This guide documents the **actually implemented** HUML template features based on [HUML v0.1.0 specification](https://huml.io/specifications/v0-1-0/). This is the authoritative guide for writing working Sthalam templates.

⚠️ **NOTE:** This guide only documents working features. See implementation status at the end.

---

## CRITICAL SYNTAX RULES (HUML v0.1.0)

### 1. String Quoting
- **ALL string values MUST be double-quoted**: `type: "screen"`, `action: "setState"`
- Numbers are unquoted: `level: 1`, `initial: 0`
- Booleans are unquoted: `isEntryPoint: true`

### 2. List Dictionary Syntax
List items that are dictionaries use `- ::` (not just `-`):
```yaml
blocks::
  - ::
    type: "heading"
    content: "Hello"
  - ::
    type: "text"
    content: "World"
```

**IMPORTANT:** Control flow blocks also need `- ::`:
```yaml
# If/else blocks
- ::
  if: "${ condition }"
  then::
    - ::
      type: "text"

# Match/cases blocks and cases
- ::
  match: "${ value }"
  cases::
    - ::
      value: "option1"
```

### 3. Indentation
- Exactly **2 spaces** per indentation level
- After `key::`, list items are indented **+2 spaces** from the key
- No tabs, no trailing spaces

### 4. Expression Syntax

**Two types of expressions:**

#### Pure CEL Expressions (use `${ }`)
Use `${ }` markers for **expressions that return typed values** (booleans, numbers, etc):
```yaml
disabled: "${ !isFormValid }"
when: "${ counter > 5 && showAdvanced }"
```

**Properties that take pure expressions:**
- `disabled` - Boolean expression for disabled state
- `when` - Boolean expression for conditional rendering
- Computed values - Any CEL expression
- `stateUpdates` values - Expressions for state updates

#### String Interpolation (use `{{ }}`)
Use `{{ }}` markers when **building strings with dynamic values**:
```yaml
content: "Hello {{userName}}, you have {{itemCount}} items"
css: "opacity: {{isValid ? 1 : 0.5}}; color: {{theme.color}}"
```

**Key difference:**
- `${ expr }` → Returns typed value (true, 42, "result")
- `{{ expr }}` → Returns string (interpolated into text)

#### Quote Escaping in CEL Expressions

When you need to include **string literals with quotes** inside CEL expressions (especially in `stateUpdates`), use a **single backslash** to escape quotes:

```yaml
# ✅ CORRECT - Use \" to escape quotes in object literals
stateUpdates::
  items: "${ items + [{\"id\": string(now()), \"name\": itemName, \"createdAt\": now()}] }"
```

```yaml
# ❌ WRONG - Triple backslash (\\") causes CEL lexer error
stateUpdates::
  items: "${ items + [{\\\"id\\\": string(now())}] }"
  # Error: Cel.Cel_lexer.Lexer_error("Unexpected character: '\\'")
```

**Common use cases:**
- Array concatenation with object literals: `items + [{\"key\": value}]`
- Creating new objects in expressions: `{\"field\": "value", \"count\": 1}`
- Nested quotes in string values: `message + \"Quote: \\\"hello\\\"\"`

#### ⚠️ CRITICAL: Ternary Operators with String Literals

When using ternary operators (`? :`) in interpolations (`{{ }}`), you **MUST use escaped double quotes** (`\"`), **NOT single quotes** (`'`):

```yaml
# ✅ CORRECT - Use escaped double quotes \" for string literals
content: "Status: {{confirmed ? \"Confirmed ✓\" : \"Not confirmed\"}}"
css: "{{isValid ? \"color: green;\" : \"color: red;\"}}"
content: "{{showAdvanced ? \"Hide Advanced\" : \"Show Advanced\"}}"
```

```yaml
# ❌ WRONG - Single quotes will NOT work
content: "Status: {{confirmed ? 'Confirmed ✓' : 'Not confirmed'}}"
# Result: Displays the literal text "Status: {{confirmed ? 'Confirmed ✓' : 'Not confirmed'}}"
# The expression is not evaluated!
```

**Why this matters:**
- CEL (Common Expression Language) requires double quotes for string literals
- Single quotes are not recognized as valid string delimiters in CEL
- Using single quotes causes the entire expression to fail silently
- The raw template string is displayed instead of the evaluated result

**Common patterns:**
```yaml
# Status messages
content: "{{isLoggedIn ? \"Welcome back!\" : \"Please log in\"}}"

# Button labels
content: "{{expanded ? \"Collapse\" : \"Expand\"}}"

# Dynamic styling
css: "background: {{isDark ? \"#1f2937\" : \"#ffffff\"}}; color: {{isDark ? \"white\" : \"black\"}};"

# Conditional icons/emojis
content: "{{completed ? \"✓ Done\" : \"○ Pending\"}}"
```

**Examples:**
```yaml
# Adding image to gallery
stateUpdates::
  images: "${ images + [{\"id\": string(now()), \"imageId\": uploadedImageId, \"title\": currentTitle, \"uploadedAt\": now()}] }"

# Adding video with metadata
stateUpdates::
  videos: "${ videos + [{\"id\": string(timestamp()), \"videoId\": uploadedVideoId, \"title\": currentTitle, \"uploadedAt\": timestamp()}] }"

# Adding audio track
stateUpdates::
  audios: "${ audios + [{\"id\": string(now()), \"audioId\": uploadedAudioId, \"title\": audioTitle, \"artist\": audioArtist, \"uploadedAt\": now()}] }"
```

---

## Template Structure

```yaml
name: "App Name"
version: "v1.0.0"

documents::
  publisherState::
    # State definitions
  publisherComputed::
    # Computed values

ui::
  publisher::
    # Publisher screens (array of screen blocks)
```

---

## 1. State Definition

Define state in `documents.publisherState`:

```yaml
documents::
  publisherState::
    counter::
      type: "number"
      initial: 0

    message::
      type: "string"
      initial: "Hello World"

    showAdvanced::
      type: "boolean"
      initial: false

    items::
      type: "array"
      initial::
        - ::
          id: 1
          name: "First Item"
        - ::
          id: 2
          name: "Second Item"
```

**Supported Types:**
- `"number"` - Integers and floats
- `"string"` - Text values
- `"boolean"` - true/false
- `"array"` - Lists of objects (must use `::` list syntax, NOT JSON arrays `[...]` or empty `[]`)

**Built-in State Variables:**
- `mouseX`, `mouseY` - Mouse position (0-100%)
- `time` - Animation time counter
- `fps` - Current frames per second

### ⚠️ IMPORTANT: Arrays and contentDoc

**For collections (galleries, lists, etc.), use `contentDoc` instead of `publisherState`:**

```yaml
documents::
  contentDoc::
    videos::
      - ::
        id::
          type: "string"
          initial: ""
        videoId::
          type: "string"
          initial: ""
        title::
          type: "string"
          initial: ""
        uploadedAt::
          type: "number"
          initial: 0

  publisherState::
    # Single values for upload UI
    currentTitle::
      type: "string"
      initial: ""
    uploadedVideoId::
      type: "string"
      initial: ""
```

**Why use contentDoc for arrays?**
- Proper CRDT synchronization across nodes
- Better persistence and state management
- Required for collections with forEach rendering
- Prevents empty placeholder issues

**When to use publisherState:**
- Single values (counters, flags, current selections)
- Temporary UI state (uploading status, form inputs)
- Non-persistent state

**When to use contentDoc:**
- Collections (videos, images, files, items)
- Data that needs to persist across sessions
- Data that needs to sync to viewers

### ✅ NEW: Collaborative State (collaborativeState)

**For bidirectional collaborative features (editable by both publisher and viewers), use `collaborativeState`:**

```yaml
documents::
  collaborativeState::
    counter::
      type: "number"
      initial: 0

    newComment::
      type: "string"
      initial: ""

    comments::
      - ::
        id::
          type: "string"
        content::
          type: "string"
        author::
          type: "string"
        timestamp::
          type: "number"
```

**What is collaborativeState?**
- A separate Loro CRDT document (`collaborative_doc`) that syncs between publisher and all viewers
- Both publisher and viewers can **read and write** to collaborative state
- Changes sync bidirectionally via the P2P network
- Perfect for real-time collaborative features like comments, likes, shared counters, etc.

**Key Differences Between Document Types:**

| Feature | publisherState | contentDoc | collaborativeState |
|---------|---------------|------------|-------------------|
| **Stored In** | `content_doc` (stateMap) | `content_doc` (contentMap) | `collaborative_doc` |
| **Publisher Access** | Read/Write | Read/Write | Read/Write |
| **Viewer Access** | Read-Only | Read-Only | **Read/Write** ✅ |
| **Use Cases** | Single values, UI state | Collections, persistent data | Collaborative features |
| **Examples** | `currentTitle`, `uploadedVideoId` | `videos[]`, `images[]` | `comments[]`, `counter`, `likes` |

**Important Implementation Details:**

1. **Array Schema Syntax**: Use `- ::` to define array item structure (same as contentDoc):
   ```yaml
   comments::
     - ::
       id::
         type: "string"
       content::
         type: "string"
   ```
   This initializes as an empty array `[]`, not as a CEL expression.

2. **Viewer State Updates**: Viewers can call `setState` action on collaborative fields:
   ```yaml
   # This works for viewers when updating collaborative state
   - ::
     type: "button"
     content: "Add Comment"
     action: "setState"
     stateUpdates::
       comments: "${ comments + [{\"id\": string(now()), \"content\": newComment, \"author\": \"Viewer\", \"timestamp\": now()}] }"
       newComment: ""
   ```

3. **Iteration with forEach**: Use `forEach` (not `repeat`) to iterate over collaborative arrays:
   ```yaml
   - ::
     type: "container"
     forEach: "${ comments }"
     as: "item"
     key: "id"
     blocks::
       - ::
         type: "text"
         content: "{{item.content}}"
       - ::
         type: "text"
         content: "— {{item.author}}"
   ```

4. **Real-time Sync**: Changes to collaborative state trigger automatic CRDT sync:
   - Publisher updates → syncs to node → syncs to all viewers
   - Viewer updates → syncs to node → syncs to publisher and other viewers
   - Conflict-free merging via Loro CRDT

**When to use collaborativeState:**
- Comments, reactions, likes
- Shared counters or polls
- Real-time collaborative editing features
- Any data that viewers should be able to modify

**When NOT to use collaborativeState:**
- Publisher-only content (use publisherState or contentDoc)
- Sensitive data that viewers shouldn't modify
- Large binary assets (use static assets)

---

## 2. Computed Values

Define computed expressions in `documents.publisherComputed` using `${ }` syntax:

```yaml
documents::
  publisherComputed::
    counterDouble: "${ counter * 2 }"
    isPositive: "${ counter > 0 }"
    isEven: "${ counter % 2 == 0 }"
```

**Features:**
- Multi-pass evaluation (handles dependencies)
- Max 5 passes for circular dependency prevention
- Access state variables and other computed values

---

## 3. CEL Expression Syntax

### Basic Operators

**Pure expressions** (use `${ }` for typed values):
```yaml
# Arithmetic
"${ counter + 1 }"
"${ counter - 1 }"
"${ counter * 2 }"
"${ counter / 2 }"
"${ counter % 2 }"

# Comparison
"${ counter > 0 }"
"${ counter >= 5 }"
"${ counter < 10 }"
"${ counter == 0 }"
"${ counter != 0 }"

# Logical
"${ !showAdvanced }"          # NOT (! operator, not "not")
"${ counter > 0 && counter < 10 }"
"${ isEven || isPositive }"
```

**String operations** (use `{{ }}` for building strings):
```yaml
# String interpolation
content: "Counter value: {{counter}}"

# String concatenation
content: "{{message + ' world'}}"
```

### ⚠️ Important: Boolean Negation

**Use `!` not `not`:**
```yaml
# ✅ CORRECT
stateUpdates::
  showAdvanced: "${ !showAdvanced }"

# ❌ WRONG (will error)
stateUpdates::
  showAdvanced: "${ not showAdvanced }"
```

### ✅ Ternary Operator

**CEL supports ternary conditionals:**
```yaml
# ✅ CORRECT - Ternary for inline conditionals
content: "{{showAdvanced ? 'Hide' : 'Show'}}"
css: "opacity: {{isValid ? 1 : 0.5}}"
content: "{{count > 0 ? string(count) + ' items' : 'No items'}}"

# Also valid - if/else blocks for complex UI
- if: "${ showAdvanced }"
  then::
    - type: "text"
      content: "Hide"
  else::
    - type: "text"
      content: "Show"
```

**Use cases:**
- Dynamic CSS values: `opacity: {{isEnabled ? 1 : 0.5}}`
- Conditional text: `{{count > 0 ? 'Items' : 'Empty'}}`
- Dynamic classes/styles based on state

### String Interpolation

```yaml
# Embed expressions in strings
content: "Counter: {{counter}}"
content: "Hello, {{message}}!"
content: "FPS: {{fps}}"

# Multiple interpolations
content: "{{counter}} * 2 = {{counterDouble}}"
```

### Built-in CEL Functions

**✅ timestamp()** - Returns current Unix timestamp in milliseconds
```yaml
# String interpolation
content: "Timestamp: {{timestamp()}}"

# Pure expression (for stateUpdates)
stateUpdates::
  createdAt: "${ timestamp() }"
```
Returns: Integer like `1762282697087`

**✅ generateId()** - Returns unique ID string
```yaml
# String interpolation
content: "ID: {{generateId()}}"

# Pure expression (for stateUpdates)
stateUpdates::
  itemId: "${ generateId() }"
```
Returns: String like `"1762282697089-074657"`

**Other Available Functions:**
- String: `contains()`, `startsWith()`, `endsWith()`, `trim()`, `toLowerCase()`, `toUpperCase()`, `split()`, `replace()`, `substring()`
- Collection: `size()`, `filter()`, `map()`, `exists()`, `all()`, `flatten()`, `unique()`, `slice()`, `find()`, `join()`
- Type conversion: `int()`, `double()`, `string()`

---

## 4. Control Flow

### `when` - Conditional Visibility

Show/hide blocks based on conditions:

```yaml
- ::
  type: "text"
  content: "Counter is positive!"
  when: "${ counter > 0 }"
```

**⚠️ IMPORTANT:** `when` uses `${ }` syntax for pure expressions!

```yaml
# ✅ CORRECT
when: "${ counter > 0 }"
when: "${ itemCount == 0 }"
when: "${ !showAdvanced }"

# ❌ WRONG - Don't use {{}} (that's for string interpolation)
when: "{{counter > 0}}"  # Wrong syntax!
```

**Note:** Use direct state variables in `when` conditions. Computed values may not be available in the evaluation context.

### `if/then/else` - Branching

```yaml
- ::
  if: "${ counter > 0 }"
  then::
    - ::
      type: "text"
      content: "Positive"
  else::
    - ::
      type: "text"
      content: "Zero or negative"
```

**Important:** Control flow blocks need `- ::` prefix!

### `match/cases` - Switch Statement

```yaml
- ::
  match: "${ currentView }"
  cases::
    - ::
      value: "grid"
      blocks::
        - ::
          type: "text"
          content: "Grid view"

    - ::
      value: "list"
      blocks::
        - ::
          type: "text"
          content: "List view"

    - ::
      default: true
      blocks::
        - ::
          type: "text"
          content: "Unknown view"
```

**Important:** Both the match block and each case need `- ::` prefix!

### `forEach` - Iteration

**⚠️ CRITICAL: Use `forEach`, NOT `repeat`**

```yaml
- ::
  type: "container"
  forEach: "${ items }"
  as: "item"
  key: "id"
  blocks::
    - ::
      type: "text"
      content: "{{item.title}}"
    - ::
      type: "text"
      content: "{{item.description}}"
```

**Properties:**
- `forEach: "${ expression }"` - Expression that returns an array to iterate over
- `as: "item"` - Loop variable name (optional, default: `"item"`)
- `key: "id"` - Property name to use as unique key for each item (optional but recommended)
- `blocks::` - Child blocks to render for each item

**Loop Variables:**
- `item` (or custom name from `as`) - Current item in the iteration
- `itemIndex` - Automatically available (0-based index)

**Examples:**

```yaml
# Iterating over collaborative comments
- ::
  type: "container"
  forEach: "${ comments }"
  as: "comment"
  key: "id"
  blocks::
    - ::
      type: "text"
      content: "{{comment.content}}"
    - ::
      type: "text"
      content: "— {{comment.author}}"

# Iterating over videos from contentDoc
- ::
  type: "container"
  forEach: "videos"
  key: "id"
  css: "display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));"
  blocks::
    - ::
      type: "video"
      videoId: "{{item.videoId}}"
      title: "{{item.title}}"
```

**Common Mistakes:**
- ❌ Using `repeat` instead of `forEach` (not supported)
- ❌ Forgetting `as` and `key` properties for clarity
- ❌ Not using CEL syntax `${ }` for dynamic arrays

---

## 5. Implemented Block Types

### ✅ `screen` - Top-level container

```yaml
- ::
  type: "screen"
  id: "home"
  name: "home"
  isEntryPoint: true
  css: "padding: 2rem; max-width: 800px; margin: 0 auto;"
  blocks::
    - ::
      type: "heading"
      # ... child blocks
```

**Properties:**
- `id` - Screen identifier (string)
- `name` - Screen name (string)
- `isEntryPoint` - Start screen? (boolean)
- `css` - Inline styles (string)
- `blocks` - Child blocks (array)

---

### ✅ `container` - Layout container

```yaml
- ::
  type: "container"
  layout: "flex"
  direction: "row"
  gap: "1rem"
  css: "justify-content: center;"
  blocks::
    - ::
      type: "text"
      # ... child blocks
```

**Properties:**
- `layout` - `"flex"` | `"grid"` | `"block"` (default: `"block"`)
- `direction` - `"row"` | `"column"` (for flex)
- `gap` - Spacing between items (CSS value)
- `alignItems` - Flex alignment
- `justifyContent` - Flex justification
- `columns` - Number of columns (for grid)
- `css` - Additional CSS

---

### ✅ `section` - Semantic section

```yaml
- ::
  type: "section"
  css: "background: #f3f4f6; padding: 2rem;"
  blocks::
    - ::
      type: "text"
      content: "Child block"
```

**Properties:**
- `css` - Inline styles
- `blocks` - Child blocks

---

### ✅ `heading` - Headings (h1-h6)

```yaml
- ::
  type: "heading"
  level: 1
  content: "Counter: {{counter}}"
  css: "color: #2563eb;"
```

**Properties:**
- `level` - 1-6 (default: 1)
- `content` - Text with CEL interpolation
- `css` - Inline styles

---

### ✅ `text` - Paragraph text

```yaml
- ::
  type: "text"
  content: "Current: {{message}}"
  css: "color: #666;"
```

**Properties:**
- `content` - Text with CEL interpolation
- `css` - Inline styles

---

### ✅ `button` - Interactive button

```yaml
- ::
  type: "button"
  content: "Increment"
  action: "setState"
  stateUpdates::
    counter: "${ counter + 1 }"
  css: "padding: 0.75rem 1.5rem; background: #10b981; color: white;"
```

**Properties:**
- `content` - Button label (CEL interpolation with `{{ }}` supported)
- `action` - Action name (string) - `"setState"` or `"navigate"`
- `stateUpdates` - State changes (use `${ }` for expressions)
- `targetScreen` - Screen name (for navigate action)
- `disabled` - Disable button (use `${ }` for boolean expressions)
- `css` - Inline styles (use `{{ }}` for dynamic CSS values)

---

### ✅ `input` - Text input field

```yaml
- ::
  type: "input"
  name: "username"
  inputType: "text"
  placeholder: "Enter your username"
  css: "padding: 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; width: 100%;"
```

**Properties:**
- `name` - Field name (used as key in state)
- `inputType` - Input type: `"text"`, `"email"`, `"password"`, `"number"`, `"tel"`, `"url"`, `"search"` (default: `"text"`)
- `value` - CEL expression for initial/bound value (optional, defaults to `context[name]`)
- `placeholder` - Placeholder text (supports CEL interpolation)
- `disabled` - CEL expression for disabled state
- `required` - Boolean for required validation
- `onChange` - Action name to trigger on input change
- `onBlur` - Action name to trigger on blur
- `onFocus` - Action name to trigger on focus
- `css` - Inline styles

**How it works:**
- Binds to state via `name` property
- Updates local state automatically on input (via `onStateChange` callback)
- Value is read from `context[name]` for reactivity
- Template authors must provide all styling via `css`

**See:** `/docs/examples/02_input_blocks.huml` for complete form example with validation

---

### ✅ `textarea` - Multi-line text input

```yaml
- ::
  type: "textarea"
  name: "message"
  placeholder: "Write your message..."
  rows: 5
  css: "padding: 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; width: 100%; resize: vertical;"
```

**Properties:**
- `name` - Field name (used as key in state)
- `value` - CEL expression for initial/bound value (optional, defaults to `context[name]`)
- `placeholder` - Placeholder text (supports CEL interpolation)
- `rows` - Number of visible rows (default: 4)
- `cols` - Number of visible columns (optional)
- `disabled` - CEL expression for disabled state
- `required` - Boolean for required validation
- `onChange` - Action name to trigger on input change
- `onBlur` - Action name to trigger on blur
- `onFocus` - Action name to trigger on focus
- `css` - Inline styles

**How it works:**
- Same local state binding as `input` block
- Updates automatically on typing
- Use `size()` function to get character count

**See:** `/docs/examples/02_form_blocks.huml` for character counter example

---

### ✅ `checkbox` - Boolean checkbox

```yaml
- ::
  type: "checkbox"
  name: "agreeToTerms"
  label: "I agree to the terms and conditions"
  css: "display: flex; align-items: center; gap: 0.5rem; cursor: pointer;"
```

**Properties:**
- `name` - Field name (used as key in state)
- `label` - Optional label text displayed next to checkbox (supports CEL interpolation)
- `checked` - CEL expression for checked state (optional, defaults to `context[name]`)
- `disabled` - CEL expression for disabled state
- `onChange` - Action name to trigger on change
- `css` - Inline styles

**How it works:**
- Binds to boolean state via `name` property
- Updates local state automatically on change
- Value is read from `context[name]` for reactivity
- Label is rendered inline with checkbox if provided

**See:** `/docs/examples/02_form_blocks.huml` for checkbox examples with validation

---

### ✅ `label` - Form label

```yaml
- ::
  type: "label"
  for: "username"
  content: "Username"
  css: "display: block; font-weight: 600; margin-bottom: 0.5rem;"
```

**Properties:**
- `for` - ID of input element to associate with
- `content` - Label text (supports CEL interpolation)
- `css` - Inline styles

**How it works:**
- Creates a `<label>` element with `for` attribute
- Clicking label focuses associated input
- Content supports dynamic interpolation

**See:** `/docs/examples/02_form_blocks.huml` for label usage with form inputs

---

### ✅ `select` - Dropdown select

```yaml
- ::
  type: "select"
  name: "country"
  options::
    - ::
      value: ""
      label: "Select a country"
    - ::
      value: "us"
      label: "United States"
    - ::
      value: "uk"
      label: "United Kingdom"
  css: "padding: 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; width: 100%;"
```

**Properties:**
- `name` - Field name (used as key in state)
- `options` - Array of option objects with `value` and `label` properties
- `value` - CEL expression for selected value (optional, defaults to `context[name]`)
- `disabled` - CEL expression for disabled state
- `required` - Boolean for required validation
- `onChange` - Action name to trigger on selection change
- `css` - Inline styles

**How it works:**
- Renders HTML `<select>` with `<option>` elements
- Updates local state automatically on selection
- Value is read from `context[name]` for reactivity
- First option typically used as placeholder

**See:** `/docs/examples/02_form_blocks.huml` for select dropdown examples

---

### ✅ `radio` - Radio button group

```yaml
- ::
  type: "radio"
  name: "favoriteColor"
  options::
    - ::
      value: "red"
      label: "Red"
    - ::
      value: "blue"
      label: "Blue"
    - ::
      value: "green"
      label: "Green"
  css: "display: flex; flex-direction: column; gap: 0.5rem;"
```

**Properties:**
- `name` - Field name (used as key in state, shared across all radio buttons)
- `options` - Array of option objects with `value` and `label` properties
- `value` - CEL expression for selected value (optional, defaults to `context[name]`)
- `disabled` - CEL expression for disabled state
- `onChange` - Action name to trigger on selection change
- `css` - Inline styles for container

**How it works:**
- Renders group of radio buttons with same `name`
- Only one option can be selected at a time
- Updates local state automatically on selection
- Each option wrapped in `<label>` for clickability

**See:** `/docs/examples/02_form_blocks.huml` for radio button group examples

---

### ✅ `link` - Hyperlink

```yaml
- ::
  type: "link"
  content: "Visit Example.com"
  href: "https://example.com"
  css: "color: #2563eb; text-decoration: underline;"
```

**Properties:**
- `content` - Link text (supports CEL interpolation for display text only)
- `href` - URL for external links (**MUST be static URL string, no CEL interpolation**)
- `action` - Action name for SPA navigation (typically `"navigate"`)
- `params` - Parameters for action (e.g., `screen: "settings"` for navigate action)
- `css` - Inline styles

**🔒 Security Model (Tauri Desktop App):**
- **External links (`href`):** ALWAYS open in system browser, NEVER in-app
- **Internal navigation (`action: "navigate"`):** SPA navigation within Sthalam only
- **External URLs must be static strings** - no dynamic/computed URLs for security
- Dynamic resource references are planned for future (not yet implemented)
- This prevents security vulnerabilities like XSS, phishing, and malicious content rendering

**How it works:**
- External links use Tauri's shell API to open in system browser
- Internal navigation uses `action: "navigate"` with `params.screen`
- Content supports dynamic interpolation
- All click events are intercepted to enforce security policy

**External link example:**
```yaml
- ::
  type: "link"
  content: "Visit Documentation"
  href: "https://docs.example.com"
  css: "color: #2563eb; text-decoration: underline;"
```

**Internal navigation example:**
```yaml
- ::
  type: "link"
  content: "Go to Settings →"
  action: "navigate"
  params::
    screen: "settings"
  css: "color: #2563eb; font-weight: 600;"
```

**See:** `/docs/examples/03_content_blocks.huml` for link examples with navigation

---

### ✅ `image` - Image display with staticAssets storage

**⚠️ BREAKING CHANGE:** Image block now only supports asset IDs from staticAssets. External URLs are no longer supported.

```yaml
- ::
  type: "image"
  src: "{{item.imageId}}"
  alt: "{{item.title}}"
  width: "100%"
  css: "display: block; aspect-ratio: 1/1; object-fit: cover;"
```

**Properties:**
- `src` - Asset ID from staticAssets (format: `asset_image_{timestamp}_{random}`) - supports CEL interpolation
- `alt` - Alt text for accessibility (supports CEL interpolation)
- `width` - Image width in pixels or CSS value (supports CEL interpolation)
- `height` - Image height in pixels or CSS value (supports CEL interpolation)
- `css` - Inline styles

**Image Storage Architecture:**
1. **Binary data** → Stored in `staticAssets` Map (non-CRDT, local storage only)
2. **Image IDs & metadata** → Stored in `contentDoc` (Loro CRDT, syncs to nodes and viewers)
3. **Asset IDs** → Generated by OCaml WASM using `generateAssetId()` (format: `asset_image_{timestamp}_{random}`)
4. **State persistence** → `publisherState` maintains upload state across app restarts

**Why this architecture?**
- Images are large binaries that don't need CRDT collaboration
- Only the IDs need to sync to viewers (via contentDoc)
- staticAssets provides efficient binary storage without CRDT overhead
- Separates concerns: storage (staticAssets) vs. sync (contentDoc)

**Complete upload and display workflow:**
```yaml
documents::
  contentDoc::
    images::
      - ::
        id::
          type: "string"
          initial: ""
        imageId::
          type: "string"
          initial: ""
        title::
          type: "string"
          initial: ""
        uploadedAt::
          type: "number"
          initial: 0

  publisherState::
    currentTitle::
      type: "string"
      initial: ""
    uploadedImageId::
      type: "string"
      initial: ""

ui::
  publisher::
    - ::
      type: "input"
      name: "currentTitle"
      placeholder: "Enter image title..."

    # Step 1: Upload image to staticAssets
    - ::
      type: "button"
      content: "🖼️ Choose Image File"
      action: "uploadImage"
      params::
        stateField: "uploadedImageId"

    # Step 2: Add metadata to contentDoc gallery
    - ::
      type: "button"
      content: "➕ Add to Gallery"
      action: "setState"
      when: "${ size(uploadedImageId) > 0 && size(currentTitle) > 0 }"
      stateUpdates::
        images: "${ images + [{\"id\": string(now()), \"imageId\": uploadedImageId, \"title\": currentTitle, \"uploadedAt\": now()}] }"
        uploadedImageId: ""
        currentTitle: ""

    # Display image gallery
    - ::
      type: "container"
      forEach: "images"
      blocks::
        - ::
          type: "image"
          src: "{{item.imageId}}"
          alt: "{{item.title}}"
          width: "100%"
```

**How it works:**
1. User clicks "Choose Image File" button
2. `uploadImage` action opens file dialog (jpg, jpeg, png, gif, webp, svg)
3. OCaml WASM generates asset ID: `asset_image_1762343546435_1e4a2426`
4. Image binary stored in staticAssets
5. Asset ID stored in publisherState.uploadedImageId
6. User enters title and clicks "Add to Gallery"
7. Metadata (imageId, title, uploadedAt) added to contentDoc.images array (CRDT)
8. Image displayed using asset ID lookup from staticAssets
9. Blob URL created for HTML img element

**See:** `/docs/examples/08_image_gallery.huml` for complete working example

---

### ✅ `video` - HTML5 Video Player

```yaml
- ::
  type: "video"
  src: "{{item.videoId}}"
  controls: true
  muted: true
  width: "100%"
  css: "display: block; background: #000; aspect-ratio: 16/9; object-fit: contain;"
```

**Properties:**
- `src` - Video source (supports CEL interpolation)
  - Can be asset ID from staticAssets (e.g., `"asset_video_1762343546435_1e4a2426"`)
  - Can be external URL (e.g., `"https://example.com/video.mp4"`)
- `controls` - Show video controls (default: true)
- `autoplay` - Auto-play video (default: false)
- `loop` - Loop video playback (default: false)
- `muted` - Mute audio (default: false for manual playback, true for autoplay)
- `width` - Video width (supports CEL interpolation)
- `height` - Video height (supports CEL interpolation)
- `css` - Inline styles

**Video Storage Architecture:**
1. **Binary data** → Stored in `staticAssets` Map (non-CRDT, local storage only)
2. **Video IDs & metadata** → Stored in `contentDoc` (Loro CRDT, syncs to nodes and viewers)
3. **Asset IDs** → Generated by OCaml WASM using `generateAssetId()` (format: `asset_video_{timestamp}_{random}`)
4. **State persistence** → `publisherState` maintains upload state across app restarts

**Why this architecture?**
- Videos are large binaries that don't need CRDT collaboration
- Only the IDs need to sync to viewers (via contentDoc)
- staticAssets provides efficient binary storage without CRDT overhead
- Separates concerns: storage (staticAssets) vs. sync (contentDoc)

**Upload Action:**
```yaml
- ::
  type: "button"
  content: "🎬 Choose Video File"
  action: "uploadVideo"
  params::
    stateField: "uploadedVideoId"  # State field to store asset ID
```

**Upload Workflow (Two-Step Process):**
```yaml
documents::
  publisherState::
    # Upload state
    uploadedVideoId::
      type: "string"
      initial: ""
    uploadedVideoFilename::
      type: "string"
      initial: ""
    uploadedVideoSize::
      type: "number"
      initial: 0
    uploading::
      type: "boolean"
      initial: false

    # Gallery data (stored in contentDoc)
    videos::
      type: "array"
      initial::
        - ::
          id: ""
          videoId: ""
          title: ""
          uploadedAt: 0

ui::
  publisher::
    - ::
      type: "screen"
      blocks::
        # Step 1: Upload video file
        - ::
          type: "button"
          content: "🎬 Choose Video File"
          action: "uploadVideo"
          params::
            stateField: "uploadedVideoId"
          when: "${ !uploading }"

        # Step 2: Add to gallery with metadata
        - ::
          type: "button"
          content: "➕ Add to Gallery"
          action: "setState"
          when: "${ size(uploadedVideoId) > 0 && size(currentTitle) > 0 }"
          stateUpdates::
            videos: "${ videos + [{\"id\": string(timestamp()), \"videoId\": uploadedVideoId, \"title\": currentTitle, \"uploadedAt\": timestamp()}] }"
            uploadedVideoId: ""
            currentTitle: ""
```

**Complete Gallery Example:**
```yaml
documents::
  publisherState::
    currentTitle::
      type: "string"
      initial: ""
    uploadedVideoId::
      type: "string"
      initial: ""
    videos::
      type: "array"
      initial::
        - ::
          id: ""
          videoId: ""
          title: ""
          uploadedAt: 0

  publisherComputed::
    hasVideos: "${ size(videos) > 0 }"
    videoCount: "${ size(videos) }"

ui::
  publisher::
    - ::
      type: "screen"
      blocks::
        # Upload Section
        - ::
          type: "input"
          name: "currentTitle"
          placeholder: "Enter video title..."
          css: "width: 100%; padding: 0.75rem;"

        - ::
          type: "button"
          content: "🎬 Choose Video File"
          action: "uploadVideo"
          params::
            stateField: "uploadedVideoId"

        - ::
          type: "button"
          content: "➕ Add to Gallery"
          action: "setState"
          when: "${ size(uploadedVideoId) > 0 }"
          stateUpdates::
            videos: "${ videos + [{\"id\": string(timestamp()), \"videoId\": uploadedVideoId, \"title\": currentTitle, \"uploadedAt\": timestamp()}] }"
            currentTitle: ""
            uploadedVideoId: ""

        # Video Gallery Grid
        - ::
          type: "container"
          forEach: "videos"
          when: "${ hasVideos }"
          css: "display: grid; grid-template-columns: repeat(auto-fill, minmax(350px, 1fr)); gap: 1.5rem;"
          blocks::
            - ::
              type: "container"
              css: "border: 2px solid #e5e7eb; border-radius: 12px; overflow: hidden;"
              blocks::
                # Video Player
                - ::
                  type: "video"
                  src: "{{item.videoId}}"
                  controls: true
                  muted: true
                  width: "100%"
                  css: "display: block; background: #000; aspect-ratio: 16/9;"

                # Video Info
                - ::
                  type: "container"
                  css: "padding: 1rem;"
                  blocks::
                    - ::
                      type: "heading"
                      level: 3
                      content: "{{item.title}}"

                    - ::
                      type: "text"
                      content: "Asset ID: {{item.videoId}}"
                      css: "font-size: 0.75rem; color: #6b7280; font-family: monospace;"
```

**How it works:**
1. User clicks button with `action: "uploadVideo"`
2. File dialog opens for video selection (**MP4, WebM, OGG only** - browser-supported formats)
3. OCaml WASM generates unique asset ID: `asset_video_{timestamp}_{random}`
4. Video binary data stored in `staticAssets` Map (not CRDT)
5. Asset ID stored in state field (specified in params.stateField)
6. User provides metadata (title, etc.) and clicks "Add to Gallery"
7. Video metadata (id, videoId, title, uploadedAt) added to `videos` array in contentDoc
8. VideoBlock retrieves binary from staticAssets using videoId
9. Creates blob URL with correct MIME type (detected from filename)
10. HTML5 `<video>` element displays with controls

**⚠️ Important Notes:**
- Only **MP4, WebM, and OGG** formats are supported (HTML5 video standard)
- MKV, AVI, MOV are **not supported** by browsers
- Videos are **stored locally** in staticAssets (not synced as binaries)
- Only **video IDs sync** to nodes/viewers via contentDoc
- Use **`muted: true`** for autoplay compliance (browsers require muted autoplay)
- Videos persist across app restarts via publisherState serialization

**External URLs:**
You can also use external video URLs directly:
```yaml
- ::
  type: "video"
  src: "https://example.com/video.mp4"
  controls: true
```

**See:** `/docs/examples/07_video_gallery.huml` for complete video gallery with upload, metadata, and grid display

---

### ✅ `file` - File display with download button

**NEW:** File block for document storage and downloads. Displays file metadata and provides download functionality.

```yaml
- ::
  type: "file"
  src: "{{item.fileId}}"
  css: "border: 2px solid #e5e7eb; border-radius: 8px; padding: 1rem; background: #f9fafb;"
```

**Properties:**
- `src` - Asset ID from staticAssets (format: `asset_file_{timestamp}_{random}`) - supports CEL interpolation
- `css` - Inline styles for the file card container

**File Storage Architecture:**
1. **Binary data** → Stored in `staticAssets` Map (non-CRDT, local storage only)
2. **File IDs & metadata** → Stored in `contentDoc` (Loro CRDT, syncs to nodes and viewers)
3. **Asset IDs** → Generated by OCaml WASM using `generateAssetId()` (format: `asset_file_{timestamp}_{random}`)
4. **File types** → Configurable via `setFileTypes` action (default: pdf, txt, doc, docx, xls, xlsx, csv)

**Display Features:**
- File icon emoji (based on extension: 📄 pdf, 📝 txt, 📃 doc, 📊 xlsx, etc.)
- Filename (retrieved from metadata)
- Human-readable file size (e.g., "1.5 MB")
- Download button (triggers browser download via blob URL)

**Complete upload and display workflow:**
```yaml
documents::
  contentDoc::
    files::
      - ::
        id::
          type: "string"
          initial: ""
        fileId::
          type: "string"
          initial: ""
        filename::
          type: "string"
          initial: ""
        size::
          type: "number"
          initial: 0
        uploadedAt::
          type: "number"
          initial: 0

  publisherState::
    uploadedFileId::
      type: "string"
      initial: ""
    uploadedFilename::
      type: "string"
      initial: ""
    uploadedFileSize::
      type: "number"
      initial: 0

ui::
  publisher::
    # Step 1: Upload file to staticAssets
    - ::
      type: "button"
      content: "📄 Choose File"
      action: "uploadFile"
      params::
        stateField: "uploadedFileId"

    # Step 2: Add metadata to contentDoc
    - ::
      type: "button"
      content: "➕ Add to Files"
      action: "setState"
      when: "${ size(uploadedFileId) > 0 }"
      stateUpdates::
        files: "${ files + [{\"id\": string(now()), \"fileId\": uploadedFileId, \"filename\": uploadedFilename, \"size\": uploadedFileSize, \"uploadedAt\": now()}] }"
        uploadedFileId: ""
        uploadedFilename: ""
        uploadedFileSize: 0

    # Display file list
    - ::
      type: "container"
      forEach: "files"
      blocks::
        - ::
          type: "file"
          src: "{{item.fileId}}"
```

**Configuring allowed file types:**
```yaml
- ::
  type: "button"
  content: "⚙️ Configure File Types"
  action: "setFileTypes"
  params::
    types::
      - "pdf"
      - "txt"
      - "zip"
      - "json"
```

**How it works:**
1. User clicks "Choose File" button
2. `uploadFile` action opens file dialog (filtered by allowed types)
3. OCaml WASM generates asset ID: `asset_file_1762343546435_1e4a2426`
4. File binary stored in staticAssets
5. Asset ID, filename, and size stored in publisherState
6. User clicks "Add to Files"
7. Metadata (fileId, filename, size, uploadedAt) added to contentDoc.files array (CRDT)
8. FileBlock displays file card with icon, filename, size, and download button
9. Download button creates blob URL with correct MIME type and triggers browser download

**⚠️ Important Notes:**
- Default allowed types: **pdf, txt, doc, docx, xls, xlsx, csv**
- Use `setFileTypes` action to configure custom file types per template
- Files are **stored locally** in staticAssets (not synced as binaries)
- Only **file IDs sync** to nodes/viewers via contentDoc
- Download uses browser's native download mechanism (respects user's download folder)

**See:** `/docs/examples/09_file_manager.huml` for complete file manager with upload, metadata, and download

---

### ✅ `form` - Form container

```yaml
- ::
  type: "form"
  name: "contactForm"
  onSubmit: "handleFormSubmit"
  css: "display: flex; flex-direction: column; gap: 1rem; max-width: 500px;"
  blocks::
    - ::
      type: "input"
      name: "email"
      inputType: "email"
      placeholder: "Your email"
      css: "padding: 0.5rem; border: 1px solid #ccc;"

    - ::
      type: "textarea"
      name: "message"
      placeholder: "Your message"
      rows: 5
      css: "padding: 0.5rem; border: 1px solid #ccc;"

    - ::
      type: "button"
      content: "Submit"
      submitForm: true
      css: "padding: 0.75rem; background: #2563eb; color: white; border: none; cursor: pointer;"
```

**Properties:**
- `name` - Form name (used as event name in submissions)
- `blocks` - Child blocks (form inputs, buttons, etc.)
- `onSubmit` - Optional action name to trigger after submission
- `css` - Inline styles

**How it works:**
- Wraps form inputs in a `<form>` element
- Handles form submission via `onsubmit` event
- Automatically collects all form field values from context
- Writes submissions to `submissionsDoc` (Loro CRDT)
- Each submission includes:
  - Unique ID (timestamp + random)
  - Timestamp
  - Event name (form name)
  - Form data (all field values)
- Optionally triggers `onSubmit` action with `{submissionId, formData}` params

**Submit button:**
- Use `submitForm: true` on button to trigger form submission
- Button type becomes `type="submit"`
- Form's `onsubmit` handler is called instead of button's `action`

**Accessing submissions:**
- Submissions are stored in the `submissionsDoc` Loro document
- Use the SubmissionsViewer component to view all submissions
- Export submissions as CSV or JSON
- Filter by form name (event name)

**See:** `/docs/examples/04_form_submissions.huml` for complete form submission example

---

### ✅ `modal` - Multi-purpose modal component

The `modal` block supports three display modes: dialog (default), drawer, and popover.

#### Dialog Mode (Default)

Centered modal with backdrop overlay:

```yaml
- ::
  type: "modal"
  name: "confirmDialog"
  visible: "${ publisherState.showConfirm }"
  modalType: "dialog"
  size: "medium"
  backdrop: true
  closeOnBackdropClick: true
  closeOnEscape: true
  closable: true
  onClose: "setState"
  closeParams::
    stateUpdates::
      showConfirm: "${ false }"
  blocks::
    - ::
      type: "heading"
      level: 2
      content: "Are you sure?"
    - ::
      type: "text"
      content: "This action cannot be undone."
    - ::
      type: "container"
      layout: "flex"
      direction: "row"
      gap: "1rem"
      css: "margin-top: 1.5rem;"
      blocks::
        - ::
          type: "button"
          content: "Cancel"
          action: "setState"
          stateUpdates::
            showConfirm: "${ false }"
        - ::
          type: "button"
          content: "Confirm"
          action: "setState"
          stateUpdates::
            showConfirm: "${ false }"
            confirmed: "${ true }"
```

#### Drawer Mode

Side panel that slides in from left/right/top/bottom:

```yaml
- ::
  type: "modal"
  name: "settingsDrawer"
  visible: "${ publisherState.showSettings }"
  modalType: "drawer"
  position: "right"
  width: "400px"
  backdrop: true
  closeOnBackdropClick: true
  closeOnEscape: true
  onClose: "setState"
  closeParams::
    stateUpdates::
      showSettings: "${ false }"
  blocks::
    - ::
      type: "heading"
      level: 2
      content: "Settings"
    - ::
      type: "text"
      content: "Configure your preferences here."
```

#### Popover Mode

Small tooltip-style popup anchored to an element:

```yaml
# First, create a button with an ID
- ::
  type: "button"
  id: "helpButton"
  content: "?"
  action: "setState"
  stateUpdates::
    showHelp: "${ !publisherState.showHelp }"

# Then, create a popover anchored to it
- ::
  type: "modal"
  name: "helpPopover"
  visible: "${ publisherState.showHelp }"
  modalType: "popover"
  anchorId: "helpButton"
  placement: "top"
  closable: true
  onClose: "setState"
  closeParams::
    stateUpdates::
      showHelp: "${ false }"
  blocks::
    - ::
      type: "text"
      content: "Click here to get help with this feature."
```

**Properties:**

**Core Properties:**
- `name` - Modal identifier (string)
- `visible` - CEL expression controlling visibility (string, e.g., `"${ publisherState.showModal }"`)
- `blocks` - Child blocks to render inside modal (array)
- `css` - Custom CSS styles (string)

**Modal Type:**
- `modalType` - `"dialog"` | `"drawer"` | `"popover"` (default: `"dialog"`)

**Dialog/Drawer Sizing:**
- `size` - `"small"` (400px) | `"medium"` (600px) | `"large"` (800px) | `"fullscreen"` (100vw)
- `width` - Custom width (e.g., `"500px"`, `"80%"`) - overrides `size`
- `height` - Custom height (e.g., `"400px"`, `"60vh"`)

**Drawer Position:**
- `position` - `"left"` | `"right"` | `"top"` | `"bottom"` (default: `"right"`)

**Popover Position:**
- `anchorId` - ID of element to anchor popover to (string)
- `placement` - `"top"` | `"bottom"` | `"left"` | `"right"` (default: `"top"`)

**Interaction:**
- `backdrop` - Show overlay backdrop (boolean, default: `true`)
- `closeOnBackdropClick` - Allow closing by clicking backdrop (boolean, default: `true`)
- `closeOnEscape` - Allow closing with Escape key (boolean, default: `true`)
- `closable` - Show close button (boolean, default: `true`)
- `onClose` - Action name to dispatch when modal closes (string)
- `closeParams` - Parameters for close action (object)

**How it works:**
- Modal visibility controlled by `visible` CEL expression
- Renders with high z-index (1000 for backdrop, 1001 for content)
- CSS transitions for smooth animations:
  - Dialog: fade + scale
  - Drawer: slide from edge
  - Popover: fade + subtle scale
- Escape key closes modal if `closeOnEscape` is `true`
- Backdrop click closes modal if `closeOnBackdropClick` is `true`
- Close button dispatches `onClose` action with `closeParams`
- Popover automatically positions itself relative to anchor element
- Supports nested blocks via BlockRenderer

**CSS Customization:**

All modal styles can be customized via the `css` property:

```yaml
- ::
  type: "modal"
  visible: "${ publisherState.showCustom }"
  css: "background: linear-gradient(to bottom, #667eea, #764ba2); color: white; border-radius: 16px; box-shadow: 0 20px 60px rgba(0,0,0,0.4);"
```

**See:** `/docs/examples/12_modal_demo.huml` for complete modal examples

---

### ✅ `canvas` - GPU-Accelerated Pattern Rendering

Real-time generative art and visualizations using CEL expressions compiled to GPU shaders.

```yaml
- ::
  type: "canvas"
  renderMode: "gpu"
  width: 600
  height: 600
  gridSize: 100
  pattern: "sin(x * 0.1 + time)"
  autoplay: true
  fps: 60
  css: "border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.1);"
```

**Properties:**
- `renderMode` - **Required**: `"cpu"` or `"gpu"`
  - `"cpu"` - OCaml evaluates 10,000 expressions, uploads to GPU texture (60 FPS)
  - `"gpu"` - CEL compiled to GLSL shader, GPU evaluates in parallel (500+ FPS)
- `pattern` - Math expression for pattern generation
  - Can be literal: `"sin(x * 0.1 + time)"`
  - Can be CEL: `"${ storedPattern }"` (evaluates to pattern string)
- `width` - Canvas width in pixels (default: 600)
- `height` - Canvas height in pixels (default: 400)
- `gridSize` - Resolution for pattern evaluation (default: 100)
  - CPU mode: evaluates gridSize × gridSize cells
  - GPU mode: renders at canvas width × height
- `autoplay` - Start animation automatically (default: true)
- `fps` - Target frame rate (default: 60)
- `entities` - Array of shapes for CPU entity rendering (alternative to pattern)
- `css` - Inline styles

**Event Handlers:**
- `onRender` - Called each frame with `{time, frameCount, fps, mouseX, mouseY}`
- `onMouseMove` - Called on mouse movement with `{mouseX, mouseY}`
- `onClick` - Called on click with `{x, y}`
- `onKeyDown` - Called on key press with `{key}`
- `onKeyUp` - Called on key release with `{key}`

**Available Functions (GPU Mode):**

Math functions (22):
- Trigonometry: `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`
- Power/Root: `sqrt`, `pow`, `exp`, `log`, `log10`
- Rounding: `floor`, `ceil`, `round`, `abs`, `sign`
- Comparison: `min`, `max`
- Random: `random()`
- Constants: `pi()`, `e()`

Geometry functions (8):
- `distance(x1, y1, x2, y2)` - Euclidean distance
- `lerp(a, b, t)` - Linear interpolation
- `clamp(value, min, max)` - Constrain value
- `map_range(value, in_min, in_max, out_min, out_max)` - Map ranges
- `angle(x1, y1, x2, y2)` - Angle between points
- `degrees(radians)` - Convert radians to degrees
- `radians(degrees)` - Convert degrees to radians
- `normalize(value, min, max)` - Normalize to 0-1

**Built-in Variables:**
- `x` - Pixel X coordinate (0 to gridSize or width)
- `y` - Pixel Y coordinate (0 to gridSize or height)
- `time` - Animation time in seconds
- `gridSize` - Grid size value
- `mouseX` - Mouse X position
- `mouseY` - Mouse Y position

**Pattern Examples:**

```yaml
# Wave pattern
pattern: "sin(x * 0.1 + time)"

# Ripple from center
pattern: "sin(distance(x, y, 50, 50) * 0.3 - time * 2)"

# Rotating spiral
pattern: "sin(atan2(y - 50, x - 50) * 5 + distance(x, y, 50, 50) * 0.2 - time)"

# Plasma effect
pattern: "sin(x * 0.1) * cos(y * 0.1) + sin(time)"

# Interactive: ripples follow mouse
pattern: "sin(distance(x, y, mouseX, mouseY) * 0.5 - time * 3)"
```

**GPU vs CPU Mode:**

| Feature | CPU Mode | GPU Mode |
|---------|----------|----------|
| Render Method | OCaml evaluates → texture | CEL → GLSL shader |
| Performance | 60 FPS (10K evaluations) | 500+ FPS (parallel) |
| Complexity Impact | High (spiral: 30 FPS) | Low (all: 60 FPS) |
| Supported | Full CEL language | Math expressions only |
| Use Case | Complex logic, entities | Real-time patterns |

**CPU Mode - Entity Rendering:**

Alternative to patterns - render game objects, sprites, shapes:

```yaml
state::
  gameEntities::
    type: "array"
    initial::
      - ::
        type: "circle"
        x: 100
        y: 50
        radius: 25
        color: "#ff0000"
      - ::
        type: "rect"
        x: 200
        y: 100
        width: 50
        height: 30
        color: "#00ff00"

canvas::
  - ::
    type: "canvas"
    renderMode: "cpu"
    entities: "${ gameEntities }"
    width: 600
    height: 400
```

**Architecture:**

GPU mode compilation pipeline:
1. HUML template defines pattern: `"sin(distance(x, y, 50, 50))"`
2. OCaml WASM parses CEL expression to AST
3. GLSL translator converts AST to fragment shader code
4. WebGL2 compiles and runs shader on GPU
5. All pixels evaluated in parallel at native GPU speed

**Performance:**
- **GPU Mode**: 500+ FPS potential (capped at 60 FPS by display)
- **CPU Mode**: 60 FPS for simple patterns, 20-40 FPS for complex
- **Resolution Impact**:
  - CPU: gridSize² evaluations per frame (100×100 = 10,000)
  - GPU: Full resolution (600×600 = 360,000 pixels) at same speed

**Limitations (GPU Mode):**
- Math expressions only (no strings, maps, conditionals)
- No custom functions beyond stdlib
- Uniforms must be float values
- Falls back to CPU if compilation fails

**Interactive Example:**

```yaml
state::
  selectedPattern::
    type: "string"
    initial: "wave"

blocks::
  # Pattern selector buttons
  - ::
    type: "button"
    content: "Wave"
    action: "setState"
    stateUpdates::
      selectedPattern: "wave"

  - ::
    type: "button"
    content: "Ripple"
    action: "setState"
    stateUpdates::
      selectedPattern: "ripple"

  # GPU-rendered patterns
  - ::
    type: "canvas"
    renderMode: "gpu"
    width: 600
    height: 600
    gridSize: 100
    pattern: "sin(x * 0.1 + time)"
    when: "${ selectedPattern == 'wave' }"
    autoplay: true

  - ::
    type: "canvas"
    renderMode: "gpu"
    width: 600
    height: 600
    gridSize: 100
    pattern: "sin(distance(x, y, 50, 50) * 0.3 - time * 2)"
    when: "${ selectedPattern == 'ripple' }"
    autoplay: true
```

**See:** `/docs/examples/13_canvas_patterns.huml` for complete GPU shader examples

---

## 6. Actions

### ✅ `setState` - Update State

**Update single field:**
```yaml
- ::
  type: "button"
  content: "Increment"
  action: "setState"
  stateUpdates::
    counter: "${ counter + 1 }"
```

**Update multiple fields:**
```yaml
- ::
  type: "button"
  content: "Reset All"
  action: "setState"
  stateUpdates::
    counter: 0
    message: ""
    showAdvanced: false
```

**Set to literal values:**
```yaml
stateUpdates::
  counter: 0              # Literal number
  message: "Hello!"       # Literal string
  showAdvanced: true      # Literal boolean
```

**Set to computed values (use `${ }`):**
```yaml
stateUpdates::
  counter: "${ counter + 1 }"        # Expression
  showAdvanced: "${ !showAdvanced }" # Toggle boolean
```

---

### ✅ `navigate` - Screen Navigation

```yaml
- ::
  type: "button"
  content: "Go to Settings"
  action: "navigate"
  targetScreen: "settings"
```

**Properties:**
- `targetScreen` - Screen `name` to navigate to

---

## 7. Working Examples

See `/docs/examples/` directory for complete, tested HUML templates:

- **`00_simple_counter.huml`** - Basic counter with state, computed values, buttons, navigation
- **`01_cel_functions.huml`** - Built-in CEL functions (timestamp, generateId)
- **`02_form_blocks.huml`** - Complete form example with inputs, textarea, checkbox, label, select, radio
- **`03_content_blocks.huml`** - Links and images with dynamic content, navigation, and sizing controls
- **`04_form_submissions.huml`** - Form submissions with persistent storage in submissionsDoc
- **`07_video_gallery.huml`** - Video upload, staticAssets storage, gallery grid, OCaml-generated asset IDs
- **`08_image_gallery.huml`** - Image upload, staticAssets storage, gallery grid, OCaml-generated asset IDs
- **`09_file_manager.huml`** - File upload/download, staticAssets storage, configurable file types, metadata display
- **`12_modal_demo.huml`** - Modal component examples (dialog, drawer, popover modes)
- **`13_canvas_patterns.huml`** - GPU-accelerated canvas patterns with CEL → GLSL compilation

Each example demonstrates working HUML v0.1.0 syntax and can be imported directly into Sthalam.

---

## 8. Best Practices

### ✅ DO:
- Use `!` for boolean negation, not `not`
- Use `${ }` for pure expressions (disabled, when, computed values, stateUpdates)
- Use `{{ }}` for string interpolation (content, css with dynamic values)
- Use literal values for static data
- Use expressions for computed values
- Test templates incrementally

### ❌ DON'T:
- Use ternary operator (`? :`) - not supported
- Use `not` keyword - use `!` instead
- Mix up `${ }` and `{{ }}` syntax
- Forget to wrap expressions in `${ }` or `{{ }}`
- Mix literal and expression syntax incorrectly

---

## 9. CSS Styling

**Inline CSS:**
```yaml
css: "padding: 1rem; background: #f3f4f6; border-radius: 8px;"
```

**Common Patterns:**
```yaml
# Flexbox
css: "display: flex; gap: 1rem; justify-content: center;"

# Grid
css: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 1rem;"

# Colors
css: "background: #2563eb; color: white;"

# Spacing
css: "padding: 2rem; margin-bottom: 1rem;"
```

---

## 10. Implementation Status

### ✅ FULLY WORKING (20 blocks)
- `screen` - Top-level screen container
- `container` - Flex/grid layout
- `section` - Semantic section wrapper
- `heading` - H1-H6 headings
- `text` - Paragraph text
- `button` - Interactive buttons
- `input` - Text input fields with local state binding
- `textarea` - Multi-line text input with local state binding
- `checkbox` - Boolean checkbox with optional label
- `label` - Form labels with `for` attribute
- `select` - Dropdown select with options
- `radio` - Radio button group
- `link` - Hyperlinks with navigation support
- `image` - Image display with staticAssets storage (asset IDs only, no external URLs)
- `video` - HTML5 video player with staticAssets storage and OCaml asset IDs
- `audio` - HTML5 audio player with staticAssets storage and OCaml asset IDs
- `file` - File display with download button, metadata, and configurable types
- `form` - Form container with submission handling
- `modal` - Multi-purpose modal component (dialog, drawer, popover modes)
- `canvas` - GPU-accelerated pattern rendering with CEL → GLSL compilation

### ✅ FULLY WORKING (Features)
- CEL expression evaluation (WASM)
- HUML parsing (WASM)
- Control flow: `when`, `if/else`, `match`, `forEach`
- Actions: `setState`, `navigate`, `uploadVideo`, `uploadImage`, `uploadFile`, `setFileTypes`
- String interpolation
- Computed values (reactive via `$derived`)
- Reactive state updates
- Local state binding for input fields
- Video upload to staticAssets with OCaml-generated asset IDs
- Video playback with blob URLs (WebKit/GStreamer compatible)
- Image upload to staticAssets with OCaml-generated asset IDs
- Image display with blob URLs (asset IDs only, no external URLs)
- File upload to staticAssets with OCaml-generated asset IDs
- File download with metadata display and browser download
- Configurable file types via `setFileTypes` action
- Built-in CEL functions: `timestamp()`, `generateId()`, `size()`, `contains()`

### 🚧 STUB/TODO (0 blocks)
All blocks are now fully implemented!

### 🚧 NOT IMPLEMENTED (Actions)
These are not registered in actionDispatcher:
- `publishPost`, `updatePost`, `deletePost`
- `addComment`, `likePost`
- Custom actions (can be added via actionDispatcher)

---

## 11. Performance

- **CEL Evaluation:** 60 FPS for 10,000 expressions/frame (CPU mode)
- **GPU Shaders:** 500+ FPS potential for canvas patterns (display-limited to 60 FPS)
- **HUML Parsing:** 10KB template in ~15ms
- **WASM Loading:** < 100ms cold start
- **State Updates:** Immediate reactivity
- **Canvas Rendering:**
  - GPU mode: All patterns at 60 FPS regardless of complexity
  - CPU mode: Simple patterns 60 FPS, complex patterns 20-40 FPS

---

## 12. Troubleshooting

### Array Syntax Error

**Error:** `Syntax error at line X, column Y: Unexpected character: [`

**Cause:** Using JSON array syntax `[...]` instead of HUML list syntax

**Solution:** In HUML state definitions, arrays must use the `::` list syntax:

```yaml
# ❌ WRONG - JSON array syntax
items::
  type: "array"
  initial: [{"id": 1, "name": "Item"}]

# ✅ CORRECT - HUML list syntax
items::
  type: "array"
  initial::
    - ::
      id: 1
      name: "Item"
    - ::
      id: 2
      name: "Another Item"
```

**Note:** Inside CEL expressions (`${ }` and `{{ }}`), you can still use array operations like `size(items)`, `items.slice()`, etc.

---

### Control Flow Syntax Error

**Error:** `Parse error at line X, column Y: strings must be quoted: if`

**Cause:** Control flow blocks missing `- ::` prefix

**Solution:** All control flow blocks need the `- ::` list dictionary marker:

```yaml
# ❌ WRONG - Missing - ::
- if: "${ counter > 0 }"
  then::
    - ::
      type: "text"

# ✅ CORRECT - Has - ::
- ::
  if: "${ counter > 0 }"
  then::
    - ::
      type: "text"

# ❌ WRONG - Match cases missing - ::
- ::
  match: "${ view }"
  cases::
    - value: "grid"

# ✅ CORRECT - Each case has - ::
- ::
  match: "${ view }"
  cases::
    - ::
      value: "grid"
```

**Rule:** Control flow blocks (`if`, `match`) and their sub-items (`cases`) are list dictionaries and need `- ::`

---

### Parser Errors

**Error:** `Cel.Cel_parser.MenhirBasics.Error`

**Common Causes:**
1. Using `not` instead of `!`
2. Using ternary operator `? :`
3. Syntax error in CEL expression

**Solution:** Check CEL syntax, use `!` for negation, avoid ternary

---

### Button Not Working

**Cause:** Missing `stateUpdates` or `targetScreen`

**Solution:**
```yaml
# ✅ CORRECT - setState needs stateUpdates
- ::
  type: "button"
  content: "Increment"
  action: "setState"
  stateUpdates::
    counter: "${ counter + 1 }"

# ✅ CORRECT - navigate needs targetScreen
- ::
  type: "button"
  content: "Navigate"
  action: "navigate"
  targetScreen: "settings"
```

---

### State Not Updating

**Cause:** Expression not wrapped in `${ }`

**Solution:**
```yaml
# ❌ WRONG - Missing expression markers
stateUpdates::
  counter: counter + 1

# ✅ CORRECT - Use ${ } for expressions
stateUpdates::
  counter: "${ counter + 1 }"
```

---

## 13. Example Templates

All examples are in `/docs/examples/` with README:

| File | Demonstrates |
|------|-------------|
| `00_simple_counter.huml` | State, computed values, buttons, navigation, conditional rendering |
| `01_cel_functions.huml` | Built-in functions: `timestamp()`, `generateId()` |
| `02_form_blocks.huml` | Complete forms with input, textarea, checkbox, label, select, radio, validation |
| `03_content_blocks.huml` | Links (external and navigation), images with dynamic sizing, grid layouts |
| `04_form_submissions.huml` | Form submissions with persistent storage, SubmissionsViewer integration |
| `05_control_flow.huml` | Control flow: if/else, match/cases, forEach iteration, when conditionals |
| `07_video_gallery.huml` | Video upload to staticAssets, gallery with metadata, grid display, OCaml asset IDs |
| `08_image_gallery.huml` | Image upload to staticAssets, gallery grid, two-step workflow, OCaml asset IDs |
| `09_file_manager.huml` | File upload/download, staticAssets storage, configurable types, metadata display |

See `/docs/examples/README.md` for detailed descriptions and usage instructions.

---

## 14. Next Steps

### Learn by Example:
1. **[Example Templates](../examples/)** - Working HUML templates you can import and modify
2. **[Examples README](../examples/README.md)** - Detailed guide to each example

### For Developers:
1. **[Integration Guide](../guides/INTEGRATION_GUIDE.md)** - Development environment setup
2. **[WASM Performance](../guides/WASM_TAURI_KNOWLEDGE.md)** - Performance optimizations
3. **[Block Renderer Spec](BLOCK_RENDERER_SPEC.md)** - Aspirational block specifications

### For LLMs Generating Templates:
- Reference `/docs/examples/` for working patterns
- Follow HUML v0.1.0 syntax in "CRITICAL SYNTAX RULES" section
- Check "Implementation Status" for available blocks
- Use only documented block properties

---

**Last Updated:** 2025-01-05
**Implementation Rate:** 83% (15/18 blocks functional)
**Status:** ✅ All documented features verified against actual code
**Syntax:** ✅ Updated to HUML v0.1.0 specification
