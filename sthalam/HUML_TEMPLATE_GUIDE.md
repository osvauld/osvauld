# HUML Template Guide - Complete Reference

**Version:** 2.0 (Complete Rewrite)
**Last Updated:** January 2025
**Purpose:** Create complete web applications using HUML declarative syntax

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

---
 ### ⚠️ CRITICAL: Property Names Are Case-Sensitive!
 **Property names ** MUST use exact camelCase spelling. 
  Lowercase will NOT work!**
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

**Properties:**
- `type: "section-container"` (required)
- `name` (optional) - Display name for editor
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

---

### Block Type: `nav-button`

**Purpose:** The universal button for ALL interactions

**This is the ONLY button type you need.** It has three modes:

1. **MODE 1:** Simple navigation (go to another screen)
2. **MODE 2:** Form submission (submit form + navigate)
3. **MODE 3:** Branching choices (set value + submit + navigate)

#### Common Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `type` | String | ✅ Yes | Must be `"nav-button"` |
| `content` | String | ✅ Yes | Button text |
| `targetContainerId` | String | ✅ Yes | Screen ID to navigate to |
| `formId` | String | For MODE 2 & 3 | Form to submit |
| `fieldName` | String | For MODE 3 only | Field name to set |
| `value` | Any | For MODE 3 only | Value to set |
| `action` | String | ❌ No | Default: `"navigate"` |
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

**Required:**
- `type: "nav-button"`
- `content` - Button text (e.g., "Submit", "Send")
- `formId` - ID of the form to submit
- `targetContainerId` - Where to go after submission

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

#### MODE 3: Branching Choices

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

#### nav-button MODE Comparison

| Feature | MODE 1 | MODE 2 | MODE 3 |
|---------|--------|--------|--------|
| Navigate | ✅ | ✅ | ✅ |
| Submit form | ❌ | ✅ | ✅ |
| Set field value | ❌ | ❌ | ✅ |
| Requires `formId` | ❌ | ✅ | ✅ |
| Requires `fieldName` | ❌ | ❌ | ✅ |
| Requires `value` | ❌ | ❌ | ✅ |
| Use case | Simple navigation | Form submission | Branching choices |

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

**Properties:**
- `type: "thread"` (required)
- `name` (optional) - Display name for this thread
- `mode: "markdown"` (optional) - Allows markdown in comments
- `description` (optional) - Placeholder text
- `css` (optional) - Thread container styling

**How threads work:**
1. Users can add comments in both builder and viewer modes
2. Comments are stored separately (in `thread_comments_doc`)
3. Comments persist after reload
4. Each thread is independent

**Example:**
```huml
- ::
  type: "thread"
  name: "Article Comments"
  mode: "markdown"
  description: "Share your thoughts about this article"
  css: "background: white; border-radius: 8px; padding: 20px; min-height: 400px;"
```

**Example: Blog with Comments**
```huml
# Blog post content
- ::
  type: "markdown-text"
  content: "## My Blog Post\n\nThis is the article content..."

# Comments section header
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

**When to use:**
- Blog post comments
- Discussion forums
- Q&A sections
- Feedback areas

---

## Forms - Complete Guide

### Understanding Forms

Forms in HUML require **THREE components** that work together:

1. **Form metadata block** (`type: "form"`) - Defines the form
2. **Form field blocks** (various `form-field-*` types) - The input fields
3. **Submit button** (`nav-button` with `formId`) - Submits the form

All three must be present and properly connected for forms to work.

---

### Component 1: Form Metadata Block

This is a special block that defines your form. It MUST come before any form fields.

**Properties:**
- `id` (required) - Unique form identifier
- `type: "form"` (required)
- `name` (optional) - Display name for editor
- `eventName` (optional) - Groups submissions (default: "form_submission")

**Example:**
```huml
- ::
  id: "form-contact"
  type: "form"
  name: "Contact Form"
  eventName: "contact_submission"
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

- [ ] Form metadata block exists with `id` and `type: "form"`
- [ ] All form fields have `formId` matching the form's `id`
- [ ] All form fields have unique `fieldName` values
- [ ] Submit button is `type: "nav-button"`
- [ ] Submit button has `formId` property
- [ ] Submit button has `targetContainerId` property
- [ ] Target success screen exists with matching `id`
- [ ] Required fields are marked with `required: true`

**If all checked:** Your form will work! ✅

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
- Apply consistent spacing with CSS
- Use appropriate block types (heading for titles, markdown for rich content)

**❌ DON'T:**
- Put everything in a flat list
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

**❌ DON'T:**
- Mix too many colors
- Use tiny font sizes (<14px)
- Forget padding/margins
- Use pure black (#000) - use dark grays instead

---

## Quick Reference

### All Block Types

| Type | Purpose |
|------|---------|
| `heading` | Large title text |
| `text` | Regular paragraph text |
| `markdown-text` | Rich text with markdown |
| `section-container` | Group blocks together |
| `nav-button` | Universal button (all modes) |
| `thread` | Comments/discussion |
| `form` | Form metadata (invisible) |
| `form-field-text` | Single-line text input |
| `form-field-email` | Email input |
| `form-field-textarea` | Multi-line text |
| `form-field-checkbox` | Checkbox |
| `form-field-number` | Number input |
| `form-field-password` | Password input |
| `image` | Image display |

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
