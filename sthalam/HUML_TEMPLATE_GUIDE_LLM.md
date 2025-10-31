# HUML Template Guide - LLM Optimized

**Version:** 2.1 LLM Edition
**Purpose:** Compressed reference for AI assistants creating HUML templates

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

**All form fields require:**
- `type` - Field type
- `formId` - Must match form `id`
- `fieldName` - Unique identifier for this field
- `label` - Display label
- `required` - true/false (optional, defaults to false)

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

---

## End of Guide

**Key Takeaways:**
1. **Form buttons:** Add `formId` to ALL buttons in the form flow
2. **Final submit:** Add `submit: true` ONLY on the final submit button
3. **Auto-caching:** All form data is automatically cached across screens
4. **Container names:** ALWAYS include descriptive `name` property
5. **Thread blocks:** Use CSS variables ONLY (regular CSS ignored)

**When in doubt:**
- Read the critical rules at the top
- Check the multi-screen form complete example
- Verify against the debugging checklist

**The new simple model:**
- No more `action: "continue"` or `action: "setValueOnly"`
- Just add `formId` to all buttons
- Add `submit: true` only on final button
- Everything else is automatic!
