# HUML Rendering System

Custom GPU-accelerated rendering engine for HUML templates with query-based data access and event-driven reactivity.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     HUML Template                           │
│            (declarative UI + CEL expressions)               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    OCaml Parser                             │
│  • Parse HUML → JSON (ParsedTemplate)                       │
│  • Extract CEL expressions + dependencies                   │
│  • huml_native.exe (native) / huml_wasm (browser)          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              HUML Renderer (Rust Runtime)                   │
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ Dependency   │  │ CEL Runtime  │  │ Render Tree      │  │
│  │ Graph        │  │ (cel_wasi)   │  │ (retained)       │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
│                                                             │
│  • Build dependency graph from parsed template              │
│  • Track dirty nodes on data changes                        │
│  • Re-evaluate only affected CEL expressions                │
│  • Re-render only dirty UI nodes                            │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Taffy (Layout)                         │
│             Flexbox layout engine - pure Rust               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Vello (Rendering)                      │
│     GPU-accelerated 2D renderer via wgpu                    │
│     Shadows, rounded corners, text, shapes                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      winit (Windowing)                      │
│              Cross-platform window management               │
│              Single shared EventLoop for all windows        │
└─────────────────────────────────────────────────────────────┘
```

## Crate Structure

| Crate | Purpose |
|-------|---------|
| `huml_parser` | Rust wrapper around OCaml HUML parser |
| `cel_runtime` | CEL expression evaluator (calls OCaml cel_wasi) |
| `huml_renderer` | Core renderer: dependency graph, render tree, event handling |
| `query_bridge` | Data layer: connects renderer to Scribe for persistence |
| `tauri_huml_plugin` | Tauri integration: window management, multi-window support |

## Data Flow

### Template Loading

```
1. Frontend calls invoke('open_huml_with_template', { huml_content, page_id })
2. Rust parses HUML via huml_parser (OCaml native executable)
3. If page_id provided:
   - Open Scribe via Butler.open_page(page_id)
   - Fetch initial data for each query in template
   - Create QueryBridge for live updates
4. Create HumlRenderer with template + initial data
5. tauri_huml_plugin creates window via shared EventLoop
```

### Event-Driven Rendering

```
User Action (click/input)
        │
        ▼
on_button_click / on_input_change
        │
        ▼
Evaluate CEL stateUpdates
        │
        ├──► local() → Update sources, invalidate deps
        │
        └──► commit() → Update sources + send to Scribe
                │
                ▼
        Scribe persists to Loro layer
                │
                ▼
        QueryDelta broadcast to subscribers
                │
                ▼
        QueryBridge receives delta
                │
                ▼
        Renderer updates sources, rebuilds dirty nodes
                │
                ▼
        window.request_redraw()
```

### Dependency Tracking

CEL expressions declare dependencies on data sources:

```yaml
computed:
  messageCount: "${ size(messages) }"  # depends on: messages
  hasMessages: "${ messageCount > 0 }" # depends on: messageCount
```

When `messages` changes:
1. Invalidate `messageCount` (direct dependency)
2. Invalidate `hasMessages` (transitive dependency)
3. Re-evaluate only these two expressions
4. Re-render only UI nodes that use these values

## Multi-Window Architecture

winit only allows one EventLoop per process. We use a single shared EventLoop managed by `tauri_huml_plugin`:

```rust
// Global manager (singleton)
static MANAGER: OnceLock<ManagerHandle> = OnceLock::new();

// Communication via channels
pub enum ManagerRequest {
    CreateWindow { label, template, cel, query_bridge, ... },
    CloseWindow { label },
    UpdateData { label, field, value },
    Navigate { label, screen },
}

// Manager runs on dedicated thread
struct HumlWindowManager {
    windows: HashMap<WindowId, HumlWindowState>,
    render_cx: RenderContext,  // Shared Vello context
    request_rx: Receiver<ManagerRequest>,
}
```

Creating a new window:
```rust
// From any thread
let handle = get_manager();
let (reply_tx, reply_rx) = oneshot::channel();
handle.send(ManagerRequest::CreateWindow { ... , reply: reply_tx });
let window_label = reply_rx.await?;
```

## Query System

Templates define queries for data access:

```yaml
queries:
  messages:
    layer: "chat_layer"      # Scribe layer name
    path: "messages"         # JSON path in layer
    sort_by: "timestamp"
    sort_order: "desc"
    limit: 100
```

### QueryBridge

Bridges HumlRenderer to Scribe:

```rust
// Spawn bridge actor
let (scribe_tx, delta_rx) = spawn_scribe_bridge(scribe_ref);
let (handle, _) = spawn_query_bridge(scribe_tx, delta_rx);

// Handle receives deltas from Scribe
// Renderer polls for changes and updates accordingly
```

### Commit Flow

CEL `commit()` function marks values for persistence:

```yaml
stateUpdates:
  messages: "${ commit(messages + [new_message]) }"
```

1. CEL returns `{ "__commit__": true, "value": [...] }`
2. Renderer detects marker, updates local sources
3. Sends `QueryBridgeMsg::Commit` to QueryBridge
4. QueryBridge forwards to Scribe as layer update
5. Scribe persists and broadcasts delta

## CEL Runtime

### Evaluation

CEL expressions evaluated via OCaml executable:

```rust
pub struct CelEvaluator {
    executable: PathBuf,  // cel_wasi.exe
}

impl CelEvaluator {
    pub fn evaluate(&self, expr: &str, context: &Value) -> Result<Value>;
}
```

### Standard Library

| Function | Purpose |
|----------|---------|
| `commit(value)` | Mark value for Scribe persistence |
| `local(value)` | Local-only update (no persistence) |
| `generateId()` | Generate UUID |
| `now()` | Current Unix timestamp |
| `size(array)` | Array length |
| `string(value)` | Convert to string |

### Dataflow Functions

Special functions for template reactivity:

| Function | Purpose |
|----------|---------|
| `__push(arr, item)` | Append to array |
| `__filter(arr, condition)` | Filter array |
| `__map(arr, transform)` | Transform array |
| `__get(obj, key)` | Safe property access |

## Render Tree

Retained-mode render tree built from template:

```rust
pub struct RenderNode {
    pub id: NodeId,
    pub kind: RenderNodeKind,
    pub style: NodeStyle,
    pub layout_node: taffy::NodeId,
    pub children: Vec<NodeId>,
}

pub enum RenderNodeKind {
    Container,
    Text { content: String },
    Input { value: String, placeholder: String },
    Button { label: String, action_id: String },
    Loop { template: Box<Block>, items_expr: String },
}
```

Only dirty nodes rebuild on data changes, minimizing work per frame.

## Vello Rendering

GPU-accelerated via wgpu:

```rust
fn render_node(&self, scene: &mut Scene, node: &RenderNode, bounds: Rect) {
    // Background with rounded corners
    if let Some(bg) = &node.style.background {
        scene.fill(Fill::NonZero, Affine::IDENTITY, bg, None,
            &RoundedRect::from_rect(bounds, node.style.corner_radius));
    }

    // Shadow
    if let Some(shadow) = &node.style.shadow {
        // Render shadow slightly offset and blurred
    }

    // Text
    if let RenderNodeKind::Text { content } = &node.kind {
        scene.draw_glyphs(&font)
            .brush(&text_color)
            .draw(&layout, bounds.origin());
    }
}
```

## File Locations

### OCaml (template-transpiler/)
- `parser-bin/huml_native.ml` - HUML parser native executable
- `eval-bin/cel_wasi.ml` - CEL evaluator executable
- `cel/stdlib_registry.ml` - CEL standard library

### Rust Crates
- `huml_parser/src/lib.rs` - Parser wrapper
- `cel_runtime/src/lib.rs` - CEL evaluator wrapper
- `huml_renderer/src/lib.rs` - Core renderer
- `huml_renderer/src/dep_graph.rs` - Dependency tracking
- `huml_renderer/src/render_tree.rs` - Retained render tree
- `query_bridge/src/lib.rs` - Scribe data bridge
- `tauri_huml_plugin/src/plugin.rs` - Tauri window integration

### Tauri Integration
- `sthalam/src-tauri/src/lib.rs` - `open_huml_with_template` command
- `tauri_handlers/src/handlers/window.rs` - Window handlers

## Performance Characteristics

| Aspect | Approach |
|--------|----------|
| CEL evaluation | Only dirty expressions (not all every frame) |
| Rendering | Event-driven, not every-frame |
| Layout | Taffy flexbox, only recalculated on tree changes |
| GPU | Vello batches draws, single GPU submission |
| Data | Query-based with pagination, not full dataset |
| Memory | Retained tree, no per-frame allocations |

## Usage Example

```typescript
// Frontend (Svelte/TypeScript)
const windowLabel = await invoke('open_huml_with_template', {
  input: {
    huml_content: template,
    title: 'Chat',
    page_id: currentPageId  // For Scribe persistence
  }
});
```

```yaml
# chat.huml
template: Chat
version: "1.0"

queries:
  messages:
    layer: "chat"
    path: "messages"
    sort_by: "timestamp"
    sort_order: "desc"

documents:
  newMessage:
    type: "string"
    initial: ""

ui:
  publisher:
    - screen: main
      root:
        type: vstack
        children:
          - type: loop
            each: "${ messages }"
            as: "msg"
            template:
              type: text
              text: "${ msg.content }"

          - type: input
            bind: newMessage
            placeholder: "Type a message..."

          - type: button
            label: "Send"
            action: sendMessage

actions:
  sendMessage:
    stateUpdates:
      messages: |
        ${ commit(messages + [{
          id: generateId(),
          content: newMessage,
          timestamp: now()
        }]) }
      newMessage: "${ local('') }"
```
