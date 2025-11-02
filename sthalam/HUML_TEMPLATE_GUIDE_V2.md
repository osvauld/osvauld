# HUML Template Guide for LLMs

**Version:** 5.0 - Concise Reference for AI Template Generation
**Target:** AI assistants creating HUML templates

---

## 📚 External References

**ALWAYS consult these first:**
- **HUML Specification:** https://huml.io/specifications/v0-1-0/
- **CEL Library (@marcbachmann/cel-js):** https://www.npmjs.com/package/@marcbachmann/cel-js
- **CEL Spec:** https://github.com/google/cel-spec

---

## 🎯 Core Syntax Rules

```huml
# Strings - ALWAYS use double quotes
name: "My Template"
content: "Hello World"

# Scalars (single values) - single colon
count: 42
active: true
price: 99.99

# Vectors (arrays/objects) - double colon
colors:: "red", "green", "blue"
items:: []
config:: {}

# Multi-line arrays
products::
  - ::
    name: "Product A"
    price: 99.99
  - ::
    name: "Product B"
    price: 149.99

# CEL expressions (in double-quoted strings)
content: "{{firstName + ' ' + lastName}}"
visible: "{{count > 5 && isActive}}"
css: "{{theme == 'dark' ? '#000' : '#fff'}}"

# Nested blocks
blocks::
  - ::
    type: "section-container"
    name: "Container Name"
    blocks::
      - ::
        type: "heading"
        content: "Nested Content"
```

**Critical Rules:**
- ✅ Exactly 2 spaces per indent level (NO TABS)
- ✅ One space after `:` and `::`
- ✅ No trailing spaces
- ✅ Use `blocks::` for nested children
- ✅ CEL uses `==` not `===`
- ✅ Inside CEL expressions, use single quotes: `"{{condition ? 'yes' : 'no'}}"`

---

## 🏗️ Template Structure

### ⚠️ Naming Requirements

**CRITICAL: All major components must have descriptive `name` properties:**

1. **Section Containers** → `name: "Hero Section"`, `name: "Product Grid"`
2. **Forms** → `name: "Contact Form"`, `name: "Registration Form"`
3. **Form Fields** → `name: "firstName"`, `name: "emailAddress"`
4. **Screens** → `name: "Home Screen"`, `name: "Dashboard"`

**Event Names:** Each form type should have a unique `eventName`:
- Contact form → `eventName: "contact_submission"`
- Registration → `eventName: "user_registration"`
- Survey → `eventName: "survey_response"`
- NOT all → `eventName: "form_submission"` ❌

```huml
name: "Template Name"
resourceType: "form" | "website" | "dashboard"

# In-memory state (optional)
state::
  currentStep: "welcome"
  count: 0

# Computed properties (optional)
computed::
  fullName: "{{firstName + ' ' + lastName}}"
  isComplete: "{{firstName != '' && email != ''}}"

# Publisher content - shared across users (optional)
content::
  products::
    - ::
      name: "Item"
      price: 99

# Per-user state - isolated (optional)
user_content::
  cartTotal: 0

screens::
  - ::
    id: "main"
    name: "Main Screen"
    isEntryPoint: true
    blocks::
      # Your blocks here
```

**Data Priority:** `state` < `content` < `user_content` (rightmost wins)

---

## 📝 Form Patterns

### Pattern 1: Real-Time Reactive Form (No Backend)

**Use when:** Building reactive UIs, calculators, filters, search

```huml
state::
  firstName: ""
  lastName: ""
  email: ""

computed::
  fullName: "{{firstName + ' ' + lastName}}"
  isComplete: "{{firstName != '' && lastName != '' && email != ''}}"

screens::
  - ::
    id: "form"
    isEntryPoint: true
    blocks::
      # Form fields with stateKey - updates state in real-time
      - ::
        type: "form-field-text"
        stateKey: "firstName"
        label: "First Name"
        placeholder: "John"

      - ::
        type: "form-field-text"
        stateKey: "lastName"
        label: "Last Name"

      - ::
        type: "form-field-email"
        stateKey: "email"
        label: "Email"

      # Reactive computed property display
      - ::
        type: "text"
        content: "Welcome, {{fullName}}!"
        visible: "{{fullName != ' '}}"

      # Button visibility controlled by computed property
      - ::
        type: "nav-button"
        content: "Continue"
        visible: "{{isComplete}}"
        action: "setState"
        stateUpdates::
          currentStep: "next"
```

**Key:** `stateKey` updates state on every keystroke → enables reactive UI

### Pattern 2: Traditional Form Submission

**Use when:** Submitting to backend (Formspree, API)

```huml
screens::
  - ::
    id: "contact"
    isEntryPoint: true
    blocks::
      # Form metadata
      - ::
        type: "form"
        id: "form-contact"
        name: "Contact Form"
        eventName: "contact_submission"
        submitEndpoint: "https://formspree.io/f/YOUR_ID"

      # Form fields
      - ::
        type: "form-field-text"
        name: "fullName"
        formId: "form-contact"
        fieldName: "name"
        label: "Name"
        placeholder: "John Doe"
        required: true

      - ::
        type: "form-field-email"
        name: "emailAddress"
        formId: "form-contact"
        fieldName: "email"
        label: "Email"
        placeholder: "john@example.com"
        required: true

      # Submit button
      - ::
        type: "nav-button"
        content: "Submit"
        formId: "form-contact"
        submit: true
```

### Pattern 3: Hybrid (Reactive + Submit)

**Best of both worlds:** Real-time UI + backend submission

```huml
state::
  firstName: ""
  email: ""

computed::
  isComplete: "{{firstName != '' && email != ''}}"

screens::
  - ::
    blocks::
      - ::
        type: "form"
        id: "form-reg"
        name: "Registration Form"
        eventName: "user_registration"
        submitEndpoint: "https://formspree.io/f/YOUR_ID"

      # Fields with BOTH stateKey and formId
      - ::
        type: "form-field-text"
        name: "firstName"
        stateKey: "firstName"      # Real-time state update
        formId: "form-reg"         # Include in submission
        fieldName: "firstName"
        label: "First Name"
        placeholder: "John"

      - ::
        type: "form-field-email"
        name: "emailAddress"
        stateKey: "email"
        formId: "form-reg"
        fieldName: "email"
        label: "Email"
        placeholder: "john@example.com"

      # Button with both submission and state update
      - ::
        type: "nav-button"
        content: "Submit"
        visible: "{{isComplete}}"
        formId: "form-reg"
        submit: true
        action: "setState"
        stateUpdates::
          currentStep: "confirmation"
```

---

## 🎨 Styling Guide

### Inline CSS

All blocks accept `css` property with standard CSS:

```huml
- ::
  type: "section-container"
  name: "Hero Section"
  css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); padding: 60px 20px; text-align: center; border-radius: 12px;"
```

### Common Layout Patterns

**Flexbox:**
```huml
css: "display: flex; gap: 20px; justify-content: space-between; align-items: center;"
```

**Grid:**
```huml
css: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 20px;"
```

**Responsive Grid:**
```huml
css: "display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px;"
```

**Centering:**
```huml
css: "max-width: 800px; margin: 0 auto; padding: 40px 20px;"
```

### Dynamic Styling with CEL

```huml
# Conditional background
css: "background: {{theme == 'dark' ? '#1a1a1a' : '#ffffff'}}; color: {{theme == 'dark' ? '#fff' : '#000'}};"

# Alternating row colors
css: "background: {{index % 2 == 0 ? '#f0f0f0' : '#ffffff'}}; padding: 10px;"

# Progress bar width
css: "width: {{progressPercent}}%; height: 4px; background: #667eea; transition: width 0.3s;"
```

### Common UI Components

**Card:**
```huml
css: "background: white; border-radius: 12px; padding: 30px; box-shadow: 0 4px 6px rgba(0,0,0,0.1);"
```

**Button (Primary):**
```huml
css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; border: none; padding: 15px 40px; border-radius: 8px; font-size: 16px; font-weight: 600; cursor: pointer;"
```

**Button (Secondary):**
```huml
css: "background: #f8f9fa; color: #333; border: 2px solid #e0e0e0; padding: 14px 32px; border-radius: 8px; font-weight: 600; cursor: pointer;"
```

**Input Field:**
```huml
css: "width: 100%; padding: 14px; border: 2px solid #e0e0e0; border-radius: 8px; font-size: 16px;"
```

### Thread Block Styling (CSS Variables Only)

⚠️ Thread blocks ONLY accept CSS custom properties. Regular CSS is ignored.

**Minimal Dark Theme:**
```huml
- ::
  type: "thread"
  name: "Comments"
  css: "--thread-bg: #1a1a1a; --thread-title-color: #00ff88; --thread-comment-content-color: #ffffff; --thread-button-bg: #00ff88; --thread-button-text: #000000; --thread-input-bg: #2a2a2a; --thread-input-border: #00ff88; --thread-input-text: #ffffff; --thread-comment-bg: #2a2a2a; --thread-comment-author-color: #00ff88; --thread-comment-time-color: #888888; --thread-toggle-bg: #2a2a2a; --thread-toggle-color: #ffffff; --thread-scrollbar-track: #1a1a1a; --thread-scrollbar-thumb: #00ff88;"
```

**Required Variables (43 total):** `--thread-bg`, `--thread-title-color`, `--thread-comment-content-color`, `--thread-button-bg`, `--thread-button-text`, `--thread-input-bg`, `--thread-input-border`, `--thread-input-text`, `--thread-comment-bg`, `--thread-comment-author-color`, `--thread-comment-time-color`, `--thread-toggle-bg`, `--thread-toggle-color`, `--thread-scrollbar-track`, `--thread-scrollbar-thumb`, etc.

---

## 🧮 CEL Expressions

### Basic Syntax

```huml
# String concatenation
content: "{{firstName + ' ' + lastName}}"

# Conditionals
visible: "{{isLoggedIn && role == 'admin'}}"

# Ternary
content: "{{count > 0 ? 'Items: ' + string(count) : 'No items'}}"

# Comparison
visible: "{{age >= 18}}"

# Math
content: "Progress: {{(completed / total) * 100}}%"
```

### Type Conversion

**CRITICAL:** Always use explicit type conversion to avoid NaN:

```huml
# ✅ CORRECT
stateUpdates::
  count: "{{Number(count) + 1}}"
  total: "{{Number(total) + Number(item.price)}}"

# ❌ WRONG - may result in NaN or string concatenation
stateUpdates::
  count: "{{count + 1}}"
  total: "{{total + item.price}}"
```

**Functions:**
- `string(value)` - Convert to string
- `int(value)` - Convert to integer
- `double(value)` - Convert to double
- `Number(value)` - JavaScript coercion

### Native CEL Methods

```huml
# Array methods
visible: "{{items.size() > 0}}"
items: "{{products.filter(p, p.price > 100)}}"
names: "{{users.map(u, u.name)}}"
allValid: "{{items.all(i, i.quantity > 0)}}"
hasExpensive: "{{products.exists(p, p.price > 1000)}}"

# String methods
visible: "{{name.contains('Admin')}}"
valid: "{{email.startsWith('user@') && email.endsWith('.com')}}"
matches: "{{phone.matches('[0-9]{3}-[0-9]{4}')}}"
```

### Custom Helper Functions

Registered in `celEvaluator.ts`:

```huml
# Number formatting
content: "Price: ${{price | toFixed(2)}}"

# String transformations
content: "{{uppercase(name)}}"
content: "{{lowercase(email)}}"
content: "{{capitalize(title)}}"

# Array operations
total: "{{sum(items, 'price')}}"
average: "{{avg(scores, 'value')}}"

# Object operations
keys: "{{keys(user)}}"
values: "{{values(settings)}}"
```

### Array Operations (No Mutations!)

CEL is pure - no `push()`, `pop()`, `splice()`. Use spread operators:

```huml
# Add to array
stateUpdates::
  cart: "{{[...cart, newItem]}}"           # Add to end
  cart: "{{[newItem, ...cart]}}"           # Add to start

# Remove from array
stateUpdates::
  cart: "{{cart.filter(item, item.id != targetId)}}"

# Update array item
stateUpdates::
  cart: "{{cart.map(item, item.id == targetId ? {...item, quantity: item.quantity + 1} : item)}}"
```

---

## 🔄 forEach - Loop Rendering

Render multiple instances from state array:

```huml
state::
  products::
    - ::
      name: "Laptop"
      price: 999
    - ::
      name: "Mouse"
      price: 29

screens::
  - ::
    blocks::
      - ::
        type: "section-container"
        forEach: "products"
        name: "Product Card"
        blocks::
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
              total: "{{Number(total) + Number(item.price)}}"
```

**Loop Context Variables:**
- `{{item}}` - Current element
- `{{index}}` - 0-based index
- `{{first}}` - Boolean, true if first item
- `{{last}}` - Boolean, true if last item

**Loop context automatically available in:**
- `content`, `css`, `visible`
- `stateUpdates`, `stateValue`
- All nested children

---

## 📦 Block Types Reference

### section-container
```huml
- ::
  type: "section-container"
  name: "Container Name"  # REQUIRED
  visible: "{{condition}}"
  css: "background: white; padding: 20px;"
  blocks::
    # Nested children
```

### heading
```huml
- ::
  type: "heading"
  content: "Page Title"
  css: "font-size: 48px; color: #667eea;"
```

### text
```huml
- ::
  type: "text"
  content: "Regular text content"
  css: "color: #666; line-height: 1.6;"
```

### markdown-text
```huml
- ::
  type: "markdown-text"
  mode: "markdown"
  content: "## Heading\n\n**Bold** and *italic*"
```

### image
```huml
- ::
  type: "image"
  src: "https://example.com/image.jpg"
  alt: "Description"
  css: "width: 100%; max-width: 600px; border-radius: 8px;"
```

### nav-button
```huml
# Simple navigation
- ::
  type: "nav-button"
  content: "Click Me"
  action: "setState"
  stateKey: "currentPage"
  stateValue: "about"

# Bulk state update
- ::
  type: "nav-button"
  content: "Reset"
  action: "setState"
  stateUpdates::
    page: "home"
    count: 0
    active: false

# Form submission
- ::
  type: "nav-button"
  content: "Submit"
  formId: "form-contact"
  submit: true
```

### Form Fields

**All form field types:**
- `form-field-text`
- `form-field-email`
- `form-field-tel`
- `form-field-password`
- `form-field-number`
- `form-field-textarea`
- `form-field-checkbox`
- `form-field-select`

**Properties:**
```huml
- ::
  type: "form-field-text"
  name: "firstName"               # Descriptive field name (REQUIRED)
  stateKey: "firstName"           # For real-time state sync
  formId: "form-id"               # For form submission
  fieldName: "first_name"         # For form submission (can differ from name)
  label: "Field Label"            # User-visible label
  placeholder: "Enter value..."
  required: true
  css: "width: 100%; padding: 14px;"
```

### thread (Comments)
```huml
- ::
  type: "thread"
  name: "Discussion"
  mode: "markdown"
  description: "Share your thoughts..."
  css: "--thread-bg: #1a1a1a; --thread-title-color: #00ff88; ..."
```

---

## 🎯 Common Patterns

### Multi-Step Form with Computed Progress

```huml
state::
  currentStep: "personal-info"
  firstName: ""
  lastName: ""
  email: ""

computed::
  isPersonalComplete: "{{firstName != '' && lastName != '' && email != ''}}"
  progress: "{{currentStep == 'personal-info' ? 33 : (currentStep == 'payment' ? 66 : 100)}}"

screens::
  - ::
    blocks::
      # Progress bar
      - ::
        type: "section-container"
        name: "Progress Bar"
        css: "width: 100%; height: 4px; background: #e0e0e0;"
        blocks::
          - ::
            type: "section-container"
            name: "Progress Fill"
            css: "width: {{progress}}%; height: 100%; background: #667eea; transition: width 0.3s;"

      # Step 1: Personal Info
      - ::
        type: "section-container"
        name: "Personal Info"
        visible: "{{currentStep == 'personal-info'}}"
        blocks::
          - ::
            type: "form-field-text"
            stateKey: "firstName"
            label: "First Name"

          - ::
            type: "nav-button"
            content: "Next"
            visible: "{{isPersonalComplete}}"
            action: "setState"
            stateKey: "currentStep"
            stateValue: "payment"
```

### E-commerce Product List

```huml
content::
  products::
    - ::
      id: "prod-1"
      name: "Laptop"
      price: 999.99
      image: "https://example.com/laptop.jpg"
    - ::
      id: "prod-2"
      name: "Mouse"
      price: 29.99
      image: "https://example.com/mouse.jpg"

user_content::
  cartTotal: 0
  cartItems: 0

screens::
  - ::
    blocks::
      # Product grid
      - ::
        type: "section-container"
        name: "Product Grid"
        css: "display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px;"
        blocks::
          - ::
            forEach: "products"
            type: "section-container"
            name: "Product Card"
            css: "background: white; border-radius: 12px; padding: 20px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
            blocks::
              - ::
                type: "image"
                src: "{{item.image}}"
                css: "width: 100%; border-radius: 8px;"

              - ::
                type: "heading"
                content: "{{item.name}}"

              - ::
                type: "text"
                content: "${{item.price}}"
                css: "font-size: 24px; font-weight: 700; color: #667eea;"

              - ::
                type: "nav-button"
                content: "Add to Cart"
                action: "setState"
                stateUpdates::
                  cartTotal: "{{Number(cartTotal) + Number(item.price)}}"
                  cartItems: "{{Number(cartItems) + 1}}"
                css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px; width: 100%;"

      # Cart summary
      - ::
        type: "section-container"
        name: "Cart"
        css: "position: fixed; top: 20px; right: 20px; background: white; padding: 20px; border-radius: 12px; box-shadow: 0 4px 12px rgba(0,0,0,0.15);"
        blocks::
          - ::
            type: "text"
            content: "Cart: {{cartItems}} items"

          - ::
            type: "text"
            content: "Total: ${{cartTotal | toFixed(2)}}"
            css: "font-size: 20px; font-weight: 700;"
```

### Conditional Navigation Sections

```huml
state::
  currentSection: "home"

screens::
  - ::
    blocks::
      # Navigation bar
      - ::
        type: "section-container"
        name: "Nav Bar"
        css: "display: flex; gap: 20px; padding: 20px; background: #1a1a1a;"
        blocks::
          - ::
            type: "nav-button"
            content: "Home"
            action: "setState"
            stateKey: "currentSection"
            stateValue: "home"
            css: "background: {{currentSection == 'home' ? '#667eea' : 'transparent'}}; color: white; padding: 10px 20px; border-radius: 6px;"

          - ::
            type: "nav-button"
            content: "About"
            action: "setState"
            stateKey: "currentSection"
            stateValue: "about"
            css: "background: {{currentSection == 'about' ? '#667eea' : 'transparent'}}; color: white; padding: 10px 20px; border-radius: 6px;"

      # Home section
      - ::
        type: "section-container"
        name: "Home"
        visible: "{{currentSection == 'home'}}"
        blocks::
          - ::
            type: "heading"
            content: "Welcome Home"

      # About section
      - ::
        type: "section-container"
        name: "About"
        visible: "{{currentSection == 'about'}}"
        blocks::
          - ::
            type: "heading"
            content: "About Us"
```

---

## ⚠️ Critical Rules & Gotchas

### 1. Container Naming
**ALL `section-container` blocks MUST have a `name` property.**

```huml
# ✅ CORRECT
- ::
  type: "section-container"
  name: "Hero Section"

# ❌ WRONG - Missing name
- ::
  type: "section-container"
```

### 2. Form Submission
**Add `submit: true` ONLY on the final submit button.**

```huml
# ✅ CORRECT
- ::
  type: "nav-button"
  content: "Submit"
  formId: "form-contact"
  submit: true

# ❌ WRONG - submit: true on intermediate button
- ::
  type: "nav-button"
  content: "Next"
  formId: "form-contact"
  submit: true  # Don't do this!
```

### 3. CEL Comparison
**CEL uses `==` not `===`!**

```huml
# ✅ CORRECT
visible: "{{role == 'admin'}}"

# ❌ WRONG
visible: "{{role === 'admin'}}"  # This will error!
```

### 4. Type Coercion
**Always use `Number()` for arithmetic to avoid NaN.**

```huml
# ✅ CORRECT
stateUpdates::
  count: "{{Number(count) + 1}}"

# ❌ WRONG - May result in string concatenation
stateUpdates::
  count: "{{count + 1}}"
```

### 5. Array Modifications
**Cannot use `push()`, `pop()`, `splice()` - use spread operators.**

```huml
# ✅ CORRECT
stateUpdates::
  items: "{{[...items, newItem]}}"

# ❌ WRONG
stateUpdates::
  items: "{{items.push(newItem)}}"  # Error!
```

### 6. Empty Arrays/Objects
**Use proper syntax for empty collections.**

```huml
# ✅ CORRECT
items:: []
config:: {}

# ❌ WRONG
items: []  # Scalars can't be arrays!
items::[]  # Missing space after ::
```

### 7. Entry Point
**Exactly ONE screen must have `isEntryPoint: true`.**

```huml
screens::
  - ::
    id: "home"
    isEntryPoint: true  # Only one screen!
```

### 8. Indentation
**Exactly 2 spaces per level. NO TABS.**

```huml
# ✅ CORRECT (2 spaces)
blocks::
  - ::
    type: "text"

# ❌ WRONG (tabs or 4 spaces)
blocks::
    - ::
        type: "text"
```

---

## 🚀 Quick Template Checklist

Before creating a template, verify:

- [ ] All strings use double quotes
- [ ] CEL uses single quotes inside expressions: `"{{x ? 'yes' : 'no'}}"`
- [ ] Exactly 2 spaces per indent (no tabs)
- [ ] No trailing spaces on lines
- [ ] All `section-container` blocks have descriptive `name` property
- [ ] All form metadata blocks have `name` property (e.g., "Contact Form")
- [ ] All form fields have `name` property (e.g., "firstName", "emailAddress")
- [ ] Each form has unique `eventName` (e.g., "contact_submission", "user_registration")
- [ ] Exactly one screen has `isEntryPoint: true`
- [ ] Form fields have either `stateKey` (reactive) or `formId`+`fieldName` (submission)
- [ ] Only final button has `submit: true`
- [ ] Thread blocks use CSS variables only
- [ ] Arrays use spread operators, not `push()`
- [ ] Math operations use `Number()` for type coercion
- [ ] CEL comparisons use `==` not `===`

---

## 📖 Template Examples by Use Case

### Calculator
```huml
state::
  display: "0"
  operation: ""
  previousValue: 0

# Use forEach for digit buttons (0-9)
# CEL for calculations
# setState for updates
```

### Search/Filter
```huml
state::
  searchQuery: ""
  category: "all"

# stateKey on search input for real-time updates
# computed filter: products.filter()
# forEach to render filtered results
```

### Dashboard
```huml
content::
  metrics::
    - name: "Users"
      value: 1234
    - name: "Revenue"
      value: 45678

# forEach over metrics
# Computed: totalRevenue
# Dynamic card styling
```

### Registration Form
```huml
state::
  step: 1
  firstName: ""
  email: ""

computed::
  canProgress: "{{firstName != '' && email != ''}}"

# stateKey for reactive validation
# formId for submission
# Computed progress bar
```

---

## 🎓 Best Practices

1. **Start with state design** - Define all state variables first
2. **Use computed properties** - For derived values (fullName, isComplete, etc.)
3. **Choose the right pattern** - stateKey for reactive, formId for submission, or both
4. **Type coerce in CEL** - Always use `Number()` for math
5. **Spread for arrays** - Never use mutating methods
6. **Name all containers** - Required for section-container blocks
7. **One submit button** - Only the final button gets `submit: true`
8. **Test incrementally** - Build section by section
9. **Use forEach** - DRY principle for repeated elements
10. **Dynamic styling** - Use CEL in css for conditional styles

---

## 🔍 Debugging Tips

**Template won't import:**
- Check indentation (2 spaces, no tabs)
- Verify no trailing spaces
- Ensure all strings use double quotes
- Check for missing `name` on containers

**CEL expressions failing:**
- Use `==` not `===`
- Check variable names match state keys
- Verify `Number()` wrapping for math
- Use single quotes inside CEL: `"{{x ? 'yes' : 'no'}}"`

**Form not submitting:**
- Verify form metadata block exists
- Check `formId` matches across fields and button
- Ensure only final button has `submit: true`
- Verify `fieldName` property on all fields

**State not updating:**
- For forms: Add `stateKey` property
- For buttons: Use `action: "setState"`
- Check computed property dependencies

---

## ✅ Complete Template Validation Checklist

Before submitting or importing a template, verify ALL items:

### 📝 HUML Syntax Validation

**Basic Syntax:**
- [ ] All strings use double quotes: `"text"` not `'text'`
- [ ] Scalars use single colon: `name: "value"`
- [ ] Vectors use double colon: `items:: []`
- [ ] Exactly ONE space after `:` and `::`
- [ ] Exactly 2 spaces per indentation level
- [ ] NO tabs used anywhere
- [ ] NO trailing spaces on any line
- [ ] Array items use `- ::` format
- [ ] Nested blocks use `blocks::` property

**Empty Collections:**
- [ ] Empty arrays: `items:: []` (double colon + space + brackets)
- [ ] Empty objects: `config:: {}` (double colon + space + braces)
- [ ] NOT using scalar syntax for arrays: `items: []` ❌

**Multi-line Strings:**
- [ ] Triple quotes for stripped whitespace: `""" text """`
- [ ] Triple backticks for preserved: ``` ``` text ``` ```

### 🏗️ Template Structure

**Root Level:**
- [ ] Has `name: "Template Name"` property
- [ ] Has `resourceType: "form" | "website" | "dashboard"` (optional but recommended)
- [ ] Has at least one of: `state::`, `content::`, or `user_content::`
- [ ] Has `screens::` block

**Screens:**
- [ ] At least one screen defined
- [ ] Exactly ONE screen has `isEntryPoint: true`
- [ ] All screens have unique `id` values
- [ ] All screens have `name` property
- [ ] All screens have `blocks::` property

**Section Containers:**
- [ ] ALL `section-container` blocks have `name` property
- [ ] Container names are descriptive, not generic (e.g., "Hero Section", "Product Grid")
- [ ] Container names are unique within their scope
- [ ] Nested children use `blocks::` property

### 🧮 CEL Expression Validation

**Syntax:**
- [ ] All CEL expressions wrapped in double quotes: `"{{expression}}"`
- [ ] Inside CEL, strings use single quotes: `"{{x ? 'yes' : 'no'}}"`
- [ ] Comparisons use `==` not `===`
- [ ] All variables referenced exist in state/content/user_content
- [ ] No undefined variables in expressions

**Type Safety:**
- [ ] All arithmetic uses `Number()`: `"{{Number(count) + 1}}"`
- [ ] No bare math operations: `"{{count + 1}}"` ❌
- [ ] String concatenation explicit: `"{{firstName + ' ' + lastName}}"`
- [ ] Type conversion functions used: `string()`, `int()`, `double()`

**Array Operations:**
- [ ] No mutation methods: `push()`, `pop()`, `splice()` ❌
- [ ] Using spread operators: `"{{[...items, newItem]}}"`
- [ ] Filter/map return new arrays: `"{{items.filter(x, x > 5)}}"`

**Method Calls:**
- [ ] Native CEL methods use parentheses: `.size()`, `.contains()`, `.filter()`
- [ ] NOT using JavaScript methods: `.length` ❌, `.toUpperCase()` ❌
- [ ] Using CEL/custom equivalents: `.size()` ✅, `uppercase()` ✅

**Computed Properties:**
- [ ] All computed keys referenced in expressions exist
- [ ] Computed expressions are valid CEL
- [ ] No circular dependencies in computed properties
- [ ] Computed properties use existing state/content keys

### 📝 Form Validation (If Template Has Forms)

**Form Metadata:**
- [ ] Has `type: "form"` block if using traditional submission
- [ ] Form has unique `id` property
- [ ] Form has descriptive `name` property (e.g., "Contact Form", "Registration Form")
- [ ] Form has `eventName` property for tracking (e.g., "contact_submission", "user_registration")
- [ ] Different form types use different `eventName` values (not all "form_submission")
- [ ] Form has `submitEndpoint` if submitting to backend
- [ ] Form `id` doesn't conflict with block IDs

**Form Fields:**
- [ ] All fields have `type: "form-field-*"` property
- [ ] All fields have descriptive `name` property (e.g., "firstName", "emailAddress")
- [ ] Fields have `label` property (user-visible label)
- [ ] Fields have `placeholder` property (optional but recommended)

**Pattern 1 - Real-Time (stateKey):**
- [ ] All fields have `stateKey` property
- [ ] `stateKey` values match state keys exactly
- [ ] State keys initialized in `state::` block
- [ ] NO `formId` or `fieldName` properties

**Pattern 2 - Traditional (formId):**
- [ ] All fields have `formId` property
- [ ] All fields have `fieldName` property
- [ ] `formId` matches form metadata `id`
- [ ] `fieldName` values are unique within form
- [ ] NO `stateKey` property

**Pattern 3 - Hybrid (both):**
- [ ] All fields have both `stateKey` AND `formId` + `fieldName`
- [ ] `stateKey` matches state keys
- [ ] `formId` matches form metadata `id`
- [ ] `fieldName` values unique within form

**Form Buttons:**
- [ ] Has at least one submit button
- [ ] Submit button has `formId` property matching form `id`
- [ ] Submit button has `submit: true` property
- [ ] Only ONE button has `submit: true` (the final one)
- [ ] Intermediate buttons have `formId` but NO `submit: true`

**Required Fields:**
- [ ] Required fields have `required: true` property
- [ ] Validation logic handles empty required fields
- [ ] Computed properties check field completion if needed

### 🎨 Styling Validation

**General CSS:**
- [ ] All `css` properties contain valid CSS syntax
- [ ] No syntax errors (missing semicolons, unmatched quotes)
- [ ] Color values valid: hex `#fff`, rgb `rgb(255,255,255)`, named `white`
- [ ] Units included where needed: `20px`, `50%`, `1.5rem`

**Dynamic CSS:**
- [ ] CEL expressions in CSS are wrapped: `css: "{{expression}}"`
- [ ] Ternary operators complete: `"{{condition ? 'value1' : 'value2'}}"`
- [ ] String concatenation correct: `"width: {{percent}}%; height: 100%;"`
- [ ] No missing quotes or syntax errors in dynamic CSS

**Thread Block Styling:**
- [ ] Thread blocks use CSS variables ONLY
- [ ] NO regular CSS properties in thread blocks
- [ ] At minimum includes these critical variables:
  - [ ] `--thread-bg`
  - [ ] `--thread-title-color`
  - [ ] `--thread-comment-content-color`
  - [ ] `--thread-button-bg`
  - [ ] `--thread-button-text`
  - [ ] `--thread-input-bg`
  - [ ] `--thread-input-border`
  - [ ] `--thread-input-text`
  - [ ] `--thread-comment-bg`
  - [ ] `--thread-comment-author-color`
  - [ ] `--thread-comment-time-color`
  - [ ] `--thread-toggle-bg`
  - [ ] `--thread-toggle-color`
  - [ ] `--thread-scrollbar-track`
  - [ ] `--thread-scrollbar-thumb`
- [ ] All CSS variable names match exact list (no typos)
- [ ] Variable values are valid CSS values

**Common Layout:**
- [ ] Flexbox properties complete: `display: flex; ...`
- [ ] Grid properties valid: `grid-template-columns`, `gap`, etc.
- [ ] Responsive units used where appropriate: `%`, `vw`, `vh`, `rem`

### 🔄 forEach Loop Validation (If Used)

**Loop Setup:**
- [ ] `forEach` value is a valid array name from state/content
- [ ] Array exists and is defined with `::` syntax
- [ ] Array contains items (or empty `[]` is intentional)

**Loop Context:**
- [ ] Using `{{item}}` to access current element
- [ ] Using `{{item.property}}` for object properties
- [ ] Using `{{index}}` for current index (if needed)
- [ ] Using `{{first}}` or `{{last}}` correctly (if needed)

**Loop Variables in CEL:**
- [ ] All `{{item.property}}` references are valid properties
- [ ] Loop context variables used in `content`, `css`, `visible`
- [ ] Loop context variables used in `stateUpdates` or `stateValue`
- [ ] No undefined properties accessed on `{{item}}`

### 🔀 Navigation & State Management

**setState Actions:**
- [ ] All buttons with state updates have `action: "setState"`
- [ ] Using `stateKey` + `stateValue` for single updates
- [ ] Using `stateUpdates::` for bulk updates
- [ ] NOT mixing `stateKey` and `stateUpdates` on same button
- [ ] All state keys referenced exist in `state::`/`content::`/`user_content::`

**Visibility Logic:**
- [ ] All `visible` properties are boolean or CEL expressions
- [ ] Visibility expressions evaluate to boolean
- [ ] Mutually exclusive sections properly controlled
- [ ] No conflicting visibility conditions

**Navigation Flow:**
- [ ] Entry point screen is appropriate
- [ ] All navigation paths reachable
- [ ] No dead-end screens (unless intentional)
- [ ] Back buttons implemented where needed

### 📦 Block Configuration

**All Blocks:**
- [ ] All blocks have valid `type` property
- [ ] Type matches one of the supported block types
- [ ] Required properties for each type present

**Heading Blocks:**
- [ ] Has `content` property
- [ ] Content is string or CEL expression

**Text Blocks:**
- [ ] Has `content` property
- [ ] Multi-line text properly formatted

**Image Blocks:**
- [ ] Has `src` property with valid URL or path
- [ ] Has `alt` property for accessibility

**nav-button Blocks:**
- [ ] Has `content` property
- [ ] Has appropriate action properties
- [ ] Navigation target valid (if applicable)

### 🧪 Logical Validation

**State Initialization:**
- [ ] All state variables have initial values
- [ ] Initial values match expected types (string, number, boolean, array, object)
- [ ] Arrays initialized as `[]` or with items
- [ ] Objects initialized as `{}` or with properties

**Computed Dependencies:**
- [ ] All dependencies in computed expressions exist in state
- [ ] No circular references between computed properties
- [ ] Computed properties computed in correct order

**Data Flow:**
- [ ] State updates flow logically
- [ ] Form data captured correctly
- [ ] No orphaned state variables (defined but never used)
- [ ] No missing state variables (used but not defined)

### 🚨 Common Mistakes Check

**Syntax Errors:**
- [ ] NOT using `===` anywhere (use `==`)
- [ ] NOT using JavaScript `.length` (use `.size()`)
- [ ] NOT using `.toUpperCase()` (use `uppercase()`)
- [ ] NOT using `.push()` (use spread: `[...arr, item]`)
- [ ] NOT missing space after `::` (use `items:: []` not `items::[]`)
- [ ] NOT using scalar syntax for arrays (use `items:: []` not `items: []`)
- [ ] NOT using tabs for indentation (use 2 spaces)

**CEL Mistakes:**
- [ ] NOT doing bare arithmetic: `"{{count + 1}}"` ❌
- [ ] Always using `Number()`: `"{{Number(count) + 1}}"` ✅
- [ ] NOT using triple equals: `"{{x === y}}"` ❌
- [ ] Always using double equals: `"{{x == y}}"` ✅

**Form Mistakes:**
- [ ] NOT using `submit: true` on intermediate buttons
- [ ] NOT missing `formId` on submit button
- [ ] NOT mixing form patterns (pick one: stateKey OR formId OR both)
- [ ] NOT missing form metadata block when using `formId`
- [ ] NOT missing `name` property on form metadata block
- [ ] NOT missing `name` property on form fields
- [ ] NOT using generic `eventName` like "form_submission" for all forms
- [ ] Each form type has unique `eventName` (e.g., "contact_submission", "user_registration")

**Styling Mistakes:**
- [ ] NOT using regular CSS in thread blocks
- [ ] NOT missing critical thread CSS variables
- [ ] NOT forgetting units: `padding: 20` ❌ → `padding: 20px` ✅
- [ ] NOT using invalid CSS property names

### ✨ Final Polish Check

**User Experience:**
- [ ] Loading states considered (if applicable)
- [ ] Error states handled
- [ ] Empty states shown when no data
- [ ] Success/confirmation messages clear
- [ ] Accessible labels and placeholders

**Performance:**
- [ ] No unnecessarily complex CEL expressions
- [ ] forEach loops not nested too deeply
- [ ] Computed properties are performant

**Code Quality:**
- [ ] Naming is descriptive and consistent
- [ ] Structure is logical and organized
- [ ] No duplicate code (use forEach where appropriate)
- [ ] Comments removed (HUML doesn't support runtime comments)

---

## 🎯 Quick Validation Commands

**Before submitting, verify:**
```bash
# 1. Check indentation (2 spaces, no tabs)
grep -P '\t' your-template.huml  # Should return nothing

# 2. Check trailing spaces
grep ' $' your-template.huml  # Should return nothing

# 3. Verify all section-containers have names
grep -A 1 'type: "section-container"' your-template.huml | grep 'name:'
# Each section-container should have a matching name line

# 4. Check for === usage (should use ==)
grep '===' your-template.huml  # Should return nothing

# 5. Verify exactly one entry point
grep 'isEntryPoint: true' your-template.huml | wc -l  # Should return 1
```

---

**End of Guide**

For complex scenarios, consult:
- HUML Spec: https://huml.io/specifications/v0-1-0/
- CEL Docs: https://www.npmjs.com/package/@marcbachmann/cel-js

**Pro Tip:** Use this checklist section by section as you build your template, not just at the end. It's easier to fix issues early than to debug a complete template!
