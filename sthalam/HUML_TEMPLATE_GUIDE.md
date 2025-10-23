# HUML Template Engine Guide for LLMs

**Version:** 1.0
**Last Updated:** January 2025
**Purpose:** Comprehensive guide for creating declarative UI templates using HUML (Hierarchical UI Markup Language)

---

## Table of Contents

1. [Introduction to HUML](#introduction-to-huml)
2. [HUML Syntax Basics](#huml-syntax-basics)
3. [Template Structure](#template-structure)
4. [Screen Containers](#screen-containers)
5. [Block Types Reference](#block-types-reference)
6. [Navigation Patterns](#navigation-patterns)
7. [Form Handling](#form-handling)
8. [Thread Blocks (Comments)](#thread-blocks-comments)
9. [CSS Styling](#css-styling)
10. [Complete Examples](#complete-examples)
11. [Best Practices](#best-practices)
12. [Common Patterns](#common-patterns)

---

## Introduction to HUML

### What is HUML?

HUML (Hierarchical UI Markup Language) is a **declarative markup language** for defining complete web applications, including:
- Multiple screens/pages
- Navigation between screens
- Form fields and submissions
- Thread-based comments
- Custom styling with CSS
- Complex branching logic

### Why HUML?

Traditional UI development requires:
- Writing component code in JavaScript/TypeScript
- Managing state with hooks or stores
- Handling routing logic
- Styling with separate CSS files

HUML lets you **declare the entire application structure** in a single file using a simple, readable syntax similar to YAML.

### Key Concepts

1. **Declarative** - You describe WHAT you want, not HOW to build it
2. **Hierarchical** - Blocks can contain children, creating nested structures
3. **Screen-based** - Applications are organized into screens (like pages)
4. **Type-safe** - Each block has a specific type with defined properties

---

## HUML Syntax Basics

### Format

HUML uses a **YAML-like syntax** with specific conventions:

```huml
# Comments start with #

# Simple property
name: "My Application"

# List property with ::
screens::
  - ::
    property: "value"
    children::
      - ::
        nested: "item"
```

### Syntax Rules

#### 1. Comments
```huml
# This is a comment
# Comments explain what sections do
```

#### 2. String Values
```huml
# Simple strings (no quotes needed if no special chars)
name: "My App"
content: "Hello, world!"

# Multi-line strings (use \n for line breaks)
content: "Line 1\nLine 2\nLine 3"
```

#### 3. Boolean Values
```huml
isEntryPoint: true
required: false
```

#### 4. Numeric Values
```huml
width: 800
height: 600
```

#### 5. Lists (Arrays)
Lists are denoted with `::` followed by items starting with `- ::`

```huml
screens::
  - ::
    id: "screen1"
  - ::
    id: "screen2"
```

#### 6. Nested Objects
```huml
screens::
  - ::
    id: "main"
    children::
      - ::
        type: "heading"
        children::
          - ::
            type: "text"
```

### Important Syntax Notes

- **Indentation matters** - Use consistent spacing (2 spaces recommended)
- **No curly braces or brackets** - Unlike JSON/JavaScript
- **Property-value pairs** - Format is `property: "value"`
- **Lists require `::`** - Both for declaration and items

---

## Template Structure

### Top-Level Structure

Every HUML template has this basic structure:

```huml
# Template metadata
name: "Application Name"

# Screen definitions
screens::
  - ::
    id: "screen-id"
    name: "Screen Display Name"
    isEntryPoint: true
    css: "optional-css-styling"
    children::
      # Blocks go here
```

### Required Top-Level Properties

#### `name` (required)
The display name of your template/application.

```huml
name: "My Blog Application"
```

**When to use:**
- Every template must have a name
- This appears in the UI when users load templates
- Keep it descriptive but concise (2-5 words)

#### `screens` (required)
An array of screen definitions. Screens are like pages in your application.

```huml
screens::
  - ::
    id: "home"
    # ... screen properties
  - ::
    id: "about"
    # ... screen properties
```

**When to use:**
- Every template needs at least ONE screen
- Multi-page applications have multiple screens
- Each screen is an independent view users can navigate to

---

## Screen Containers

### Screen Definition

A **screen** is a top-level container representing a page or view in your application.

```huml
screens::
  - ::
    id: "unique-screen-id"
    name: "Display Name"
    isEntryPoint: true
    css: "background: #f0f0f0; padding: 20px;"
    children::
      # Blocks that appear on this screen
```

### Screen Properties

#### `id` (required)
**Type:** String
**Purpose:** Unique identifier for this screen (used for navigation)

```huml
id: "screen-home"
```

**Rules:**
- Must be unique across all screens in the template
- Use kebab-case or camelCase (e.g., "screen-home", "contactPage")
- No spaces or special characters except hyphens/underscores
- Referenced by nav-button blocks for navigation

**Example:**
```huml
screens::
  - ::
    id: "home"
    # ... other properties
  - ::
    id: "about"
    # ... other properties
```

#### `name` (optional but recommended)
**Type:** String
**Purpose:** Human-readable display name for the screen

```huml
name: "Homepage"
```

**When to use:**
- Helps developers identify screens in the editor
- May be displayed in breadcrumbs or navigation UI
- Use descriptive names like "Contact Page", "Blog Post 1", "About Us"

#### `isEntryPoint` (optional, default: false)
**Type:** Boolean
**Purpose:** Marks this screen as the starting point of the application

```huml
isEntryPoint: true
```

**Rules:**
- **EXACTLY ONE screen** should have `isEntryPoint: true`
- This is the first screen users see when they open the application
- If no screen has this property, the first screen in the list is used

**Example:**
```huml
screens::
  - ::
    id: "home"
    name: "Homepage"
    isEntryPoint: true  # This screen loads first
  - ::
    id: "about"
    name: "About"
    # This screen is only shown when navigated to
```

#### `css` (optional)
**Type:** String (CSS properties)
**Purpose:** Inline CSS styling for the entire screen

```huml
css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); padding: 40px; min-height: 100vh;"
```

**When to use:**
- Set background colors/gradients for the screen
- Add padding/margins around screen content
- Control layout properties (flexbox, grid, etc.)

**CSS Properties Examples:**
```huml
# Solid background
css: "background: #ffffff; padding: 20px;"

# Gradient background
css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);"

# Flexbox layout
css: "display: flex; flex-direction: column; align-items: center; gap: 20px;"

# Full viewport height
css: "min-height: 100vh; padding: 40px;"
```

See [CSS Styling](#css-styling) section for more details.

#### `children` (optional)
**Type:** Array of blocks
**Purpose:** The blocks/components that appear on this screen

```huml
children::
  - ::
    type: "heading"
    content: "Welcome"
  - ::
    type: "text"
    content: "This is the homepage"
```

**When to use:**
- Every screen should have children blocks (otherwise it's empty)
- Children are rendered in the order they appear
- Can include any block type (heading, text, nav-button, form fields, etc.)

---

## Block Types Reference

### Overview

Blocks are the building elements of your application. Each block has a **type** that determines its behavior and appearance.

### Common Properties (All Blocks)

These properties are available on ALL block types:

#### `type` (required for all blocks)
**Type:** String
**Purpose:** Defines what kind of block this is

```huml
type: "heading"
```

**Available types:**
- `heading` - Large text for titles
- `text` - Regular paragraph text
- `markdown-text` - Rich text with markdown formatting
- `nav-button` - Navigation and form submission button
- `section-container` - Container for grouping blocks
- `screen-container` - (Rarely used in templates; screens are top-level)
- `thread` - Comments/discussion thread
- `form` - Form metadata block
- `form-field-text` - Single-line text input
- `form-field-email` - Email input with validation
- `form-field-textarea` - Multi-line text input
- `form-field-checkbox` - Checkbox input
- `image` - Image display (requires URL)

#### `content` (optional, depends on type)
**Type:** String
**Purpose:** The text content of the block

```huml
content: "This is the content"
```

**Used by:** heading, text, markdown-text, nav-button

#### `css` (optional)
**Type:** String (CSS properties)
**Purpose:** Inline styling for this specific block

```huml
css: "color: #333; font-size: 18px; margin-bottom: 20px;"
```

**When to use:**
- Customize appearance of individual blocks
- Override default styling
- Apply colors, spacing, fonts, borders, etc.

#### `children` (optional)
**Type:** Array of blocks
**Purpose:** Nested blocks inside this block

```huml
children::
  - ::
    type: "heading"
  - ::
    type: "text"
```

**Used by:** section-container, screen-container
**NOT used by:** heading, text, nav-button, form fields (they are leaf nodes)

---

### Block Type: `heading`

**Purpose:** Display large, prominent text for titles and section headers

#### Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `type` | String | ✅ Yes | Must be `"heading"` |
| `content` | String | ✅ Yes | The text to display |
| `css` | String | ❌ No | Custom styling |

#### Default Styling
- Large font size (varies by context)
- Bold weight
- Margin spacing

#### Example: Basic Heading
```huml
- ::
  type: "heading"
  content: "Welcome to My Website"
```

#### Example: Styled Heading
```huml
- ::
  type: "heading"
  content: "Featured Articles"
  css: "font-size: 48px; color: #89b4fa; text-align: center; margin-bottom: 30px; font-weight: 800;"
```

#### CSS Properties for Headings

**Common properties:**
```huml
# Font size
css: "font-size: 48px;"

# Color
css: "color: #333333;"

# Alignment
css: "text-align: center;"

# Weight
css: "font-weight: 700;"

# Spacing
css: "margin-bottom: 20px; margin-top: 10px;"

# Combined
css: "font-size: 36px; color: #667eea; text-align: center; margin-bottom: 20px; font-weight: 800;"
```

#### When to Use
- Page titles
- Section headers
- Article headlines
- Category names

---

### Block Type: `text`

**Purpose:** Display regular paragraph text

#### Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `type` | String | ✅ Yes | Must be `"text"` |
| `content` | String | ✅ Yes | The text to display |
| `css` | String | ❌ No | Custom styling |

#### Default Styling
- Regular font size (16px typically)
- Normal weight
- Line height for readability

#### Example: Basic Text
```huml
- ::
  type: "text"
  content: "This is a paragraph of text explaining something to the user."
```

#### Example: Styled Text
```huml
- ::
  type: "text"
  content: "Subscribe to our newsletter for weekly updates"
  css: "color: #718096; text-align: center; font-size: 14px; margin-top: 10px;"
```

#### Multi-line Text
Use `\n` for line breaks:

```huml
- ::
  type: "text"
  content: "Line 1\nLine 2\nLine 3"
```

#### CSS Properties for Text

**Common properties:**
```huml
# Font size
css: "font-size: 16px;"

# Color
css: "color: #666666;"

# Line height (for readability)
css: "line-height: 1.6;"

# Alignment
css: "text-align: left;"

# Spacing
css: "margin-bottom: 15px;"

# Combined
css: "color: #4a5568; font-size: 16px; line-height: 1.8; margin-bottom: 20px;"
```

#### When to Use
- Body text / paragraphs
- Descriptions
- Instructions
- Short messages
- Metadata (dates, author names, etc.)

---

### Block Type: `markdown-text`

**Purpose:** Display rich text with markdown formatting (headers, bold, italic, lists, code blocks, etc.)

#### Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `type` | String | ✅ Yes | Must be `"markdown-text"` |
| `content` | String | ✅ Yes | Markdown-formatted text |
| `mode` | String | ❌ No | Rendering mode: `"markdown"` (default) or `"html"` |
| `css` | String | ❌ No | Custom styling |

#### Markdown Support

**Headers:**
```huml
content: "## This is H2\n### This is H3"
```

**Bold and Italic:**
```huml
content: "This is **bold** and this is *italic*"
```

**Lists:**
```huml
content: "- Item 1\n- Item 2\n- Item 3"
```

**Ordered Lists:**
```huml
content: "1. First\n2. Second\n3. Third"
```

**Code Blocks:**
```huml
content: "```javascript\nfunction hello() {\n  console.log('Hello');\n}\n```"
```

**Inline Code:**
```huml
content: "Use the `useState` hook for state management"
```

**Links:**
```huml
content: "[Click here](https://example.com)"
```

**Blockquotes:**
```huml
content: "> This is a quote"
```

#### Example: Article Content
```huml
- ::
  type: "markdown-text"
  content: "## Introduction\n\nThis is a **comprehensive guide** to building apps.\n\n### Key Features\n\n- Easy to use\n- Powerful\n- Flexible\n\n```javascript\nconst app = createApp();\n```"
  mode: "markdown"
  css: "background: white; padding: 30px; border-radius: 8px; line-height: 1.8;"
```

#### Example: Blog Post
```huml
- ::
  type: "markdown-text"
  content: "# Getting Started\n\nWelcome to our platform! Here's what you need to know:\n\n## Step 1: Sign Up\n\nCreate your account by clicking the button below.\n\n## Step 2: Explore\n\nCheck out our features:\n- Dashboard\n- Analytics\n- Reports\n\n> **Tip:** Start with the tutorial for best results!"
  mode: "markdown"
```

#### CSS Properties for Markdown Text

```huml
# Background and padding (makes it look like a card)
css: "background: white; padding: 30px; border-radius: 8px;"

# Line height for readability
css: "line-height: 1.8;"

# Text color
css: "color: #2d3748;"

# Combined (article style)
css: "background: white; padding: 40px; border-radius: 12px; line-height: 1.8; color: #2d3748; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
```

#### When to Use
- Blog post content
- Article bodies
- Documentation
- Rich descriptions
- Tutorials
- Any content that needs formatting (bold, lists, code, etc.)

---

### Block Type: `section-container`

**Purpose:** Group related blocks together with shared styling/layout

#### Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `type` | String | ✅ Yes | Must be `"section-container"` |
| `name` | String | ❌ No | Display name for this section (for editor) |
| `css` | String | ❌ No | Container styling |
| `children` | Array | ❌ No | Blocks inside this container |

#### Example: Card Container
```huml
- ::
  type: "section-container"
  name: "Feature Card"
  css: "background: white; border-radius: 12px; padding: 30px; margin-bottom: 20px; box-shadow: 0 4px 6px rgba(0,0,0,0.1);"
  children::
    - ::
      type: "heading"
      content: "Feature Title"
    - ::
      type: "text"
      content: "Description of the feature"
```

#### Example: Two-Column Layout
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

#### Common CSS Patterns for Containers

**Card style:**
```huml
css: "background: white; border-radius: 12px; padding: 30px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
```

**Flexbox (vertical stack):**
```huml
css: "display: flex; flex-direction: column; gap: 15px;"
```

**Flexbox (horizontal row):**
```huml
css: "display: flex; flex-direction: row; gap: 20px;"
```

**Centered container:**
```huml
css: "max-width: 800px; margin: 0 auto; padding: 40px;"
```

**Border/outline:**
```huml
css: "border: 2px solid #667eea; border-radius: 8px; padding: 20px;"
```

#### When to Use
- Group related content (cards, sections)
- Apply shared styling to multiple blocks
- Create layout structures (columns, grids)
- Visual separation of content areas

---

### Block Type: `nav-button`

**Purpose:** Create clickable buttons for navigation and form submission

This is the **most versatile block type** with 3 distinct modes:

1. **MODE 1:** Simple navigation (just navigate to another screen)
2. **MODE 2:** Form submission + navigation (submit entire form, then navigate)
3. **MODE 3:** Branching with field values (set a specific field value, submit, then navigate)

#### Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `type` | String | ✅ Yes | Must be `"nav-button"` |
| `content` | String | ✅ Yes | Button text |
| `action` | String | ❌ No | Navigation action: `"navigate"`, `"show"`, `"hide"`, `"toggle"` (default: `"navigate"`) |
| `targetContainerId` | String | ❌ No | ID of screen to navigate to |
| `formId` | String | ❌ No | ID of form to submit (enables MODE 2 or 3) |
| `fieldName` | String | ❌ No | Field name to set (enables MODE 3) |
| `value` | Any | ❌ No | Value to set for the field (MODE 3) |
| `css` | String | ❌ No | Button styling |

#### MODE 1: Simple Navigation

**When to use:** Navigate from one screen to another without any form data.

**Required properties:**
- `content` - Button text
- `targetContainerId` - Screen ID to navigate to

**Optional properties:**
- `action` - Usually `"navigate"` (default)

**Example:**
```huml
- ::
  type: "nav-button"
  content: "Go to About Page"
  action: "navigate"
  targetContainerId: "screen-about"
  css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px;"
```

**Use cases:**
- "Read More" buttons on blog cards
- "Back to Home" buttons
- "View Details" links
- Navigation between pages

---

#### MODE 2: Form Submission + Navigation

**When to use:** Submit an entire form (all its fields), then navigate to another screen.

This replaces the old `form-submit-button` component.

**Required properties:**
- `content` - Button text (e.g., "Submit", "Send", "Continue")
- `formId` - ID of the form to submit
- `targetContainerId` - Screen ID to navigate to after submission

**How it works:**
1. Collects all form fields with matching `formId`
2. Validates required fields
3. Creates a submission record
4. Saves to `form_submissions_doc`
5. Navigates to target screen

**Example:**
```huml
# First, define the form metadata
- ::
  id: "form-contact"
  type: "form"
  name: "Contact Form"
  eventName: "contact_submission"

# Then add form fields
- ::
  type: "form-field-text"
  formId: "form-contact"
  fieldName: "name"
  label: "Your Name"
  required: true

- ::
  type: "form-field-email"
  formId: "form-contact"
  fieldName: "email"
  label: "Email Address"
  required: true

- ::
  type: "form-field-textarea"
  formId: "form-contact"
  fieldName: "message"
  label: "Message"
  required: true

# Finally, the submit button (MODE 2)
- ::
  type: "nav-button"
  content: "Submit Contact Form"
  formId: "form-contact"
  action: "navigate"
  targetContainerId: "screen-thank-you"
  css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px; width: 100%;"
```

**What gets submitted:**
```json
{
  "formId": "form-contact",
  "eventName": "contact_submission",
  "data": {
    "name": "John Doe",
    "email": "john@example.com",
    "message": "Hello, I have a question..."
  },
  "timestamp": 1234567890
}
```

**Validation:**
- Checks all fields with `required: true`
- Shows alert if any required field is empty
- Won't submit or navigate if validation fails

---

#### MODE 3: Branching with Field Values

**When to use:** Set a specific field to a specific value, submit, then navigate.

This replaces the old `branching-question` component.

**Required properties:**
- `content` - Button text (e.g., "Yes", "No", "Blue", "Option A")
- `formId` - ID of the form
- `fieldName` - Name of the field to set
- `value` - The value to set
- `targetContainerId` - Screen to navigate to

**How it works:**
1. Collects any existing form field values
2. Sets the specified `fieldName` to the specified `value`
3. Creates a submission with this data
4. Navigates to the target screen

**Example: Yes/No Question**
```huml
# Define form
- ::
  id: "form-survey"
  type: "form"
  name: "Customer Survey"
  eventName: "survey_response"

# Question
- ::
  type: "heading"
  content: "Are you satisfied with our service?"
  css: "text-align: center; margin-bottom: 20px;"

# Yes button (MODE 3)
- ::
  type: "nav-button"
  content: "Yes, I'm satisfied"
  formId: "form-survey"
  fieldName: "satisfaction"
  value: "yes"
  targetContainerId: "screen-thank-you"
  css: "background: #a6e3a1; color: #1e1e2e; padding: 15px 30px; border-radius: 8px; margin: 10px;"

# No button (MODE 3)
- ::
  type: "nav-button"
  content: "No, needs improvement"
  formId: "form-survey"
  fieldName: "satisfaction"
  value: "no"
  targetContainerId: "screen-feedback"
  css: "background: #f38ba8; color: white; padding: 15px 30px; border-radius: 8px; margin: 10px;"
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

**Example: Multiple Choice Question**
```huml
# Form definition
- ::
  id: "form-quiz"
  type: "form"
  eventName: "quiz_answer"

# Question
- ::
  type: "text"
  content: "What's your favorite color?"
  css: "font-size: 24px; text-align: center; margin-bottom: 20px;"

# Option 1
- ::
  type: "nav-button"
  content: "🔵 Blue"
  formId: "form-quiz"
  fieldName: "favorite_color"
  value: "blue"
  targetContainerId: "result-blue"
  css: "background: #89b4fa; color: white; padding: 15px; width: 200px; margin: 5px;"

# Option 2
- ::
  type: "nav-button"
  content: "🟢 Green"
  formId: "form-quiz"
  fieldName: "favorite_color"
  value: "green"
  targetContainerId: "result-green"
  css: "background: #a6e3a1; color: #1e1e2e; padding: 15px; width: 200px; margin: 5px;"

# Option 3
- ::
  type: "nav-button"
  content: "🔴 Red"
  formId: "form-quiz"
  fieldName: "favorite_color"
  value: "red"
  targetContainerId: "result-red"
  css: "background: #f38ba8; color: white; padding: 15px; width: 200px; margin: 5px;"
```

**Advanced: Branching with Multiple Fields**

You can have multiple nav-buttons that set different fields before navigating:

```huml
# Customer type form
- ::
  id: "form-onboarding"
  type: "form"
  eventName: "customer_onboarding"

# Step 1: Ask customer type
- ::
  type: "heading"
  content: "Are you a new customer?"

- ::
  type: "nav-button"
  content: "Yes, I'm new"
  formId: "form-onboarding"
  fieldName: "customer_type"
  value: "new"
  targetContainerId: "screen-new-customer-flow"

- ::
  type: "nav-button"
  content: "No, returning customer"
  formId: "form-onboarding"
  fieldName: "customer_type"
  value: "returning"
  targetContainerId: "screen-returning-customer-flow"
```

---

#### MODE Comparison Table

| Feature | MODE 1 | MODE 2 | MODE 3 |
|---------|--------|--------|--------|
| Navigate | ✅ | ✅ | ✅ |
| Submit form | ❌ | ✅ | ✅ |
| Set field value | ❌ | ❌ | ✅ |
| Required: `formId` | ❌ | ✅ | ✅ |
| Required: `fieldName` | ❌ | ❌ | ✅ |
| Required: `value` | ❌ | ❌ | ✅ |
| Use case | Simple navigation | Form submission | Branching/choices |
| Replaces | - | `form-submit-button` | `branching-question` |

---

#### CSS Properties for Buttons

**Primary button:**
```huml
css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px; font-weight: 600; border: none; cursor: pointer;"
```

**Secondary button:**
```huml
css: "background: #e2e8f0; color: #2d3748; padding: 12px 24px; border-radius: 8px; border: none; cursor: pointer;"
```

**Outlined button:**
```huml
css: "background: transparent; color: #667eea; padding: 12px 24px; border: 2px solid #667eea; border-radius: 8px; cursor: pointer;"
```

**Full-width button:**
```huml
css: "width: 100%; padding: 15px; background: #89b4fa; color: white; border-radius: 8px; font-size: 16px; font-weight: 600;"
```

**Success button:**
```huml
css: "background: #a6e3a1; color: #1e1e2e; padding: 12px 24px; border-radius: 8px; font-weight: 600;"
```

**Danger button:**
```huml
css: "background: #f38ba8; color: white; padding: 12px 24px; border-radius: 8px; font-weight: 600;"
```

---

### Block Type: `thread`

**Purpose:** Create a comments/discussion thread where users can post messages

#### Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `type` | String | ✅ Yes | Must be `"thread"` |
| `name` | String | ❌ No | Display name for this thread |
| `mode` | String | ❌ No | `"markdown"` (supports rich text) or `"html"` (default: `"markdown"`) |
| `description` | String | ❌ No | Placeholder/description text |
| `css` | String | ❌ No | Thread container styling |

#### How Threads Work

1. Users can add comments in both **editor mode** and **viewer mode**
2. Comments are stored in `thread_comments_doc` (separate from main blocks)
3. Comments persist after reload
4. Each thread is independent (identified by its block ID)
5. Markdown mode allows **rich text formatting** in comments

#### Example: Basic Thread
```huml
- ::
  type: "thread"
  name: "Article Comments"
  mode: "markdown"
  description: "Share your thoughts about this article"
  css: "background: white; border-radius: 8px; padding: 20px; min-height: 400px;"
```

#### Example: Blog Post Comments
```huml
# Blog post content
- ::
  type: "markdown-text"
  content: "## My Blog Post\n\nThis is the article content..."

# Divider
- ::
  type: "heading"
  content: "💬 Comments & Discussion"
  css: "margin-top: 40px; padding-top: 40px; border-top: 2px solid #e2e8f0;"

# Comments thread
- ::
  type: "thread"
  name: "Blog Post Comments"
  mode: "markdown"
  description: "Join the discussion - what are your thoughts?"
  css: "min-height: 500px;"
```

#### Example: Q&A Forum
```huml
- ::
  type: "heading"
  content: "Ask Questions"

- ::
  type: "text"
  content: "Have a question? Post it below and our community will help!"

- ::
  type: "thread"
  name: "Q&A Thread"
  mode: "markdown"
  description: "Type your question here..."
  css: "background: #f7fafc; border: 1px solid #e2e8f0; border-radius: 8px; padding: 20px; min-height: 600px;"
```

#### Thread Storage

**Important:** Thread comments are stored in a **separate Yjs document** called `thread_comments_doc`.

This means:
- Comments don't clutter the main blocks document
- Better performance for resources with many comments
- Comments can be loaded separately if needed

**Reference:** See `blocksuiteCoordinator.ts:131-142` for thread loading logic.

#### When to Use
- Blog post comments
- Article discussions
- Forum threads
- Q&A sections
- Feedback areas
- Collaborative notes

---

### Block Type: `form`

**Purpose:** Define form metadata (event name, form ID)

This is a **metadata block** - it doesn't render visually but defines the form's properties.

#### Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `type` | String | ✅ Yes | Must be `"form"` |
| `id` | String | ✅ Yes | Unique form identifier |
| `name` | String | ❌ No | Display name (for editor) |
| `eventName` | String | ❌ No | Event name for submissions (default: `"form_submission"`) |

#### How Forms Work

1. **Define the form metadata** with a `form` block
2. **Add form fields** (form-field-text, form-field-email, etc.) with matching `formId`
3. **Add a submit button** (nav-button with `formId`)

#### Example: Contact Form
```huml
# 1. Form metadata
- ::
  id: "form-contact"
  type: "form"
  name: "Contact Form"
  eventName: "contact_submission"

# 2. Form fields (see next section)
# 3. Submit button (see nav-button MODE 2)
```

#### Event Names

The `eventName` property groups submissions. This allows you to:
- Filter submissions by event in the submissions viewer
- Track different form types separately
- Export submissions by event

**Examples:**
```huml
eventName: "contact_submission"
eventName: "newsletter_signup"
eventName: "customer_feedback"
eventName: "survey_response"
eventName: "registration_form"
```

---

### Form Field Blocks

All form fields share common properties and behavior.

#### Common Form Field Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `type` | String | ✅ Yes | Field type (see below) |
| `formId` | String | ✅ Yes | ID of the parent form |
| `fieldName` | String | ✅ Yes | Name of this field (used in submission data) |
| `label` | String | ❌ No | Display label shown to user |
| `placeholder` | String | ❌ No | Placeholder text |
| `required` | Boolean | ❌ No | Whether field is required (default: false) |
| `css` | String | ❌ No | Field styling |

---

#### Block Type: `form-field-text`

**Purpose:** Single-line text input

**Example:**
```huml
- ::
  type: "form-field-text"
  formId: "form-contact"
  fieldName: "full_name"
  label: "Full Name"
  placeholder: "John Doe"
  required: true
  css: "margin-bottom: 15px;"
```

**Renders as:**
```
Full Name *
[________________]
```

**When to use:**
- Name fields
- Short answers
- Single-word inputs
- Usernames

---

#### Block Type: `form-field-email`

**Purpose:** Email input with validation

**Example:**
```huml
- ::
  type: "form-field-email"
  formId: "form-newsletter"
  fieldName: "email"
  label: "Email Address"
  placeholder: "you@example.com"
  required: true
```

**Features:**
- Browser email validation
- Proper keyboard on mobile (shows @ and .com keys)

**When to use:**
- Email collection
- Newsletter signups
- Contact forms
- Registration

---

#### Block Type: `form-field-textarea`

**Purpose:** Multi-line text input

**Example:**
```huml
- ::
  type: "form-field-textarea"
  formId: "form-feedback"
  fieldName: "comments"
  label: "Your Feedback"
  placeholder: "Tell us what you think..."
  required: false
  css: "margin-bottom: 20px;"
```

**Renders as:**
```
Your Feedback
┌─────────────────┐
│                 │
│                 │
│                 │
└─────────────────┘
```

**When to use:**
- Comments
- Feedback
- Messages
- Descriptions
- Long-form text

---

#### Block Type: `form-field-checkbox`

**Purpose:** Checkbox for yes/no or agreement

**Example:**
```huml
- ::
  type: "form-field-checkbox"
  formId: "form-signup"
  fieldName: "agree_to_terms"
  label: "I agree to the Terms and Conditions"
  required: true
```

**Renders as:**
```
☐ I agree to the Terms and Conditions *
```

**When to use:**
- Terms agreement
- Newsletter opt-in
- Preferences
- Boolean choices

---

#### Complete Form Example

```huml
# Screen with form
- ::
  id: "screen-contact"
  name: "Contact Us"
  css: "padding: 40px; background: #f7fafc;"
  children::
    - ::
      type: "heading"
      content: "Contact Us"
      css: "text-align: center; margin-bottom: 30px;"

    # Form metadata
    - ::
      id: "form-contact"
      type: "form"
      name: "Contact Form"
      eventName: "contact_submission"

    # Form container
    - ::
      type: "section-container"
      name: "Form Container"
      css: "max-width: 600px; margin: 0 auto; background: white; padding: 30px; border-radius: 12px;"
      children::
        # Name field
        - ::
          type: "form-field-text"
          formId: "form-contact"
          fieldName: "name"
          label: "Your Name"
          placeholder: "John Doe"
          required: true
          css: "margin-bottom: 15px;"

        # Email field
        - ::
          type: "form-field-email"
          formId: "form-contact"
          fieldName: "email"
          label: "Email Address"
          placeholder: "you@example.com"
          required: true
          css: "margin-bottom: 15px;"

        # Message field
        - ::
          type: "form-field-textarea"
          formId: "form-contact"
          fieldName: "message"
          label: "Message"
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
          css: "margin-bottom: 20px;"

        # Submit button (MODE 2)
        - ::
          type: "nav-button"
          content: "Send Message"
          formId: "form-contact"
          targetContainerId: "screen-thank-you"
          css: "width: 100%; background: #667eea; color: white; padding: 15px; border-radius: 8px; font-size: 16px; font-weight: 600;"

# Thank you screen
- ::
  id: "screen-thank-you"
  name: "Thank You"
  css: "padding: 40px; background: #f0fdf4; text-align: center; min-height: 100vh; display: flex; flex-direction: column; justify-content: center;"
  children::
    - ::
      type: "heading"
      content: "✅ Thank You!"
      css: "color: #16a34a; font-size: 48px; margin-bottom: 20px;"

    - ::
      type: "text"
      content: "Your message has been sent successfully. We'll get back to you soon!"
      css: "font-size: 18px; color: #4a5568; margin-bottom: 30px;"

    - ::
      type: "nav-button"
      content: "← Back to Home"
      targetContainerId: "screen-home"
      css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px;"
```

---

## Navigation Patterns

### Basic Navigation

**Simple page navigation:**

```huml
screens::
  # Home screen
  - ::
    id: "home"
    name: "Home"
    isEntryPoint: true
    children::
      - ::
        type: "heading"
        content: "Welcome"
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

### Hub and Spoke Pattern

One central screen with links to multiple pages:

```huml
screens::
  # Hub screen
  - ::
    id: "hub"
    name: "Main Menu"
    isEntryPoint: true
    children::
      - ::
        type: "heading"
        content: "Main Menu"
      - ::
        type: "nav-button"
        content: "Features"
        targetContainerId: "features"
      - ::
        type: "nav-button"
        content: "Pricing"
        targetContainerId: "pricing"
      - ::
        type: "nav-button"
        content: "Contact"
        targetContainerId: "contact"

  # Spoke 1
  - ::
    id: "features"
    name: "Features"
    children::
      - ::
        type: "heading"
        content: "Features"
      - ::
        type: "nav-button"
        content: "← Back to Menu"
        targetContainerId: "hub"

  # Spoke 2
  - ::
    id: "pricing"
    # ... similar structure

  # Spoke 3
  - ::
    id: "contact"
    # ... similar structure
```

### Linear Flow Pattern

Step-by-step progression:

```huml
screens::
  # Step 1
  - ::
    id: "step1"
    name: "Step 1: Welcome"
    isEntryPoint: true
    children::
      - ::
        type: "heading"
        content: "Step 1: Welcome"
      - ::
        type: "nav-button"
        content: "Next →"
        targetContainerId: "step2"

  # Step 2
  - ::
    id: "step2"
    name: "Step 2: Details"
    children::
      - ::
        type: "heading"
        content: "Step 2: Enter Details"
      - ::
        type: "nav-button"
        content: "Next →"
        targetContainerId: "step3"

  # Step 3
  - ::
    id: "step3"
    name: "Step 3: Confirm"
    children::
      - ::
        type: "heading"
        content: "Step 3: Confirmation"
      - ::
        type: "nav-button"
        content: "Complete"
        targetContainerId: "complete"
```

### Branching Flow Pattern

Different paths based on user choices:

```huml
screens::
  # Question screen
  - ::
    id: "question"
    name: "Question"
    isEntryPoint: true
    children::
      - ::
        type: "heading"
        content: "Are you a new customer?"

      # Branch A
      - ::
        type: "nav-button"
        content: "Yes, I'm new"
        targetContainerId: "new-customer"

      # Branch B
      - ::
        type: "nav-button"
        content: "No, returning customer"
        targetContainerId: "returning-customer"

  # Branch A destination
  - ::
    id: "new-customer"
    name: "New Customer Flow"
    children::
      - ::
        type: "heading"
        content: "Welcome, New Customer!"

  # Branch B destination
  - ::
    id: "returning-customer"
    name: "Returning Customer Flow"
    children::
      - ::
        type: "heading"
        content: "Welcome Back!"
```

---

## Form Handling

### Form Submission Flow

1. **Define form metadata**
2. **Add form fields**
3. **Add submit button** (nav-button MODE 2)
4. **Create success screen**

### Complete Form Example

See [Form Field Blocks](#form-field-blocks) section for full example.

### Form Validation

**Required field validation:**
- Set `required: true` on form fields
- Validation happens when submit button is clicked
- User sees alert if required fields are missing
- Form won't submit until all required fields are filled

**Example:**
```huml
- ::
  type: "form-field-text"
  formId: "form-signup"
  fieldName: "username"
  label: "Username"
  required: true  # ← Validation enforced
```

### Accessing Form Submissions

**In the application:**
- Switch to viewer mode
- Open the "Submissions" panel
- Filter by event name
- Export as CSV or JSON

**Reference:** See `SubmissionsViewer.svelte` component.

---

## Thread Blocks (Comments)

### Basic Thread Usage

```huml
- ::
  type: "thread"
  name: "Discussion"
  mode: "markdown"
  description: "Share your thoughts"
  css: "min-height: 400px;"
```

### Blog with Comments Pattern

```huml
screens::
  - ::
    id: "blog-post"
    name: "Blog Post"
    children::
      # Article content
      - ::
        type: "markdown-text"
        content: "## Article Title\n\nArticle content here..."

      # Comments section
      - ::
        type: "heading"
        content: "💬 Comments"
        css: "margin-top: 40px; border-top: 2px solid #eee; padding-top: 40px;"

      - ::
        type: "thread"
        name: "Article Comments"
        mode: "markdown"
        description: "Join the discussion"
```

### Multiple Threads

You can have multiple independent threads:

```huml
screens::
  - ::
    id: "forum"
    name: "Forum"
    children::
      # Thread 1
      - ::
        type: "heading"
        content: "Topic 1: Getting Started"
      - ::
        type: "thread"
        name: "Getting Started Thread"
        mode: "markdown"
        description: "Ask questions about getting started"

      # Thread 2
      - ::
        type: "heading"
        content: "Topic 2: Advanced Features"
      - ::
        type: "thread"
        name: "Advanced Features Thread"
        mode: "markdown"
        description: "Discuss advanced topics"
```

---

## CSS Styling

### Inline CSS Format

CSS is provided as a string with semicolon-separated properties:

```huml
css: "property: value; property: value;"
```

### Common CSS Properties

#### Colors

```huml
# Text color
css: "color: #333333;"

# Background color
css: "background: #ffffff;"

# Background gradient
css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);"
```

#### Typography

```huml
# Font size
css: "font-size: 18px;"

# Font weight
css: "font-weight: 600;"  # 400=normal, 600=semi-bold, 700=bold, 800=extra-bold

# Text alignment
css: "text-align: center;"  # left, center, right, justify

# Line height
css: "line-height: 1.6;"
```

#### Spacing

```huml
# Padding (inside)
css: "padding: 20px;"  # All sides
css: "padding: 20px 40px;"  # Top/bottom, Left/right
css: "padding: 10px 20px 30px 40px;"  # Top, Right, Bottom, Left

# Margin (outside)
css: "margin: 20px;"
css: "margin-bottom: 30px;"
css: "margin: 0 auto;"  # Centers block horizontally
```

#### Layout

```huml
# Width
css: "width: 800px;"
css: "width: 100%;"
css: "max-width: 600px;"
css: "min-width: 300px;"

# Height
css: "height: 400px;"
css: "min-height: 100vh;"  # Full viewport height

# Flexbox (for containers)
css: "display: flex; flex-direction: column; gap: 20px;"
css: "display: flex; justify-content: center; align-items: center;"
```

#### Borders and Shadows

```huml
# Border
css: "border: 1px solid #e2e8f0;"
css: "border: 2px solid #667eea;"
css: "border-radius: 8px;"  # Rounded corners

# Box shadow
css: "box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
css: "box-shadow: 0 4px 6px rgba(0,0,0,0.1);"
```

### Complete CSS Examples

#### Card Style
```huml
css: "background: white; padding: 30px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); margin-bottom: 20px;"
```

#### Primary Button
```huml
css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px; font-weight: 600; border: none; cursor: pointer; transition: all 0.2s;"
```

#### Centered Container
```huml
css: "max-width: 800px; margin: 0 auto; padding: 40px;"
```

#### Full-Screen Hero
```huml
css: "min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); padding: 40px; text-align: center;"
```

#### Article Content
```huml
css: "background: white; padding: 40px; border-radius: 12px; line-height: 1.8; color: #2d3748; max-width: 800px; margin: 0 auto; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
```

---

## Complete Examples

### Example 1: Simple Website

**Use case:** Basic informational website with 3 pages

```huml
# Simple Website Template
name: "Simple Website"

screens::
  # Homepage
  - ::
    id: "home"
    name: "Homepage"
    isEntryPoint: true
    css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; padding: 40px; text-align: center;"
    children::
      - ::
        type: "heading"
        content: "Welcome to Our Website"
        css: "color: white; font-size: 48px; margin-bottom: 20px;"

      - ::
        type: "text"
        content: "We provide amazing services to help you succeed"
        css: "color: rgba(255,255,255,0.9); font-size: 20px; margin-bottom: 40px;"

      - ::
        type: "section-container"
        name: "Navigation"
        css: "display: flex; gap: 20px; justify-content: center;"
        children::
          - ::
            type: "nav-button"
            content: "Our Services"
            targetContainerId: "services"
            css: "background: white; color: #667eea; padding: 15px 30px; border-radius: 8px; font-weight: 600;"

          - ::
            type: "nav-button"
            content: "Contact Us"
            targetContainerId: "contact"
            css: "background: transparent; color: white; border: 2px solid white; padding: 15px 30px; border-radius: 8px; font-weight: 600;"

  # Services page
  - ::
    id: "services"
    name: "Services"
    css: "background: #f7fafc; padding: 40px; min-height: 100vh;"
    children::
      - ::
        type: "nav-button"
        content: "← Back to Home"
        targetContainerId: "home"
        css: "background: #e2e8f0; color: #2d3748; padding: 10px 20px; border-radius: 6px; margin-bottom: 30px;"

      - ::
        type: "heading"
        content: "Our Services"
        css: "text-align: center; font-size: 36px; margin-bottom: 40px; color: #2d3748;"

      - ::
        type: "section-container"
        name: "Service Cards"
        css: "max-width: 1000px; margin: 0 auto; display: flex; gap: 20px; flex-wrap: wrap;"
        children::
          - ::
            type: "section-container"
            name: "Service 1"
            css: "flex: 1; min-width: 280px; background: white; padding: 30px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
            children::
              - ::
                type: "heading"
                content: "Web Development"
                css: "color: #667eea; font-size: 24px; margin-bottom: 15px;"
              - ::
                type: "text"
                content: "Build modern, responsive websites with the latest technologies"
                css: "color: #4a5568; line-height: 1.6;"

          - ::
            type: "section-container"
            name: "Service 2"
            css: "flex: 1; min-width: 280px; background: white; padding: 30px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
            children::
              - ::
                type: "heading"
                content: "Mobile Apps"
                css: "color: #667eea; font-size: 24px; margin-bottom: 15px;"
              - ::
                type: "text"
                content: "Create native mobile experiences for iOS and Android"
                css: "color: #4a5568; line-height: 1.6;"

          - ::
            type: "section-container"
            name: "Service 3"
            css: "flex: 1; min-width: 280px; background: white; padding: 30px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
            children::
              - ::
                type: "heading"
                content: "Consulting"
                css: "color: #667eea; font-size: 24px; margin-bottom: 15px;"
              - ::
                type: "text"
                content: "Expert guidance for your digital transformation journey"
                css: "color: #4a5568; line-height: 1.6;"

  # Contact page
  - ::
    id: "contact"
    name: "Contact"
    css: "background: #f7fafc; padding: 40px; min-height: 100vh;"
    children::
      - ::
        type: "nav-button"
        content: "← Back to Home"
        targetContainerId: "home"
        css: "background: #e2e8f0; color: #2d3748; padding: 10px 20px; border-radius: 6px; margin-bottom: 30px;"

      - ::
        type: "heading"
        content: "Contact Us"
        css: "text-align: center; font-size: 36px; margin-bottom: 40px; color: #2d3748;"

      # Form metadata
      - ::
        id: "form-contact"
        type: "form"
        eventName: "contact_inquiry"

      # Form container
      - ::
        type: "section-container"
        name: "Contact Form"
        css: "max-width: 600px; margin: 0 auto; background: white; padding: 40px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
        children::
          - ::
            type: "form-field-text"
            formId: "form-contact"
            fieldName: "name"
            label: "Your Name"
            placeholder: "John Doe"
            required: true
            css: "margin-bottom: 20px;"

          - ::
            type: "form-field-email"
            formId: "form-contact"
            fieldName: "email"
            label: "Email Address"
            placeholder: "you@example.com"
            required: true
            css: "margin-bottom: 20px;"

          - ::
            type: "form-field-textarea"
            formId: "form-contact"
            fieldName: "message"
            label: "Message"
            placeholder: "How can we help you?"
            required: true
            css: "margin-bottom: 30px;"

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
    css: "background: #f0fdf4; padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center; text-align: center;"
    children::
      - ::
        type: "heading"
        content: "✅ Message Sent!"
        css: "color: #16a34a; font-size: 48px; margin-bottom: 20px;"

      - ::
        type: "text"
        content: "Thank you for reaching out. We'll get back to you soon!"
        css: "color: #4a5568; font-size: 18px; margin-bottom: 30px;"

      - ::
        type: "nav-button"
        content: "← Back to Home"
        targetContainerId: "home"
        css: "background: #16a34a; color: white; padding: 12px 24px; border-radius: 8px;"
```

---

### Example 2: Blog with Comments

**Use case:** Blog with multiple posts and comment threads

```huml
# Blog Template
name: "Tech Blog"

screens::
  # Homepage - Post list
  - ::
    id: "home"
    name: "Blog Home"
    isEntryPoint: true
    css: "background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); padding: 40px; min-height: 100vh;"
    children::
      - ::
        type: "heading"
        content: "Tech Insights Blog"
        css: "color: white; font-size: 48px; text-align: center; margin-bottom: 40px;"

      # Post card 1
      - ::
        type: "section-container"
        name: "Post 1 Card"
        css: "max-width: 800px; margin: 0 auto 20px auto; background: white; padding: 30px; border-radius: 12px; box-shadow: 0 4px 6px rgba(0,0,0,0.1);"
        children::
          - ::
            type: "heading"
            content: "Getting Started with Svelte 5"
            css: "color: #2d3748; font-size: 24px; margin-bottom: 10px;"
          - ::
            type: "text"
            content: "Learn about Svelte 5's new Runes and how they improve reactivity..."
            css: "color: #4a5568; margin-bottom: 15px; line-height: 1.6;"
          - ::
            type: "text"
            content: "📅 January 15, 2025  •  5 min read"
            css: "color: #718096; font-size: 14px; margin-bottom: 15px;"
          - ::
            type: "nav-button"
            content: "Read Article →"
            targetContainerId: "post1"
            css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px; font-weight: 600;"

      # Post card 2
      - ::
        type: "section-container"
        name: "Post 2 Card"
        css: "max-width: 800px; margin: 0 auto; background: white; padding: 30px; border-radius: 12px; box-shadow: 0 4px 6px rgba(0,0,0,0.1);"
        children::
          - ::
            type: "heading"
            content: "Understanding CRDTs"
            css: "color: #2d3748; font-size: 24px; margin-bottom: 10px;"
          - ::
            type: "text"
            content: "Dive into Conflict-free Replicated Data Types for distributed systems..."
            css: "color: #4a5568; margin-bottom: 15px; line-height: 1.6;"
          - ::
            type: "text"
            content: "📅 January 18, 2025  •  8 min read"
            css: "color: #718096; font-size: 14px; margin-bottom: 15px;"
          - ::
            type: "nav-button"
            content: "Read Article →"
            targetContainerId: "post2"
            css: "background: #667eea; color: white; padding: 12px 24px; border-radius: 8px; font-weight: 600;"

  # Blog Post 1
  - ::
    id: "post1"
    name: "Post: Svelte 5"
    css: "background: #f7fafc; padding: 40px; min-height: 100vh;"
    children::
      - ::
        type: "nav-button"
        content: "← Back to Home"
        targetContainerId: "home"
        css: "background: #e2e8f0; color: #2d3748; padding: 10px 20px; border-radius: 6px; margin-bottom: 30px;"

      - ::
        type: "section-container"
        name: "Article"
        css: "max-width: 800px; margin: 0 auto; background: white; padding: 40px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
        children::
          - ::
            type: "heading"
            content: "Getting Started with Svelte 5"
            css: "color: #1a202c; font-size: 36px; margin-bottom: 10px;"

          - ::
            type: "text"
            content: "📅 January 15, 2025  •  By Sarah Chen"
            css: "color: #718096; margin-bottom: 30px;"

          - ::
            type: "markdown-text"
            content: "Svelte 5 introduces **Runes**, a revolutionary approach to reactivity.\n\n## What are Runes?\n\nRunes are special symbols:\n- `$state` - Reactive state\n- `$derived` - Computed values\n- `$effect` - Side effects\n\n## Example\n\n```svelte\nlet count = $state(0);\nlet doubled = $derived(count * 2);\n```\n\nTry them out today!"
            mode: "markdown"
            css: "line-height: 1.8; margin-bottom: 40px;"

          - ::
            type: "heading"
            content: "💬 Comments"
            css: "margin-top: 40px; padding-top: 40px; border-top: 2px solid #e2e8f0; margin-bottom: 20px;"

          - ::
            type: "thread"
            name: "Post 1 Comments"
            mode: "markdown"
            description: "Share your thoughts"
            css: "min-height: 400px;"

  # Blog Post 2
  - ::
    id: "post2"
    name: "Post: CRDTs"
    css: "background: #f7fafc; padding: 40px; min-height: 100vh;"
    children::
      - ::
        type: "nav-button"
        content: "← Back to Home"
        targetContainerId: "home"
        css: "background: #e2e8f0; color: #2d3748; padding: 10px 20px; border-radius: 6px; margin-bottom: 30px;"

      - ::
        type: "section-container"
        name: "Article"
        css: "max-width: 800px; margin: 0 auto; background: white; padding: 40px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
        children::
          - ::
            type: "heading"
            content: "Understanding CRDTs"
            css: "color: #1a202c; font-size: 36px; margin-bottom: 10px;"

          - ::
            type: "text"
            content: "📅 January 18, 2025  •  By Alex Rivera"
            css: "color: #718096; margin-bottom: 30px;"

          - ::
            type: "markdown-text"
            content: "**CRDTs** (Conflict-free Replicated Data Types) enable distributed systems to work seamlessly.\n\n## Key Benefits\n\n- Automatic conflict resolution\n- Offline support\n- No central authority needed\n\n## Use Cases\n\n1. Collaborative editors\n2. Distributed databases\n3. Real-time multiplayer games\n\nLearn more in our detailed guide!"
            mode: "markdown"
            css: "line-height: 1.8; margin-bottom: 40px;"

          - ::
            type: "heading"
            content: "💬 Comments"
            css: "margin-top: 40px; padding-top: 40px; border-top: 2px solid #e2e8f0; margin-bottom: 20px;"

          - ::
            type: "thread"
            name: "Post 2 Comments"
            mode: "markdown"
            description: "Join the discussion"
            css: "min-height: 400px;"
```

---

### Example 3: Survey with Branching

**Use case:** Customer survey with different paths based on responses

```huml
# Customer Survey
name: "Customer Survey"

screens::
  # Welcome
  - ::
    id: "welcome"
    name: "Welcome"
    isEntryPoint: true
    css: "background: linear-gradient(135deg, #a6e3a1 0%, #89b4fa 100%); padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center; text-align: center;"
    children::
      - ::
        type: "heading"
        content: "Customer Satisfaction Survey"
        css: "color: white; font-size: 48px; margin-bottom: 20px;"

      - ::
        type: "text"
        content: "Help us improve by sharing your feedback"
        css: "color: rgba(255,255,255,0.9); font-size: 20px; margin-bottom: 40px;"

      - ::
        type: "nav-button"
        content: "Start Survey"
        targetContainerId: "q1"
        css: "background: white; color: #667eea; padding: 15px 40px; border-radius: 8px; font-size: 18px; font-weight: 600;"

  # Question 1: Satisfaction
  - ::
    id: "q1"
    name: "Question 1"
    css: "background: #f7fafc; padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center;"
    children::
      # Form for tracking
      - ::
        id: "form-survey"
        type: "form"
        eventName: "customer_survey"

      - ::
        type: "heading"
        content: "Are you satisfied with our service?"
        css: "text-align: center; font-size: 32px; margin-bottom: 40px; color: #2d3748;"

      # Yes option (MODE 3)
      - ::
        type: "nav-button"
        content: "😊 Yes, very satisfied"
        formId: "form-survey"
        fieldName: "satisfaction"
        value: "satisfied"
        targetContainerId: "q2-satisfied"
        css: "background: #a6e3a1; color: #1e1e2e; padding: 20px 40px; border-radius: 8px; font-size: 18px; font-weight: 600; margin: 10px; min-width: 300px;"

      # Neutral option (MODE 3)
      - ::
        type: "nav-button"
        content: "😐 It's okay"
        formId: "form-survey"
        fieldName: "satisfaction"
        value: "neutral"
        targetContainerId: "q2-neutral"
        css: "background: #f9e2af; color: #1e1e2e; padding: 20px 40px; border-radius: 8px; font-size: 18px; font-weight: 600; margin: 10px; min-width: 300px;"

      # No option (MODE 3)
      - ::
        type: "nav-button"
        content: "😞 No, needs improvement"
        formId: "form-survey"
        fieldName: "satisfaction"
        value: "unsatisfied"
        targetContainerId: "q2-unsatisfied"
        css: "background: #f38ba8; color: white; padding: 20px 40px; border-radius: 8px; font-size: 18px; font-weight: 600; margin: 10px; min-width: 300px;"

  # Q2 for satisfied customers
  - ::
    id: "q2-satisfied"
    name: "Q2 Satisfied"
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

  # Q2 for neutral customers
  - ::
    id: "q2-neutral"
    name: "Q2 Neutral"
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

  # Q2 for unsatisfied customers
  - ::
    id: "q2-unsatisfied"
    name: "Q2 Unsatisfied"
    css: "background: #fef2f2; padding: 40px; min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center;"
    children::
      - ::
        type: "heading"
        content: "We're sorry to hear that. What went wrong?"
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
        css: "color: white; font-size: 48px; margin-bottom: 20px;"

      - ::
        type: "text"
        content: "Your feedback has been recorded. We appreciate you taking the time to help us improve!"
        css: "color: rgba(255,255,255,0.9); font-size: 20px; max-width: 600px;"
```

---

## Best Practices

### 1. Screen Organization

**✅ DO:**
- Use descriptive screen IDs (`screen-home`, `blog-post-1`)
- Set one clear entry point
- Group related screens together in the file
- Use consistent naming conventions

**❌ DON'T:**
- Use generic IDs (`s1`, `page`, `screen`)
- Have multiple entry points
- Create unreachable screens (no navigation to them)

### 2. Content Structure

**✅ DO:**
- Use section-containers to group related blocks
- Apply consistent spacing with margins/padding
- Use appropriate block types (heading for titles, markdown-text for rich content)
- Keep content hierarchy clear (h2, h3 in markdown)

**❌ DON'T:**
- Put everything in one flat list
- Use heading blocks for regular text
- Nest containers more than 3-4 levels deep

### 3. Forms

**✅ DO:**
- Always define form metadata first
- Use descriptive `fieldName` values (`email`, `full_name`)
- Set `required: true` for essential fields
- Provide clear labels and placeholders
- Create a thank-you/success screen

**❌ DON'T:**
- Forget the form metadata block
- Use generic field names (`field1`, `input`)
- Skip labels on form fields
- Submit without showing confirmation

### 4. Navigation

**✅ DO:**
- Provide "back" buttons on detail screens
- Use consistent button styling
- Test all navigation paths
- Ensure no dead ends (screens with no way out)

**❌ DON'T:**
- Create circular navigation loops without escape
- Use unclear button text ("Click here", "Go")
- Forget to set targetContainerId

### 5. Styling

**✅ DO:**
- Use consistent color palette
- Apply responsive max-widths (800px, 1000px)
- Use proper spacing (20px, 40px increments)
- Test gradient backgrounds for readability
- Use semantic colors (green for success, red for errors)

**❌ DON'T:**
- Mix too many colors (stick to 2-3 primary colors)
- Use tiny font sizes (<14px)
- Forget padding/margins (makes content cramped)
- Use pure black (#000) - use dark grays instead (#1a202c, #2d3748)

### 6. Thread Comments

**✅ DO:**
- Set `mode: "markdown"` for rich text support
- Provide descriptive placeholders
- Use min-height to reserve space
- Place threads at logical endpoints (end of articles)

**❌ DON'T:**
- Create multiple threads for the same discussion
- Forget to test comment persistence (reload after adding comments)

---

## Common Patterns

### Pattern: Landing Page with CTA

```huml
- ::
  id: "landing"
  name: "Landing Page"
  isEntryPoint: true
  css: "min-height: 100vh; display: flex; flex-direction: column; justify-content: center; align-items: center; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); text-align: center; padding: 40px;"
  children::
    - ::
      type: "heading"
      content: "Transform Your Business"
      css: "color: white; font-size: 56px; margin-bottom: 20px; font-weight: 800;"

    - ::
      type: "text"
      content: "Get started today with our powerful platform"
      css: "color: rgba(255,255,255,0.9); font-size: 24px; margin-bottom: 40px;"

    - ::
      type: "nav-button"
      content: "Get Started →"
      targetContainerId: "signup"
      css: "background: white; color: #667eea; padding: 20px 50px; border-radius: 12px; font-size: 20px; font-weight: 700; box-shadow: 0 4px 12px rgba(0,0,0,0.2);"
```

### Pattern: Card Grid

```huml
- ::
  type: "section-container"
  name: "Card Grid"
  css: "display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px; max-width: 1200px; margin: 0 auto;"
  children::
    - ::
      type: "section-container"
      css: "background: white; padding: 30px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
      children::
        - ::
          type: "heading"
          content: "Card 1"
        - ::
          type: "text"
          content: "Content here"

    - ::
      type: "section-container"
      css: "background: white; padding: 30px; border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);"
      children::
        - ::
          type: "heading"
          content: "Card 2"
        - ::
          type: "text"
          content: "Content here"
```

### Pattern: FAQ with Expandable Sections

```huml
- ::
  type: "section-container"
  name: "FAQ Container"
  css: "max-width: 800px; margin: 0 auto;"
  children::
    - ::
      type: "heading"
      content: "Frequently Asked Questions"
      css: "text-align: center; font-size: 36px; margin-bottom: 40px;"

    - ::
      type: "section-container"
      name: "FAQ Item"
      css: "background: white; padding: 20px; border-radius: 8px; margin-bottom: 15px; border: 1px solid #e2e8f0;"
      children::
        - ::
          type: "heading"
          content: "Q: How does it work?"
          css: "font-size: 20px; color: #2d3748; margin-bottom: 10px;"
        - ::
          type: "text"
          content: "A: It's simple! Just follow these steps..."
          css: "color: #4a5568; line-height: 1.6;"
```

### Pattern: Multi-step Wizard

```huml
screens::
  # Step 1
  - ::
    id: "step1"
    isEntryPoint: true
    children::
      - ::
        type: "text"
        content: "Step 1 of 3"
        css: "color: #718096; margin-bottom: 10px;"
      - ::
        type: "heading"
        content: "Personal Information"
      # ... form fields
      - ::
        type: "nav-button"
        content: "Next →"
        targetContainerId: "step2"

  # Step 2
  - ::
    id: "step2"
    children::
      - ::
        type: "text"
        content: "Step 2 of 3"
        css: "color: #718096; margin-bottom: 10px;"
      - ::
        type: "heading"
        content: "Preferences"
      # ... form fields
      - ::
        type: "nav-button"
        content: "Next →"
        targetContainerId: "step3"

  # Step 3
  - ::
    id: "step3"
    children::
      - ::
        type: "text"
        content: "Step 3 of 3"
        css: "color: #718096; margin-bottom: 10px;"
      - ::
        type: "heading"
        content: "Review & Submit"
      # ... review + submit
```

---

## Reference: Implementation Files

### Key Files to Reference

**Template Importer:**
- `src/utils/templateImporter.ts` - Parses HUML into Yjs blocks
- Lines 11-51: TemplateBlock interface (all properties)
- Lines 176-283: importBlocks() - How blocks are created
- Lines 288-306: parseCSSToStyles() - CSS parsing

**Blocksuite Coordinator:**
- `src/lib/blocksuiteCoordinator.ts` - Manages Yjs documents
- Lines 230-299: detectResourceType() and checkForThreadOrFormBlocks()
- Shows how thread and form detection works

**Nav Button Component:**
- `src/lib/blocks/NavButton.svelte` - Navigation button implementation
- Lines 47-95: handleClick() - MODE routing
- Lines 98-144: MODE 3 (branching)
- Lines 147-225: MODE 2 (form submission)

**Form Handling:**
- `src/lib/submissionsStore.ts` - Form submission storage
- Shows how submissions are stored and retrieved

**Thread Handling:**
- `src/lib/threadCommentsStore.ts` - Thread comments storage
- Shows how comments are stored separately

---

## Conclusion

This guide covers all aspects of creating HUML templates. Key takeaways:

1. **HUML uses YAML-like syntax** with `::` for lists
2. **Screens are top-level containers** - at least one required
3. **Blocks are the building elements** - each has a specific type
4. **Nav-button has 3 modes** - navigation, form submission, branching
5. **Forms require metadata + fields + submit button**
6. **Threads enable comments** stored in separate document
7. **CSS is inline** - use for custom styling
8. **Everything is declarative** - describe what you want, not how

### Quick Checklist for Creating Templates

- [ ] Set template `name`
- [ ] Create at least one screen
- [ ] Mark one screen with `isEntryPoint: true`
- [ ] Add blocks to each screen (headings, text, buttons, etc.)
- [ ] Set up navigation between screens
- [ ] If using forms: create form metadata, add fields, add submit button
- [ ] If using threads: add thread blocks where needed
- [ ] Apply CSS styling for appearance
- [ ] Test all navigation paths
- [ ] Verify forms submit correctly
- [ ] Check thread comments persist after reload

**Remember:** Every HUML template should tell a story - guide users through a logical flow from entry point to completion. Happy templating!
