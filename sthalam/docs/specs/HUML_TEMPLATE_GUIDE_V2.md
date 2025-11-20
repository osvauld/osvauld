# HUML Template Guide v2.0

**Version**: 2.0.0
**Date**: 2025-11-20
**Target Audience**: LLMs and developers creating Sthalam templates

---

## Table of Contents

1. [Introduction](#introduction)
2. [Document Architecture](#document-architecture)
3. [Template Structure](#template-structure)
4. [Types Section](#types-section)
5. [Documents Section](#documents-section)
6. [Computed Section](#computed-section)
7. [UI Section](#ui-section)
8. [Block Types Reference](#block-types-reference)
9. [Actions Reference](#actions-reference)
10. [CEL Expression Syntax](#cel-expression-syntax)
11. [Best Practices](#best-practices)
12. [Complete Example](#complete-example)

---

## Introduction

### What is HUML?

HUML (Human-Usable Markup Language) is a YAML-like declarative language for building collaborative, reactive web applications in Sthalam. Templates are:

- **Template-driven**: All document routing and permissions come from the template
- **Reactive**: Computed values automatically update using OCaml React FRP
- **Collaborative**: State syncs in real-time via Loro CRDTs
- **Type-safe**: Custom types with nested structures
- **Multi-mode**: Separate UIs for publishers (creators) and viewers (consumers)

### Architecture Philosophy

```
Template (HUML source)
    ↓
Defines which UCAN document each field belongs to
    ↓
Frontend automatically subscribes to those documents
    ↓
Changes sync via Loro CRDTs
    ↓
Reactive system updates computed values
    ↓
UI re-renders automatically
```

**Key Principle**: The template is the single source of truth. No hardcoded document names in application code.

---

## Document Architecture

### Available UCAN Documents

Sthalam currently provides **6 UCAN documents**. You **MUST** only use these document names:

| Document Name | Purpose | Who Can Edit | Syncs? | Use For |
|---------------|---------|--------------|--------|---------|
| `content_doc` | Publisher-created content | Publisher only | ✅ Yes | Posts, articles, main content |
| `collaborative_doc` | Shared collaborative state | Publisher + Viewers | ✅ Yes | Comments, votes, shared data |
| `user_content_doc` | Per-user private content | User only | ✅ Yes | Form inputs, drafts, UI state |
| `submissions_doc` | User submissions | Viewers only | ✅ Yes | Form submissions, responses |
| `static_assets` | Static assets (images, videos) | Publisher only | ✅ Yes | Images, videos, audio, files |
| `template_doc` | Template source (HUML) | Publisher only | ✅ Yes | DO NOT USE in templates |

**IMPORTANT**:
- `template_doc` is managed by the system - do not use it in your templates
- Always specify `document:` for each field in the `documents::` section
- Choose the document based on who should edit and whether it should sync

### Document Selection Guide

**Use `content_doc` when**:
- Only the publisher should create/edit
- Example: Blog posts, product listings, course content

**Use `collaborative_doc` when**:
- Both publishers and viewers should edit
- Example: Comments, reactions, collaborative notes, chat messages

**Use `user_content_doc` when**:
- Each user has their own private version (syncs per-user)
- Example: Form inputs, personal drafts, bookmarks, reading progress, UI state

**Use `submissions_doc` when**:
- Viewers submit data for publisher review
- Example: Form responses, survey answers, applications

**Use `static_assets` when**:
- Storing media files and static assets
- Example: Images, videos, audio files, PDFs, documents

---

## Template Structure

### Top-Level Sections

Every HUML template has this structure:

```yaml
name: "Template Name"
version: "v1.0.0"

types::
  # Custom type definitions (optional)

documents::
  # Field definitions with document mappings (required)

computed::
  # Reactive computed values (optional)

ui::
  publisher::
    # UI for publishers (creators)

  viewer::
    # UI for viewers (consumers)
```

### Section Order

**Always use this order**:
1. `name` and `version` (top-level fields)
2. `types::` (if you have custom types)
3. `documents::` (required - defines all state)
4. `computed::` (if you have computed values)
5. `ui::`
   - `publisher::` (optional)
   - `viewer::` (optional)

---

## Types Section

### Purpose

Define reusable custom types for complex data structures.

### Syntax

```yaml
types::
  TypeName::
    fieldName::
      type: "string" | "number" | "boolean" | "array"
      items: "OtherTypeName"  # Only for arrays
```

### Primitive Types

- `"string"` - Text values
- `"number"` - Integers or floats
- `"boolean"` - true/false

### Example: Simple Type

```yaml
types::
  User::
    name::
      type: "string"
    email::
      type: "string"
    age::
      type: "number"
```

### Example: Nested Types

```yaml
types::
  Post::
    id::
      type: "string"
    title::
      type: "string"
    author::
      type: "string"
    timestamp::
      type: "number"
    upvotes::
      type: "number"

  Comment::
    id::
      type: "string"
    postId::
      type: "string"        # Reference to Post.id
    text::
      type: "string"
    author::
      type: "string"
    upvotes::
      type: "number"
    parentCommentId::
      type: "string"        # For nested comments
```

### Best Practices for Types

1. **Use references, not nesting**: Instead of `Post.comments: Comment[]`, use `Comment.postId: string`
2. **Always include `id` field** for items in arrays
3. **Use descriptive names**: `postId` not just `post`
4. **Keep types flat** when possible - deep nesting makes queries complex

---

## Documents Section

### Purpose

Define all application state fields and map them to UCAN documents.

### Syntax

```yaml
documents::
  fieldName::
    type: "string" | "number" | "boolean" | "array"
    items: "TypeName"           # Required for arrays
    initial: <default value>    # Required
    document: "document_name"   # Required - must be one of the 6 documents
```

### Field Types

**Primitives**:
```yaml
counter::
  type: "number"
  initial: 0
  document: "ui_state_doc"

username::
  type: "string"
  initial: "Guest"
  document: "collaborative_doc"

isPublished::
  type: "boolean"
  initial: false
  document: "content_doc"
```

**Arrays** (must reference a type from `types::`):
```yaml
posts::
  type: "array"
  items: "Post"               # Must match a type from types::
  initial:: []                # Empty array
  document: "content_doc"

comments::
  type: "array"
  items: "Comment"
  initial:: []
  document: "collaborative_doc"
```

### Initial Values

**Primitives** - use simple syntax:
```yaml
counter::
  type: "number"
  initial: 42              # Simple number
  document: "ui_state_doc"
```

**Arrays and complex values** - use `::` syntax:
```yaml
posts::
  type: "array"
  items: "Post"
  initial:: []             # Use :: for arrays
  document: "content_doc"

settings::
  type: "object"
  initial::                # Use :: for objects
    theme: "dark"
    fontSize: 14
  document: "ui_state_doc"
```

### Example: Complete documents:: Section

```yaml
documents::
  # Publisher-only content (posts)
  posts::
    type: "array"
    items: "Post"
    initial:: []
    document: "content_doc"

  # Collaborative content (comments)
  comments::
    type: "array"
    items: "Comment"
    initial:: []
    document: "collaborative_doc"

  # Shared user state
  currentUser::
    type: "string"
    initial: "Guest"
    document: "collaborative_doc"

  # UI-only state (form inputs)
  newPostTitle::
    type: "string"
    initial: ""
    document: "ui_state_doc"

  newPostContent::
    type: "string"
    initial: ""
    document: "ui_state_doc"

  newCommentText::
    type: "string"
    initial: ""
    document: "ui_state_doc"
```

---

## Computed Section

### Purpose

Define reactive computed values using CEL (Common Expression Language). These automatically update when their dependencies change.

### Syntax

```yaml
computed::
  computedName::
    expr: "${ CEL expression }"
    deps::
      - "dependency1"
      - "dependency2"
```

### Available CEL Functions

**Array functions**:
- `size(array)` - Length of array
- `array.filter(item => condition)` - Filter array
- `array.map(item => expression)` - Transform array
- `array.exists(item => condition)` - Check if any item matches
- `array.all(item => condition)` - Check if all items match

**String functions**:
- `length(string)` - String length
- `contains(string, substring)` - Check substring
- `startsWith(string, prefix)` - Check prefix
- `endsWith(string, suffix)` - Check suffix

**Math functions**:
- `sin(x)`, `cos(x)`, `tan(x)` - Trigonometry
- `sqrt(x)`, `pow(base, exp)` - Power functions
- `min(a, b)`, `max(a, b)` - Min/max
- `abs(x)` - Absolute value
- `floor(x)`, `ceil(x)`, `round(x)` - Rounding

**Operators**:
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Logical: `&&`, `||`, `!`
- Ternary: `condition ? valueIfTrue : valueIfFalse`

### Examples

**Simple computed value**:
```yaml
computed::
  totalPosts::
    expr: "${ size(posts) }"
    deps::
      - "posts"
```

**With filtering**:
```yaml
computed::
  publishedPosts::
    expr: "${ posts.filter(p => p.isPublished) }"
    deps::
      - "posts"

  publishedCount::
    expr: "${ size(publishedPosts) }"
    deps::
      - "publishedPosts"
```

**Complex expression**:
```yaml
computed::
  postCommentCounts::
    expr: "${ posts.map(p => size(comments.filter(c => c.postId == p.id))) }"
    deps::
      - "posts"
      - "comments"
```

### Best Practices for Computed

1. **Keep expressions simple** - Complex logic should be split into multiple computed values
2. **Always list all dependencies** - Missing deps = stale values
3. **Use `${ }` syntax** - Required for all CEL expressions
4. **Computed values can depend on other computed values** - They resolve in multiple passes

---

## UI Section

### Purpose

Define the user interface screens for publishers and viewers.

### Structure

```yaml
ui::
  publisher::
    - ::
      type: "screen"
      id: "unique_id"
      name: "Screen Name"
      isEntryPoint: true | false
      css: "CSS styling"
      blocks::
        # Array of UI blocks

  viewer::
    - ::
      type: "screen"
      # Same structure as publisher
```

### Screen Properties

| Property | Required | Type | Description |
|----------|----------|------|-------------|
| `type` | ✅ Yes | `"screen"` | Must be "screen" |
| `id` | ✅ Yes | `string` | Unique identifier for navigation |
| `name` | ✅ Yes | `string` | Display name |
| `isEntryPoint` | ❌ No | `boolean` | First screen to show (default: false) |
| `css` | ❌ No | `string` | CSS styling for screen container |
| `blocks` | ✅ Yes | `array` | Array of UI blocks |

### Multi-Screen Navigation

```yaml
ui::
  publisher::
    - ::
      type: "screen"
      id: "home"
      isEntryPoint: true
      blocks::
        - ::
          type: "button"
          content: "Go to Editor"
          action: "navigate"
          targetScreen: "editor"

    - ::
      type: "screen"
      id: "editor"
      blocks::
        - ::
          type: "button"
          content: "Back to Home"
          action: "navigate"
          targetScreen: "home"
```

---

## Block Types Reference

### Conditional Rendering

**All blocks** support the `when` attribute for conditional rendering:

```yaml
- ::
  type: "text"
  content: "This only shows when counter is positive"
  when: "${ counter > 0 }"
  css: "color: green;"
```

**How it works**:
- `when` accepts a CEL expression wrapped in `${ }`
- Block only renders when expression evaluates to `true`
- Works with **all block types** (text, button, container, etc.)
- Use for showing/hiding validation messages, status indicators, etc.

**Examples**:
```yaml
# Show message when field has value
when: "${ size(username) > 0 }"

# Show when email is valid
when: "${ contains(email, '@') }"

# Show when email is invalid AND has been entered
when: "${ !isValidEmail && size(email) > 0 }"

# Show when form can be submitted
when: "${ hasUsername && isValidEmail && agreeToTerms }"

# Show when NOT ready
when: "${ !canSubmit }"
```

---

### Container Blocks

#### `container`
Generic container for grouping blocks.

```yaml
- ::
  type: "container"
  css: "padding: 1rem; background: white;"
  blocks::
    # Nested blocks
```

#### `section`
Semantic section (same as container but semantic HTML `<section>`).

```yaml
- ::
  type: "section"
  css: "margin-bottom: 2rem;"
  blocks::
    # Nested blocks
```

### Text Blocks

#### `heading`
Heading text (h1-h6).

```yaml
- ::
  type: "heading"
  level: 1 | 2 | 3 | 4 | 5 | 6
  content: "Static text or {{interpolation}}"
  css: "color: #333; margin-bottom: 1rem;"
```

#### `text`
Paragraph text.

```yaml
- ::
  type: "text"
  content: "Static text or {{variable}} or {{computed}}"
  css: "color: #666; line-height: 1.6;"
```

### Input Blocks

#### `input`
Text input field.

```yaml
- ::
  type: "input"
  name: "fieldName"              # Maps to documents:: field
  placeholder: "Enter text..."
  css: "width: 100%; padding: 0.5rem;"
```

#### `textarea`
Multi-line text input.

```yaml
- ::
  type: "textarea"
  name: "fieldName"
  placeholder: "Enter text..."
  css: "width: 100%; min-height: 100px;"
```

### Interactive Blocks

#### `button`
Clickable button.

```yaml
- ::
  type: "button"
  content: "Button Text"
  action: "setState" | "navigate"
  stateUpdates::                 # For setState action
    field1: "${ expression }"
    field2: "static value"
  targetScreen: "screenId"       # For navigate action
  css: "padding: 0.5rem 1rem; background: #007bff; color: white;"
```

### Loop Block

#### `loop`
Iterate over arrays.

```yaml
- ::
  type: "loop"
  each: "${ posts }"             # CEL expression returning array
  as: "post"                     # Variable name for each item
  key: "id"                      # Field to use as React key
  blocks::
    - ::
      type: "text"
      content: "{{post.title}}"  # Access loop variable
```

**Nested loops** (for threaded comments, etc.):
```yaml
- ::
  type: "loop"
  each: "${ posts }"
  as: "post"
  key: "id"
  blocks::
    - ::
      type: "heading"
      level: 3
      content: "{{post.title}}"

    - ::
      type: "loop"
      each: "${ comments.filter(c => c.postId == post.id) }"
      as: "comment"
      key: "id"
      blocks::
        - ::
          type: "text"
          content: "{{comment.text}}"
```

---

## Actions Reference

### `setState` Action

Update state fields (usually via button clicks).

```yaml
- ::
  type: "button"
  content: "Add Post"
  action: "setState"
  stateUpdates::
    posts: "${ posts + [{\"id\": generateId(), \"title\": newPostTitle, \"content\": newPostContent, \"author\": currentUser, \"timestamp\": now(), \"upvotes\": 0}] }"
    newPostTitle: ""
    newPostContent: ""
```

**Available functions in setState expressions**:
- `generateId()` - Generate unique ID
- `now()` - Current timestamp (milliseconds)

**Array operations**:
- `array + [newItem]` - Append to array
- `array.filter(item => condition)` - Remove items

### `navigate` Action

Navigate to different screen.

```yaml
- ::
  type: "button"
  content: "Go to Settings"
  action: "navigate"
  targetScreen: "settings"
```

---

## CEL Expression Syntax

### Two Syntaxes

**1. Pure CEL expressions** (in `expr`, `each`, `stateUpdates`):
```yaml
expr: "${ size(posts) }"
each: "${ posts.filter(p => p.isPublished) }"
```

**2. String interpolation** (in `content`):
```yaml
content: "Total: {{totalPosts}}"
content: "Posted by {{post.author}} • {{post.upvotes}} upvotes"
```

### Escaping in setState

When creating objects in `stateUpdates`, you must **escape quotes**:

```yaml
stateUpdates::
  posts: "${ posts + [{\"id\": generateId(), \"title\": newPostTitle}] }"
  #                     ^^^^                  ^^^^^^^
  #                     Escaped quotes required!
```

### Accessing Fields

**Top-level fields**:
```yaml
"{{counter}}"
"${ counter + 1 }"
```

**Object fields**:
```yaml
"{{user.name}}"
"${ user.age > 18 }"
```

**Loop variables**:
```yaml
each: "${ posts }"
as: "post"
# Then access:
"{{post.title}}"
"{{post.author}}"
```

**Nested loop variables**:
```yaml
# Outer loop
each: "${ posts }"
as: "post"
blocks::
  # Inner loop
  - ::
    type: "loop"
    each: "${ comments.filter(c => c.postId == post.id) }"
    #                                         ^^^^^^^^ outer variable
    as: "comment"
    blocks::
      - type: "text"
        content: "{{comment.text}} on {{post.title}}"
        #         ^^^^^^^^^^^^^^^    ^^^^^^^^^^^
        #         Inner variable     Outer variable
```

---

## Best Practices

### 1. Document Selection

✅ **DO**:
```yaml
posts::
  type: "array"
  items: "Post"
  initial:: []
  document: "content_doc"      # Publisher-only

comments::
  type: "array"
  items: "Comment"
  initial:: []
  document: "collaborative_doc" # Viewers can add
```

❌ **DON'T**:
```yaml
posts::
  type: "array"
  items: "Post"
  initial:: []
  # Missing document! Won't work in v2.0
```

### 2. Type References

✅ **DO** (separate arrays, use references):
```yaml
types::
  Post::
    id:: { type: "string" }
    title:: { type: "string" }

  Comment::
    id:: { type: "string" }
    postId:: { type: "string" }    # Reference!

documents::
  posts::
    type: "array"
    items: "Post"
    initial:: []
    document: "content_doc"

  comments::
    type: "array"
    items: "Comment"
    initial:: []
    document: "collaborative_doc"
```

❌ **DON'T** (nested arrays):
```yaml
types::
  Post::
    id:: { type: "string" }
    comments::                    # Don't nest!
      type: "array"
      items: "Comment"
```

### 3. Computed Dependencies

✅ **DO**:
```yaml
computed::
  totalPosts::
    expr: "${ size(posts) }"
    deps::
      - "posts"                  # List all deps!
```

❌ **DON'T**:
```yaml
computed::
  totalPosts::
    expr: "${ size(posts) }"
    # Missing deps! Value won't update!
```

### 4. Loop Keys

✅ **DO**:
```yaml
- ::
  type: "loop"
  each: "${ posts }"
  as: "post"
  key: "id"                      # Always use unique ID
```

❌ **DON'T**:
```yaml
- ::
  type: "loop"
  each: "${ posts }"
  as: "post"
  # Missing key! Performance issues!
```

### 5. CSS Styling

✅ **DO** (inline CSS in template):
```yaml
- ::
  type: "container"
  css: "padding: 1.5rem; background: white; border-radius: 8px;"
```

❌ **DON'T** (referencing external CSS classes):
```yaml
- ::
  type: "container"
  css: "my-custom-class"         # Won't work - no external CSS!
```

### 6. Initial Values

✅ **DO**:
```yaml
counter::
  type: "number"
  initial: 0                     # Simple value

posts::
  type: "array"
  items: "Post"
  initial:: []                   # Use :: for arrays
```

❌ **DON'T**:
```yaml
posts::
  type: "array"
  items: "Post"
  initial: []                    # Wrong syntax!
```

---

## Complete Example

Here's the full `reddit-comments-demo.huml` showing all concepts:

```yaml
name: "Reddit Comments Demo"
version: "v2.0.0"

types::
  Post::
    id::
      type: "string"
    title::
      type: "string"
    content::
      type: "string"
    author::
      type: "string"
    timestamp::
      type: "number"
    upvotes::
      type: "number"

  Comment::
    id::
      type: "string"
    postId::
      type: "string"
    text::
      type: "string"
    author::
      type: "string"
    timestamp::
      type: "number"
    upvotes::
      type: "number"
    parentCommentId::
      type: "string"

documents::
  posts::
    type: "array"
    items: "Post"
    initial:: []
    document: "content_doc"

  comments::
    type: "array"
    items: "Comment"
    initial:: []
    document: "collaborative_doc"

  currentUser::
    type: "string"
    initial: "Guest"
    document: "collaborative_doc"

  newPostTitle::
    type: "string"
    initial: ""
    document: "ui_state_doc"

  newPostContent::
    type: "string"
    initial: ""
    document: "ui_state_doc"

  newCommentText::
    type: "string"
    initial: ""
    document: "ui_state_doc"

computed::
  totalPosts::
    expr: "${ size(posts) }"
    deps::
      - "posts"

  totalComments::
    expr: "${ size(comments) }"
    deps::
      - "comments"

ui::
  publisher::
    - ::
      type: "screen"
      id: "main"
      name: "main"
      isEntryPoint: true
      css: "padding: 2rem; max-width: 1200px; margin: 0 auto;"
      blocks::
        - ::
          type: "heading"
          level: 1
          content: "Reddit-Style Discussion Board"
          css: "color: #ff4500; margin-bottom: 2rem;"

        - ::
          type: "container"
          css: "background: white; padding: 1.5rem; border-radius: 8px; margin-bottom: 2rem;"
          blocks::
            - ::
              type: "heading"
              level: 2
              content: "Stats"

            - ::
              type: "text"
              content: "Total Posts: {{totalPosts}} | Total Comments: {{totalComments}}"

        - ::
          type: "section"
          css: "background: white; padding: 1.5rem; border-radius: 8px; margin-bottom: 2rem;"
          blocks::
            - ::
              type: "heading"
              level: 2
              content: "Create New Post"

            - ::
              type: "input"
              name: "newPostTitle"
              placeholder: "Enter post title..."
              css: "width: 100%; padding: 0.75rem; margin-bottom: 1rem;"

            - ::
              type: "textarea"
              name: "newPostContent"
              placeholder: "Enter post content..."
              css: "width: 100%; padding: 0.75rem; margin-bottom: 1rem; min-height: 100px;"

            - ::
              type: "button"
              content: "Post"
              action: "setState"
              stateUpdates::
                posts: "${ posts + [{\"id\": generateId(), \"title\": newPostTitle, \"content\": newPostContent, \"author\": currentUser, \"timestamp\": now(), \"upvotes\": 0}] }"
                newPostTitle: ""
                newPostContent: ""
              css: "padding: 0.75rem 2rem; background: #ff4500; color: white;"

        - ::
          type: "section"
          blocks::
            - ::
              type: "heading"
              level: 2
              content: "Posts"

            - ::
              type: "loop"
              each: "${ posts }"
              as: "post"
              key: "id"
              blocks::
                - ::
                  type: "container"
                  css: "background: white; padding: 1.5rem; margin-bottom: 1.5rem; border-radius: 8px;"
                  blocks::
                    - ::
                      type: "heading"
                      level: 3
                      content: "{{post.title}}"

                    - ::
                      type: "text"
                      content: "{{post.content}}"
                      css: "margin-bottom: 1rem;"

                    - ::
                      type: "text"
                      content: "Posted by {{post.author}} • {{post.upvotes}} upvotes"
                      css: "font-size: 0.875rem; color: #888;"

                    - ::
                      type: "container"
                      css: "border-top: 1px solid #e0e0e0; padding-top: 1rem; margin-top: 1rem;"
                      blocks::
                        - ::
                          type: "heading"
                          level: 4
                          content: "Comments ({{size(comments.filter(c => c.postId == post.id))}})"

                        - ::
                          type: "loop"
                          each: "${ comments.filter(c => c.postId == post.id) }"
                          as: "comment"
                          key: "id"
                          blocks::
                            - ::
                              type: "container"
                              css: "background: #f9f9f9; padding: 1rem; margin-bottom: 1rem; border-radius: 6px;"
                              blocks::
                                - ::
                                  type: "text"
                                  content: "{{comment.text}}"

                                - ::
                                  type: "text"
                                  content: "by {{comment.author}} • {{comment.upvotes}} upvotes"
                                  css: "font-size: 0.8125rem; color: #888;"

                        - ::
                          type: "container"
                          css: "margin-top: 1rem;"
                          blocks::
                            - ::
                              type: "input"
                              name: "newCommentText"
                              placeholder: "Add a comment..."
                              css: "width: 100%; padding: 0.625rem; margin-bottom: 0.75rem;"

                            - ::
                              type: "button"
                              content: "Comment"
                              action: "setState"
                              stateUpdates::
                                comments: "${ comments + [{\"id\": generateId(), \"postId\": post.id, \"text\": newCommentText, \"author\": currentUser, \"timestamp\": now(), \"upvotes\": 0, \"parentCommentId\": \"\"}] }"
                                newCommentText: ""
                              css: "padding: 0.5rem 1.25rem; background: #0079d3; color: white;"
```

---

## Migration Checklist

When migrating old templates to v2.0:

- [ ] Add `document:` to every field in `documents::`
- [ ] Use only the 6 approved document names
- [ ] Change computed syntax to `expr:` and `deps::`
- [ ] Use `${ }` for all CEL expressions
- [ ] Use `{{variable}}` for string interpolation
- [ ] Use `::` suffix for arrays and complex initial values
- [ ] Add `key:` to all loops
- [ ] Separate nested arrays into references (e.g., `postId` instead of nested comments)

---

## Next Steps

1. **Test your template** in Sthalam publisher mode
2. **Verify reactivity** - computed values should update automatically
3. **Test sync** - changes should propagate to viewers
4. **Check console** - look for `📡 [PublisherApp] Subscribing to documents:` to confirm dynamic subscriptions

---

## FAQ

**Q: Can I create custom documents beyond the 6 listed?**
A: Not yet. For now, only use the 6 UCAN documents listed in this guide.

**Q: Why separate arrays instead of nesting (Post.comments)?**
A: Different sync permissions! Posts go to `content_doc` (publisher-only), comments to `collaborative_doc` (viewers can add). Can't have different permissions within the same object.

**Q: Can computed values depend on other computed values?**
A: Yes! The reactive system resolves them in multiple passes (up to 5 passes).

**Q: What if I don't specify `isEntryPoint`?**
A: The first screen in the array becomes the entry point by default.

**Q: Can I use external CSS classes?**
A: No, all styling must be inline CSS in the template.

---

**End of Guide**
