# Sthalam Template Language Specification

**Version**: 1.0.0-draft
**Status**: Living Specification
**Last Updated**: 2025-11-04

---

## Overview

Sthalam Template Language is a **domain-specific language (DSL)** for building collaborative, reactive **single-page applications (SPAs)**. It combines three core technologies:

1. **HUML** - Human-readable markup for structure (data format)
2. **CEL** - Common Expression Language for logic (expressions)
3. **Custom Keywords** - Domain-specific vocabulary for UI, state, and behavior

### Design Philosophy

- **Explicit over Implicit** - All behavior is declared
- **Secure by Default** - Safe for user-generated templates
- **Collaborative First** - Built for real-time collaboration via Loro CRDTs
- **Reactive** - Automatic UI updates when state changes
- **Type-Safe** - Every value has an explicit type
- **Single-Page Application** - Multiple screens, client-side navigation, no page reloads

---

## 1. File Structure

Every Sthalam template follows this structure:

```huml
name: "Template Name"
version: "1.0.0"

documents::
  # State schema definitions

computed::
  # Derived values with dependencies

ui::
  # User interface definitions
```

### Example: Minimal Template

```huml
name: "Hello World"
version: "1.0.0"

documents::
  appState:
    message:
      type: string
      initial: "Hello, World!"

ui::
  viewer::
    - type: screen
      blocks::
        - type: text
          content: "{{ message }}"
```

---

## 2. Type System

Sthalam supports the following primitive and composite types:

### Primitive Types

| Type | Description | Example Values |
|------|-------------|----------------|
| `string` | Text data | `"Hello"`, `""`, `"Multi\nline"` |
| `number` | Numeric values (int/float) | `42`, `3.14`, `-10`, `0` |
| `boolean` | True/false | `true`, `false` |
| `date` | Date/time values | `2025-01-15T10:30:00Z`, `2025-11-04` |

### Composite Types

| Type | Description | Example |
|------|-------------|---------|
| `list` | Ordered array of primitives | `[1, 2, 3]`, `["a", "b"]` |
| `collection` | Array of objects with schema | `[{id: 1, name: "Alice"}, ...]` |
| `object` | Nested key-value structure | `{name: "Alice", age: 30}` |

### Type Declarations

```huml
documents::
  userState:
    # Primitive types
    username:
      type: string
      initial: ""

    age:
      type: number
      initial: 0

    isActive:
      type: boolean
      initial: false

    createdAt:
      type: date
      initial: 2025-01-01T00:00:00Z

    # Composite types
    tags:
      type: list
      initial: []

    preferences:
      type: object
      initial: {theme: "dark", notifications: true}

    posts:
      type: collection
      initial: []
      schema:
        id: string
        title: string
        body: string
        publishedAt: date
        tags: list
```

---

## 3. Document Definitions

Documents define the **state schema** for your application. Each document is a named container of typed fields.

### Syntax

```huml
documents::
  <documentName>:
    <fieldName>:
      type: <type>
      initial: <initialValue>
      schema: <schemaDefinition>  # Optional, for collections/objects
```

### Document Scopes

Templates typically have multiple documents for different purposes:

- **publisherState** - Draft/editing state (local to publisher)
- **viewerState** - UI state for viewers (filters, selections)
- **content** - Shared collaborative content (posts, comments)
- **appState** - Shared application state (settings, config)

### Example: Blog Documents

```huml
documents::
  # Publisher's local state (not shared)
  publisherState:
    draftTitle:
      type: string
      initial: ""

    draftBody:
      type: string
      initial: ""

    draftTags:
      type: list
      initial: []

    isPublishing:
      type: boolean
      initial: false

    lastSaved:
      type: date
      initial: null

  # Viewer's local state (filters, UI state)
  viewerState:
    selectedTag:
      type: string
      initial: ""

    currentView:
      type: string
      initial: "grid"

    searchQuery:
      type: string
      initial: ""

  # Shared collaborative content
  content:
    posts:
      type: collection
      initial: []
      schema:
        id: string
        title: string
        body: string
        author: string
        publishedAt: date
        tags: list
        viewCount: number
```

---

## 4. Computed Values

Computed values are **derived state** that automatically updates when dependencies change.

### Syntax

```huml
computed::
  <scope>:
    <computedName>:
      type: <type>
      expr: "<CEL expression>"
      depends::
        - document.field
        - document.otherField
```

### Scopes

- `publisher` - Computed values for publisher mode
- `viewer` - Computed values for viewer mode
- `shared` - Computed values available in both modes

### Dependency Tracking

**IMPORTANT**: Dependencies must be **explicitly declared** using the `depends` array. This enables:
- Fine-grained reactivity (only recompute when dependencies change)
- Performance optimization (avoid unnecessary evaluations)
- Clear understanding of data flow

### Example: Blog Computed Values

```huml
computed::
  publisher:
    canPublish:
      type: boolean
      expr: "draftTitle != '' && draftBody != '' && !isPublishing"
      depends::
        - publisherState.draftTitle
        - publisherState.draftBody
        - publisherState.isPublishing

    wordCount:
      type: number
      expr: "draftBody.split(' ').filter(w, w.trim() != '').size()"
      depends::
        - publisherState.draftBody

    estimatedReadTime:
      type: number
      expr: "int(wordCount / 200)"
      depends::
        - computed.publisher.wordCount

    hasUnsavedChanges:
      type: boolean
      expr: "(draftTitle != '' || draftBody != '') && lastSaved == null"
      depends::
        - publisherState.draftTitle
        - publisherState.draftBody
        - publisherState.lastSaved

  viewer:
    filteredPosts:
      type: collection
      expr: "posts.filter(p, (selectedTag == '' || p.tags.exists(t, t == selectedTag)) && (searchQuery == '' || p.title.contains(searchQuery) || p.body.contains(searchQuery)))"
      depends::
        - content.posts
        - viewerState.selectedTag
        - viewerState.searchQuery

    postCount:
      type: number
      expr: "filteredPosts.size()"
      depends::
        - computed.viewer.filteredPosts

    allTags:
      type: list
      expr: "posts.map(p, p.tags).flatten().unique()"
      depends::
        - content.posts

  shared:
    currentDate:
      type: date
      expr: "now()"
      depends:: []
```

---

## 5. CEL Expression Language

Sthalam uses **CEL (Common Expression Language)** for all logical expressions. CEL is a standard, sandboxed expression language used in Kubernetes, Firebase, and Google Cloud.

### CEL Resources

- **Spec**: https://github.com/google/cel-spec
- **Language Definition**: https://github.com/google/cel-spec/blob/master/doc/langdef.md

### Core Operators

```cel
# Comparison
x == y        # Equal
x != y        # Not equal
x < y         # Less than
x > y         # Greater than
x <= y        # Less than or equal
x >= y        # Greater than or equal

# Logical
x && y        # Logical AND
x || y        # Logical OR
!x            # Logical NOT

# Arithmetic
x + y         # Addition
x - y         # Subtraction
x * y         # Multiplication
x / y         # Division
x % y         # Modulo

# String
s + t         # Concatenation
s.contains(t) # Substring check
s.startsWith(t)
s.endsWith(t)

# Ternary
condition ? trueValue : falseValue
```

### Common Functions

```cel
# String functions
"hello".size()                    # → 5
"hello".contains("ell")           # → true
"hello".startsWith("hel")         # → true
"hello".endsWith("lo")            # → true
"  hello  ".trim()                # → "hello"
"hello".toUpperCase()             # → "HELLO"
"hello world".split(" ")          # → ["hello", "world"]

# Collection functions
[1, 2, 3].size()                  # → 3
[1, 2, 3].filter(x, x > 1)        # → [2, 3]
[1, 2, 3].map(x, x * 2)           # → [2, 4, 6]
[1, 2, 3].exists(x, x == 2)       # → true
[1, 2, 3].all(x, x > 0)           # → true
["a", "b"].exists_one(x, x == "a") # → true

# Type conversions
int("42")                         # → 42
double(42)                        # → 42.0
string(42)                        # → "42"

# Date/time
timestamp("2025-01-15T10:30:00Z") # Parse timestamp
now()                             # Current time
duration("1h30m")                 # Parse duration
```

### Collection Operations

```cel
# filter: Select items matching predicate
posts.filter(p, p.author == currentUser && p.status == "published")

# map: Transform each item
posts.map(p, p.title)

# exists: Check if any item matches
posts.exists(p, p.draft == true)

# all: Check if all items match
posts.all(p, p.published == true)

# Chaining operations
posts
  .filter(p, p.author == user)
  .map(p, p.title)
  .size()
```

### Null Safety

```cel
# Use default value if null
userAvatar != null ? userAvatar : "/default.png"

# Or operator for defaults
selectedTag != "" ? selectedTag : "all"
```

### Expression Syntax in HUML

CEL expressions in HUML can use either single-line strings or multi-line strings:

```huml
# Single-line expression (simple)
expr: "posts.size() > 0"

# Multi-line expression (complex, HUML uses """)
expr: """
  posts.filter(p,
    p.author == currentUser &&
    p.status == "published"
  ).size()
"""
```

### Variable Interpolation

In template content, use `{{ }}` to interpolate CEL expressions:

```huml
- type: text
  content: "You have {{ posts.size() }} posts"

- type: text
  content: "Welcome, {{ user.name }}!"

- type: text
  content: "Published on {{ post.publishedAt.format('YYYY-MM-DD') }}"
```

---

## 6. Screens and Navigation

Sthalam applications are **single-page applications** with multiple screens. Navigation happens client-side without page reloads.

### Screen Definitions

Each mode (publisher/viewer) can have multiple screens:

```huml
ui::
  viewer::
    # Define multiple screens
    home:
      - type: screen
        blocks::
          - type: heading
            content: "Home"
          - type: link
            content: "View Posts"
            href: "/posts"

    posts:
      - type: screen
        blocks::
          - type: heading
            content: "All Posts"
          # ... posts list

    post-detail:
      - type: screen
        blocks::
          - type: heading
            content: "{{ currentPost.title }}"
          # ... post content
```

### Navigation

#### Link Block (Declarative Navigation)

```huml
# Navigate to another screen
- type: link
  content: "Go to Posts"
  href: "/posts"
  css: "color: #0066cc; text-decoration: none;"

# Navigate with parameters
- type: link
  content: "View Post"
  href: "/post/{{ post.id }}"

# External link (opens in new tab)
- type: link
  content: "External Site"
  href: "https://example.com"
  external: true
  css: "color: #0066cc;"
```

#### Navigate Action (Programmatic Navigation)

```huml
# Navigate on button click
- type: button
  content: "View Details"
  action: navigate
  params::
    screen: post-detail
    postId: "{{ post.id }}"

# Navigate after form submission
- type: button
  content: "Create Post"
  action: createPost
  onSuccess:
    action: navigate
    params::
      screen: posts
```

### Route Parameters

Access route parameters in computed values:

```huml
computed::
  viewer:
    currentPostId:
      type: string
      expr: "route.params.postId"
      depends::
        - route.params

    currentPost:
      type: object
      expr: "posts.find(p, p.id == currentPostId)"
      depends::
        - content.posts
        - computed.viewer.currentPostId
```

---

## 7. Modals

Modals are overlay UI elements that appear on top of the current screen.

### Modal Block Type

```huml
# Simple modal
- type: modal
  name: confirm-delete
  visible: "{{ showDeleteModal }}"
  blocks::
    - type: heading
      content: "Confirm Delete"

    - type: text
      content: "Are you sure you want to delete this post?"

    - type: container
      layout: flex
      direction: row
      gap: 1rem
      blocks::
        - type: button
          content: "Cancel"
          action: closeModal
          params::
            modal: confirm-delete

        - type: button
          content: "Delete"
          action: deletePost
          params::
            postId: "{{ selectedPostId }}"
          css: "background: red; color: white;"
```

### Modal with Form

```huml
# Edit modal with form
- type: modal
  name: edit-post
  visible: "{{ showEditModal }}"
  size: large                # small | medium | large | fullscreen
  closable: true            # Show X button
  css: "max-width: 600px;"
  blocks::
    - type: heading
      content: "Edit Post"

    - type: form
      name: edit-form
      blocks::
        - type: input
          name: title
          value: "{{ editingPost.title }}"
          placeholder: "Post title"

        - type: textarea
          name: body
          value: "{{ editingPost.body }}"
          rows: 10

        - type: container
          layout: flex
          direction: row
          gap: 1rem
          css: "justify-content: flex-end;"
          blocks::
            - type: button
              content: "Cancel"
              action: closeModal
              params::
                modal: edit-post

            - type: button
              content: "Save"
              type: submit
              action: updatePost
              css: "background: #0066cc; color: white;"
```

### Modal Actions

```huml
# Open modal
- type: button
  content: "Delete Post"
  action: openModal
  params::
    modal: confirm-delete
    postId: "{{ post.id }}"

# Close modal
- type: button
  content: "Close"
  action: closeModal
  params::
    modal: confirm-delete

# Open modal on condition
- type: button
  content: "Publish"
  action: publishPost
  onError:
    action: openModal
    params::
      modal: error-modal
      message: "{{ error.message }}"
```

### Modal State

Track modal state in documents:

```huml
documents::
  uiState:
    activeModal:
      type: string
      initial: ""

    modalData:
      type: object
      initial: {}

# Modal visibility computed
computed::
  viewer:
    showDeleteModal:
      type: boolean
      expr: "activeModal == 'confirm-delete'"
      depends::
        - uiState.activeModal
```

---

## 8. UI Block Types

UI blocks define the visual structure and interactive elements of your application.

### Layout Blocks

```huml
# screen: Top-level container (full application view)
- type: screen
  css: "max-width: 1200px; margin: 0 auto;"
  blocks::
    # ... child blocks

# container: Generic layout container
- type: container
  layout: flex              # flex | grid | block
  direction: column         # row | column (for flex)
  gap: 1rem
  blocks::
    # ... child blocks

# section: Semantic section grouping
- type: section
  css: "padding: 2rem; background: white;"
  blocks::
    # ... child blocks
```

### Content Blocks

```huml
# text: Display text/paragraphs
- type: text
  content: "This is a paragraph of text."
  css: "color: #333; line-height: 1.6;"

# heading: Titles (h1-h6)
- type: heading
  level: 1                  # 1-6 (default: 1)
  content: "Page Title"
  css: "font-size: 2rem; margin-bottom: 1rem;"

# label: Form labels
- type: label
  for: email-input
  content: "Email Address"
  css: "font-weight: 500;"

# image: Display image
- type: image
  src: "{{ post.imageUrl }}"
  alt: "{{ post.title }}"
  css: "max-width: 100%; height: auto;"

# video: Display video
- type: video
  src: "{{ post.videoUrl }}"
  controls: true
  css: "width: 100%;"
```

### Input Blocks

```huml
# input: Single-line text input
- type: input
  name: email
  value: "{{ userEmail }}"
  placeholder: "Enter your email"
  validate: "value.contains('@') && value.contains('.')"
  error: "Please enter a valid email"
  css: "padding: 0.5rem; border: 1px solid #ddd;"
  onChange: updateEmail

# textarea: Multi-line text input
- type: textarea
  name: body
  value: "{{ draftBody }}"
  placeholder: "Write your post..."
  rows: 10
  css: "width: 100%; padding: 0.5rem;"
  onChange: updateBody

# checkbox: Boolean input
- type: checkbox
  name: subscribe
  checked: "{{ isSubscribed }}"
  label: "Subscribe to newsletter"
  onChange: toggleSubscribe

# select: Dropdown selection
- type: select
  name: category
  value: "{{ selectedCategory }}"
  options::
    - value: "tech"
      label: "Technology"
    - value: "design"
      label: "Design"
    - value: "writing"
      label: "Writing"
  onChange: updateCategory

# radio: Radio button group
- type: radio
  name: view
  value: "{{ currentView }}"
  options::
    - value: "grid"
      label: "Grid View"
    - value: "list"
      label: "List View"
  onChange: updateView
```

### Action Blocks

```huml
# button: Clickable button
- type: button
  content: "Publish Post"
  action: publishPost
  disabled: "!canPublish"
  css: "padding: 0.75rem 1.5rem; background: #0066cc; color: white;"
  onClick: handlePublish

# link: Navigation link
- type: link
  content: "View Profile"
  href: "/profile/{{ user.id }}"
  css: "color: #0066cc; text-decoration: none;"

# form: Form wrapper (groups inputs)
- type: form
  name: publish-form
  blocks::
    - type: input
      name: title
    - type: textarea
      name: body
    - type: button
      content: "Submit"
      type: submit
  onSubmit: handleSubmit
```

### Special Blocks

```huml
# canvas: Canvas for graphics/patterns
- type: canvas
  width: 400
  height: 400
  render: animatedPattern     # References a render function
  css: "border: 1px solid #ddd;"

# modal: Overlay dialog
- type: modal
  name: confirm-action
  visible: "{{ showModal }}"
  size: medium               # small | medium | large | fullscreen
  closable: true            # Show close button
  blocks::
    # Modal content
```

---

## 7. Control Flow

Control flow keywords enable conditional rendering and iteration.

### Simple Conditional: `when`

Show/hide blocks based on a condition:

```huml
# Only show if condition is true
- type: text
  content: "Loading..."
  when: isLoading

# Only show if computed value is true
- type: button
  content: "Publish"
  action: publishPost
  when: canPublish
```

### If-Else Branching: `if` / `then` / `else`

```huml
# Conditional with branches
- if: "posts.size() > 0"
  then::
    - type: container
      blocks::
        - type: text
          content: "{{ posts.size() }} posts available"

        - type: container
          forEach: posts
          as: post
          blocks::
            - type: heading
              content: "{{ post.title }}"

  else::
    - type: container
      blocks::
        - type: text
          content: "No posts yet."

        - type: text
          content: "Be the first to publish!"
```

### Pattern Matching: `match` / `cases`

Switch between multiple branches based on a value:

```huml
- match: currentView
  cases::
    # Case 1: Grid view
    - value: "grid"
      blocks::
        - type: container
          layout: grid
          columns: 3
          gap: 1.5rem
          blocks::
            - type: container
              forEach: filteredPosts
              as: post
              blocks::
                # Grid card layout

    # Case 2: List view
    - value: "list"
      blocks::
        - type: container
          layout: flex
          direction: column
          gap: 1rem
          blocks::
            - type: container
              forEach: filteredPosts
              as: post
              blocks::
                # List row layout

    # Case 3: Compact view
    - value: "compact"
      blocks::
        - type: container
          css: "font-size: 0.875rem;"
          blocks::
            # Compact layout

    # Default case (fallback)
    - default: true
      blocks::
        - type: text
          content: "Unknown view: {{ currentView }}"
          css: "color: red;"
```

### Loops: `forEach` / `as`

Iterate over collections:

```huml
# Simple iteration
- type: container
  forEach: posts
  as: post
  blocks::
    - type: text
      content: "{{ post.title }}"

# With key for reconciliation
- type: container
  forEach: posts
  as: post
  key: post.id              # Unique key for efficient updates
  blocks::
    - type: section
      blocks::
        - type: heading
          content: "{{ post.title }}"
        - type: text
          content: "{{ post.body }}"

# With pagination
- type: container
  forEach: posts
  as: post
  key: post.id
  limit: 10                  # Show 10 items per page
  offset: "{{ currentPage * 10 }}"  # Offset based on page
  blocks::
    # Post display

# Nested loops
- type: container
  forEach: categories
  as: category
  blocks::
    - type: heading
      content: "{{ category.name }}"

    - type: container
      forEach: category.posts
      as: post
      blocks::
        - type: text
          content: "{{ post.title }}"
```

---

## 8. Block Properties

All block types support common properties for behavior, styling, and events.

### Core Properties

```huml
type: <blockType>           # Required: Block type
content: <string>           # Display text (for text/button/heading)
blocks:: [<blocks>]         # Nested child blocks
```

### Behavior Properties

```huml
action: <actionName>        # Action to trigger (for buttons)
disabled: <expr>            # Disable condition (boolean CEL expression)
visible: <expr>             # Visibility condition (boolean)
when: <expr>                # Alias for visible
```

### Form Properties

```huml
name: <string>              # Form field name
value: <expr>               # Field value (CEL expression or literal)
placeholder: <string>       # Input placeholder text
validate: <expr>            # Validation expression (boolean)
error: <string>             # Error message when validation fails
checked: <expr>             # Checkbox checked state (boolean)
options:: [<options>]       # Select/radio options
```

### Style Properties

```huml
css: <string>               # Inline CSS styles
class: <string>             # CSS class name
layout: <layoutType>        # Layout type: flex | grid | block
direction: <direction>      # Flex direction: row | column
gap: <size>                 # Gap between items (e.g., "1rem")
columns: <number>           # Grid columns count
rows: <number>              # Grid rows count
```

### Event Properties

```huml
onClick: <actionName>       # Click event handler
onChange: <actionName>      # Change event handler
onFocus: <actionName>       # Focus event handler
onBlur: <actionName>        # Blur event handler
onSubmit: <actionName>      # Form submit handler
```

### Iteration Properties

```huml
forEach: <expr>             # Collection to iterate (CEL expression)
as: <varName>               # Iterator variable name
key: <expr>                 # Unique key for reconciliation
limit: <number>             # Pagination: items per page
offset: <expr>              # Pagination: start offset
```

### Conditional Properties

```huml
if: <expr>                  # Condition for if-else
then:: [<blocks>]           # Blocks when condition is true
else:: [<blocks>]           # Blocks when condition is false
match: <expr>               # Value to match against
cases:: [<cases>]           # Match cases
```

---

## 9. Actions

Actions are triggered by user interactions (clicks, form submissions, etc.).

### Action Definition

Actions are defined as strings that reference action handlers in the runtime:

```huml
- type: button
  content: "Save Draft"
  action: saveDraft          # Action name
```

### Built-in Actions

```huml
# Navigation
action: navigate
params::
  screen: posts              # Navigate to screen
  postId: "{{ post.id }}"   # Optional route params

# Modal actions
action: openModal
params::
  modal: confirm-delete
  data: {}                  # Optional modal data

action: closeModal
params::
  modal: confirm-delete

# State updates
action: setState
params::
  field: currentView
  value: "grid"

# Data operations (defined by template/runtime)
action: createPost
action: updatePost
action: deletePost
action: publishPost
```

### Action Parameters

Pass data to actions:

```huml
- type: button
  content: "Delete"
  action: deletePost
  params::
    postId: "{{ post.id }}"

- type: button
  content: "Set View"
  action: setState
  params::
    field: currentView
    value: "grid"
```

### Form Actions

Form submissions automatically collect field values:

```huml
- type: form
  name: publish-form
  blocks::
    - type: input
      name: title
      value: "{{ draftTitle }}"

    - type: textarea
      name: body
      value: "{{ draftBody }}"

    - type: button
      content: "Publish"
      type: submit
      action: publishPost    # Receives form data: {title, body}
  onSubmit: handlePublish
```

---

## 10. Complete Example: Blog Template

Here's a comprehensive example demonstrating all features:

```huml
name: "Blog Publisher & Viewer"
version: "1.0.0"

# ============================================================================
# STATE DEFINITIONS
# ============================================================================

documents::
  # Publisher's draft state (local)
  publisherState:
    draftTitle:
      type: string
      initial: ""

    draftBody:
      type: string
      initial: ""

    draftTags:
      type: list
      initial: []

    isPublishing:
      type: boolean
      initial: false

    lastSaved:
      type: date
      initial: null

  # Viewer's UI state (local)
  viewerState:
    selectedTag:
      type: string
      initial: ""

    currentView:
      type: string
      initial: "grid"

    searchQuery:
      type: string
      initial: ""

    currentPage:
      type: number
      initial: 0

  # Shared content (collaborative)
  content:
    posts:
      type: collection
      initial: []
      schema:
        id: string
        title: string
        body: string
        author: string
        publishedAt: date
        tags: list
        viewCount: number

# ============================================================================
# COMPUTED VALUES
# ============================================================================

computed::
  publisher:
    canPublish:
      type: boolean
      expr: "draftTitle != '' && draftBody != '' && !isPublishing"
      depends::
        - publisherState.draftTitle
        - publisherState.draftBody
        - publisherState.isPublishing

    wordCount:
      type: number
      expr: "draftBody.split(' ').filter(w, w.trim() != '').size()"
      depends::
        - publisherState.draftBody

    estimatedReadTime:
      type: number
      expr: "int(wordCount / 200)"
      depends::
        - computed.publisher.wordCount

    hasUnsavedChanges:
      type: boolean
      expr: "(draftTitle != '' || draftBody != '') && lastSaved == null"
      depends::
        - publisherState.draftTitle
        - publisherState.draftBody
        - publisherState.lastSaved

  viewer:
    filteredPosts:
      type: collection
      expr: "posts.filter(p, (selectedTag == '' || p.tags.exists(t, t == selectedTag)) && (searchQuery == '' || p.title.contains(searchQuery) || p.body.contains(searchQuery)))"
      depends::
        - content.posts
        - viewerState.selectedTag
        - viewerState.searchQuery

    postCount:
      type: number
      expr: "filteredPosts.size()"
      depends::
        - computed.viewer.filteredPosts

    allTags:
      type: list
      expr: "posts.map(p, p.tags).flatten().unique()"
      depends::
        - content.posts

    paginatedPosts:
      type: collection
      expr: "filteredPosts.slice(currentPage * 10, (currentPage + 1) * 10)"
      depends::
        - computed.viewer.filteredPosts
        - viewerState.currentPage

# ============================================================================
# USER INTERFACE - PUBLISHER
# ============================================================================

ui::
  publisher::
    - type: screen
      css: "max-width: 800px; margin: 0 auto; padding: 2rem;"
      blocks::
        # Header
        - type: heading
          level: 1
          content: "Write New Post"
          css: "font-size: 2rem; margin-bottom: 2rem;"

        # Status indicators
        - type: container
          layout: flex
          direction: row
          gap: 1rem
          css: "margin-bottom: 1rem; font-size: 0.875rem; color: #666;"
          blocks::
            - type: text
              content: "{{ wordCount }} words"
              when: "wordCount > 0"

            - type: text
              content: "~{{ estimatedReadTime }} min read"
              when: "estimatedReadTime > 0"

            - type: text
              content: "⚠️ Unsaved changes"
              css: "color: orange;"
              when: hasUnsavedChanges

            - type: text
              content: "✓ Saved"
              css: "color: green;"
              when: "!hasUnsavedChanges && lastSaved != null"

        # Publishing form
        - type: form
          name: publish-form
          blocks::
            # Title input
            - type: input
              name: title
              value: "{{ draftTitle }}"
              placeholder: "Post title..."
              validate: "value.trim() != ''"
              error: "Title is required"
              css: "font-size: 1.5rem; padding: 0.5rem; border: 1px solid #ddd; border-radius: 4px; width: 100%; margin-bottom: 1rem;"
              onChange: updateTitle

            # Body textarea
            - type: textarea
              name: body
              value: "{{ draftBody }}"
              placeholder: "Write your post..."
              rows: 15
              validate: "value.trim() != ''"
              error: "Body is required"
              css: "padding: 0.5rem; border: 1px solid #ddd; border-radius: 4px; width: 100%; font-family: inherit; margin-bottom: 1rem;"
              onChange: updateBody

            # Tags input
            - type: input
              name: tags
              value: "{{ draftTags.join(', ') }}"
              placeholder: "Tags (comma-separated)..."
              css: "padding: 0.5rem; border: 1px solid #ddd; border-radius: 4px; width: 100%; margin-bottom: 1rem;"
              onChange: updateTags

            # Action buttons
            - type: container
              layout: flex
              direction: row
              gap: 1rem
              blocks::
                # Publish button (conditional states)
                - if: isPublishing
                  then::
                    - type: button
                      content: "Publishing..."
                      disabled: true
                      css: "padding: 0.75rem 1.5rem; background: #ccc; border: none; border-radius: 4px; cursor: not-allowed;"

                  else::
                    - if: canPublish
                      then::
                        - type: button
                          content: "Publish Post"
                          action: publishPost
                          type: submit
                          css: "padding: 0.75rem 1.5rem; background: #0066cc; color: white; border: none; border-radius: 4px; cursor: pointer;"
                          onClick: handlePublish

                      else::
                        - type: button
                          content: "Publish Post"
                          disabled: true
                          css: "padding: 0.75rem 1.5rem; background: #ccc; color: #666; border: none; border-radius: 4px; cursor: not-allowed;"

                # Save draft button
                - type: button
                  content: "Save Draft"
                  action: saveDraft
                  css: "padding: 0.75rem 1.5rem; background: white; color: #0066cc; border: 1px solid #0066cc; border-radius: 4px; cursor: pointer;"
                  when: hasUnsavedChanges
                  onClick: handleSaveDraft

# ============================================================================
# USER INTERFACE - VIEWER
# ============================================================================

  viewer::
    - type: screen
      css: "max-width: 1200px; margin: 0 auto; padding: 2rem;"
      blocks::
        # Header section
        - type: section
          css: "margin-bottom: 2rem;"
          blocks::
            - type: heading
              level: 1
              content: "Blog Posts ({{ postCount }})"
              css: "font-size: 2rem; margin-bottom: 1rem;"

            # Search and filters
            - type: container
              layout: flex
              direction: row
              gap: 1rem
              css: "margin-bottom: 1rem;"
              blocks::
                # Search input
                - type: input
                  name: search
                  value: "{{ searchQuery }}"
                  placeholder: "Search posts..."
                  css: "flex: 1; padding: 0.5rem; border: 1px solid #ddd; border-radius: 4px;"
                  onChange: updateSearch

                # View switcher
                - type: container
                  layout: flex
                  direction: row
                  gap: 0.5rem
                  blocks::
                    - type: button
                      content: "Grid"
                      action: setState
                      params::
                        field: currentView
                        value: "grid"
                      css: "padding: 0.5rem 1rem; background: {{ currentView == 'grid' ? '#0066cc' : 'white' }}; color: {{ currentView == 'grid' ? 'white' : '#0066cc' }}; border: 1px solid #0066cc; border-radius: 4px; cursor: pointer;"

                    - type: button
                      content: "List"
                      action: setState
                      params::
                        field: currentView
                        value: "list"
                      css: "padding: 0.5rem 1rem; background: {{ currentView == 'list' ? '#0066cc' : 'white' }}; color: {{ currentView == 'list' ? 'white' : '#0066cc' }}; border: 1px solid #0066cc; border-radius: 4px; cursor: pointer;"

            # Tag filters
            - type: container
              layout: flex
              direction: row
              gap: 0.5rem
              css: "flex-wrap: wrap;"
              blocks::
                # "All" tag
                - type: button
                  content: "All"
                  action: setState
                  params::
                    field: selectedTag
                    value: ""
                  css: "padding: 0.25rem 0.75rem; background: {{ selectedTag == '' ? '#0066cc' : '#f0f0f0' }}; color: {{ selectedTag == '' ? 'white' : '#333' }}; border: none; border-radius: 16px; font-size: 0.875rem; cursor: pointer;"

                # Individual tags
                - type: container
                  forEach: allTags
                  as: tag
                  blocks::
                    - type: button
                      content: "{{ tag }}"
                      action: setState
                      params::
                        field: selectedTag
                        value: "{{ tag }}"
                      css: "padding: 0.25rem 0.75rem; background: {{ selectedTag == tag ? '#0066cc' : '#f0f0f0' }}; color: {{ selectedTag == tag ? 'white' : '#333' }}; border: none; border-radius: 16px; font-size: 0.875rem; cursor: pointer;"

        # Posts display (match view type)
        - match: currentView
          cases::
            # Grid view
            - value: "grid"
              blocks::
                - if: "postCount > 0"
                  then::
                    - type: container
                      layout: grid
                      columns: 3
                      gap: 1.5rem
                      blocks::
                        - type: container
                          forEach: paginatedPosts
                          as: post
                          key: post.id
                          blocks::
                            - type: section
                              css: "border: 1px solid #ddd; border-radius: 8px; padding: 1.5rem; background: white; display: flex; flex-direction: column; height: 100%;"
                              blocks::
                                - type: heading
                                  level: 2
                                  content: "{{ post.title }}"
                                  css: "font-size: 1.25rem; margin-bottom: 0.5rem;"

                                - type: text
                                  content: "{{ post.body.substring(0, 150) }}..."
                                  css: "color: #666; margin-bottom: 1rem; flex: 1;"

                                - type: text
                                  content: "By {{ post.author }} • {{ post.publishedAt.format('MMM DD, YYYY') }}"
                                  css: "font-size: 0.875rem; color: #999; margin-bottom: 0.5rem;"

                                - type: container
                                  layout: flex
                                  direction: row
                                  gap: 0.5rem
                                  css: "flex-wrap: wrap;"
                                  blocks::
                                    - type: container
                                      forEach: post.tags
                                      as: tag
                                      blocks::
                                        - type: text
                                          content: "{{ tag }}"
                                          css: "padding: 0.25rem 0.5rem; background: #f0f0f0; border-radius: 4px; font-size: 0.75rem;"

                  else::
                    - type: section
                      css: "text-align: center; padding: 4rem 2rem; color: #999;"
                      blocks::
                        - type: heading
                          level: 2
                          content: "No posts found"
                          css: "font-size: 1.5rem; margin-bottom: 0.5rem;"

                        - type: text
                          content: "Try adjusting your filters or search query"

            # List view
            - value: "list"
              blocks::
                - if: "postCount > 0"
                  then::
                    - type: container
                      layout: flex
                      direction: column
                      gap: 1rem
                      blocks::
                        - type: container
                          forEach: paginatedPosts
                          as: post
                          key: post.id
                          blocks::
                            - type: section
                              css: "border-bottom: 1px solid #ddd; padding-bottom: 1rem;"
                              blocks::
                                - type: heading
                                  level: 2
                                  content: "{{ post.title }}"
                                  css: "font-size: 1.5rem; margin-bottom: 0.5rem;"

                                - type: text
                                  content: "{{ post.body }}"
                                  css: "color: #333; margin-bottom: 1rem; line-height: 1.6;"

                                - type: container
                                  layout: flex
                                  direction: row
                                  gap: 1rem
                                  css: "align-items: center; color: #666; font-size: 0.875rem;"
                                  blocks::
                                    - type: text
                                      content: "By {{ post.author }}"

                                    - type: text
                                      content: "•"

                                    - type: text
                                      content: "{{ post.publishedAt.format('MMMM DD, YYYY') }}"

                                    - type: text
                                      content: "•"

                                    - type: text
                                      content: "{{ post.viewCount }} views"

                                    - type: container
                                      layout: flex
                                      direction: row
                                      gap: 0.5rem
                                      css: "margin-left: auto;"
                                      blocks::
                                        - type: container
                                          forEach: post.tags
                                          as: tag
                                          blocks::
                                            - type: text
                                              content: "#{{ tag }}"
                                              css: "color: #0066cc;"

                  else::
                    - type: text
                      content: "No posts to display"
                      css: "text-align: center; color: #999; padding: 2rem;"

            # Default case
            - default: true
              blocks::
                - type: text
                  content: "Unknown view type: {{ currentView }}"
                  css: "color: red; padding: 1rem;"

        # Pagination
        - type: container
          layout: flex
          direction: row
          gap: 0.5rem
          css: "justify-content: center; margin-top: 2rem;"
          when: "postCount > 10"
          blocks::
            - type: button
              content: "Previous"
              action: setState
              params::
                field: currentPage
                value: "{{ currentPage - 1 }}"
              disabled: "currentPage == 0"
              css: "padding: 0.5rem 1rem; border: 1px solid #ddd; border-radius: 4px; cursor: pointer;"

            - type: text
              content: "Page {{ currentPage + 1 }} of {{ int((postCount + 9) / 10) }}"
              css: "padding: 0.5rem 1rem; color: #666;"

            - type: button
              content: "Next"
              action: setState
              params::
                field: currentPage
                value: "{{ currentPage + 1 }}"
              disabled: "(currentPage + 1) * 10 >= postCount"
              css: "padding: 0.5rem 1rem; border: 1px solid #ddd; border-radius: 4px; cursor: pointer;"
```

---

## 11. Security Model

Sthalam templates are **user-generated content** and must be treated as **untrusted**. The security model consists of four layers:

### Layer 1: Structural Validation

```typescript
// Validate HUML structure
- Field names: /^[a-zA-Z_][a-zA-Z0-9_]*$/
- Types: string|number|boolean|date|list|collection|object only
- CSS: Block dangerous patterns (javascript:, expression(), url())
- Nesting: Maximum 10 levels deep
- Size: Maximum 1MB template size
```

### Layer 2: Expression Parsing

```ocaml
(* OCaml parser validates CEL syntax *)
- Parse CEL expression to AST
- Check for blocked keywords (eval, import, require, etc.)
- Validate function names against whitelist
- Maximum 1000 characters per expression
```

### Layer 3: Sandboxed Evaluation

```ocaml
(* OCaml evaluator runs in sandbox *)
- ONLY access to Loro document data (no globals)
- NO access to: DOM, localStorage, window, document
- NO network access (fetch, XMLHttpRequest blocked)
- NO module system (import/require blocked)
- NO constructor/prototype access
- Timeout: 100ms per expression
```

### Layer 4: Rendering

```svelte
<!-- Svelte auto-escapes all interpolated values -->
<p>{evaluatedValue}</p>  <!-- Safe: auto-escaped -->

<!-- Never use dangerouslySetInnerHTML -->
```

---

## 12. Runtime Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Template File (HUML)                                         │
│ - Structure: Parsed by @huml-lang/huml                     │
│ - Expressions: Stored as CEL strings                        │
└─────────────────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│ Validation Layer (TypeScript)                               │
│ - Structural validation (field names, types, CSS)          │
│ - Expression syntax validation (OCaml parser)               │
└─────────────────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│ State Management (Svelte 5 + Loro)                          │
│ - Document state: $state runes (synced with Loro)          │
│ - Computed values: $derived runes (CEL evaluation)         │
│ - Dependency tracking: Explicit depends arrays              │
└─────────────────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│ Expression Evaluation (OCaml/WASM)                          │
│ - Parse CEL to AST (Menhir parser)                         │
│ - Evaluate in sandbox (custom logic for Loro)              │
│ - Return typed result                                       │
└─────────────────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────────────────┐
│ Rendering (Svelte Components)                               │
│ - BlockRenderer interprets UI blocks                        │
│ - Auto-escaped interpolation                                │
│ - Reactive updates on state changes                         │
└─────────────────────────────────────────────────────────────┘
```

---

## 13. Implementation Status

**Status**: Design Complete, Implementation Pending

### Next Steps

1. **Implement TemplateValidator** (TypeScript)
   - Structural validation
   - CSS sanitization
   - Expression validation

2. **Implement CEL Parser** (OCaml + Menhir)
   - Parse CEL grammar to AST
   - Validate syntax

3. **Implement CEL Evaluator** (OCaml)
   - Standard library functions
   - Custom Loro integration
   - Lazy loading for collections
   - Sandboxing

4. **Update BlockRenderer** (Svelte)
   - Support all block types
   - Implement control flow (when, if/else, match, forEach)
   - Event handling
   - Validation display

5. **Create RuntimeStateManager** (TypeScript)
   - Svelte 5 $state/$derived integration
   - Loro synchronization
   - Dependency tracking
   - Lazy collection loading

6. **Visual Template Editor** (Svelte + CodeMirror)
   - HUML structure editor
   - CEL expression editor with syntax highlighting
   - Live preview
   - Type autocomplete

---

## 14. Appendix: Quick Reference

### Type System
`string`, `number`, `boolean`, `date`, `list`, `collection`, `object`

### Structure Keywords
`name`, `version`, `documents`, `computed`, `ui`

### Block Types
**Layout**: `screen`, `container`, `section`
**Content**: `text`, `heading`, `label`, `image`, `video`
**Input**: `input`, `textarea`, `checkbox`, `select`, `radio`
**Action**: `button`, `link`, `form`
**Special**: `canvas`, `modal`

### Control Flow
`when`, `if`, `then`, `else`, `forEach`, `as`, `match`, `cases`, `default`

### Properties
**Core**: `type`, `content`, `blocks`
**Behavior**: `action`, `disabled`, `visible`
**Form**: `name`, `value`, `placeholder`, `validate`, `error`
**Style**: `css`, `class`, `layout`, `direction`, `gap`, `columns`
**Event**: `onClick`, `onChange`, `onFocus`, `onBlur`, `onSubmit`, `onSuccess`, `onError`
**Iteration**: `forEach`, `as`, `key`, `limit`, `offset`
**Navigation**: `href`, `external`, `screen`
**Modal**: `name`, `size`, `closable`, `modal`

### CEL Operators
`==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`, `+`, `-`, `*`, `/`, `%`, `? :`

### CEL Functions
`size()`, `contains()`, `startsWith()`, `endsWith()`, `filter()`, `map()`, `exists()`, `all()`, `trim()`, `split()`, `join()`, `toUpperCase()`, `toLowerCase()`, `int()`, `double()`, `string()`, `now()`, `timestamp()`

---

**End of Specification**

This is a living document. Update as implementation progresses.
