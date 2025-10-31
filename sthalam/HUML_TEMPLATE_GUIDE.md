# HUML Template Guide - Complete Reference

**Version:** 2.0 (Complete Rewrite)
**Last Updated:** January 2025
**Purpose:** Create complete web applications using HUML declarative syntax

---

## ⚠️ CRITICAL - FOR LLM ASSISTANTS CREATING TEMPLATES

**IF YOU ARE AN AI ASSISTANT GENERATING HUML TEMPLATES, READ THIS FIRST:**

### Container Naming Requirement

**ALL `section-container` blocks MUST have a descriptive `name` property.**

This is mandatory for template maintainability and organization. Every container needs a clear, meaningful name that describes its purpose.

**Examples of good names:**
- "Header Navigation"
- "Hero Section"
- "Blog Post Card"
- "Comment Thread Area"
- "Footer Links"

**❌ NEVER create unnamed containers!** Always include the `name` property.

### Thread Blocks Have Special CSS Requirements

**Thread blocks (`type: "thread"`) ONLY accept CSS custom properties (CSS variables). Regular CSS will NOT work.**

**CRITICAL FOR DARK THEMES:** You MUST include these variables or the theme will break:
- `--thread-comment-content-color` - Comment text (or comments will be invisible!)
- `--thread-toggle-bg` - Toggle button background (or it will be white!)
- `--thread-toggle-color` - Toggle button text
- `--thread-scrollbar-track` - Scrollbar background (or it will be white!)
- `--thread-scrollbar-thumb` - Scrollbar thumb (or it will be white!)
- `--thread-no-comments-color` - Comment count label
- `--thread-description-color` - Description text

**IMPORTANT:** After you finish generating the template, you MUST re-analyze all thread block CSS variables to ensure they are correct and match the exact variable names from the available list below. Double-check for typos and invalid variable names.

**COMMON MISTAKES:**
- Using `--thread-padding`, `--thread-border-width`, `--thread-reply-bg` (these don't exist!)
- Using `--thread-comment-date-color` instead of `--thread-comment-time-color`
- Forgetting scrollbar and toggle variables (causes white elements on dark backgrounds!)

**✅ CORRECT - Use CSS Variables:**
```huml
- ::
  type: "thread"
  name: "Discussion"
  mode: "markdown"
  description: "Share your thoughts..."
  css: "--thread-bg: #1a1a1a; --thread-title-color: #00ff88; --thread-button-bg: #00ff88; --thread-button-text: #000000; --thread-input-bg: #2a2a2a; --thread-input-border: #00ff88; --thread-comment-author-color: #00ff88;"
```

**KEY POINTS:**
- If you use `name` property, you MUST include `--thread-title-color` or the heading will be invisible on dark backgrounds
- Only use variable names from the official list below (see "Available CSS Custom Properties")
- Common mistakes: `--thread-border` ❌ (should be `--thread-border-color`), `--thread-text-primary` ❌ (should be `--thread-text-color`)

**❌ WRONG - Regular CSS (Will Be Ignored):**
```huml
- ::
  type: "thread"
  name: "Discussion"
  css: "background: #1a1a1a; border-radius: 8px; padding: 20px;"
```

**❌ ALSO WRONG - Class-based CSS (Will Be Ignored):**
```huml
- ::
  type: "thread"
  name: "Discussion"
  css: """
    .thread-container {
      background: #1a1a1a;
    }
    textarea {
      border: 1px solid #00ff88;
    }
  """
```

**Why?** Thread blocks use CSS custom properties for scoping. Each thread can have its own theme without conflicts. Regular CSS properties and class selectors don't work with this system.

**See the [Thread Block Styling Section](#block-type-thread) for all available CSS variables.**

---

## What You'll Learn

This guide teaches you how to build complete web applications using HUML (Hierarchical UI Markup Language). You'll learn to create:

- **Multiple screens/pages** - Build multi-page applications
- **Navigation flows** - Connect screens with buttons
- **Forms with validation** - Collect user input and submissions
- **Discussion threads** - Add comment sections
- **Custom styling** - Style everything with CSS
- **Branching logic** - Create choose-your-own-adventure flows

---

## 🤔 Important: When to Ask Questions

**If you are an AI assistant using this guide to help users create HUML templates, ALWAYS ask clarifying questions when requirements are ambiguous or unclear!**

### When to Ask Questions:

- **Unclear Design Requirements**: If the user says "make it look nice" without specifying colors, layout, or style preferences
- **Missing Content**: When you don't know what text, images, or data should go in sections
- **Ambiguous User Flows**: When navigation paths or user journeys aren't clearly defined
- **Styling Preferences**: If color schemes, fonts, spacing, or overall aesthetic aren't specified
- **Form Field Requirements**: When it's unclear what fields a form needs or which should be required
- **Feature Priorities**: If multiple approaches are possible and the user hasn't specified which they prefer

### Example Questions to Ask:

- "What color scheme would you like? (e.g., professional blue/white, dark mode, vibrant colors)"
- "Should this form have a required email field, or just optional feedback?"
- "Where should users go after submitting the form? A thank-you page, back to home, or somewhere else?"
- "Do you want the comments section to support markdown formatting?"
- "Should the navigation button be prominent (large, colorful) or subtle (small, minimal)?"
- "What happens if a user clicks 'No' on this question? Different screen or different message?"

**Remember**: It's better to ask 2-3 clarifying questions upfront than to build something that doesn't match the user's vision. Specific requirements lead to better results!

---

## Quick Navigation

**Building your first app?** → Start with [HUML Syntax Basics](#huml-syntax-basics)
**Need forms?** → Jump to [Forms - Complete Guide](#forms---complete-guide)
**Want examples?** → See [Complete Working Examples](#complete-working-examples)
**Styling help?** → Check [CSS Styling Reference](#css-styling-reference)

---

## HUML Syntax Basics

### What is HUML?

HUML uses a YAML-like syntax to define your entire application structure in one file. Think of it as writing a blueprint for your app.

### Basic Syntax Rules

#### 1. Comments
```huml
# This is a comment
# Comments help explain what each section does
```

#### 2. Properties
```huml
name: "My Application"
content: "Hello, world!"
```

#### 3. Numbers and Booleans
```huml
width: 800
isEntryPoint: true
required: false
```

#### 4. Lists (Arrays)
Use `::` followed by items with `- ::`

```huml
screens::
  - ::
    id: "screen1"
  - ::
    id: "screen2"
```

#### 5. Nested Structures
```huml
screens::
  - ::
    id: "home"
    children::
      - ::
        type: "heading"
        content: "Welcome"
      - ::
        type: "text"
        content: "This is my app"
```

### Important Syntax Notes

- **Indentation matters** - Use 2 spaces per level
- **No curly braces** - Unlike JSON
- **Strings** - Use quotes for safety: `"My Text"`
- **Lists need `::`** - Both for declaration and items
- **⚠️ NO TRAILING SPACES** - Lines cannot end with spaces or whitespace (parser will reject)
- **Empty lines must be truly empty** - Blank lines with spaces will cause errors

---

### ⚠️ CRITICAL: Property Names Are Case-Sensitive!

**Property names MUST use exact camelCase spelling. Lowercase will NOT work!**

---

### ⚠️ CRITICAL: No Trailing Whitespace!

**The HUML parser is strict about whitespace:**
- Lines **cannot end with spaces or tabs**
- Blank lines **must be completely empty** (no spaces)
- If you get an error like `trailing spaces are not allowed`, check for:
  - Spaces at the end of lines
  - Blank lines with invisible whitespace
  - Use your editor's "show whitespace" feature to find them

**Wrong:**
```huml
children::␣␣
  - ::␣
    type: "heading"␣␣␣
```

**Right:**
```huml
children::
  - ::
    type: "heading"
```
## Template Structure

Every HUML template has this structure:

```huml
name: "Application Name"

screens::
  - ::
    id: "screen-id"
    name: "Screen Name"
    isEntryPoint: true
    children::
      # Blocks go here
```

### Required Top-Level Properties

#### `name` (required)
Your application's name

```huml
name: "My Blog Application"
```

#### `screens` (required)
Array of screen definitions. You need at least ONE screen.

```huml
screens::
  - ::
    id: "home"
    name: "Homepage"
    isEntryPoint: true
```

---

## Screens - The Foundation

### What is a Screen?

A **screen** is like a page in your application. Users navigate between screens using buttons.

### Screen Properties

#### `id` (required)
Unique identifier for this screen. Used for navigation.

```huml
id: "screen-home"
```

**Rules:**
- Must be unique across all screens
- Use kebab-case or camelCase
- No spaces or special characters (except - and _)
- Examples: `"home"`, `"about-us"`, `"contact-page"`

#### `name` (optional but recommended)
Human-readable display name

```huml
name: "Homepage"
```

#### `isEntryPoint` (optional, default: false)
Marks this screen as the starting point. **Exactly ONE screen** should have this set to `true`.

```huml
isEntryPoint: true
```

#### `css` (optional)
Inline CSS styling for the entire screen

```huml
css: "background: #f0f0f0; padding: 40px; min-height: 100vh;"
```

#### `children` (optional)
Array of blocks that appear on this screen

```huml
children::
  - ::
    type: "heading"
    content: "Welcome"
  - ::
    type: "text"
    content: "This is the homepage"
```

### Complete Screen Example

```huml
screens::
  - ::
    id: "home"
    name: "Homepage"
    isEntryPoint: true
    css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); padding: 40px; min-height: 100vh;"
    children::
      - ::
        type: "heading"
        content: "Welcome to My App"
        css: "color: white; font-size: 48px; text-align: center;"

      - ::
        type: "text"
        content: "This is the best app ever!"
        css: "color: white; font-size: 20px; text-align: center;"
```

---

## Block Types - Complete Reference

### Common Properties (All Blocks)

Every block type has these properties:

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `type` | String | ✅ Yes | What kind of block this is |
| `content` | String | Depends on type | The text/data content |
| `css` | String | ❌ No | Custom CSS styling |
| `children` | Array | Only for containers | Nested blocks |

---

### Block Type: `heading`

**Purpose:** Large, prominent text for titles

**Properties:**
- `type: "heading"` (required)
- `content` (required) - The heading text
- `css` (optional) - Custom styling

**Example:**
```huml
- ::
  type: "heading"
  content: "Welcome to My Website"
  css: "font-size: 48px; color: #667eea; text-align: center; margin-bottom: 20px;"
```

**When to use:**
- Page titles
- Section headers
- Article headlines

---

### Block Type: `text`

**Purpose:** Regular paragraph text

**Properties:**
- `type: "text"` (required)
- `content` (required) - The paragraph text
- `css` (optional) - Custom styling

**Example:**
```huml
- ::
  type: "text"
  content: "This is a paragraph explaining something to the user."
  css: "color: #4a5568; font-size: 16px; line-height: 1.6; margin-bottom: 20px;"
```

**Multi-line text:**
```huml
content: "Line 1\nLine 2\nLine 3"
```

**When to use:**
- Body text
- Descriptions
- Instructions
- Metadata (dates, author names)

---

### Block Type: `markdown-text`

**Purpose:** Rich text with markdown formatting (bold, lists, code, links)

**Properties:**
- `type: "markdown-text"` (required)
- `content` (required) - Markdown-formatted text
- `mode: "markdown"` (optional) - Rendering mode
- `css` (optional) - Custom styling

**Markdown Support:**

Headers:
```huml
content: "## This is H2\n### This is H3"
```

Bold and Italic:
```huml
content: "This is **bold** and this is *italic*"
```

Lists:
```huml
content: "- Item 1\n- Item 2\n- Item 3"
```

Code blocks:
```huml
content: "```javascript\nfunction hello() {\n  console.log('Hello');\n}\n```"
```

Links:
```huml
content: "[Click here](https://example.com)"
```

**Example:**
```huml
- ::
  type: "markdown-text"
  content: "## Introduction\n\nThis is a **comprehensive guide** to building apps.\n\n### Key Features\n\n- Easy to use\n- Powerful\n- Flexible"
  mode: "markdown"
  css: "background: white; padding: 30px; border-radius: 8px; line-height: 1.8;"
```

**When to use:**
- Blog post content
- Documentation
- Articles
- Rich descriptions

---

### Block Type: `section-container`

**Purpose:** Group related blocks together with shared styling

**⚠️ IMPORTANT: Always Use Meaningful Names!**

**Container blocks MUST have a descriptive `name` property.** This is critical for:
- Identifying containers in the builder/editor
- Making templates maintainable
- Understanding structure at a glance
- Debugging and organizing complex layouts

**Properties:**
- `type: "section-container"` (required)
- `name` (required for best practices) - Descriptive display name (e.g., "Header Section", "Feature Card", "Blog Post Container")
- `css` (optional) - Container styling
- `children` (optional) - Blocks inside this container

**Example: Card Container**
```huml
- ::
  type: "section-container"
  name: "Feature Card"
  css: "background: white; border-radius: 12px; padding: 30px; margin-bottom: 20px; box-shadow: 0 4px 6px rgba(0,0,0,0.1);"
  children::
    - ::
      type: "heading"
      content: "Feature Title"
      css: "font-size: 24px; margin-bottom: 10px;"
    - ::
      type: "text"
      content: "Description of the feature"
      css: "color: #666;"
```

**Example: Flexbox Layout**
```huml
- ::
  type: "section-container"
  name: "Two Columns"
  css: "display: flex; gap: 20px;"
  children::
    - ::
      type: "section-container"
      name: "Left Column"
      css: "flex: 1; background: #f0f0f0; padding: 20px;"
      children::
        - ::
          type: "heading"
          content: "Left Side"

    - ::
      type: "section-container"
      name: "Right Column"
      css: "flex: 1; background: #e0e0e0; padding: 20px;"
      children::
        - ::
          type: "heading"
          content: "Right Side"
```

**When to use:**
- Group related content
- Create cards
- Build layouts (columns, grids)
- Apply shared styling to multiple blocks

#### Modal Overlay Mode

`section-container` can also be used as a modal/popup overlay:

**Additional Properties for Modals:**
- `isModal` (optional, boolean) - Enables modal behavior with backdrop (default: false)
- `visible` (optional, boolean) - Controls modal visibility (default: true)
- `zIndex` (optional, number) - Stacking order for overlay (default: 1000)

**Example: Modal Dialog**
```huml
- ::
  type: "section-container"
  name: "Confirmation Modal"
  isModal: true
  visible: true
  zIndex: 1000
  css: "background: white; padding: 40px; border-radius: 12px; max-width: 400px; position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); box-shadow: 0 20px 60px rgba(0,0,0,0.3);"
  children::
    - ::
      type: "heading"
      content: "Are you sure?"
      css: "margin-bottom: 20px; font-size: 24px;"
    - ::
      type: "text"
      content: "This action cannot be undone."
      css: "margin-bottom: 30px; color: #666;"
    - ::
      type: "nav-button"
      content: "Confirm"
      targetContainerId: "next-screen"
      css: "background: #dc2626; color: white; padding: 12px 24px; border-radius: 8px;"
```

**Modal behavior:**
- Automatically adds dark backdrop behind modal (rgba(0, 0, 0, 0.5))
- Backdrop has blur effect (`backdrop-filter: blur(4px)`)
- Backdrop z-index is automatically set to one less than modal
- Use `visible: false` to hide modal initially (can be toggled programmatically)
- Position modal with CSS (`position: fixed`, `top`, `left`, `transform`)

---

### Block Type: `nav-button`

**Purpose:** The universal button for ALL interactions

**This is the ONLY button type you need.** It has five modes:

1. **MODE 1:** Simple navigation (go to another screen)
2. **MODE 2:** Form submission (submit all collected data + navigate)
3. **MODE 3A:** Set single value without submitting (for branching choices)
4. **MODE 3B:** Branching with immediate submission (set value + submit + navigate)
5. **MODE 4:** Cache and continue (save visible fields and navigate, for multi-screen forms)

#### ⚡ Quick Reference: Which Properties to Use

| Property | MODE 1<br>Navigate | MODE 2<br>Submit | MODE 3A<br>Choice | MODE 3B<br>Choice+Submit | MODE 4<br>Continue |
|----------|:------------------:|:----------------:|:-----------------:|:------------------------:|:------------------:|
| `type: "nav-button"` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `content` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `targetContainerId` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `formId` | ❌ | ✅ | ✅ | ✅ | ✅ |
| `fieldName` | ❌ | ❌ | ✅ | ✅ | ❌ |
| `value` | ❌ | ❌ | ✅ | ✅ | ❌ |
| `action: "setValueOnly"` | ❌ | ❌ | ✅ **REQUIRED** | ❌ | ❌ |
| `action: "continue"` | ❌ | ❌ | ❌ | ❌ | ✅ **REQUIRED** |

**⚠️ Most common mistakes:**
- MODE 3A: Forgetting `action: "setValueOnly"` (button navigates but doesn't save the choice)
- MODE 4: Forgetting `formId` or `action: "continue"` (button navigates but doesn't save form fields)

#### Common Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `type` | String | ✅ Yes | Must be `"nav-button"` |
| `content` | String | ✅ Yes | Button text |
| `targetContainerId` | String | ✅ Yes | Screen ID to navigate to |
| `formId` | String | For MODE 2, 3, 4 | Form to interact with |
| `fieldName` | String | For MODE 3 only | Field name to set |
| `value` | Any | For MODE 3 only | Value to set |
| `action` | String | ❌ No | `"continue"`, `"setValueOnly"`, or default `"navigate"` |
| `css` | String | ❌ No | Button styling |

---

#### MODE 1: Simple Navigation

**Use this when:** You just want to go from one screen to another.

**Required:**
- `type: "nav-button"`
- `content` - Button text
- `targetContainerId` - Where to go

**Example:**
```huml
- ::
  type: "nav-button"
  content: "Go to About Page"
  targetContainerId: "about"
  css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px; font-weight: 600;"
```

**Complete Two-Screen Example:**
```huml
name: "Simple Navigation Example"

screens::
  # Home screen
  - ::
    id: "home"
    name: "Home"
    isEntryPoint: true
    children::
      - ::
        type: "heading"
        content: "Homepage"
      - ::
        type: "nav-button"
        content: "Go to About"
        targetContainerId: "about"

  # About screen
  - ::
    id: "about"
    name: "About"
    children::
      - ::
        type: "heading"
        content: "About Us"
      - ::
        type: "nav-button"
        content: "← Back to Home"
        targetContainerId: "home"
```

---

#### MODE 2: Form Submission

**Use this when:** You want to submit a form and then navigate to a thank-you page.

**⚠️ CRITICAL: This is the DEFAULT mode when you have `formId` but NO `action` property!**

If you add `formId` to a button but forget `action: "continue"` or `action: "setValueOnly"`, the button will SUBMIT the form instead of caching data. This creates MULTIPLE submissions instead of ONE final submission!

**Required:**
- `type: "nav-button"`
- `content` - Button text (e.g., "Submit", "Send")
- `formId` - ID of the form to submit
- `targetContainerId` - Where to go after submission
- **NO `action` property** - If you add `action`, it becomes MODE 3 or 4!

**How it works:**
1. User clicks button
2. System finds all form fields with matching `formId`
3. Validates required fields
4. If valid: Creates submission → Saves to database → Navigates to target screen
5. If invalid: Shows alert with missing fields

**Example:**
```huml
# The submit button (MODE 2)
- ::
  type: "nav-button"
  content: "Submit Form"
  formId: "form-contact"
  targetContainerId: "thank-you"
  css: "background: #667eea; color: white; padding: 15px 30px; border-radius: 8px; font-weight: 600; width: 100%;"
```

**See [Forms - Complete Guide](#forms---complete-guide) for full form examples.**

---

#### MODE 3A: Set Value Without Submitting (Multi-Screen Forms)

**Use this when:** You want to collect data across multiple screens and submit everything together at the end.

**⚠️ CRITICAL: ALL 7 properties below are REQUIRED. Missing `action: "setValueOnly"` is the #1 mistake!**

**Required Properties (ALL 7 are mandatory):**
- ✅ `type: "nav-button"`
- ✅ `content` - Button text
- ✅ `formId` - Form to associate with
- ✅ `fieldName` - Name of the field to set
- ✅ `value` - The value to set
- ✅ `targetContainerId` - Where to go
- ✅ `action: "setValueOnly"` - **CRITICAL:** Tells the system NOT to submit yet (NEVER omit this!)

**Common mistake:** Forgetting `action: "setValueOnly"` - this will cause the button to navigate without saving the choice!

**How it works:**
1. User clicks button
2. System stores `fieldName` = `value` in temporary storage
3. Navigates to target screen (NO submission happens)
4. User continues filling more fields on other screens
5. When final submit button (MODE 2) is clicked, ALL accumulated data is submitted together

**Example: Initial Registration Type Selection**
```huml
# This button stores the choice but doesn't submit
- ::
  type: "nav-button"
  content: "🎤 Speaker Registration"
  formId: "form-registration"
  fieldName: "registration_type"
  value: "speaker"
  targetContainerId: "speaker-info"
  action: "setValueOnly"
  css: "background: #667eea; color: white; padding: 20px; border-radius: 12px; font-weight: 600;"
```

**Complete Multi-Screen Example:**
```huml
name: "Multi-Screen Registration"

screens::
  # Screen 1: Choose registration type
  - ::
    id: "welcome"
    name: "Welcome"
    isEntryPoint: true
    children::
      # Form metadata
      - ::
        id: "form-registration"
        type: "form"
        name: "Registration Form"
        eventName: "registration_submission"

      - ::
        type: "heading"
        content: "Select Registration Type"

      # MODE 3A button - stores value but doesn't submit
      - ::
        type: "nav-button"
        content: "Speaker"
        formId: "form-registration"
        fieldName: "registration_type"
        value: "speaker"
        targetContainerId: "speaker-form"
        action: "setValueOnly"
        css: "background: #667eea; color: white; padding: 15px 30px; border-radius: 8px;"

  # Screen 2: Collect speaker details
  - ::
    id: "speaker-form"
    name: "Speaker Form"
    children::
      - ::
        type: "heading"
        content: "Speaker Information"

      # Regular form fields
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

      - ::
        type: "form-field-text"
        formId: "form-registration"
        fieldName: "talk_title"
        label: "Talk Title"
        required: true

      # MODE 4: Continue button - caches fields without submitting
      - ::
        type: "nav-button"
        content: "Continue"
        formId: "form-registration"
        action: "continue"
        targetContainerId: "speaker-preferences"
        css: "background: #667eea; color: white; padding: 15px 30px; border-radius: 8px;"

  # Screen 3: More preferences
  - ::
    id: "speaker-preferences"
    name: "Speaker Preferences"
    children::
      - ::
        type: "heading"
        content: "Select Your Topic"

      # Another MODE 3A button - adds to accumulated data
      - ::
        type: "nav-button"
        content: "AI & Machine Learning"
        formId: "form-registration"
        fieldName: "topic"
        value: "ai_ml"
        targetContainerId: "final-review"
        action: "setValueOnly"
        css: "background: #667eea; color: white; padding: 15px 30px; border-radius: 8px;"

  # Screen 4: Final submission
  - ::
    id: "final-review"
    name: "Review & Submit"
    children::
      - ::
        type: "heading"
        content: "Review Your Information"

      - ::
        type: "text"
        content: "Click submit to complete your registration"

      # MODE 2 button - submits ALL accumulated data
      - ::
        type: "nav-button"
        content: "Submit Registration"
        formId: "form-registration"
        targetContainerId: "thank-you"
        css: "background: #00ff88; color: #000; padding: 15px 30px; border-radius: 8px; font-weight: 600;"

  # Success screen
  - ::
    id: "thank-you"
    name: "Thank You"
    children::
      - ::
        type: "heading"
        content: "✅ Registration Complete!"
```

**What gets submitted:**
```json
{
  "formId": "form-registration",
  "eventName": "registration_submission",
  "data": {
    "registration_type": "speaker",
    "full_name": "John Doe",
    "email": "john@example.com",
    "talk_title": "The Future of AI",
    "topic": "ai_ml"
  }
}
```

**Key Points:**
- `action: "setValueOnly"` prevents immediate submission
- All values are accumulated across screens
- Final MODE 2 button submits everything together
- User can navigate back/forth without losing data
- Perfect for complex multi-step forms

---

#### MODE 3B: Branching Choices (With Immediate Submission)

**Use this when:** You want to present choices (Yes/No, Option A/B/C) where each choice sets a specific value and navigates somewhere.

**Required:**
- `type: "nav-button"`
- `content` - Button text (e.g., "Yes", "No", "Blue", "Option A")
- `formId` - Form to submit
- `fieldName` - Name of the field to set
- `value` - The value to set
- `targetContainerId` - Where to go

**How it works:**
1. User clicks button
2. System sets `fieldName` to `value`
3. Collects any other existing form fields
4. Creates submission with all data
5. Navigates to target screen

**Example: Yes/No Question**
```huml
name: "Branching Example"

screens::
  # Question screen
  - ::
    id: "question"
    name: "Question"
    isEntryPoint: true
    children::
      # Define form first
      - ::
        id: "form-survey"
        type: "form"
        eventName: "survey_response"

      # Ask question
      - ::
        type: "heading"
        content: "Are you satisfied with our service?"
        css: "text-align: center; margin-bottom: 30px;"

      # Yes button (MODE 3)
      - ::
        type: "nav-button"
        content: "😊 Yes, very satisfied"
        formId: "form-survey"
        fieldName: "satisfaction"
        value: "yes"
        targetContainerId: "thank-you"
        css: "background: #a6e3a1; color: #1e1e2e; padding: 15px 30px; border-radius: 8px; font-weight: 600; margin: 10px;"

      # No button (MODE 3)
      - ::
        type: "nav-button"
        content: "😞 No, needs improvement"
        formId: "form-survey"
        fieldName: "satisfaction"
        value: "no"
        targetContainerId: "feedback"
        css: "background: #f38ba8; color: white; padding: 15px 30px; border-radius: 8px; font-weight: 600; margin: 10px;"

  # Thank you screen
  - ::
    id: "thank-you"
    name: "Thank You"
    children::
      - ::
        type: "heading"
        content: "Thank you for your feedback!"

  # Feedback screen
  - ::
    id: "feedback"
    name: "Feedback"
    children::
      - ::
        type: "heading"
        content: "We're sorry to hear that. How can we improve?"
```

**What gets submitted when clicking "Yes":**
```json
{
  "formId": "form-survey",
  "eventName": "survey_response",
  "data": {
    "satisfaction": "yes"
  },
  "timestamp": 1234567890
}
```

**Example: Multiple Choice (3+ options)**
```huml
# Question
- ::
  type: "heading"
  content: "What's your favorite color?"
  css: "text-align: center; margin-bottom: 20px;"

# Form metadata
- ::
  id: "form-quiz"
  type: "form"
  eventName: "quiz_answer"

# Option 1 - Blue
- ::
  type: "nav-button"
  content: "🔵 Blue"
  formId: "form-quiz"
  fieldName: "favorite_color"
  value: "blue"
  targetContainerId: "result-blue"
  css: "background: #89b4fa; color: white; padding: 15px; width: 200px; margin: 5px;"

# Option 2 - Green
- ::
  type: "nav-button"
  content: "🟢 Green"
  formId: "form-quiz"
  fieldName: "favorite_color"
  value: "green"
  targetContainerId: "result-green"
  css: "background: #a6e3a1; color: #1e1e2e; padding: 15px; width: 200px; margin: 5px;"

# Option 3 - Red
- ::
  type: "nav-button"
  content: "🔴 Red"
  formId: "form-quiz"
  fieldName: "favorite_color"
  value: "red"
  targetContainerId: "result-red"
  css: "background: #f38ba8; color: white; padding: 15px; width: 200px; margin: 5px;"
```

---

#### MODE 4: Cache and Continue (Multi-Screen Forms)

**Use this when:** You have a form spread across multiple screens and want to save visible fields before navigating to the next screen.

**⚠️ CRITICAL: ALL 5 properties below are REQUIRED. Missing `formId` or `action: "continue"` is the #1 mistake!**

**Required Properties (ALL 5 are mandatory):**
- ✅ `type: "nav-button"`
- ✅ `content` - Button text (usually "Continue" or "Next")
- ✅ `formId` - Form to cache data for (MUST match the form id!)
- ✅ `action: "continue"` - **CRITICAL:** Tells the system to cache fields without submitting (NEVER omit this!)
- ✅ `targetContainerId` - Next screen to go to

**Common mistakes:**
1. Forgetting `action: "continue"` - button will navigate but NOT save the field data
2. Forgetting `formId` - system won't know which form to cache data for

**How it works:**
1. User fills form fields on current screen
2. User clicks Continue button (`action: "continue"`)
3. System caches all visible field values in memory
4. Navigates to next screen (NO submission happens)
5. Process repeats across screens
6. Final Submit button (MODE 2) collects ALL cached data + visible fields and submits ONCE

**Complete Multi-Screen Form Pattern:**
```huml
name: "Event Registration - Multi-Screen Pattern"

screens::
  # Screen 1: Welcome - Choose registration type
  - ::
    id: "welcome"
    name: "Welcome"
    isEntryPoint: true
    children::
      - ::
        type: "heading"
        content: "Select Registration Type"

      # Simple MODE 1 navigation - no formId needed
      - ::
        type: "nav-button"
        content: "Speaker Registration"
        targetContainerId: "speaker-form"
        css: "background: #667eea; color: white; padding: 20px;"

  # Screen 2: Speaker form fields
  - ::
    id: "speaker-form"
    name: "Speaker Details"
    children::
      # Define the form
      - ::
        id: "form-speaker"
        type: "form"
        eventName: "speaker_registration"

      - ::
        type: "form-field-text"
        formId: "form-speaker"
        fieldName: "full_name"
        label: "Full Name"
        required: true

      - ::
        type: "form-field-email"
        formId: "form-speaker"
        fieldName: "email"
        label: "Email"
        required: true

      - ::
        type: "form-field-text"
        formId: "form-speaker"
        fieldName: "talk_title"
        label: "Talk Title"
        required: true

      # MODE 4: Cache and Continue button
      - ::
        type: "nav-button"
        content: "Continue →"
        formId: "form-speaker"
        action: "continue"
        targetContainerId: "speaker-topic"
        css: "background: #667eea; color: white; padding: 15px 30px;"

  # Screen 3: Topic selection
  - ::
    id: "speaker-topic"
    name: "Select Topic"
    children::
      - ::
        type: "heading"
        content: "What will you speak about?"

      # MODE 3A: Set value without submitting
      - ::
        type: "nav-button"
        content: "AI & Machine Learning"
        formId: "form-speaker"
        fieldName: "topic_area"
        value: "ai_ml"
        action: "setValueOnly"
        targetContainerId: "speaker-review"
        css: "background: #3d4571; color: white; padding: 15px;"

      - ::
        type: "nav-button"
        content: "Web Development"
        formId: "form-speaker"
        fieldName: "topic_area"
        value: "web_dev"
        action: "setValueOnly"
        targetContainerId: "speaker-review"
        css: "background: #3d4571; color: white; padding: 15px;"

  # Screen 4: Review and submit
  - ::
    id: "speaker-review"
    name: "Review & Submit"
    children::
      - ::
        type: "heading"
        content: "Review Your Information"

      - ::
        type: "form-field-textarea"
        formId: "form-speaker"
        fieldName: "additional_notes"
        label: "Additional Notes"
        required: false

      # MODE 2: Submit button (no action property)
      # This collects ALL cached data + visible fields and submits ONCE
      - ::
        type: "nav-button"
        content: "Submit Registration"
        formId: "form-speaker"
        targetContainerId: "confirmation"
        css: "background: #00ff88; color: #000; padding: 15px 30px; font-weight: 600;"

  # Screen 5: Confirmation
  - ::
    id: "confirmation"
    name: "Success"
    children::
      - ::
        type: "heading"
        content: "✅ Registration Complete!"
```

**What gets submitted when clicking final Submit button:**
```json
{
  "formId": "form-speaker",
  "eventName": "speaker_registration",
  "data": {
    "full_name": "John Doe",         // Cached from screen 2
    "email": "john@example.com",     // Cached from screen 2
    "talk_title": "The Future of AI", // Cached from screen 2
    "topic_area": "ai_ml",           // Cached from screen 3
    "additional_notes": "Looking forward to it!" // Visible on screen 4
  }
}
```

**Key Points:**
- `action: "continue"` caches visible fields without submitting
- `action: "setValueOnly"` caches single choice values without submitting
- Submit button (no `action` property) submits ALL accumulated data in ONE event
- Cache is automatically cleared after successful submission
- User can navigate back/forth without losing data (cache persists)

---

#### nav-button MODE Comparison

| Feature | MODE 1 | MODE 2 | MODE 3A | MODE 3B | MODE 4 |
|---------|--------|--------|---------|---------|--------|
| Navigate | ✅ | ✅ | ✅ | ✅ | ✅ |
| Submit form | ❌ | ✅ | ❌ | ✅ | ❌ |
| Cache single value | ❌ | ❌ | ✅ | ✅ | ❌ |
| Cache all visible fields | ❌ | ❌ | ❌ | ❌ | ✅ |
| Requires `formId` | ❌ | ✅ | ✅ | ✅ | ✅ |
| Requires `fieldName` | ❌ | ❌ | ✅ | ✅ | ❌ |
| Requires `value` | ❌ | ❌ | ✅ | ✅ | ❌ |
| Requires `action: "setValueOnly"` | ❌ | ❌ | ✅ | ❌ | ❌ |
| Requires `action: "continue"` | ❌ | ❌ | ❌ | ❌ | ✅ |
| Use case | Simple navigation | Final submission | Branching choices | Branching with submit | Continue between form screens |

---

#### Button Styling Examples

**Primary button:**
```huml
css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px; font-weight: 600;"
```

**Secondary button:**
```huml
css: "background: #e2e8f0; color: #2d3748; padding: 12px 24px; border-radius: 8px;"
```

**Success button (green):**
```huml
css: "background: #a6e3a1; color: #1e1e2e; padding: 12px 24px; border-radius: 8px; font-weight: 600;"
```

**Danger button (red):**
```huml
css: "background: #f38ba8; color: white; padding: 12px 24px; border-radius: 8px; font-weight: 600;"
```

**Full-width button:**
```huml
css: "width: 100%; background: #667eea; color: white; padding: 15px; border-radius: 8px; font-weight: 600;"
```

---

### Block Type: `thread`

**Purpose:** Create a comments/discussion section

**⚠️ CRITICAL STYLING REQUIREMENT:**
**Thread blocks ONLY accept CSS custom properties (variables). DO NOT use regular CSS properties like `background`, `color`, `padding`, etc. or class-based selectors. They will be ignored. You MUST use the `--thread-*` variables listed below.**

**Properties:**
- `type: "thread"` (required)
- `name` (optional) - Display name for this thread. **If you use this, you MUST include `--thread-title-color` in CSS or the heading will be invisible!**
- `mode: "markdown"` (optional) - Allows markdown in comments
- `description` (optional) - Placeholder text shown in comment input
- `css` (optional) - **MUST use CSS custom properties only** (see styling section below)

**How Threads Work:**
1. Users can add comments in both builder and viewer modes
2. Comments are stored separately and persist after reload
3. Each thread is completely independent - you can have multiple threads in one template
4. Threads have default GitHub-inspired light theme styling

**Styling Thread Blocks:**

Thread blocks have default light theme styles. To customize them, use CSS custom properties (CSS variables) in the `css` field. Each thread's styles are scoped to that thread only - multiple threads can have completely different themes without interfering with each other.

**⚠️ CRITICAL: Thread blocks ONLY accept CSS custom properties (variables).**

Thread blocks have a special styling system. Unlike other blocks where you can write regular CSS like `background: #fff; color: #000;`, thread blocks ONLY accept CSS custom properties (also called CSS variables).

**✅ CORRECT WAY - Use CSS Variables:**
```huml
css: "--thread-bg: #f6f8fa; --thread-button-bg: #0366d6; --thread-button-text: white;"
```

**❌ WRONG WAY - Regular CSS or Classes (Will Be Ignored):**
```huml
# This will NOT work:
css: "background: #f6f8fa; color: #24292e;"

# This will also NOT work:
css: """
  .thread-container {
    background: #f6f8fa;
  }
  textarea {
    border: 1px solid blue;
  }
"""
```

**Why?** Thread blocks use CSS custom properties for scoping - each thread can have its own theme without affecting others. Regular CSS selectors don't work with this scoping system.

**Available CSS Custom Properties:**

**⚠️ THESE ARE THE ONLY VALID VARIABLE NAMES. Using any other variable names will NOT work!**

| Property | Default | Description | Required for Dark Theme? |
|----------|---------|-------------|-------------------------|
| `--thread-bg` | `#f6f8fa` | Main container background | ✅ YES |
| `--thread-border-color` | `#e1e4e8` | Container border color | ✅ YES |
| `--thread-border-radius` | `8px` | Container border radius | Optional |
| `--thread-text-color` | `#24292e` | Main text color | ✅ YES |
| `--thread-title-color` | `#24292e` | Thread title color | ✅ YES (if using `name`) |
| `--thread-description-color` | `#6e7681` | Description text color | ✅ YES |
| `--thread-content-color` | `#24292e` | Content text color | Optional |
| `--thread-content-heading-color` | `#24292e` | Content headings color | Optional |
| `--thread-link-color` | `#0366d6` | Link color | Optional |
| `--thread-code-bg` | `#f6f8fa` | Code block background | Optional |
| `--thread-code-color` | `inherit` | Code text color | Optional |
| `--thread-input-bg` | `white` | Textarea background | ✅ YES |
| `--thread-input-border` | `#d1d5da` | Textarea border | ✅ YES |
| `--thread-input-text` | `#24292e` | Textarea text color | ✅ YES |
| `--thread-input-placeholder` | `#6e7681` | Placeholder text color | ✅ YES |
| `--thread-input-focus-border` | `#0366d6` | Textarea focus border | ✅ YES |
| `--thread-input-focus-shadow` | `rgba(3,102,214,0.1)` | Focus shadow color | ✅ YES |
| `--thread-button-bg` | `#0366d6` | Submit button background | ✅ YES |
| `--thread-button-text` | `white` | Submit button text | ✅ YES |
| `--thread-button-hover-bg` | `#0256c7` | Button hover background | ✅ YES |
| `--thread-button-disabled-bg` | `#94a3b8` | Disabled button background | ✅ YES |
| `--thread-button-disabled-text` | `#cbd5e0` | Disabled button text | ✅ YES |
| `--thread-comment-bg` | `white` | Comment box background | ✅ YES |
| `--thread-comment-border` | `#e1e4e8` | Comment box border | ✅ YES |
| `--thread-comment-text` | `#24292e` | Comment text color | Optional |
| `--thread-comment-author-color` | `#24292e` | Author name color | ✅ YES |
| `--thread-comment-time-color` | `#586069` | Timestamp color | ✅ YES |
| `--thread-comment-content-color` | `#24292e` | **Comment content text color** | ✅ **CRITICAL** |
| `--thread-toggle-bg` | `white` | **Toggle button background** | ✅ **CRITICAL** |
| `--thread-toggle-border` | `#d1d5da` | **Toggle button border** | ✅ **CRITICAL** |
| `--thread-toggle-color` | `#586069` | **Toggle button text** | ✅ **CRITICAL** |
| `--thread-toggle-hover-bg` | `#f6f8fa` | Toggle hover background | ✅ YES |
| `--thread-toggle-hover-border` | `#0366d6` | Toggle hover border | Optional |
| `--thread-toggle-hover-color` | `#0366d6` | Toggle hover text | Optional |
| `--thread-toggle-icon-color` | `#6a737d` | Toggle icon color | Optional |
| `--thread-no-comments-color` | `#6e7681` | **"X comments" label color** | ✅ **CRITICAL** |
| `--thread-form-bg` | `transparent` | Comment form background | Optional |
| `--thread-scrollbar-track` | `#f1f3f5` | **Scrollbar track background** | ✅ **CRITICAL** |
| `--thread-scrollbar-thumb` | `#adb5bd` | **Scrollbar thumb** | ✅ **CRITICAL** |
| `--thread-scrollbar-thumb-hover` | `#868e96` | **Scrollbar thumb hover** | ✅ **CRITICAL** |
| `--thread-error-bg` | `#f8d7da` | Error message background | Optional |
| `--thread-error-text` | `#721c24` | Error message text | Optional |
| `--thread-error-border` | `#f5c6cb` | Error message border | Optional |

**❌ INVALID VARIABLE NAMES (These will NOT work):**
- `--thread-padding` ❌
- `--thread-border-width` ❌
- `--thread-button-border-radius` ❌
- `--thread-input-border-width` ❌
- `--thread-input-border-radius` ❌
- `--thread-reply-bg` ❌
- `--thread-reply-border-color` ❌
- `--thread-comment-border-width` ❌
- `--thread-comment-border-radius` ❌
- `--thread-comment-date-color` ❌ (use `--thread-comment-time-color` instead)
- `--thread-link-hover-color` ❌
- Any variable not listed in the table above ❌

**Example: Complete Dark Theme Thread (ALL Required Variables)**
```huml
- ::
  type: "thread"
  name: "Community Discussion"
  mode: "markdown"
  description: "Share your thoughts..."
  css: "--thread-bg: #0a0a0a; --thread-border-color: #2a2a2a; --thread-text-color: #e0e0e0; --thread-title-color: #00ff88; --thread-description-color: #808080; --thread-no-comments-color: #808080; --thread-input-bg: #1a1a1a; --thread-input-border: #00ff88; --thread-input-text: #e0e0e0; --thread-input-placeholder: #606060; --thread-input-focus-border: #00ff88; --thread-input-focus-shadow: rgba(0, 255, 136, 0.15); --thread-button-bg: #00ff88; --thread-button-text: #0a0a0a; --thread-button-hover-bg: #00dd77; --thread-button-disabled-bg: #2a2a2a; --thread-button-disabled-text: #606060; --thread-comment-bg: #1a1a1a; --thread-comment-border: #2a2a2a; --thread-comment-author-color: #00ff88; --thread-comment-time-color: #808080; --thread-comment-content-color: #d0d0d0; --thread-toggle-bg: #1a1a1a; --thread-toggle-border: #2a2a2a; --thread-toggle-color: #808080; --thread-toggle-hover-bg: #2a2a2a; --thread-scrollbar-track: #1a1a1a; --thread-scrollbar-thumb: #404040; --thread-scrollbar-thumb-hover: #505050;"
```

**⚠️ CRITICAL VARIABLES INCLUDED:**
- `--thread-title-color` - Title text (required when using `name`)
- `--thread-comment-content-color` - Comment text content (MUST be set for dark themes!)
- `--thread-toggle-bg` - Toggle button background (fixes white toggle button!)
- `--thread-toggle-border` - Toggle button border
- `--thread-toggle-color` - Toggle button text color
- `--thread-no-comments-color` - "X comments" label color
- `--thread-scrollbar-track` - Scrollbar background (fixes white scrollbar!)
- `--thread-scrollbar-thumb` - Scrollbar thumb color (fixes white scrollbar!)
- `--thread-scrollbar-thumb-hover` - Scrollbar thumb hover state

**Without these variables, you'll get:**
- ❌ White toggle button on dark background
- ❌ White scrollbar on dark background
- ❌ Invisible comment text on dark background

**Example: Light Theme Thread (Custom Colors)**
```huml
- ::
  type: "thread"
  name: "Comments"
  mode: "markdown"
  description: "Join the discussion"
  css: "--thread-bg: white; --thread-border-color: #e2e8f0; --thread-title-color: #667eea; --thread-input-bg: #f7fafc; --thread-input-border: #cbd5e0; --thread-button-bg: #667eea; --thread-button-text: white; --thread-button-hover-bg: #5568d3;"
```

**Example: Multiple Independent Threads (Each with Different Styling)**
```huml
# Article 1 with blue-themed comments
- ::
  type: "markdown-text"
  content: "## Article 1: Introduction to HUML"

- ::
  type: "thread"
  name: "Article 1 Discussion"
  mode: "markdown"
  description: "Discuss Article 1..."
  css: "--thread-bg: #f7fafc; --thread-button-bg: #667eea; --thread-button-text: white; --thread-title-color: #667eea;"

# Article 2 with red-themed comments (completely independent)
- ::
  type: "markdown-text"
  content: "## Article 2: Advanced Features"

- ::
  type: "thread"
  name: "Article 2 Discussion"
  mode: "markdown"
  description: "Discuss Article 2..."
  css: "--thread-bg: #fff5f5; --thread-button-bg: #e53e3e; --thread-button-text: white; --thread-title-color: #e53e3e; --thread-border-color: #feb2b2;"

# Article 3 with green-themed comments (also independent)
- ::
  type: "markdown-text"
  content: "## Article 3: Best Practices"

- ::
  type: "thread"
  name: "Article 3 Discussion"
  mode: "markdown"
  description: "Discuss Article 3..."
  css: "--thread-bg: #f0fdf4; --thread-button-bg: #10b981; --thread-button-text: white; --thread-title-color: #10b981;"
```

**Note:** Each thread has its own independent styling. The CSS variables are scoped to each thread, so they don't interfere with each other.

---

### Thread CSS Validation Checklist

**Before finalizing your template, validate ALL thread blocks:**

✅ **Variable Names:**
- All variables start with `--thread-`
- Variable names match EXACTLY from the available list above
- No typos (e.g., `--thread-border` ❌ should be `--thread-border-color` ✅)

✅ **CRITICAL Variables for Dark Themes (MUST be included):**
- `--thread-comment-content-color` - Comment text (or it will be invisible!)
- `--thread-toggle-bg` - Toggle button background (or it will be white!)
- `--thread-toggle-border` - Toggle button border
- `--thread-toggle-color` - Toggle button text
- `--thread-no-comments-color` - Comment count label color
- `--thread-scrollbar-track` - Scrollbar background (or it will be white!)
- `--thread-scrollbar-thumb` - Scrollbar thumb (or it will be white!)
- `--thread-title-color` - Title text (if using `name` property)
- `--thread-description-color` - Description text color
- `--thread-input-placeholder` - Input placeholder text

✅ **Color Contrast Requirements:**
- If using dark background → all text colors must be light (e.g., `#e0e0e0`, `#d0d0d0`)
- If using light background → all text colors must be dark (e.g., `#24292e`, `#34495e`)
- Toggle button background should contrast with screen background
- Scrollbar should be visible against background

✅ **Common Mistakes to Avoid:**
- ❌ `--thread-border` → ✅ `--thread-border-color`
- ❌ `--thread-text-primary` → ✅ `--thread-text-color`
- ❌ `--thread-padding` → Not a valid variable
- ❌ `--thread-reply-bg` → Not a valid variable
- ❌ `--thread-comment-date-color` → ✅ `--thread-comment-time-color`
- ❌ `background: #1a1a1a;` → ✅ `--thread-bg: #1a1a1a;`
- ❌ Forgetting scrollbar variables → White scrollbar on dark theme!
- ❌ Forgetting toggle variables → White toggle button on dark theme!
- ❌ Forgetting comment content color → Invisible comment text!

✅ **Format Check:**
- No regular CSS properties (like `background:`, `padding:`, `color:`)
- No class selectors (like `.thread-container` or `textarea`)
- Only semicolon-separated CSS variable declarations
- All variable names from the valid list only

---

**When to use threads:**
- Blog post comments
- Discussion forums
- Q&A sections
- Feedback areas
- Multiple independent discussion topics in one template

---

### Block Type: `image`

**Purpose:** Display images in your application

**Properties:**
- `type: "image"` (required)
- `content` (required) - URL or path to image file
- `css` (optional) - Custom styling

**Example:**
```huml
- ::
  type: "image"
  content: "https://example.com/hero-image.jpg"
  css: "width: 100%; max-width: 800px; border-radius: 12px; margin: 20px 0;"
```

**Example: Profile Photo**
```huml
- ::
  type: "image"
  content: "https://example.com/avatar.jpg"
  css: "width: 120px; height: 120px; border-radius: 50%; object-fit: cover; border: 3px solid #667eea;"
```

**Example: Responsive Hero Image**
```huml
- ::
  type: "image"
  content: "https://example.com/hero.jpg"
  css: "width: 100%; max-width: 100%; height: auto; display: block; margin-bottom: 40px;"
```

**When to use:**
- Hero images for landing pages
- Blog post illustrations
- Product photos
- Diagrams and infographics
- Profile pictures
- Logos

**Styling tips:**
- Always include `max-width: 100%` for responsiveness
- Use `object-fit: cover` for fixed dimensions
- Add `border-radius` for rounded corners
- Use `display: block` to remove bottom spacing
- Add `margin` for spacing around images

---

### Block Type: `html`

**Purpose:** Embed custom HTML for advanced layouts or third-party widgets

**⚠️ Security Note:** HTML content is rendered without sanitization. Only use HTML blocks with trusted content you create yourself. Since publishers create all content in Sthalam, this is safe in your sovereign publishing model.

**Properties:**
- `type: "html"` (required)
- `content` (required) - Raw HTML string
- `css` (optional) - Container styling

**Example: Custom Widget**
```huml
- ::
  type: "html"
  content: "<div class='custom-widget'><h3>Custom Component</h3><p>Advanced HTML with custom classes</p></div>"
  css: "padding: 20px; background: #f0f0f0; border-radius: 8px;"
```

**Example: SVG Graphic**
```huml
- ::
  type: "html"
  content: "<svg width='100' height='100'><circle cx='50' cy='50' r='40' stroke='#667eea' stroke-width='3' fill='none' /></svg>"
  css: "text-align: center; margin: 20px 0;"
```

**Example: Embedded Video**
```huml
- ::
  type: "html"
  content: "<iframe width='560' height='315' src='https://www.youtube.com/embed/dQw4w9WgXcQ' frameborder='0' allowfullscreen></iframe>"
  css: "width: 100%; max-width: 800px; aspect-ratio: 16/9;"
```

**When to use:**
- Complex custom layouts not possible with other blocks
- Embedding third-party widgets (Twitter, YouTube, etc.)
- Custom interactive elements
- SVG graphics and animations
- Specialized HTML structures

**When NOT to use:**
- Regular content (use `text` or `markdown-text` instead)
- Simple images (use `image` block)
- Forms (use form field blocks)
- Buttons (use `nav-button`)

**Security reminder:** Never use HTML blocks with user-generated or untrusted content as it could execute malicious scripts.

---

## Forms - Complete Guide

### Understanding Forms

Forms in HUML require **THREE components** that work together:

1. **Form metadata block** (`type: "form"`) - Defines the form
2. **Form field blocks** (various `form-field-*` types) - The input fields
3. **Submit button** (`nav-button` with `formId`) - Submits the form

All three must be present and properly connected for forms to work.

---

### ⚠️ CRITICAL: Multi-Screen Form Requirements

**If your form spans multiple screens, you MUST use these button patterns correctly:**

| Button Type | Required Properties | What It Does |
|------------|---------------------|--------------|
| **Continue button**<br>(between screens) | `formId` + `action: "continue"` | Saves visible fields, navigates to next screen |
| **Choice button**<br>(T-shirt size, topic, etc.) | `formId` + `fieldName` + `value` + `action: "setValueOnly"` | Saves single choice, navigates to next screen |
| **Submit button**<br>(final screen only) | `formId` + NO `action` property | Submits ALL cached data + visible fields |

**⚠️ Missing `action: "continue"` or `action: "setValueOnly"` will cause data loss!**

The button will navigate but won't save the data. Only the last screen's data will be submitted.

**See [🚫 WRONG vs ✅ RIGHT Examples](#-wrong-vs--right-multi-screen-form-button-examples) for visual examples.**

---

### Component 1: Form Metadata Block

This is a special block that defines your form. It MUST come before any form fields.

**⚠️ CRITICAL: Use `id` not `formId`, and use `eventName` not `thankYouMessage`!**

**Required Properties (ALL 4 are mandatory):**
- ✅ `id` - Unique form identifier (⚠️ NOT `formId`!)
- ✅ `type: "form"` - Must be exactly "form"
- ✅ `name` - Display name for editor
- ✅ `eventName` - Groups submissions (⚠️ NOT `thankYouMessage`!)

**✅ CORRECT Example:**
```huml
- ::
  id: "form-contact"           # ✅ Use "id" not "formId"
  type: "form"
  name: "Contact Form"
  eventName: "contact_submission"  # ✅ Use "eventName" not "thankYouMessage"
```

**❌ COMMON MISTAKES:**
```huml
# ❌ WRONG - Will break the entire form!
- ::
  formId: "form-contact"       # ❌ Should be "id"
  type: "form"
  name: "Contact Form"
  thankYouMessage: "Thanks!"   # ❌ Should be "eventName"
```

**The `id` is crucial:** Form fields reference this with their `formId` property.

**The `eventName` groups submissions:** All submissions from this form will have this event name, making it easy to filter and export them later.

---

### Component 2: Form Field Blocks

These are the actual input fields users fill out. Each field MUST have a `formId` that matches the form's `id`.

#### Available Field Types

##### `form-field-text`
Single-line text input

**Properties:**
- `type: "form-field-text"` (required)
- `formId` (required) - Must match form's `id`
- `fieldName` (required) - Key name in submission data
- `label` (optional) - Display label
- `placeholder` (optional) - Placeholder text
- `required` (optional) - Whether field is required
- `css` (optional) - Custom styling

**Example:**
```huml
- ::
  type: "form-field-text"
  formId: "form-contact"
  fieldName: "full_name"
  label: "Your Name"
  placeholder: "John Doe"
  required: true
  css: "margin-bottom: 20px;"
```

---

##### `form-field-email`
Email input with validation

**Properties:** Same as `form-field-text`

**Features:**
- Browser validates email format
- Mobile keyboards show @ and .com keys

**Example:**
```huml
- ::
  type: "form-field-email"
  formId: "form-newsletter"
  fieldName: "email"
  label: "Email Address"
  placeholder: "you@example.com"
  required: true
  css: "margin-bottom: 20px;"
```

---

##### `form-field-textarea`
Multi-line text input

**Properties:** Same as `form-field-text`

**Example:**
```huml
- ::
  type: "form-field-textarea"
  formId: "form-feedback"
  fieldName: "message"
  label: "Your Message"
  placeholder: "Tell us what you think..."
  required: true
  css: "margin-bottom: 20px;"
```

---

##### `form-field-checkbox`
True/false checkbox

**Properties:**
- `type: "form-field-checkbox"` (required)
- `formId` (required)
- `fieldName` (required)
- `label` (required) - Checkbox label
- `required` (optional)
- `css` (optional)

**Example:**
```huml
- ::
  type: "form-field-checkbox"
  formId: "form-signup"
  fieldName: "agree_to_terms"
  label: "I agree to the Terms and Conditions"
  required: true
  css: "margin-bottom: 20px;"
```

---

##### `form-field-number`
Numeric input

**Properties:** Same as `form-field-text`

**Example:**
```huml
- ::
  type: "form-field-number"
  formId: "form-order"
  fieldName: "quantity"
  label: "Quantity"
  placeholder: "1"
  required: true
```

---

##### `form-field-password`
Password input (masked text)

**Properties:** Same as `form-field-text`

**Example:**
```huml
- ::
  type: "form-field-password"
  formId: "form-login"
  fieldName: "password"
  label: "Password"
  required: true
```

---

### Component 3: Submit Button

Use `nav-button` in **MODE 2** to submit the form.

**Required properties:**
- `type: "nav-button"`
- `content` - Button text (e.g., "Submit", "Send", "Continue")
- `formId` - Must match form's `id`
- `targetContainerId` - Where to navigate after submission

**Example:**
```huml
- ::
  type: "nav-button"
  content: "Submit Form"
  formId: "form-contact"
  targetContainerId: "thank-you"
  css: "width: 100%; background: #667eea; color: white; padding: 15px; border-radius: 8px; font-weight: 600;"
```

---

### Complete Working Form Example

Here's a **complete, working** contact form:

```huml
name: "Contact Form Example"

screens::
  # Contact page
  - ::
    id: "contact"
    name: "Contact Page"
    css: "padding: 40px; background: #f7fafc; min-height: 100vh;"
    children::
      - ::
        type: "heading"
        content: "Contact Us"
        css: "text-align: center; font-size: 36px; margin-bottom: 40px;"

      # COMPONENT 1: Form metadata (MUST COME FIRST)
      - ::
        id: "form-contact"
        type: "form"
        name: "Contact Form"
        eventName: "contact_submission"

      # Form container
      - ::
        type: "section-container"
        name: "Form Container"
        css: "max-width: 600px; margin: 0 auto; background: white; padding: 40px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
        children::
          # COMPONENT 2: Form fields (all have matching formId)

          # Name field
          - ::
            type: "form-field-text"
            formId: "form-contact"
            fieldName: "name"
            label: "Your Name"
            placeholder: "John Doe"
            required: true
            css: "margin-bottom: 20px;"

          # Email field
          - ::
            type: "form-field-email"
            formId: "form-contact"
            fieldName: "email"
            label: "Email Address"
            placeholder: "you@example.com"
            required: true
            css: "margin-bottom: 20px;"

          # Message field
          - ::
            type: "form-field-textarea"
            formId: "form-contact"
            fieldName: "message"
            label: "Your Message"
            placeholder: "How can we help you?"
            required: true
            css: "margin-bottom: 20px;"

          # Terms checkbox
          - ::
            type: "form-field-checkbox"
            formId: "form-contact"
            fieldName: "agree_to_contact"
            label: "I agree to be contacted via email"
            required: false
            css: "margin-bottom: 30px;"

          # COMPONENT 3: Submit button (nav-button MODE 2)
          - ::
            type: "nav-button"
            content: "Send Message"
            formId: "form-contact"
            targetContainerId: "thank-you"
            css: "width: 100%; background: #667eea; color: white; padding: 15px; border-radius: 8px; font-size: 16px; font-weight: 600;"

  # Thank you page
  - ::
    id: "thank-you"
    name: "Thank You"
    css: "padding: 40px; background: #f0fdf4; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center; text-align: center;"
    children::
      - ::
        type: "heading"
        content: "✅ Thank You!"
        css: "color: #16a34a; font-size: 48px; margin-bottom: 20px;"

      - ::
        type: "text"
        content: "Your message has been sent successfully. We'll get back to you soon!"
        css: "font-size: 18px; color: #4a5568; margin-bottom: 30px; max-width: 600px;"

      - ::
        type: "nav-button"
        content: "← Back to Contact"
        targetContainerId: "contact"
        css: "background: #16a34a; color: white; padding: 12px 24px; border-radius: 8px;"
```

---

### Form Validation

**How validation works:**

1. Mark fields as required: `required: true`
2. When user clicks submit button:
   - System checks all fields with `required: true`
   - If any are empty → Shows browser alert
   - If all filled → Submits form and navigates

**Example:**
```huml
- ::
  type: "form-field-text"
  formId: "form-signup"
  fieldName: "username"
  label: "Username"
  required: true  # User MUST fill this
```

---

### Viewing Submissions

After users submit forms:

1. Switch to **Viewer mode**
2. Click the **Submissions** toggle
3. See all submissions grouped by `eventName`
4. Export as CSV or JSON

**Submission data structure:**
```json
{
  "formId": "form-contact",
  "eventName": "contact_submission",
  "data": {
    "name": "John Doe",
    "email": "john@example.com",
    "message": "Hello!",
    "agree_to_contact": true
  },
  "timestamp": 1234567890
}
```

---

### Form Checklist

Before testing your form, verify:

**Form Metadata (CRITICAL - check these first!):**
- [ ] **Form metadata uses `id` NOT `formId`** ⚠️ (Will break entire form!)
- [ ] **Form metadata uses `eventName` NOT `thankYouMessage`** ⚠️ (Will break entire form!)
- [ ] Form metadata block has `type: "form"`

**Form Fields and Buttons:**
- [ ] All form fields have `formId` matching the form's `id`
- [ ] All form fields have unique `fieldName` values
- [ ] Submit button is `type: "nav-button"`
- [ ] **Submit button `formId` EXACTLY matches the form's `id`** ⚠️ (Most common mistake!)
- [ ] Submit button has `targetContainerId` property
- [ ] Target success screen exists with matching `id`
- [ ] Required fields are marked with `required: true`

**For multi-screen forms, also verify:**
- [ ] All "Continue" buttons have `action: "continue"`
- [ ] All choice/topic buttons have `action: "setValueOnly"` + `fieldName` + `value`
- [ ] **ALL screens use the SAME `formId` value** ⚠️ (Second most common mistake!)
- [ ] Only the FINAL submit button has NO `action` property

**If all checked:** Your form will work! ✅

---

### 🚫 WRONG vs ✅ RIGHT: Multi-Screen Form Button Examples

**These examples show the MOST COMMON mistakes when creating multi-screen forms:**

#### ❌ WRONG: Choice Button Missing `action: "setValueOnly"`
```huml
# This button will navigate but WON'T save the choice!
- ::
  type: "nav-button"
  content: "Small"
  formId: "form-registration"
  fieldName: "tshirt_size"
  value: "S"
  targetContainerId: "next-screen"
  # ❌ MISSING: action: "setValueOnly"
```

#### ✅ RIGHT: Choice Button With All Required Properties
```huml
# This button saves the choice AND navigates
- ::
  type: "nav-button"
  content: "Small"
  formId: "form-registration"
  fieldName: "tshirt_size"
  value: "S"
  action: "setValueOnly"  # ✅ MUST HAVE THIS!
  targetContainerId: "next-screen"
```

---

#### ❌ WRONG: Continue Button Missing `formId` and `action: "continue"`
```huml
# This button will navigate but WON'T save form fields!
- ::
  type: "nav-button"
  content: "Continue →"
  targetContainerId: "next-screen"
  # ❌ MISSING: formId
  # ❌ MISSING: action: "continue"
```

#### ✅ RIGHT: Continue Button With All Required Properties
```huml
# This button saves all visible form fields AND navigates
- ::
  type: "nav-button"
  content: "Continue →"
  formId: "form-registration"  # ✅ MUST HAVE THIS!
  action: "continue"            # ✅ MUST HAVE THIS!
  targetContainerId: "next-screen"
```

---

#### ❌ WRONG: Submit Button With `action` Property
```huml
# This button won't submit! It will just navigate
- ::
  type: "nav-button"
  content: "Submit"
  formId: "form-registration"
  action: "continue"  # ❌ WRONG! Submit buttons should NOT have action property
  targetContainerId: "thank-you"
```

#### ✅ RIGHT: Submit Button (NO `action` property)
```huml
# This button submits ALL cached data + visible fields
- ::
  type: "nav-button"
  content: "Submit"
  formId: "form-registration"
  # ✅ CORRECT: NO action property for submit buttons!
  targetContainerId: "thank-you"
```

---

### ⚠️ Common Form Mistakes & Troubleshooting

**❌ MISTAKE #1: Invalid Form Metadata Properties (CRITICAL - Breaks ALL Forms!)**

**Problem:** Form doesn't work at all - NO submissions happen, fields don't validate, nothing works.

**Cause:** Using wrong property names in form metadata block (`formId` instead of `id`, or `thankYouMessage` instead of `eventName`).

**Example of the bug:**
```huml
# ❌ WRONG - This will break the ENTIRE form!
- ::
  formId: "form-registration"     # ❌ Should be "id"
  type: "form"
  name: "Registration Form"
  thankYouMessage: "Thanks!"      # ❌ Should be "eventName"
```

**Fix:**
```huml
# ✅ CORRECT
- ::
  id: "form-registration"         # ✅ Use "id"
  type: "form"
  name: "Registration Form"
  eventName: "registration_submission"  # ✅ Use "eventName"
```

**Why this happens:** The system can't find the form because it has no valid `id`. All form fields, continue buttons, and submit buttons reference `formId`, but if the form doesn't have an `id`, nothing can connect to it.

---

**❌ MISTAKE #2: Wrong `formId` on Submit Button**

**Problem:** Multi-screen form only submits data from the last screen, missing all previous screens' data.

**Example of the bug:**
```huml
screens::
  - ::
    id: "attendee-info"
    children::
      - ::
        id: "form-attendee"  # ✅ Correct form ID
        type: "form"
        eventName: "attendee_registration"

      # Form fields...
      - ::
        type: "form-field-text"
        formId: "form-attendee"  # ✅ Correct
        fieldName: "full_name"

      # Continue button
      - ::
        type: "nav-button"
        content: "Continue →"
        formId: "form-attendee"  # ✅ Correct
        action: "continue"
        targetContainerId: "attendee-preferences"

  - ::
    id: "attendee-preferences"
    children::
      # More form fields...

      # Submit button with WRONG formId
      - ::
        type: "nav-button"
        content: "Submit Registration"
        formId: "form-sponsor"  # ❌ WRONG! Should be "form-attendee"
        targetContainerId: "confirmation"
```

**Result:** Form submits nothing or only partial data because it's looking for `form-sponsor` fields that don't exist!

**Fix:** Ensure submit button `formId` matches the form's `id`:
```huml
- ::
  type: "nav-button"
  content: "Submit Registration"
  formId: "form-attendee"  # ✅ Now matches the form ID
  targetContainerId: "confirmation"
```

---

**❌ MISTAKE #3: Missing `action: "continue"` on Multi-Screen Forms**

**Problem:** Form submits prematurely on first screen instead of collecting all data.

**Wrong:**
```huml
- ::
  type: "nav-button"
  content: "Continue →"
  formId: "form-registration"
  targetContainerId: "next-screen"  # Missing action: "continue"
```

**Correct:**
```huml
- ::
  type: "nav-button"
  content: "Continue →"
  formId: "form-registration"
  action: "continue"  # ✅ Caches data without submitting
  targetContainerId: "next-screen"
```

---

**❌ MISTAKE #4: Missing `action: "setValueOnly"` on Choice Buttons**

**Problem:** Form submits immediately on topic/choice selection instead of continuing to next screen.

**Wrong:**
```huml
- ::
  type: "nav-button"
  content: "Web Development"
  formId: "form-speaker"
  fieldName: "topic"
  value: "web_dev"
  targetContainerId: "next-screen"  # Missing action: "setValueOnly"
```

**Correct:**
```huml
- ::
  type: "nav-button"
  content: "Web Development"
  formId: "form-speaker"
  fieldName: "topic"
  value: "web_dev"
  action: "setValueOnly"  # ✅ Caches choice without submitting
  targetContainerId: "next-screen"
```

---

**❌ MISTAKE #5: Inconsistent `formId` Across Screens**

**Problem:** Different screens use different `formId` values for the same logical form.

**Wrong:**
```huml
# Screen 1
- ::
  type: "form-field-text"
  formId: "form-registration"  # ❌ Inconsistent
  fieldName: "name"

# Screen 2
- ::
  type: "form-field-email"
  formId: "registration-form"  # ❌ Different ID!
  fieldName: "email"

# Submit
- ::
  type: "nav-button"
  formId: "form-submit"  # ❌ Yet another ID!
```

**Correct:**
```huml
# ALL use the same formId
# Screen 1
- ::
  type: "form-field-text"
  formId: "form-registration"  # ✅ Consistent
  fieldName: "name"

# Screen 2
- ::
  type: "form-field-email"
  formId: "form-registration"  # ✅ Same ID
  fieldName: "email"

# Submit
- ::
  type: "nav-button"
  formId: "form-registration"  # ✅ Same ID
```

---

**❌ MISTAKE #6: Missing Form Metadata Block**

**Problem:** Form fields exist but no `type: "form"` metadata block to define the form.

**Wrong:**
```huml
# No form metadata!
- ::
  type: "form-field-text"
  formId: "form-contact"  # ❌ No matching form metadata
  fieldName: "name"
```

**Correct:**
```huml
# Define form metadata first
- ::
  id: "form-contact"  # ✅ Form metadata exists
  type: "form"
  eventName: "contact_submission"

# Then add form fields
- ::
  type: "form-field-text"
  formId: "form-contact"  # ✅ Now has matching form
  fieldName: "name"
```

---

**❌ MISTAKE #7: Multiple Submissions (Forgetting `action` Property)**

**Problem:** Form submits 3-4 times instead of once because continue/choice buttons are missing `action` property.

**Cause:** When a `nav-button` has `formId` but NO `action` property, it defaults to MODE 2 (submit). Every screen transition creates a submission!

**Example of the bug:**
```huml
# Student Registration - Creates 3 SUBMISSIONS instead of 1!

# Screen 1: Continue button WITHOUT action
- ::
  type: "nav-button"
  content: "Next →"
  formId: "form-student"  # ❌ Has formId but NO action property
  targetContainerId: "student-screen2"  # Will SUBMIT instead of caching!

# Screen 2: T-shirt buttons WITHOUT action
- ::
  type: "nav-button"
  content: "M"
  formId: "form-student"
  fieldName: "tshirt_size"
  value: "M"
  targetContainerId: "student-screen3"  # ❌ Missing action: "setValueOnly" - Will SUBMIT!

# Screen 3: Final submit
- ::
  type: "nav-button"
  content: "Submit"
  formId: "form-student"
  targetContainerId: "confirmation"  # ✅ This one is correct, but 2 submissions already happened!
```

**Result:** You get 3 submissions in the database instead of 1 final submission with all data!

**Fix:**
```huml
# Screen 1: Continue button WITH action
- ::
  type: "nav-button"
  content: "Next →"
  formId: "form-student"
  action: "continue"  # ✅ MUST HAVE THIS!
  targetContainerId: "student-screen2"

# Screen 2: T-shirt buttons WITH action
- ::
  type: "nav-button"
  content: "M"
  formId: "form-student"
  fieldName: "tshirt_size"
  value: "M"
  action: "setValueOnly"  # ✅ MUST HAVE THIS!
  targetContainerId: "student-screen3"

# Screen 3: Final submit (NO action)
- ::
  type: "nav-button"
  content: "Submit"
  formId: "form-student"
  # ✅ NO action property - this is MODE 2
  targetContainerId: "confirmation"
```

**Key Rule:** If a button has `formId`, it MUST have either:
- `action: "continue"` (cache fields, go to next screen)
- `action: "setValueOnly"` (cache single value, go to next screen)
- NO action property (submit form - only for FINAL submit button!)

---

**🔍 Troubleshooting Checklist:**

**If your form isn't submitting:**
1. ✅ **Check form metadata uses `id` NOT `formId`** (Most critical!)
2. ✅ **Check form metadata uses `eventName` NOT `thankYouMessage`** (Most critical!)
3. Check browser console for `formId` not found errors
4. Verify ALL buttons/fields use the SAME `formId` value
5. Ensure form metadata block (`type: "form"`) exists with matching `id`
6. Confirm submit button has `formId` property (no `action` property)

**If only partial data is submitting (multi-screen forms):**
1. ✅ Verify submit button `formId` matches the form's `id` (not a different form!)
2. ✅ Check all "Continue" buttons have `action: "continue"`
3. ✅ Check all choice buttons have `action: "setValueOnly"` AND `fieldName` + `value`
4. ✅ Ensure consistent `formId` across ALL screens

**If form submits prematurely (before reaching last screen):**
1. Check "Continue" buttons have `action: "continue"`
2. Check choice buttons have `action: "setValueOnly"`
3. Ensure intermediate buttons DON'T have default navigation action

**If you're getting MULTIPLE submissions (3-4 submissions instead of 1):**
1. ✅ **Check ALL continue buttons have `action: "continue"`** (Most common!)
2. ✅ **Check ALL choice buttons have `action: "setValueOnly"`** (Most common!)
3. ✅ Remember: `formId` without `action` = SUBMIT (MODE 2)
4. ✅ Only the FINAL submit button should have NO `action` property

---

## CSS Styling Reference

### Inline CSS Format

CSS is provided as a string with semicolon-separated properties:

```huml
css: "property: value; property: value;"
```

---

### Colors

**Text color:**
```huml
css: "color: #333333;"
```

**Background color:**
```huml
css: "background: #ffffff;"
```

**Gradient background:**
```huml
css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);"
```

---

### Typography

**Font size:**
```huml
css: "font-size: 18px;"
```

**Font weight:**
```huml
css: "font-weight: 600;"  # 400=normal, 600=semi-bold, 700=bold
```

**Text alignment:**
```huml
css: "text-align: center;"  # left, center, right, justify
```

**Line height (for readability):**
```huml
css: "line-height: 1.6;"
```

---

### Spacing

**Padding (inside):**
```huml
css: "padding: 20px;"  # All sides
css: "padding: 20px 40px;"  # Top/bottom, Left/right
css: "padding-bottom: 30px;"  # Specific side
```

**Margin (outside):**
```huml
css: "margin: 20px;"
css: "margin-bottom: 30px;"
css: "margin: 0 auto;"  # Centers block horizontally
```

---

### Layout

**Width:**
```huml
css: "width: 800px;"
css: "width: 100%;"
css: "max-width: 600px;"
```

**Height:**
```huml
css: "height: 400px;"
css: "min-height: 100vh;"  # Full viewport height
```

**Flexbox (for containers):**
```huml
# Vertical stack
css: "display: flex; flex-direction: column; gap: 20px;"

# Horizontal row
css: "display: flex; flex-direction: row; gap: 20px;"

# Centered
css: "display: flex; justify-content: center; align-items: center;"
```

---

### Borders and Shadows

**Border:**
```huml
css: "border: 1px solid #e2e8f0;"
css: "border: 2px solid #667eea;"
css: "border-radius: 8px;"  # Rounded corners
```

**Box shadow:**
```huml
css: "box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
css: "box-shadow: 0 4px 12px rgba(0,0,0,0.15);"
```

---

### Complete CSS Examples

**Card style:**
```huml
css: "background: white; padding: 30px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); margin-bottom: 20px;"
```

**Centered container:**
```huml
css: "max-width: 800px; margin: 0 auto; padding: 40px;"
```

**Full-screen hero:**
```huml
css: "min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); padding: 40px; text-align: center;"
```

**Primary button:**
```huml
css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px; font-weight: 600; transition: all 0.2s;"
```

---

## Complete Working Examples

### Example 1: Simple Website

A basic website with homepage and about page.

```huml
name: "Simple Website"

screens::
  # Homepage
  - ::
    id: "home"
    name: "Homepage"
    isEntryPoint: true
    css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; padding: 40px; text-align: center; display: flex; flex-direction: column; justify-content: center;"
    children::
      - ::
        type: "heading"
        content: "Welcome to Our Website"
        css: "color: white; font-size: 56px; margin-bottom: 20px; font-weight: 800;"

      - ::
        type: "text"
        content: "We provide amazing services to help you succeed"
        css: "color: rgba(255,255,255,0.9); font-size: 24px; margin-bottom: 40px;"

      - ::
        type: "nav-button"
        content: "Learn More About Us →"
        targetContainerId: "about"
        css: "background: white; color: #667eea; padding: 20px 40px; border-radius: 12px; font-size: 20px; font-weight: 700;"

  # About page
  - ::
    id: "about"
    name: "About Us"
    css: "background: #f7fafc; padding: 40px; min-height: 100vh;"
    children::
      - ::
        type: "nav-button"
        content: "← Back to Home"
        targetContainerId: "home"
        css: "background: #e2e8f0; color: #2d3748; padding: 10px 20px; border-radius: 6px; margin-bottom: 30px;"

      - ::
        type: "section-container"
        name: "Content Container"
        css: "max-width: 800px; margin: 0 auto; background: white; padding: 60px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
        children::
          - ::
            type: "heading"
            content: "About Our Company"
            css: "font-size: 42px; margin-bottom: 30px; color: #1a202c;"

          - ::
            type: "text"
            content: "We've been in business since 2020, helping customers achieve their goals with innovative solutions."
            css: "font-size: 18px; line-height: 1.8; color: #4a5568; margin-bottom: 20px;"

          - ::
            type: "text"
            content: "Our team of experts is dedicated to providing the best service possible."
            css: "font-size: 18px; line-height: 1.8; color: #4a5568;"
```

---

### Example 2: Blog with Newsletter

Blog with posts and newsletter subscription form.

```huml
name: "Tech Blog"

screens::
  # Blog homepage
  - ::
    id: "home"
    name: "Blog Home"
    isEntryPoint: true
    css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); padding: 40px; min-height: 100vh;"
    children::
      - ::
        type: "heading"
        content: "Tech Insights Blog"
        css: "color: white; font-size: 56px; text-align: center; margin-bottom: 50px; font-weight: 800;"

      # Blog post card
      - ::
        type: "section-container"
        name: "Post Card"
        css: "max-width: 800px; margin: 0 auto 30px auto; background: white; padding: 40px; border-radius: 16px; box-shadow: 0 8px 16px rgba(0,0,0,0.15);"
        children::
          - ::
            type: "heading"
            content: "Getting Started with Svelte 5"
            css: "color: #2d3748; font-size: 32px; margin-bottom: 15px; font-weight: 700;"

          - ::
            type: "text"
            content: "Learn about Svelte 5's revolutionary Runes system and how they improve reactivity..."
            css: "color: #4a5568; margin-bottom: 20px; line-height: 1.8; font-size: 16px;"

          - ::
            type: "text"
            content: "📅 January 20, 2025  •  8 min read"
            css: "color: #718096; font-size: 14px; margin-bottom: 25px;"

          - ::
            type: "nav-button"
            content: "Read Article →"
            targetContainerId: "post"
            css: "background: #667eea; color: white; padding: 14px 28px; border-radius: 8px; font-weight: 600;"

      # Newsletter section
      - ::
        type: "section-container"
        name: "Newsletter"
        css: "max-width: 800px; margin: 40px auto 0 auto; background: rgba(255,255,255,0.95); padding: 50px; border-radius: 16px; text-align: center;"
        children::
          - ::
            type: "heading"
            content: "📬 Subscribe to Our Newsletter"
            css: "color: #2d3748; font-size: 32px; margin-bottom: 15px;"

          - ::
            type: "text"
            content: "Get weekly tech insights delivered to your inbox"
            css: "color: #4a5568; font-size: 16px; margin-bottom: 30px;"

          - ::
            type: "nav-button"
            content: "Subscribe Now →"
            targetContainerId: "subscribe"
            css: "background: #a6e3a1; color: #1e1e2e; padding: 16px 36px; border-radius: 8px; font-weight: 700;"

  # Blog post page
  - ::
    id: "post"
    name: "Blog Post"
    css: "background: #f7fafc; padding: 40px; min-height: 100vh;"
    children::
      - ::
        type: "nav-button"
        content: "← Back to Blog"
        targetContainerId: "home"
        css: "background: #e2e8f0; color: #2d3748; padding: 12px 24px; border-radius: 6px; margin-bottom: 30px;"

      - ::
        type: "section-container"
        name: "Article"
        css: "max-width: 900px; margin: 0 auto; background: white; padding: 60px; border-radius: 12px;"
        children::
          - ::
            type: "heading"
            content: "Getting Started with Svelte 5"
            css: "color: #1a202c; font-size: 42px; margin-bottom: 15px; font-weight: 800;"

          - ::
            type: "text"
            content: "📅 January 20, 2025"
            css: "color: #718096; margin-bottom: 40px;"

          - ::
            type: "markdown-text"
            content: "Svelte 5 introduces **Runes**, a revolutionary approach to reactivity.\n\n## What are Runes?\n\nRunes are special symbols:\n- `$state` - Reactive state\n- `$derived` - Computed values\n- `$effect` - Side effects\n\n## Example\n\n```svelte\nlet count = $state(0);\nlet doubled = $derived(count * 2);\n```\n\nTry them out today!"
            mode: "markdown"
            css: "line-height: 1.9; margin-bottom: 50px;"

          # Comments section
          - ::
            type: "heading"
            content: "💬 Comments"
            css: "margin-top: 50px; padding-top: 50px; border-top: 3px solid #e2e8f0; margin-bottom: 25px;"

          - ::
            type: "thread"
            name: "Post Comments"
            mode: "markdown"
            description: "Share your thoughts..."
            css: "min-height: 400px;"

  # Newsletter subscription page
  - ::
    id: "subscribe"
    name: "Subscribe"
    css: "background: linear-gradient(135deg, #a6e3a1 0%, #89b4fa 100%); padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center;"
    children::
      # Form metadata
      - ::
        id: "form-newsletter"
        type: "form"
        name: "Newsletter Subscription"
        eventName: "newsletter_signup"

      # Form container
      - ::
        type: "section-container"
        name: "Form"
        css: "max-width: 550px; margin: 0 auto; background: white; padding: 50px; border-radius: 16px;"
        children::
          - ::
            type: "heading"
            content: "📬 Join Our Newsletter"
            css: "text-align: center; font-size: 36px; margin-bottom: 15px; font-weight: 800;"

          - ::
            type: "text"
            content: "Get weekly tech insights. No spam, unsubscribe anytime."
            css: "text-align: center; color: #4a5568; margin-bottom: 35px;"

          # Name field
          - ::
            type: "form-field-text"
            formId: "form-newsletter"
            fieldName: "name"
            label: "Your Name"
            placeholder: "John Doe"
            required: true
            css: "margin-bottom: 20px;"

          # Email field
          - ::
            type: "form-field-email"
            formId: "form-newsletter"
            fieldName: "email"
            label: "Email Address"
            placeholder: "you@example.com"
            required: true
            css: "margin-bottom: 30px;"

          # Submit button
          - ::
            type: "nav-button"
            content: "Subscribe →"
            formId: "form-newsletter"
            targetContainerId: "thank-you"
            css: "width: 100%; background: #667eea; color: white; padding: 16px; border-radius: 8px; font-weight: 700;"

          # Back button
          - ::
            type: "nav-button"
            content: "← Back"
            targetContainerId: "home"
            css: "width: 100%; margin-top: 15px; background: transparent; color: #718096;"

  # Thank you page
  - ::
    id: "thank-you"
    name: "Thank You"
    css: "background: linear-gradient(135deg, #a6e3a1 0%, #89b4fa 100%); padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center; text-align: center;"
    children::
      - ::
        type: "heading"
        content: "✅ Welcome Aboard!"
        css: "color: white; font-size: 52px; margin-bottom: 20px; font-weight: 800;"

      - ::
        type: "text"
        content: "Thanks for subscribing! Check your inbox for confirmation."
        css: "color: rgba(255,255,255,0.95); font-size: 20px; margin-bottom: 40px;"

      - ::
        type: "nav-button"
        content: "← Back to Blog"
        targetContainerId: "home"
        css: "background: white; color: #667eea; padding: 16px 40px; border-radius: 8px; font-weight: 700;"
```

---

### Example 3: Survey with Branching

Customer survey with different paths based on responses.

```huml
name: "Customer Survey"

screens::
  # Welcome screen
  - ::
    id: "welcome"
    name: "Welcome"
    isEntryPoint: true
    css: "background: linear-gradient(135deg, #a6e3a1 0%, #89b4fa 100%); padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center; text-align: center;"
    children::
      - ::
        type: "heading"
        content: "Customer Satisfaction Survey"
        css: "color: white; font-size: 48px; margin-bottom: 20px; font-weight: 800;"

      - ::
        type: "text"
        content: "Help us improve by sharing your feedback"
        css: "color: rgba(255,255,255,0.9); font-size: 20px; margin-bottom: 40px;"

      - ::
        type: "nav-button"
        content: "Start Survey"
        targetContainerId: "q1"
        css: "background: white; color: #667eea; padding: 15px 40px; border-radius: 8px; font-size: 18px; font-weight: 600;"

  # Question 1: Satisfaction level
  - ::
    id: "q1"
    name: "Question 1"
    css: "background: #f7fafc; padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center;"
    children::
      # Form metadata
      - ::
        id: "form-survey"
        type: "form"
        name: "Customer Survey"
        eventName: "customer_survey"

      - ::
        type: "heading"
        content: "Are you satisfied with our service?"
        css: "text-align: center; font-size: 32px; margin-bottom: 40px; color: #2d3748;"

      # Satisfied option (MODE 3)
      - ::
        type: "nav-button"
        content: "😊 Yes, very satisfied"
        formId: "form-survey"
        fieldName: "satisfaction"
        value: "satisfied"
        targetContainerId: "feedback-positive"
        css: "background: #a6e3a1; color: #1e1e2e; padding: 20px 40px; border-radius: 8px; font-size: 18px; font-weight: 600; margin: 10px; min-width: 300px;"

      # Neutral option (MODE 3)
      - ::
        type: "nav-button"
        content: "😐 It's okay"
        formId: "form-survey"
        fieldName: "satisfaction"
        value: "neutral"
        targetContainerId: "feedback-neutral"
        css: "background: #f9e2af; color: #1e1e2e; padding: 20px 40px; border-radius: 8px; font-size: 18px; font-weight: 600; margin: 10px; min-width: 300px;"

      # Unsatisfied option (MODE 3)
      - ::
        type: "nav-button"
        content: "😞 No, needs improvement"
        formId: "form-survey"
        fieldName: "satisfaction"
        value: "unsatisfied"
        targetContainerId: "feedback-negative"
        css: "background: #f38ba8; color: white; padding: 20px 40px; border-radius: 8px; font-size: 18px; font-weight: 600; margin: 10px; min-width: 300px;"

  # Feedback for satisfied customers
  - ::
    id: "feedback-positive"
    name: "Positive Feedback"
    css: "background: #f0fdf4; padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center;"
    children::
      - ::
        type: "heading"
        content: "Great! What do you love most?"
        css: "text-align: center; font-size: 28px; margin-bottom: 30px; color: #16a34a;"

      - ::
        type: "form-field-textarea"
        formId: "form-survey"
        fieldName: "positive_feedback"
        label: "Tell us what you love"
        placeholder: "We appreciate your feedback..."
        required: true
        css: "max-width: 600px; width: 100%; margin-bottom: 20px;"

      - ::
        type: "nav-button"
        content: "Submit Feedback"
        formId: "form-survey"
        targetContainerId: "thank-you"
        css: "background: #16a34a; color: white; padding: 15px 40px; border-radius: 8px; font-weight: 600;"

  # Feedback for neutral customers
  - ::
    id: "feedback-neutral"
    name: "Neutral Feedback"
    css: "background: #fffbeb; padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center;"
    children::
      - ::
        type: "heading"
        content: "How can we improve?"
        css: "text-align: center; font-size: 28px; margin-bottom: 30px; color: #92400e;"

      - ::
        type: "form-field-textarea"
        formId: "form-survey"
        fieldName: "improvement_suggestions"
        label: "Your suggestions"
        placeholder: "What would make it better?"
        required: true
        css: "max-width: 600px; width: 100%; margin-bottom: 20px;"

      - ::
        type: "nav-button"
        content: "Submit Feedback"
        formId: "form-survey"
        targetContainerId: "thank-you"
        css: "background: #d97706; color: white; padding: 15px 40px; border-radius: 8px; font-weight: 600;"

  # Feedback for unsatisfied customers
  - ::
    id: "feedback-negative"
    name: "Negative Feedback"
    css: "background: #fef2f2; padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center;"
    children::
      - ::
        type: "heading"
        content: "We're sorry. What went wrong?"
        css: "text-align: center; font-size: 28px; margin-bottom: 30px; color: #dc2626;"

      - ::
        type: "form-field-textarea"
        formId: "form-survey"
        fieldName: "complaint"
        label: "Please describe the issue"
        placeholder: "We want to make this right..."
        required: true
        css: "max-width: 600px; width: 100%; margin-bottom: 20px;"

      - ::
        type: "form-field-checkbox"
        formId: "form-survey"
        fieldName: "contact_me"
        label: "I'd like someone to contact me about this"
        css: "margin-bottom: 20px;"

      - ::
        type: "nav-button"
        content: "Submit Feedback"
        formId: "form-survey"
        targetContainerId: "thank-you"
        css: "background: #dc2626; color: white; padding: 15px 40px; border-radius: 8px; font-weight: 600;"

  # Thank you
  - ::
    id: "thank-you"
    name: "Thank You"
    css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center; text-align: center;"
    children::
      - ::
        type: "heading"
        content: "✅ Thank You!"
        css: "color: white; font-size: 48px; margin-bottom: 20px; font-weight: 800;"

      - ::
        type: "text"
        content: "Your feedback has been recorded. We appreciate you taking the time!"
        css: "color: rgba(255,255,255,0.9); font-size: 20px; max-width: 600px;"
```

---

## Best Practices

### Screen Organization

**✅ DO:**
- Use descriptive screen IDs (`"home"`, `"about"`, `"contact"`)
- Set exactly ONE entry point
- Ensure all screens are reachable via navigation

**❌ DON'T:**
- Use generic IDs (`"s1"`, `"page"`)
- Have multiple entry points
- Create orphaned screens with no way to reach them

---

### Content Structure

**✅ DO:**
- Use `section-container` to group related blocks
- **ALWAYS give containers meaningful names** (e.g., "Hero Section", "Blog Post Card", "Comment Area")
- Apply consistent spacing with CSS
- Use appropriate block types (heading for titles, markdown for rich content)

**❌ DON'T:**
- Put everything in a flat list
- Create unnamed containers (always use the `name` property!)
- Nest containers more than 3-4 levels deep
- Use heading blocks for regular text

---

### Forms

**✅ DO:**
- Always define form metadata first
- Use descriptive `fieldName` values
- Set `required: true` for essential fields
- Provide clear labels and placeholders
- Create a thank-you screen

**❌ DON'T:**
- Forget the form metadata block
- Use generic field names (`"field1"`)
- Skip labels on fields
- Submit without confirmation

---

### Navigation

**✅ DO:**
- Provide "back" buttons on detail screens
- Use consistent button styling
- Test all navigation paths
- Ensure no dead ends

**❌ DON'T:**
- Create circular loops without escape
- Use unclear button text
- Forget to set `targetContainerId`

---

### Styling

**✅ DO:**
- Use a consistent color palette
- Apply responsive max-widths
- Use proper spacing
- Use semantic colors (green for success, red for errors)
- **For thread blocks: ALWAYS use CSS custom properties (`--thread-*` variables)**

**❌ DON'T:**
- Mix too many colors
- Use tiny font sizes (<14px)
- Forget padding/margins
- Use pure black (#000) - use dark grays instead
- **Use regular CSS or class selectors in thread blocks (they won't work!)**

---

## Quick Reference
refer : https://huml.io/specifications/v0-1-0/ for guide on huml, this is your bible for huml ref

**⚠️ IMPORTANT: Thread blocks require CSS custom properties (variables), not regular CSS!**

### All Block Types

| Type | Purpose | CSS Note |
|------|---------|----------|
| `heading` | Large title text | Regular CSS |
| `text` | Regular paragraph text | Regular CSS |
| `markdown-text` | Rich text with markdown | Regular CSS |
| `section-container` | Group blocks together | Regular CSS |
| `nav-button` | Universal button (all modes) | Regular CSS |
| `thread` | Comments/discussion | **CSS variables ONLY** |
| `form` | Form metadata (invisible) |
| `form-field-text` | Single-line text input |
| `form-field-email` | Email input |
| `form-field-textarea` | Multi-line text |
| `form-field-checkbox` | Checkbox |
| `form-field-number` | Number input |
| `form-field-password` | Password input |
| `image` | Image display |
| `html` | Custom HTML content |

### nav-button Quick Reference

| Mode | When to Use | Required Properties |
|------|-------------|-------------------|
| MODE 1 | Simple navigation | `content`, `targetContainerId` |
| MODE 2 | Form submission | `content`, `formId`, `targetContainerId` |
| MODE 3 | Branching choices | `content`, `formId`, `fieldName`, `value`, `targetContainerId` |

---

## Conclusion

You now have everything you need to create complete web applications using HUML!

**Key Takeaways:**
1. HUML uses YAML-like syntax
2. Every app needs at least ONE screen with `isEntryPoint: true`
3. Use `nav-button` for ALL button interactions (3 modes)
4. Forms require form metadata + fields + submit button
5. Style everything with inline CSS
6. Test your navigation flows

**Need help?** Refer back to the [Complete Working Examples](#complete-working-examples).

**Happy building!** 🚀
