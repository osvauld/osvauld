# HUML Template Guide V3 - Publisher/Viewer Architecture

## Overview

HUML (Human Markup Language) templates define the structure, state, and behavior of applications with separate Publisher and Viewer modes. Each mode has its own UI, state, and interactions while sharing common content.

## Core Structure

```yaml
name: "Template Name"
resourceType: "website"

# Shared persistent content (stored in contentDoc)
content::
  key: value
  posts:: [...]

# Publisher mode template
publisher::
  state:: {...}      # Publisher UI state
  computed:: {...}   # Publisher computed values
  screens:: [...]    # Publisher screens

# Viewer mode template
viewer::
  state:: {...}      # Viewer UI state
  computed:: {...}   # Viewer computed values
  screens:: [...]    # Viewer screens
```

## 1. Content Section (Shared)

The `content::` section contains data that both Publisher and Viewer modes can access. This is stored in the Loro `contentDoc` and persists across sessions.

```yaml
content::
  # Static content
  title: "My App"

  # Dynamic content (Publisher can modify)
  posts::
    - ::
      id: "post-1"
      content: "Hello World"
      author: "Publisher"
      timestamp: 1704067200000
```

### Key Points:
- **Read-only for Viewers** - Only Publishers can modify content
- **Persistent** - Saved to database
- **Shared** - Both modes access the same data
- **Accessed via** - `{{content.posts}}` in expressions

## 2. Publisher Template

The Publisher section defines the UI for content creators/administrators:

```yaml
publisher::
  # Publisher-specific UI state (session only)
  state::
    newPostContent: ""
    isPublishing: false
    editingPostId: ""

  # Computed values derived from state
  computed::
    canPublish: "{{newPostContent is not empty and not isPublishing}}"
    postsCount: "{{size of content.posts}}"

  # Publisher screens
  screens::
    - ::
      id: "main"
      name: "Publisher Dashboard"
      isEntryPoint: true
      blocks:: [...]
```

### Publisher Capabilities:
- Create/Edit/Delete content
- Access analytics
- Manage settings
- Cannot interact with viewer features (comments, likes)

## 3. Viewer Template

The Viewer section defines the UI for end users:

```yaml
viewer::
  # Viewer-specific UI state (session only)
  state::
    selectedPostId: ""
    newCommentContent: ""
    likedPosts:: []

  # Computed values
  computed::
    hasSelectedPost: "{{selectedPostId != ''}}"
    canComment: "{{newCommentContent != ''}}"

  # Viewer screens
  screens::
    - ::
      id: "main"
      name: "Feed"
      isEntryPoint: true
      blocks:: [...]
```

### Viewer Capabilities:
- View content (read-only)
- Add comments (stored in collaborativeDoc)
- Like/interact with content
- Personal state (stored in userContentDoc)

## 4. State Management

### State Types:

1. **content** - Shared persistent data (contentDoc)
2. **publisher.state** - Publisher UI state (uiStateDoc)
3. **viewer.state** - Viewer UI state (uiStateDoc)
4. **userContent** - Per-user persistent state (userContentDoc)
5. **threads** - Collaborative comments (collaborativeDoc)

### Accessing State in Expressions:

```yaml
# Access content (both modes)
"{{content.posts[0].title}}"

# Access mode-specific state
"{{newPostContent}}"  # Accesses current mode's state

# Access user content
"{{userContent.preferences.theme}}"

# Access threads
"{{threads['post-1'].comments}}"
```

## 5. Block Types

### Core Blocks:

```yaml
# Text block
- ::
  type: "text"
  content: "{{post.title}}"
  css: "font-size: 16px;"

# Container
- ::
  type: "section-container"
  css: "padding: 20px;"
  blocks:: [...]

# Heading
- ::
  type: "heading"
  content: "Dashboard"
  level: 1

# Form
- ::
  type: "form"
  id: "post-form"
  blocks:: [...]

# Form field
- ::
  type: "form-field-text"
  name: "title"
  stateKey: "newTitle"
  placeholder: "Enter title"
  formId: "post-form"

# Button
- ::
  type: "nav-button"
  content: "Submit"
  action: "publishPost"
  formId: "post-form"
  disabled: "{{!canSubmit}}"

# Thread (comments)
- ::
  type: "thread"
  id: "thread-1"
  eventName: "post-1"

# Canvas pattern (animated backgrounds)
- ::
  type: "canvas-pattern"
  id: "animated-bg"
  css: "width: 100%; height: 300px;"
```

**Canvas Pattern Usage:**
Canvas patterns render mathematical expressions as animated visuals. The expression is determined by `selectedPattern` state variable:

```yaml
state::
  selectedPattern: "waves"        # Looks for state.wavesExpr
  wavesExpr: "(sin(x * 0.2 + time) + sin(y * 0.2 + time)) / 2"
```

The canvas evaluates the expression for each grid cell at 60+ FPS.

## 6. Actions

### Publisher Actions:
- `publishPost` - Create new content
- `updatePost` - Edit existing content
- `deletePost` - Remove content
- `setState` - Update UI state

### Viewer Actions:
- `addComment` - Add to thread
- `likePost` - Like interaction
- `setState` - Update UI state

### Action Examples:

```yaml
# Publisher publishing content
- ::
  type: "nav-button"
  content: "Publish"
  action: "publishPost"
  formId: "new-post-form"

# Viewer adding comment
- ::
  type: "nav-button"
  content: "Comment"
  action: "addComment"
  eventName: "post-{{post.id}}"
```

## 7. Expressions

HUML uses template expressions with `{{...}}` syntax, powered by an OCaml-compiled WebAssembly evaluator for sub-millisecond performance.

### Logical Operators (Human-Readable):
- `and` - Logical AND
- `or` - Logical OR
- `not` - Logical NOT

### Comparison Operators:
- `equals` - Equality check
- `not equals` - Inequality check
- `greater than` / `>` - Greater than
- `less than` / `<` - Less than
- `at least` / `>=` - Greater than or equal
- `at most` / `<=` - Less than or equal

### State Checks:
- `is empty` - Check if string/array is empty
- `is not empty` - Check if string/array has content
- `in` - Check if item is in array

### Collection Functions:
- `size of array` - Get array length
- `length of string` - Get string length

### Arithmetic:
- `+`, `-`, `*`, `/` - Standard arithmetic operators
- **Note**: Modulo `%` operator not supported in canvas expressions

### Math Functions:
- `sin(x)` - Sine function
- `cos(x)` - Cosine function
- `tan(x)` - Tangent function
- `sqrt(x)` - Square root
- `abs(x)` - Absolute value
- `pow(base, exponent)` - Power function
- `atan2(y, x)` - Arc tangent of y/x
- `min(a, b)` - Minimum of two values
- `max(a, b)` - Maximum of two values
- `floor(x)` - Round down
- `ceil(x)` - Round up
- `round(x)` - Round to nearest integer

**Important**: Use `pow(x, 2)` instead of `x^2` - the `^` power operator is **not supported**.

### Canvas Pattern Expressions:

Canvas patterns use mathematical expressions with special variables:
- `x`, `y` - Grid coordinates (0 to gridSize)
- `time` - Animation time in seconds
- `gridSize` - Canvas grid dimensions
- `mouseX`, `mouseY` - Mouse position (when tracked)

```yaml
# Example: Traveling dot
state::
  selectedPattern: "dot"
  dotExpr: "max(0, 1 - sqrt(pow(x - sin(time * 0.5) * gridSize / 2, 2) + pow(y - gridSize / 2, 2)) / 5)"

# Example: Animated waves
state::
  selectedPattern: "waves"
  wavesExpr: "(sin(x * 0.2 + time) + sin(y * 0.2 + time)) / 2"
```

### Performance: Canvas vs CSS Animations

**Canvas Pattern Approach:**
- ⚠️ Evaluates expression for **every pixel** in grid
- Grid 80×80 = 6,400 evaluations per frame
- Complex expressions with trig → 60-70ms per frame (~15 FPS)
- Use for: Patterns, effects that need pixel-level control

**CSS Animation Approach (Recommended for simple elements):**
- ✅ Evaluates only position/transform expressions (2-3 per frame)
- <1ms evaluation time → smooth 60 FPS
- Hardware-accelerated rendering via browser compositor
- Use for: Moving elements, rotating objects, mouse-reactive UI

```yaml
# CSS approach: Single animated block (fast!)
- ::
  type: "section-container"
  id: "falling-block"
  css: "position: fixed;
        top: {{(time * 50 - floor(time * 50 / 100) * 100)}}vh;
        left: {{mouseX}}vw;
        width: 8px;
        height: 8px;
        background: rgba(255, 255, 255, 0.4);
        transform: rotate({{time * 300}}deg);
        transition: left 0.1s ease-out;"
  blocks:: []
```

**Mouse Tracking:**
Mouse position is automatically tracked and available as `mouseX`, `mouseY` (0-100% of viewport).

**Performance Comparison:**

| Metric | Canvas Pattern | CSS Animation |
|--------|---------------|---------------|
| OCaml evaluations/frame | 6,400 | 3 |
| Evaluation time | 60-70ms | <1ms |
| Rendering | CPU (software) | GPU (hardware) |
| FPS | ~15 | 60 |
| Best for | Patterns, grids | Individual elements |

**Rule of thumb:** Use CSS animations for individual elements, canvas patterns for full-screen effects.

### Examples:

```yaml
# Conditional visibility (human-readable)
visible: "{{selectedPostId equals post.id}}"

# Logical combinations
disabled: "{{title is empty or isSubmitting}}"

# Computed text
content: "{{size of content.posts}} posts published"

# Complex conditions
enabled: "{{length of newPostContent > 0 and not isPublishing}}"

# Dynamic CSS
css: "opacity: {{isActive ? 1 : 0.5}};"

# Ternary operator
content: "{{count > 10 ? 'many' : 'few'}} items"
```

### Performance Note:
Expressions are evaluated by an OCaml-powered WASM engine, providing:
- < 1ms evaluation time per expression
- 60 FPS smooth animations with 1000+ expressions
- Type-safe compilation
- See `WHY_OCAML.md` for technical details

## 8. Iteration

Use `forEach` and `forEachAs` for lists:

```yaml
- ::
  type: "section-container"
  forEach: "content.posts"
  forEachAs: "post"
  blocks::
    - ::
      type: "text"
      content: "{{post.title}}"
```

### Nested Iteration:

```yaml
forEach: "posts"
forEachAs: "post"
blocks::
  - ::
    forEach: "post.comments"
    forEachAs: "comment"
    blocks::
      - ::
        content: "{{comment.text}}"
```

## 9. Styling

CSS can be static or dynamic:

```yaml
# Static CSS
css: "padding: 20px; background: white;"

# Dynamic CSS with expressions
css: "{{\"opacity: \" + (isActive ? \"1\" : \"0.5\") + \";\"}}"

# Multi-line CSS
css: |
  padding: 20px;
  background: white;
  border-radius: 8px;
```

## 10. Best Practices

### 1. Separation of Concerns
- Keep Publisher and Viewer templates completely separate
- Don't use mode checks in expressions
- Each mode should be self-contained

### 2. State Management
- Use `content::` for shared persistent data
- Use mode-specific `state::` for UI state
- Use `computed::` for derived values

### 3. Performance
- Minimize expression complexity
- Use computed values for complex calculations
- Leverage forEach for lists instead of manual indexing

### 4. Naming Conventions
- camelCase for state variables
- kebab-case for IDs
- Descriptive names for actions

### 5. Reusability
- Create consistent patterns for common UI elements
- Use forEach for repeating structures
- Keep styling modular

## Example: Complete Social Feed

```yaml
name: "Social Feed"
resourceType: "website"

content::
  posts:: [...]

publisher::
  state::
    newPostContent: ""
    isPublishing: false

  computed::
    canPublish: "{{newPostContent is not empty and not isPublishing}}"

  screens::
    - ::
      id: "dashboard"
      blocks::
        - ::
          type: "form"
          id: "new-post"
          blocks::
            - ::
              type: "form-field-textarea"
              stateKey: "newPostContent"
            - ::
              type: "nav-button"
              content: "Publish"
              action: "publishPost"
              disabled: "{{!canPublish}}"

viewer::
  state::
    selectedPostId: ""
    newCommentContent: ""

  screens::
    - ::
      id: "feed"
      blocks::
        - ::
          forEach: "content.posts"
          forEachAs: "post"
          blocks::
            - ::
              type: "text"
              content: "{{post.content}}"
            - ::
              type: "thread"
              eventName: "post-{{post.id}}"
```

## Document Storage

The template data is stored across multiple Loro CRDT documents:

1. **templateDoc** - Template structure (Tree)
2. **contentDoc** - Shared content (Map)
3. **userContentDoc** - Per-user state (Map)
4. **uiStateDoc** - Session UI state (Tree)
5. **collaborativeDoc** - Comments/threads (Map of Trees)
6. **submissionsDoc** - Form submissions (Map of Lists)

This architecture ensures:
- **Separation** - Publisher and Viewer UIs are independent
- **Performance** - Only load what's needed
- **Clarity** - Clear data flow and ownership
- **Scalability** - Easy to extend with new modes or features