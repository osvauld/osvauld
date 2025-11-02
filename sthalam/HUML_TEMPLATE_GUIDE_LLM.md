# HUML Template Guide - LLM Optimized

**Version:** 4.0 CEL Expression Language with Fine-Grained Reactivity
**Purpose:** Compressed reference for AI assistants creating HUML templates

---

## 📖 HUML Specification

**ALWAYS refer to the official HUML specification first:**
- **Spec URL:** https://huml.io/specifications/v0-1-0/
- **Key points:**
  - Strings use double quotes: `"text"`
  - CEL expressions inside strings: `"{{expression}}"`
  - Inside CEL, use single quotes for strings: `"{{condition ? 'yes' : 'no'}}"`
  - **Library**: Uses `@marcbachmann/cel-js` which supports method syntax: `items.size()`, `str.contains()`, etc.
  - Indentation: Strictly 2 spaces per level
  - Spacing: Exactly one space after `:` or `::`
  - Scalar keys: Single colon `:`
  - Vector keys: Double colon `::`
  - No trailing spaces allowed

### 🎯 HUML Syntax TL;DR

**Quick reference for common patterns:**

```huml
# Scalars (single values) - use single colon
name: "Tech Summit"
count: 42
active: true

# Vectors (collections) - use double colon
colors:: "red", "green", "blue"    # Inline list
items:: []                          # Empty array
config:: {}                         # Empty object

# Multi-line arrays
products::
  - ::
    name: "Product A"
    price: 99.99
  - ::
    name: "Product B"
    price: 149.99

# Strings
text: "Double quotes for strings"
multi: """
  Multi-line text
  Stripped whitespace
  """
preserved: ```
  Preserved
    spacing
  ```

# CEL expressions (always in double-quoted strings)
content: "{{firstName + ' ' + lastName}}"
visible: "{{count > 5 && isActive}}"
css: "{{isSelected ? 'bg-blue' : 'bg-gray'}}"

# Nested blocks (use blocks:: for children)
screens::
  - ::
    id: "main"
    name: "Main Screen"
    blocks::
      - ::
        type: "section-container"
        blocks::
          - ::
            type: "heading"
            content: "Hello World"
```

**Common gotchas:**
- ✅ `items:: []` (empty array - double colon + space + brackets)
- ❌ `items: []` (wrong - scalars can't be arrays)
- ❌ `items::[]` (wrong - missing space after ::)
- ✅ Exactly 2 spaces per indent level (not tabs!)
- ✅ One space after `:` and `::`
- ✅ No trailing spaces on any line
- ✅ Use `blocks::` to nest child blocks

---

## ⚡ PERFORMANCE & REACTIVITY

**HUML uses fine-grained Preact signals for optimal performance.**

### How It Works

- Each state property is a **separate signal** (not one big state object)
- Blocks **automatically track** which properties they use
- Blocks **only re-render** when properties they ACCESS actually change
- **Zero unnecessary re-renders** - maximum performance

### What This Means For You

```huml
state::
  totalItems: 0
  totalPrice: 0
  products::
    - name: "Item 1"
      price: 10.00
```

**Example behavior:**
- Text showing `{{totalItems}}` - only re-renders when `totalItems` changes
- Text showing `{{totalPrice}}` - only re-renders when `totalPrice` changes
- Product grid showing `{{products}}` - only re-renders when `products` changes
- **Updating `totalItems` does NOT re-render blocks using only `totalPrice` or `products`**

**You don't need to do anything special - this is automatic!** 🎉

---

## 🆕 CEL SUPPORT - STATE-DRIVEN TEMPLATES

### ⚡ Quick Start: CEL in 3 Steps

1. **Define state** in template root:
```huml
state::
  currentSection: "home"
```

2. **Use `setState` action** on buttons:
```huml
- ::
  type: "nav-button"
  content: "Go to About"
  action: "setState"
  stateKey: "currentSection"
  stateValue: "about"
```

3. **Control visibility** with CEL expressions:
```huml
- ::
  type: "section-container"
  visible: "{{currentSection == 'about'}}"
```

### 🔄 Breaking Changes
- **❌ REMOVED**: `action: "show"`, `"hide"`, `"toggle"`
- **✅ USE INSTEAD**: `action: "setState"` + CEL `visible` expressions

### ⚡ CEL Evaluation: Synchronous & Sandboxed

**All CEL expressions are evaluated synchronously in a secure sandbox:**

**Architecture Benefits:**
- ✅ Enables true fine-grained reactivity with Preact signals
- ✅ Predictable execution order - no race conditions
- ✅ Better performance - no promise overhead
- ✅ Works perfectly with dependency tracking

**Security: Safe Expression Language**
CEL (Common Expression Language) is a non-Turing complete language designed for safe expression evaluation:

**Available Operations:**
- ✅ **Native Array Methods:** `.size()`, `.filter()`, `.map()`, `.all()`, `.exists()`
- ✅ **String Methods:** `.contains()`, `.startsWith()`, `.endsWith()`, `.matches()`
- ✅ **Helper Functions:** `sum()`, `avg()`, `toFixed()`, `uppercase()`, `lowercase()`, `capitalize()`
- ✅ **Type Conversion:** `Number()`, `String()`, `Boolean()`, `parseInt()`, `parseFloat()`, `isNaN()`
- ✅ **Math:** `Math.round()`, `Math.floor()`, `Math.ceil()`, `Math.abs()`, `Math.min()`, `Math.max()`, `Math.pow()`, `Math.sqrt()`, `Math.random()`
- ✅ **Operators:** `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `>`, `<`, `>=`, `<=`, `&&`, `||`, `!`, `?:`
- ✅ **Template state:** All state properties defined in `state::`
- ✅ **Loop variables:** `item`, `index`, `first`, `last` (inside forEach)

**NOT Available (Sandboxed Out):**
- ❌ `fetch`, `XMLHttpRequest` - No network access
- ❌ `eval`, `Function` - No dynamic code execution
- ❌ `window`, `document`, `global` - No global object access
- ❌ `require`, `import` - No module loading
- ❌ File system, DOM APIs, timers, etc.

**Example:**
```huml
# ✅ GOOD - Whitelisted operations
display: "{{String(previousValue + Number(display))}}"
expression: "{{display + ' ÷ ' + operand}}"
visible: "{{operation != '' && !isLoading}}"
rounded: "{{Math.round(price * 100) / 100}}"
upperName: "{{name | uppercase}}"

# ❌ NOT AVAILABLE - Not in whitelist
data: "{{fetch('/api/data')}}"  # fetch not exposed
code: "{{eval('malicious')}}"    # eval not exposed
elem: "{{document.createElement('div')}}"  # DOM not exposed
```

**For dynamic/async data:** Load data into state first (via Yjs sync, initial data, backend) → Then display with CEL

---

## 🚨 CRITICAL RULES - READ FIRST

### 1. Form Buttons - Automatic Caching

**✅ GOOD NEWS: Form data is automatically cached as users navigate!**

**How it works:**
- Any button with `formId` automatically caches all form data when clicked
- Add `submit: true` only on the FINAL submit button
- All other buttons just cache and navigate

**Simple Pattern:**
```huml
# Screen 1 - Intermediate button
- ::
  type: "nav-button"
  content: "Next"
  formId: "form-reg"
  targetContainerId: "screen-2"
  # Automatically caches all visible fields!

# Screen 2 - Choice button
- ::
  type: "nav-button"
  content: "Large"
  formId: "form-reg"
  fieldName: "tshirt_size"
  value: "L"
  targetContainerId: "screen-3"
  # Automatically caches the choice + all visible fields!

# Screen 3 - Final submit button
- ::
  type: "nav-button"
  content: "Submit"
  formId: "form-reg"
  submit: true  # ← Only add this on final button!
  targetContainerId: "confirmation"
  # Caches + SUBMITS all data!
```

**Key Rules:**
1. Add `formId` to ALL buttons in the form flow
2. Add `submit: true` ONLY on the final submit button
3. All data is automatically cached across screens

### 2. Container Naming Requirement

**ALL `section-container` blocks MUST have a descriptive `name` property.**

```huml
# ✅ CORRECT
- ::
  type: "section-container"
  name: "Hero Section"
  children::
    # ...

# ❌ WRONG - Missing name
- ::
  type: "section-container"
  children::
    # ...
```

### 3. Thread Blocks Use CSS Variables ONLY

Thread blocks ONLY accept CSS custom properties. Regular CSS and classes DON'T work.

```huml
# ✅ CORRECT
- ::
  type: "thread"
  name: "Discussion"
  css: "--thread-bg: #1a1a1a; --thread-title-color: #00ff88; --thread-button-bg: #00ff88; --thread-input-bg: #2a2a2a;"

# ❌ WRONG - Regular CSS (ignored)
- ::
  type: "thread"
  name: "Discussion"
  css: "background: #1a1a1a; border-radius: 8px;"
```

**Critical for dark themes:** Must include `--thread-comment-content-color`, `--thread-toggle-bg`, `--thread-toggle-color`, `--thread-scrollbar-track`, `--thread-scrollbar-thumb` or text/elements will be invisible!

---

## CEL Expressions - Complete Guide

### State Definition

Add state at template root level:
```huml
name: "My Website"

state::
  currentSection: "home"
  theme: "dark"
  formSubmitted: false
  user::
    name: "Guest"
    role: "viewer"

screens::
  # ... your screens
```

### Multi-Document Architecture - Separating Content & State

**NEW**: For complex apps (e-commerce, content-heavy sites), separate publisher content from user state.

#### Three Data Sources

1. **`state::`** - In-memory state (backward compatible)
2. **`content::`** - Publisher-owned data (products, articles) - **shared across users**
3. **`user_content::`** - Per-user state (cart, progress) - **isolated per user**

**Merge Priority**: `state` < `content` < `user_content` (user state wins)

#### E-commerce Example

```huml
name: "Product Store"

# Publisher content (products catalog - shared)
content::
  products::
    - ::
      name: "Running Shoes"
      price: 89.99
      image: "https://example.com/shoes.jpg"
      description: "Comfortable running shoes"
    - ::
      name: "T-Shirt"
      price: 24.99
      image: "https://example.com/shirt.jpg"
      description: "Cotton t-shirt"

# Per-user state (cart - isolated)
user_content::
  totalItems: 0
  totalPrice: 0

screens::
  - ::
    children::
      # Loop over products (from content::)
      - ::
        forEach: "products"
        type: "section-container"
        children::
          - ::
            type: "heading"
            content: "{{item.name}}"

          - ::
            type: "text"
            content: "${{item.price}}"

          # Update user state
          - ::
            type: "nav-button"
            content: "Add to Cart"
            action: "setState"
            stateUpdates::
              totalItems: "{{Number(totalItems) + 1}}"
              totalPrice: "{{Number(totalPrice) + Number(item.price)}}"

      # Display cart (from user_content::)
      - ::
        type: "text"
        content: "Cart: {{totalItems}} items (${{totalPrice | toFixed(2)}})"
```

**When to Use Multi-Document**:
- ✅ E-commerce (products in `content::`, cart in `user_content::`)
- ✅ Courses (lessons in `content::`, progress in `user_content::`)
- ✅ CMS (articles in `content::`, bookmarks in `user_content::`)
- ❌ Simple sites (just use `state::`)

**Backend Storage**:
- `content::` → `content_doc` (Yjs document, synced)
- `user_content::` → `user_content_doc` (Yjs document, per-user)
- All three are merged in CEL expressions

**⚡ Performance Note**:
Each property (from any source) gets its own fine-grained signal. Updating `totalItems` in `user_content::` only re-renders blocks that USE `totalItems` - blocks displaying `products` from `content::` won't re-render. Maximum performance!

### CEL Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `==` | Equality **(NOT ===!)** | `{{section == 'home'}}` |
| `!=` | Inequality | `{{role != 'admin'}}` |
| `&&` | Logical AND | `{{isAdmin && isActive}}` |
| `\|\|` | Logical OR | `{{isAdmin \|\| isModerator}}` |
| `!` | Logical NOT | `{{!isHidden}}` |
| `>`, `<`, `>=`, `<=` | Comparison | `{{count > 5}}` |
| `? :` | Ternary | `{{theme == 'dark' ? '#000' : '#fff'}}` |

**⚠️ CRITICAL**: CEL uses `==` not `===`!

### Common Patterns

#### Pattern 1: Single-Screen with Sections

```huml
state::
  currentSection: "home"

screens::
  - ::
    id: "main"
    isEntryPoint: true
    children::
      # Navigation sidebar
      - ::
        type: "nav-button"
        content: "Home"
        action: "setState"
        stateKey: "currentSection"
        stateValue: "home"

      - ::
        type: "nav-button"
        content: "About"
        action: "setState"
        stateKey: "currentSection"
        stateValue: "about"

      # Content sections (conditionally visible)
      - ::
        type: "section-container"
        name: "Home Section"
        visible: "{{currentSection == 'home'}}"
        children::
          # Home content

      - ::
        type: "section-container"
        name: "About Section"
        visible: "{{currentSection == 'about'}}"
        children::
          # About content
```

#### Pattern 2: Multi-Step Form with CEL

```huml
state::
  currentStep: 1
  formSubmitted: false

screens::
  - ::
    id: "form"
    isEntryPoint: true
    children::
      - ::
        type: "form"
        id: "my-form"

      # Step 1
      - ::
        type: "section-container"
        name: "Step 1"
        visible: "{{currentStep == 1 && !formSubmitted}}"
        children::
          - ::
            type: "form-field-text"
            fieldName: "name"
            formId: "my-form"

          - ::
            type: "nav-button"
            content: "Next"
            action: "setState"
            stateKey: "currentStep"
            stateValue: 2
            formId: "my-form"  # Caches data

      # Step 2
      - ::
        type: "section-container"
        name: "Step 2"
        visible: "{{currentStep == 2 && !formSubmitted}}"
        children::
          - ::
            type: "form-field-email"
            fieldName: "email"
            formId: "my-form"

          - ::
            type: "nav-button"
            content: "Submit"
            action: "setState"
            stateKey: "formSubmitted"
            stateValue: true
            formId: "my-form"
            submit: true  # ← Submits!

      # Success
      - ::
        type: "section-container"
        name: "Success"
        visible: "{{formSubmitted}}"
        children::
          - ::
            type: "heading"
            content: "Thank you!"
```

**Key Points:**
- `formId` on buttons → caches form data
- `submit: true` on final button → actually submits
- CEL `visible` → controls what shows
- State updates → sections reactively show/hide

#### Pattern 3: Dynamic Styling

```huml
state::
  theme: "dark"

- ::
  type: "section-container"
  name: "Themed Container"
  css: "background: {{theme == 'dark' ? '#1a1a1a' : '#ffffff'}}; color: {{theme == 'dark' ? '#ffffff' : '#000000'}};"
  children::
    - ::
      type: "nav-button"
      content: "Toggle Theme"
      action: "setState"
      stateKey: "theme"
      stateValue: "{{theme == 'dark' ? 'light' : 'dark'}}"
```

#### Pattern 4: Content Interpolation

```huml
state::
  user::
    name: "John Doe"

- ::
  type: "heading"
  content: "Welcome, {{user.name}}!"
```

### setState Actions

#### Single Property Update
```huml
- ::
  type: "nav-button"
  content: "Show About"
  action: "setState"
  stateKey: "currentSection"
  stateValue: "about"
```

#### Bulk Update (Multiple Properties)
```huml
- ::
  type: "nav-button"
  content: "Reset"
  action: "setState"
  stateUpdates::
    currentSection: "home"
    formSubmitted: false
    currentStep: 1
```

#### Toggle Boolean
```huml
- ::
  type: "nav-button"
  content: "Toggle Menu"
  action: "setState"
  stateKey: "isMenuOpen"
  stateValue: "{{!isMenuOpen}}"
```

### Conditional Visibility

**Any block type** can use `visible` property:

```huml
# Boolean
visible: true
visible: false

# CEL expression (evaluate to boolean)
visible: "{{currentSection == 'home'}}"
visible: "{{isAdmin || isModerator}}"
visible: "{{count > 0}}"
visible: "{{user.role == 'admin' && isActive}}"
```

### Types and Data Handling

#### Type Coercion

CEL expressions should explicitly coerce types to avoid NaN errors:

```huml
# ✅ GOOD - Explicit type coercion
stateUpdates::
  count: "{{Number(count) + 1}}"
  total: "{{Number(total) + Number(item.price)}}"
  percentage: "{{(Number(completed) / Number(total)) * 100}}"

# ❌ BAD - Can result in NaN or string concatenation
stateUpdates::
  count: "{{count + 1}}"  # Might do "01" instead of 1
  total: "{{total + item.price}}"  # Might do "089.99" instead of 89.99
```

#### Number Formatting

Use CEL transforms (with pipe `|`) for formatting:

```huml
# Format currency with 2 decimals (use | transform)
content: "Total: ${{totalPrice | toFixed(2)}}"

# Round to integer (use Math functions)
content: "Score: {{Math.round(score)}}"

# Percentage
content: "Progress: {{((Number(completed) / Number(total)) * 100) | toFixed(1)}}%"

# ❌ WRONG - Cannot use JavaScript methods directly
content: "Total: ${{totalPrice.toFixed(2)}}"  # Error: toFixed not defined

# ✅ RIGHT - Use transform with pipe
content: "Total: ${{totalPrice | toFixed(2)}}"
```

#### Array Operations - Why No `push()`?

CEL is a **pure expression language** - it doesn't support mutations like `push()`, `pop()`, `splice()`.

**Why?** Pure expressions:
- Have no side effects
- Always return new values
- Are easier to reason about and debug
- Enable time-travel debugging and undo/redo

**Solution:** Use spread operator to create new arrays:

```huml
state::
  cart::
    - ::
      name: "Item 1"
      price: 10.00

# ❌ DOESN'T WORK - Cannot mutate arrays
stateUpdates::
  cart: "{{cart.push(newItem)}}"  # Error!

# ✅ WORKS - Create new array with spread
stateUpdates::
  cart: "{{[...cart, newItem]}}"  # Adds to end
  cart: "{{[newItem, ...cart]}}"  # Adds to start

# Remove item at index
stateUpdates::
  cart: "{{cart.filter((item, i) => i !== indexToRemove)}}"

# Update item property
stateUpdates::
  cart: "{{cart.map(item => item.id === targetId ? {...item, quantity: item.quantity + 1} : item)}}"
```

#### Available CEL Functions & Methods

**Using @marcbachmann/cel-js** - Supports both method syntax AND function syntax!

```huml
# Native CEL Methods (use these!)
items.size()            # Get array/string length
str.contains("text")    # Check if string contains substring
str.startsWith("pre")   # Check if starts with prefix
str.endsWith("suf")     # Check if ends with suffix
str.matches("regex")    # Match against regex pattern
items.filter(x, x > 5)  # Filter array elements
items.map(x, x * 2)     # Transform array elements
items.all(x, x > 0)     # Check if all elements match condition
items.exists(x, x > 5)  # Check if any element matches condition

# Built-in Type Conversion
string(value)           # Convert to string: string(123) → "123"
int(value)              # Convert to integer: int("42") → 42
double(value)           # Convert to double: double("3.14") → 3.14
type(value)             # Get type name: type(42) → "int"

# Custom Helper Functions (registered in celEvaluator.ts)
toFixed(num, decimals)  # Format number with decimals: toFixed(19.99, 2) → "19.99"
uppercase(str)          # Convert to UPPERCASE
lowercase(str)          # Convert to lowercase
capitalize(str)         # Capitalize First Letter
sum(array, prop)        # Sum array values: sum(items, 'price')
avg(array, prop)        # Average array values: avg(items, 'quantity')
keys(object)            # Get object keys
values(object)          # Get object values
```

**Examples:**
```huml
# ✅ Correct (native CEL method syntax)
content: "{{items.size()}}"
content: "{{name.contains('John')}}"
visible: "{{selectedWorkshops.exists(w, w == 'ws1')}}"
items: "{{products.filter(p, p.price > 100)}}"

# ✅ Correct (built-in function syntax)
content: "{{string(count) + ' items'}}"
content: "{{int('42') + 10}}"

# ✅ Correct (custom helper functions)
content: "{{uppercase(name)}}"
content: "{{toFixed(price, 2)}}"
total: "{{sum(items, 'quantity')}}"

# ❌ Wrong (JavaScript methods - not supported)
content: "{{name.toUpperCase()}}"      # NO! Use uppercase(name)
content: "{{items.length}}"             # NO! Use items.size()
```

---

## Basic Structure

```huml
name: "Application Name"

screens::
  - ::
    id: "home"
    name: "Home Screen"
    isEntryPoint: true
    css: "background: #f0f0f0; padding: 20px;"
    children::
      - ::
        type: "heading"
        content: "Welcome"
      - ::
        type: "nav-button"
        content: "Go to Next Page"
        targetContainerId: "about"

  - ::
    id: "about"
    name: "About Screen"
    children::
      - ::
        type: "text"
        content: "About page content"
```

**Rules:**
- Exactly ONE screen has `isEntryPoint: true`
- All screen `id` values must be unique
- Use 2 spaces for indentation
- NO trailing spaces on lines
- Strings should use quotes

---

## Block Types Quick Reference

### `heading`
```huml
- ::
  type: "heading"
  content: "Page Title"
  css: "font-size: 48px; color: #667eea; text-align: center;"
```

### `text`
```huml
- ::
  type: "text"
  content: "Regular paragraph text"
  css: "color: #4a5568; font-size: 16px; line-height: 1.6;"
```

### `markdown-text`
```huml
- ::
  type: "markdown-text"
  mode: "markdown"
  content: "## Header\n\nThis is **bold** and *italic*.\n\n- List item 1\n- List item 2"
  css: "padding: 20px; line-height: 1.8;"
```

### `section-container`
```huml
- ::
  type: "section-container"
  name: "Card Container"  # ← REQUIRED
  css: "background: white; border-radius: 12px; padding: 30px; box-shadow: 0 4px 6px rgba(0,0,0,0.1);"
  children::
    - ::
      type: "heading"
      content: "Card Title"
    - ::
      type: "text"
      content: "Card content"
```

### `image`
```huml
- ::
  type: "image"
  src: "https://example.com/image.jpg"
  alt: "Description"
  css: "width: 100%; max-width: 600px; border-radius: 8px;"
```

### `thread` (Comments)
```huml
- ::
  type: "thread"
  name: "Discussion"  # Optional heading
  mode: "markdown"    # or "plain"
  description: "Share your thoughts..."  # Placeholder text
  css: "--thread-bg: #1a1a1a; --thread-title-color: #00ff88; --thread-comment-content-color: #ffffff; --thread-button-bg: #00ff88; --thread-button-text: #000000; --thread-input-bg: #2a2a2a; --thread-input-border: #00ff88; --thread-input-text: #ffffff; --thread-comment-bg: #2a2a2a; --thread-comment-author-color: #00ff88; --thread-toggle-bg: #2a2a2a; --thread-toggle-color: #ffffff; --thread-scrollbar-track: #1a1a1a; --thread-scrollbar-thumb: #00ff88;"
```

**Valid CSS variables only (see reference below)**

### `modal`
```huml
- ::
  type: "modal"
  name: "Confirmation Modal"
  css: "background: white; border-radius: 12px; padding: 30px; max-width: 500px; position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); z-index: 1000;"
  visible: false  # Start hidden
  children::
    - ::
      type: "heading"
      content: "Are you sure?"
    - ::
      type: "nav-button"
      content: "Confirm"
      targetContainerId: "next-screen"
```

---

## Navigation Button (`nav-button`)

**The ONLY button type in HUML.** Has 3 simple modes:

### MODE 1: Simple Navigation (No Form)
```huml
- ::
  type: "nav-button"
  content: "Go to About"
  targetContainerId: "about"
  css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px;"
```

### MODE 2: Form Navigation (Auto-Cache)
```huml
# Between form screens - auto-caches data
- ::
  type: "nav-button"
  content: "Next →"
  formId: "form-reg"
  targetContainerId: "screen-2"
  css: "background: #667eea; color: white; padding: 15px 30px;"

# Choice button - auto-caches choice + visible fields
- ::
  type: "nav-button"
  content: "Large"
  formId: "form-reg"
  fieldName: "tshirt_size"
  value: "L"
  targetContainerId: "screen-3"
  css: "background: #45475a; color: white; padding: 20px;"
```

### MODE 3: Form Submission (Final Button)
```huml
- ::
  type: "nav-button"
  content: "Submit Form"
  formId: "form-contact"
  submit: true  # ← Add this!
  targetContainerId: "thank-you"
  css: "background: #667eea; color: white; padding: 15px 30px;"
```

**Quick Comparison:**

| Mode | formId | submit | fieldName+value | Behavior |
|------|--------|--------|-----------------|----------|
| 1 | ❌ | ❌ | ❌ | Navigate only |
| 2 | ✅ | ❌ | Optional | Cache + Navigate |
| 3 | ✅ | ✅ | Optional | Cache + Submit + Navigate |

---

## Forms - Complete Guide

### 🎯 Decision Tree: Which Button Properties?

```
Creating a form button?
│
├─ Is this the FINAL submit button?
│  ├─ YES → Add formId + submit: true + targetContainerId
│  └─ NO  → Add formId + targetContainerId (auto-caches)
│
└─ Is this a choice button (like t-shirt size)?
   ├─ YES → Add formId + fieldName + value + targetContainerId
   └─ NO  → Just add formId + targetContainerId
```

**Simple Rules:**
1. **All form buttons:** Add `formId`
2. **Final submit button:** Add `submit: true`
3. **Choice buttons:** Add `fieldName` + `value`
4. **Everything else:** Automatic!

### Form Metadata Properties

**Required:**
- `type: "form"` - Identifies this as a form definition
- `id` - Unique form identifier (referenced by all fields/buttons)

**Optional:**
- `name` - Display name for the form
- `eventName` - Custom event name for tracking/analytics
- `submitEndpoint` - URL where form data will be submitted (e.g., Formspree)

```huml
- ::
  type: "form"
  id: "form-contact"
  name: "Contact Form"
  eventName: "contact_submission"
  submitEndpoint: "https://formspree.io/f/YOUR_FORM_ID"
```

### Single-Screen Form (Simple)

```huml
screens::
  - ::
    id: "contact"
    name: "Contact Form"
    isEntryPoint: true
    children::
      # 1. Form metadata
      - ::
        type: "form"
        id: "form-contact"
        name: "Contact Form"
        eventName: "contact_submission"
        submitEndpoint: "https://formspree.io/f/YOUR_FORM_ID"

      # 2. Form container
      - ::
        type: "section-container"
        name: "Form Container"
        css: "max-width: 600px; margin: 0 auto; padding: 40px; background: white; border-radius: 12px;"
        children::
          - ::
            type: "heading"
            content: "Contact Us"

          # 3. Form fields
          - ::
            type: "form-field-text"
            formId: "form-contact"
            fieldName: "name"
            label: "Full Name"
            placeholder: "Enter your name"
            required: true

          - ::
            type: "form-field-email"
            formId: "form-contact"
            fieldName: "email"
            label: "Email"
            placeholder: "you@example.com"
            required: true

          - ::
            type: "form-field-textarea"
            formId: "form-contact"
            fieldName: "message"
            label: "Message"
            placeholder: "Your message..."
            required: true

          # 4. Submit button
          - ::
            type: "nav-button"
            content: "Send Message"
            formId: "form-contact"
            submit: true  # ← Mark as submit button
            targetContainerId: "thank-you"
            css: "width: 100%; background: #667eea; color: white; padding: 15px;"

  - ::
    id: "thank-you"
    name: "Thank You"
    children::
      - ::
        type: "heading"
        content: "Thank you! We'll be in touch soon."
```

### Multi-Screen Form (Complete Pattern)

```huml
screens::
  - ::
    id: "home"
    name: "Registration Home"
    isEntryPoint: true
    children::
      - ::
        type: "heading"
        content: "Register for Event"
      - ::
        type: "nav-button"
        content: "Start Registration"
        targetContainerId: "reg-screen-1"

  # SCREEN 1: Basic Info
  - ::
    id: "reg-screen-1"
    name: "Registration - Step 1"
    children::
      # Form metadata (only needed once)
      - ::
        type: "form"
        id: "form-registration"
        submitEndpoint: "https://formspree.io/f/YOUR_FORM_ID"

      - ::
        type: "section-container"
        name: "Step 1 Container"
        css: "max-width: 600px; margin: 0 auto; padding: 40px; background: white;"
        children::
          - ::
            type: "heading"
            content: "Step 1: Your Information"

          - ::
            type: "form-field-text"
            formId: "form-registration"
            fieldName: "full_name"
            label: "Full Name"
            required: true

          - ::
            type: "form-field-email"
            formId: "form-registration"
            fieldName: "email"
            label: "Email"
            required: true

          # Continue button (auto-caches)
          - ::
            type: "nav-button"
            content: "Next →"
            formId: "form-registration"
            targetContainerId: "reg-screen-2"
            css: "background: #667eea; color: white; padding: 15px 30px;"

  # SCREEN 2: T-Shirt Size Selection
  - ::
    id: "reg-screen-2"
    name: "Registration - Step 2"
    children::
      - ::
        type: "section-container"
        name: "Step 2 Container"
        css: "max-width: 600px; margin: 0 auto; padding: 40px;"
        children::
          - ::
            type: "heading"
            content: "Step 2: Choose T-Shirt Size"

          # Choice buttons (auto-cache choice + visible fields)
          - ::
            type: "section-container"
            name: "Size Buttons"
            css: "display: flex; gap: 15px; margin-top: 30px;"
            children::
              - ::
                type: "nav-button"
                content: "S"
                formId: "form-registration"
                fieldName: "tshirt_size"
                value: "S"
                targetContainerId: "reg-screen-3"
                css: "flex: 1; background: #45475a; color: white; padding: 20px;"

              - ::
                type: "nav-button"
                content: "M"
                formId: "form-registration"
                fieldName: "tshirt_size"
                value: "M"
                targetContainerId: "reg-screen-3"
                css: "flex: 1; background: #45475a; color: white; padding: 20px;"

              - ::
                type: "nav-button"
                content: "L"
                formId: "form-registration"
                fieldName: "tshirt_size"
                value: "L"
                targetContainerId: "reg-screen-3"
                css: "flex: 1; background: #45475a; color: white; padding: 20px;"

          - ::
            type: "nav-button"
            content: "← Back"
            targetContainerId: "reg-screen-1"
            css: "margin-top: 20px; background: #6c757d; color: white; padding: 12px 24px;"

  # SCREEN 3: Final Details & Submit
  - ::
    id: "reg-screen-3"
    name: "Registration - Step 3"
    children::
      - ::
        type: "section-container"
        name: "Step 3 Container"
        css: "max-width: 600px; margin: 0 auto; padding: 40px; background: white;"
        children::
          - ::
            type: "heading"
            content: "Step 3: Final Details"

          - ::
            type: "form-field-textarea"
            formId: "form-registration"
            fieldName: "dietary_restrictions"
            label: "Dietary Restrictions"
            placeholder: "Any dietary needs?"
            required: false

          - ::
            type: "section-container"
            name: "Button Group"
            css: "display: flex; gap: 15px; margin-top: 30px;"
            children::
              - ::
                type: "nav-button"
                content: "← Back"
                targetContainerId: "reg-screen-2"
                css: "flex: 1; background: #6c757d; color: white; padding: 12px 24px;"

              # Final submit button
              - ::
                type: "nav-button"
                content: "Complete Registration ✓"
                formId: "form-registration"
                submit: true  # ← Mark as submit button
                targetContainerId: "confirmation"
                css: "flex: 2; background: #28a745; color: white; padding: 12px 24px;"

  - ::
    id: "confirmation"
    name: "Registration Complete"
    children::
      - ::
        type: "section-container"
        name: "Confirmation Container"
        css: "text-align: center; padding: 60px 20px;"
        children::
          - ::
            type: "heading"
            content: "✅ Registration Complete!"
            css: "color: #28a745; font-size: 48px;"

          - ::
            type: "text"
            content: "Thank you for registering! Check your email for confirmation."
```

### Form Field Types

#### `form-field-text`
```huml
- ::
  type: "form-field-text"
  formId: "form-contact"
  fieldName: "full_name"
  label: "Full Name"
  placeholder: "Enter your name"
  required: true
  css: "margin-bottom: 20px;"
```

#### `form-field-email`
```huml
- ::
  type: "form-field-email"
  formId: "form-contact"
  fieldName: "email"
  label: "Email Address"
  placeholder: "you@example.com"
  required: true
```

#### `form-field-textarea`
```huml
- ::
  type: "form-field-textarea"
  formId: "form-contact"
  fieldName: "message"
  label: "Message"
  placeholder: "Your message here..."
  required: false
```

#### `form-field-checkbox`
```huml
- ::
  type: "form-field-checkbox"
  formId: "form-survey"
  fieldName: "subscribe"
  label: "Subscribe to newsletter"
  css: "margin-bottom: 15px;"
```

#### `form-field-tel`
```huml
- ::
  type: "form-field-tel"
  formId: "form-contact"
  fieldName: "phone"
  label: "Phone Number"
  placeholder: "+1 (555) 123-4567"
  required: false
```

#### `form-field-select`
```huml
- ::
  type: "form-field-select"
  formId: "form-survey"
  fieldName: "subject"
  label: "Subject"
  required: true
  options::
    - "General Inquiry"
    - "Technical Support"
    - "Sales Question"
    - "Partnership Opportunity"
    - "Other"
  css: "margin-bottom: 20px;"
```

**All form fields require:**
- `type` - Field type
- `formId` - Must match form `id` (for traditional submission pattern)
- `fieldName` - Unique identifier for this field (for traditional submission pattern)
- `label` - Display label
- `required` - true/false (optional, defaults to false)

### 🆕 Real-Time State Synchronization with `stateKey`

**NEW**: Form fields can sync directly with template state in real-time using `stateKey` instead of `formId`.

#### Two Form Field Patterns

**Pattern 1: Traditional Submission** (for backend forms)
- Use `formId` + `fieldName`
- Data cached and submitted on button click
- Good for: Contact forms, registrations that POST to backend

**Pattern 2: Real-Time State Sync** (for reactive UIs)
- Use `stateKey` instead of `formId`
- Field value updates state instantly on input
- Enables reactive computed properties and visibility
- Good for: Calculators, filters, search, conditional forms

#### Real-Time State Sync Example

```huml
name: "Event Registration"

state::
  firstName: ""
  lastName: ""
  email: ""

computed::
  fullName: "{{firstName + ' ' + lastName}}"
  personalInfoComplete: "{{firstName != '' && lastName != '' && email != ''}}"

screens::
  - ::
    id: "registration"
    isEntryPoint: true
    children::
      # Form fields with stateKey - update state on every keystroke
      - ::
        type: "form-field-text"
        stateKey: "firstName"  # ← Links to state.firstName
        label: "First Name"
        placeholder: "John"

      - ::
        type: "form-field-text"
        stateKey: "lastName"   # ← Links to state.lastName
        label: "Last Name"
        placeholder: "Doe"

      - ::
        type: "form-field-email"
        stateKey: "email"      # ← Links to state.email
        label: "Email"
        placeholder: "john@example.com"

      # Display computed property (updates in real-time!)
      - ::
        type: "text"
        content: "Welcome, {{fullName}}!"
        visible: "{{fullName != ' '}}"

      # Button visibility controlled by computed property
      - ::
        type: "nav-button"
        content: "Continue"
        visible: "{{personalInfoComplete}}"
        targetContainerId: "next-step"

      # Disabled message when form incomplete
      - ::
        type: "text"
        content: "Please fill all required fields"
        visible: "{{!personalInfoComplete}}"
        css: "color: #999;"
```

#### Key Points for `stateKey`

1. **Instant Updates**: Field value updates state on every keystroke (`oninput` event)
2. **Computed Properties**: Combine with `computed::` for reactive calculations
3. **Conditional Visibility**: Use computed properties in `visible` expressions
4. **No form metadata needed**: Don't need `type: "form"` block when using `stateKey`
5. **Initialization**: Field initializes with state value if it exists

#### Choosing the Right Pattern

**Use `stateKey` when:**
- ✅ Building reactive UIs (search, filters, calculators)
- ✅ Need conditional visibility based on field values
- ✅ Want computed properties that update in real-time
- ✅ Form doesn't submit to backend (single-page app logic)

**Use `formId` when:**
- ✅ Submitting to external service (Formspree, backend API)
- ✅ Multi-step forms with final submission
- ✅ Need all data collected at once (not continuously)

**Can combine both!** Use `stateKey` for reactive UI + `formId` for final submission:

```huml
- ::
  type: "form-field-text"
  stateKey: "email"           # Updates state in real-time
  formId: "form-contact"      # Also included in form submission
  fieldName: "email"
  label: "Email"
```

---

## Form Validation

### ✅ Multi-Screen Form Checklist

**Before creating a multi-screen form, verify:**

**Form Setup:**
- [ ] Form metadata block exists (`type: "form"`) with unique `id`
- [ ] All screens reference the SAME `formId` value
- [ ] Submit endpoint configured (`submitEndpoint` property)

**Button Configuration:**
- [ ] ALL buttons (including intermediate) have `formId` matching the form's `id`
- [ ] ONLY the final submit button has `submit: true`
- [ ] Choice buttons have `fieldName` and `value` properties
- [ ] All buttons have valid `targetContainerId`

**Common Mistakes:**
- [ ] No intermediate button has `submit: true` (only final button should)
- [ ] All screens use consistent `formId` (not switching form IDs)
- [ ] Final submit button has `submit: true` property

### Common Form Mistakes

#### ❌ MISTAKE #1: Missing `submit: true` on Final Button
```huml
# ❌ WRONG - Will cache but not submit
- ::
  type: "nav-button"
  content: "Submit"
  formId: "form-reg"
  targetContainerId: "confirmation"
  # Missing submit: true
```

```huml
# ✅ CORRECT
- ::
  type: "nav-button"
  content: "Submit"
  formId: "form-reg"
  submit: true  # ← Must have this!
  targetContainerId: "confirmation"
```

#### ❌ MISTAKE #2: Adding `submit: true` to Intermediate Buttons
```huml
# ❌ WRONG - Will submit prematurely
- ::
  type: "nav-button"
  content: "Next"
  formId: "form-reg"
  submit: true  # ← Should NOT be here!
  targetContainerId: "screen-2"
```

```huml
# ✅ CORRECT
- ::
  type: "nav-button"
  content: "Next"
  formId: "form-reg"
  targetContainerId: "screen-2"
  # No submit property - auto-caches only
```

#### ❌ MISTAKE #3: formId Mismatch
```huml
# ❌ WRONG - Field references wrong form
- ::
  type: "form"
  id: "form-registration"
  submitEndpoint: "https://formspree.io/f/ABC123"

- ::
  type: "form-field-text"
  formId: "form-contact"  # ← Wrong! Should be "form-registration"
  fieldName: "name"
```

```huml
# ✅ CORRECT
- ::
  type: "form"
  id: "form-registration"
  submitEndpoint: "https://formspree.io/f/ABC123"

- ::
  type: "form-field-text"
  formId: "form-registration"  # ← Matches form id
  fieldName: "name"
```

---

## forEach - Loop Rendering

### **NEW: Data-Driven Component Rendering**

The `forEach` property allows you to render multiple instances of a block from an array in state.

**Why use forEach:**
- Define component ONCE, render multiple times
- Data-driven UI (separation of data and presentation)
- Cleaner, more maintainable templates
- Easy to add/remove items (just update state array)

### Basic Usage

```huml
state::
  items::
    - 1
    - 2
    - 3
    - 4
    - 5

- ::
  type: "nav-button"
  forEach: "items"        # Array name from state
  content: "{{item}}"     # {{item}} = current element
  action: "setState"
  stateKey: "selected"
  stateValue: "{{item}}"
```

**Result:** Creates 5 buttons displaying "1", "2", "3", "4", "5"

### Loop Context Variables

Inside forEach blocks, you have access to:

| Variable | Description | Example |
|----------|-------------|---------|
| `{{item}}` | Current array element | `{{item}}` |
| `{{index}}` | Current index (0-based) | `{{index}}` |
| `{{first}}` | Boolean, true if first item | `{{first}}` |
| `{{last}}` | Boolean, true if last item | `{{last}}` |

### With Object Arrays

```huml
state::
  products::
    - ::
      name: "Laptop"
      price: 999
    - ::
      name: "Mouse"
      price: 29
    - ::
      name: "Keyboard"
      price: 79

- ::
  type: "section-container"
  forEach: "products"
  name: "Product {{index + 1}}"
  css: "background: white; padding: 20px; margin: 10px; border-radius: 8px;"
  children::
    - ::
      type: "heading"
      content: "{{item.name}}"
    - ::
      type: "text"
      content: "Price: ${{item.price}}"
    - ::
      type: "nav-button"
      content: "Buy Now"
      action: "setState"
      stateKey: "selectedProduct"
      stateValue: "{{item.name}}"
```

### Custom Variable Name

```huml
state::
  colors::
    - "red"
    - "green"
    - "blue"

- ::
  type: "nav-button"
  forEach: "colors"
  forEachAs: "color"      # Use {{color}} instead of {{item}}
  content: "{{color}}"
  css: "background: {{color}}; color: white; padding: 10px; margin: 5px;"
```

### 🔥 Buttons Inside forEach - Automatic Loop Context Access

**IMPORTANT**: Buttons (and their children) inside forEach loops automatically have access to loop variables (`item`, `index`, etc.) in ALL CEL expressions, including `stateUpdates`.

#### E-commerce Example: Add to Cart

```huml
state::
  totalItems: 0
  totalPrice: 0

content::
  products::
    - ::
      name: "Laptop"
      price: 999.99
    - ::
      name: "Mouse"
      price: 29.99
    - ::
      name: "Keyboard"
      price: 79.99

- ::
  type: "section-container"
  forEach: "products"
  name: "Product Card"
  children::
    - ::
      type: "heading"
      content: "{{item.name}}"

    - ::
      type: "text"
      content: "${{item.price}}"

    - ::
      type: "nav-button"
      content: "Add to Cart"
      action: "setState"
      stateUpdates::
        totalItems: "{{Number(totalItems) + 1}}"
        totalPrice: "{{Number(totalPrice) + Number(item.price)}}"  # ✅ item.price available!
```

**How it works:**
1. The forEach loop creates one button per product
2. Each button instance has its own `item` (current product)
3. When button is clicked, `item.price` refers to THAT product's price
4. **Loop context is automatically passed to button's CEL evaluations**

**Key Point**: You don't need to do anything special - just reference `{{item}}`, `{{item.propertyName}}`, or `{{index}}` in your button's CEL expressions and they will work correctly!

### Dynamic Styling Based on Index

```huml
- ::
  type: "text"
  forEach: "items"
  content: "Item {{item}}"
  css: "background: {{index % 2 == 0 ? '#f0f0f0' : '#ffffff'}}; padding: 10px;"
```

### Conditional Rendering in Loops

```huml
- ::
  type: "nav-button"
  forEach: "numbers"
  content: "{{item}}"
  visible: "{{item > 5}}"  # Only show if item > 5
```

### 🧮 Complete Calculator Example

**Full working calculator with expression display, demonstrating:**
- State management with CEL
- forEach loops for buttons
- Expression tracking
- Multiple state updates

```huml
name: "Calculator App"
resourceType: "website"

state::
  display: "0"
  previousValue: 0
  operation: ""
  waitingForNewValue: false
  expression: ""  # Track the full expression like "5 ÷ 7 ="
  digits::
    - 7
    - 8
    - 9
    - 4
    - 5
    - 6
    - 1
    - 2
    - 3
    - 0

screens::
  - ::
    id: "calculator"
    isEntryPoint: true
    children::
      - ::
        type: "section-container"
        css: "background: #1f2937; border-radius: 24px; padding: 24px;"
        children::
          # Display with expression tracking
          - ::
            type: "section-container"
            css: "background: #111827; border-radius: 16px; padding: 24px; display: flex; flex-direction: column;"
            children::
              # Expression display (shows "5 ÷ 7 =")
              - ::
                type: "text"
                content: "{{expression}}"
                visible: "{{expression != ''}}"
                css: "color: #9ca3af; font-size: 20px; margin-bottom: 8px;"

              # Result display
              - ::
                type: "text"
                content: "{{display}}"
                css: "color: #f3f4f6; font-size: 48px; font-weight: 300;"

          # Button Grid
          - ::
            type: "section-container"
            css: "display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px;"
            children::
              # Clear button
              - ::
                type: "nav-button"
                content: "C"
                action: "setState"
                stateUpdates::
                  display: "0"
                  previousValue: 0
                  operation: ""
                  expression: ""
                  waitingForNewValue: false
                css: "grid-column: span 2; padding: 24px; background: #ef4444;"

              # Number buttons (using forEach - creates 10 buttons from 1 definition!)
              - ::
                type: "nav-button"
                forEach: "digits"
                content: "{{item}}"
                action: "setState"
                stateUpdates::
                  display: "{{waitingForNewValue ? String(item) : (display == '0' ? String(item) : display + String(item))}}"
                  waitingForNewValue: false
                css: "padding: 24px; background: #374151; {{index == 9 ? 'grid-column: span 2;' : ''}}"

              # Operation buttons (÷, ×, -, +)
              - ::
                type: "nav-button"
                content: "÷"
                action: "setState"
                stateUpdates::
                  previousValue: "{{Number(display)}}"
                  operation: "divide"
                  expression: "{{display + ' ÷'}}"
                  waitingForNewValue: true
                css: "padding: 24px; background: #f59e0b;"

              # Equals button
              - ::
                type: "nav-button"
                content: "="
                action: "setState"
                stateUpdates::
                  display: "{{operation == 'add' ? String(Number(previousValue) + Number(display)) : (operation == 'subtract' ? String(Number(previousValue) - Number(display)) : (operation == 'multiply' ? String(Number(previousValue) * Number(display)) : (operation == 'divide' ? String(Number(previousValue) / Number(display)) : display)))}}"
                  expression: "{{operation != '' ? expression + ' ' + display + ' =' : ''}}"
                  operation: ""
                  waitingForNewValue: true
                css: "grid-column: span 4; padding: 24px; background: #10b981;"
```

**What this demonstrates:**
1. **Expression tracking:** Shows "5 ÷ 7 =" above the result
2. **forEach loops:** Single definition creates all 10 digit buttons
3. **Conditional CSS:** Zero button spans 2 columns using `{{index == 9 ? ... : ...}}`
4. **Complex CEL:** Nested ternary operations for calculations
5. **Multiple state updates:** Each button updates multiple state properties at once

### Important Notes

1. **Array must exist in state** - `forEach: "myArray"` requires array defined in state using YAML list syntax
2. **Works with any block type** - buttons, text, containers, images, etc.
3. **Children are NOT repeated** - forEach only applies to the block itself, not its children
4. **CEL expressions evaluated per item** - Each instance gets its own evaluated values
5. **Array syntax** - Use YAML list format with `::` and `-` items (not bracket notation)
6. **🔥 Buttons automatically inherit loop context** - `nav-button` inside forEach can access `{{item}}`, `{{index}}`, etc. in ALL properties including `stateUpdates`, `stateValue`, `content`, `css`, and `visible`

---

## Thread Block CSS Variables

**⚠️ YOU MUST SET ALL THESE COLORS - NO DEFAULTS!**

**Thread blocks ONLY accept CSS variables. Regular CSS is IGNORED.**

### All 43 Valid Variable Names

| Variable | What It Styles |
|----------|----------------|
| `--thread-bg` | Container background |
| `--thread-border-color` | Container border |
| `--thread-border-radius` | Container border radius |
| `--thread-text-color` | Main text color |
| `--thread-title-color` | Thread heading (if using `name` property) |
| `--thread-description-color` | Description text |
| `--thread-content-color` | Content text color |
| `--thread-content-heading-color` | Content headings |
| `--thread-link-color` | Link color |
| `--thread-code-bg` | Code block background |
| `--thread-code-color` | Code text color |
| `--thread-input-bg` | Textarea background |
| `--thread-input-border` | Textarea border |
| `--thread-input-text` | Textarea text color |
| `--thread-input-placeholder` | Placeholder text |
| `--thread-input-focus-border` | Textarea focus border |
| `--thread-input-focus-shadow` | Focus shadow |
| `--thread-button-bg` | Submit button background |
| `--thread-button-text` | Submit button text |
| `--thread-button-hover-bg` | Button hover background |
| `--thread-button-disabled-bg` | Disabled button background |
| `--thread-button-disabled-text` | Disabled button text |
| `--thread-comment-bg` | Comment box background |
| `--thread-comment-border` | Comment box border |
| `--thread-comment-text` | Comment text color |
| `--thread-comment-author-color` | Author name color |
| `--thread-comment-time-color` | Timestamp color |
| `--thread-comment-content-color` | Comment content text |
| `--thread-toggle-bg` | Toggle button background |
| `--thread-toggle-border` | Toggle button border |
| `--thread-toggle-color` | Toggle button text |
| `--thread-toggle-hover-bg` | Toggle hover background |
| `--thread-toggle-hover-border` | Toggle hover border |
| `--thread-toggle-hover-color` | Toggle hover text |
| `--thread-toggle-icon-color` | Toggle icon color |
| `--thread-no-comments-color` | "X comments" label |
| `--thread-form-bg` | Comment form background |
| `--thread-scrollbar-track` | Scrollbar track background |
| `--thread-scrollbar-thumb` | Scrollbar thumb |
| `--thread-scrollbar-thumb-hover` | Scrollbar thumb hover |
| `--thread-error-bg` | Error message background |
| `--thread-error-text` | Error message text |
| `--thread-error-border` | Error message border |

### Complete Example (Copy This Template)

```huml
- ::
  type: "thread"
  name: "Discussion"
  mode: "markdown"
  description: "Share your thoughts..."
  css: "--thread-bg: #1a1a1a; --thread-border-color: #333333; --thread-text-color: #ffffff; --thread-title-color: #00ff88; --thread-description-color: #888888; --thread-input-bg: #2a2a2a; --thread-input-border: #00ff88; --thread-input-text: #ffffff; --thread-input-placeholder: #888888; --thread-input-focus-border: #00ff88; --thread-input-focus-shadow: rgba(0,255,136,0.2); --thread-button-bg: #00ff88; --thread-button-text: #000000; --thread-button-hover-bg: #00dd77; --thread-button-disabled-bg: #444444; --thread-button-disabled-text: #666666; --thread-comment-bg: #2a2a2a; --thread-comment-border: #444444; --thread-comment-author-color: #00ff88; --thread-comment-time-color: #888888; --thread-comment-content-color: #ffffff; --thread-toggle-bg: #2a2a2a; --thread-toggle-border: #444444; --thread-toggle-color: #ffffff; --thread-toggle-hover-bg: #333333; --thread-no-comments-color: #888888; --thread-scrollbar-track: #1a1a1a; --thread-scrollbar-thumb: #00ff88; --thread-scrollbar-thumb-hover: #00dd77;"
```

**❌ Invalid variables (don't exist - will be ignored):**
- `--thread-padding` ❌
- `--thread-border-width` ❌
- `--thread-reply-bg` ❌
- `--thread-comment-date-color` ❌ (use `--thread-comment-time-color`)
- `--thread-border` ❌ (use `--thread-border-color`)
- `--thread-text-primary` ❌ (use `--thread-text-color`)

---

## Common Patterns

### Card Layout
```huml
- ::
  type: "section-container"
  name: "Card Grid"
  css: "display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px; padding: 40px;"
  children::
    - ::
      type: "section-container"
      name: "Card 1"
      css: "background: white; border-radius: 12px; padding: 30px; box-shadow: 0 4px 6px rgba(0,0,0,0.1);"
      children::
        - ::
          type: "heading"
          content: "Feature 1"
        - ::
          type: "text"
          content: "Description of feature 1"

    - ::
      type: "section-container"
      name: "Card 2"
      css: "background: white; border-radius: 12px; padding: 30px; box-shadow: 0 4px 6px rgba(0,0,0,0.1);"
      children::
        - ::
          type: "heading"
          content: "Feature 2"
        - ::
          type: "text"
          content: "Description of feature 2"
```

### Hero Section
```huml
- ::
  type: "section-container"
  name: "Hero Section"
  css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; padding: 40px 20px;"
  children::
    - ::
      type: "heading"
      content: "Welcome to Our App"
      css: "color: white; font-size: 56px; font-weight: 800; margin-bottom: 20px;"

    - ::
      type: "text"
      content: "The best solution for your needs"
      css: "color: rgba(255,255,255,0.9); font-size: 24px; margin-bottom: 40px;"

    - ::
      type: "nav-button"
      content: "Get Started"
      targetContainerId: "signup"
      css: "background: white; color: #667eea; padding: 15px 40px; border-radius: 8px; font-size: 18px; font-weight: 600;"
```

### Footer
```huml
- ::
  type: "section-container"
  name: "Footer"
  css: "background: #2d3748; color: white; padding: 40px 20px; text-align: center;"
  children::
    - ::
      type: "text"
      content: "© 2025 Your Company. All rights reserved."
      css: "color: rgba(255,255,255,0.8);"
```

### Navigation Bar
```huml
- ::
  type: "section-container"
  name: "Navigation Bar"
  css: "background: #1a202c; padding: 20px 40px; display: flex; justify-content: space-between; align-items: center;"
  children::
    - ::
      type: "heading"
      content: "MyApp"
      css: "color: white; font-size: 24px; margin: 0;"

    - ::
      type: "section-container"
      name: "Nav Links"
      css: "display: flex; gap: 20px;"
      children::
        - ::
          type: "nav-button"
          content: "Home"
          targetContainerId: "home"
          css: "background: transparent; color: white; padding: 10px 20px; border: none;"

        - ::
          type: "nav-button"
          content: "About"
          targetContainerId: "about"
          css: "background: transparent; color: white; padding: 10px 20px; border: none;"

        - ::
          type: "nav-button"
          content: "Contact"
          targetContainerId: "contact"
          css: "background: #667eea; color: white; padding: 10px 20px; border-radius: 6px;"
```

---

## Syntax Rules Summary

1. **Indentation:** 2 spaces per level (consistent)
2. **No trailing spaces:** Lines must not end with whitespace
3. **Lists:** Use `::` after property name, then `- ::` for each item
4. **Strings:** Always use quotes for safety
5. **Comments:** Use `#` at line start
6. **Screen IDs:** Must be unique, no spaces
7. **Exactly one entry point:** One screen with `isEntryPoint: true`
8. **Container names:** All `section-container` blocks need `name` property

---

## Quick Debugging Checklist

### Form not submitting:
1. ✅ Verify form metadata block exists with unique `id`
2. ✅ Check final submit button has `submit: true`
3. ✅ Confirm submit button `formId` matches form's `id`
4. ✅ Verify all fields/buttons use same `formId`
5. ✅ Check that intermediate buttons do NOT have `submit: true`

### Thread block not styled correctly:
1. ✅ Using CSS variables only (not regular CSS)?
2. ✅ Variable names match exact list (no typos)?
3. ✅ Included critical dark theme variables?
4. ✅ `--thread-comment-content-color` set?
5. ✅ `--thread-toggle-bg` and `--thread-toggle-color` set?
6. ✅ `--thread-scrollbar-track` and `--thread-scrollbar-thumb` set?

### Navigation not working:
1. ✅ Target screen `id` exists?
2. ✅ `targetContainerId` spelled correctly?
3. ✅ Exactly one screen has `isEntryPoint: true`?

### Container issues:
1. ✅ All `section-container` blocks have `name` property?
2. ✅ Children properly nested under `children::`?

### Getting NaN in calculations (forEach loops):
1. ✅ Using `Number()` to coerce values? Example: `{{Number(totalPrice) + Number(item.price)}}`
2. ✅ Button is inside a forEach loop and trying to access `{{item.property}}`? This should work automatically!
3. ✅ Check that property exists on the item object in your state/content definition
4. ✅ Verify the forEach container has a `name` property (required for all section-containers)

### Performance issues (too many re-renders):
1. ✅ Check console - blocks should only log re-evaluation when properties THEY USE change
2. ✅ If all blocks re-render on every state change, there may be a system issue (report it!)
3. ✅ Fine-grained signals should handle this automatically - no manual optimization needed

---

## End of Guide

**Key Takeaways:**
1. **⚡ Performance:** Fine-grained signals = automatic optimal performance. Blocks only re-render when properties THEY USE change.
2. **🔥 forEach + Buttons:** Buttons inside forEach loops automatically have access to `{{item}}`, `{{index}}`, etc. in ALL CEL expressions including `stateUpdates`.
3. **Form buttons:** Add `formId` to ALL buttons in the form flow
4. **Final submit:** Add `submit: true` ONLY on the final submit button
5. **Auto-caching:** All form data is automatically cached across screens
6. **Container names:** ALWAYS include descriptive `name` property
7. **Thread blocks:** Use CSS variables ONLY (regular CSS ignored)
8. **Type coercion:** Always use `Number()` in arithmetic operations to avoid NaN

**Performance Best Practices:**
- ✅ Each state property gets its own signal
- ✅ No manual optimization needed - it's automatic!
- ✅ Updating `totalItems` won't re-render blocks that only use `products`
- ✅ Perfect for e-commerce, dashboards, and complex apps

**When in doubt:**
- Read the Performance & Reactivity section at the top
- Read the critical rules
- Check the multi-screen form complete example
- Check the forEach + buttons e-commerce example
- Verify against the debugging checklist

**The new simple model:**
- No more `action: "continue"` or `action: "setValueOnly"`
- Just add `formId` to all buttons
- Add `submit: true` only on final button
- Everything else is automatic!
- Performance is automatic - fine-grained signals handle it!
