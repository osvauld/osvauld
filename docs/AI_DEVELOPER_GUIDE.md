# Osvauld AI Developer Guide

> Comprehensive documentation for AI agents to build, modify, debug, and test osvauld apps.

## Table of Contents

1. [Protocol Overview](#1-protocol-overview)
2. [Lua Runtime & APIs](#2-lua-runtime--apis)
3. [App Structure](#3-app-structure)
4. [CRDT Layer System](#4-crdt-layer-system)
5. [Permit System](#5-permit-system)
6. [Slint UI Patterns](#6-slint-ui-patterns)
7. [AI Automation API](#7-ai-automation-api)
8. [Debugging & Testing](#8-debugging--testing)
9. [Space & Page Hierarchy](#9-space--page-hierarchy)
10. [Asset Handling](#10-asset-handling)
11. [Ephemeral Events](#11-ephemeral-events)
12. [Persistence](#12-persistence)
13. [Quick Reference Tables](#13-quick-reference-tables)

---

## 1. Protocol Overview

### System Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Owner     │────▶│    Node     │◀────│  Customer   │
│ (shop app)  │     │ (always on) │     │ (shop app)  │
└─────────────┘     └─────────────┘     └─────────────┘
       │                   │                   │
       ▼                   ▼                   ▼
  products layer     runs derivation     orders/{did} layer
  (shared, list)     + validation        (private, list)
                           │
                           ▼
                    orders_summary layer
                    (derived, visible to owner)
```

### How Multi-User Apps Work

1. **Owner** creates space, publishes to node, shares link
2. **Node** runs 24/7, handles sync, runs validation + derivation
3. **Customers** connect via link, get their own private layers
4. **Derivation** aggregates private data into summaries for owner

### Key Mental Models

| Concept | Purpose | Example |
|---------|---------|---------|
| **Permits** | Capabilities (what you CAN do) | `operations.write: "allow"` |
| **Layers** | Data partitions (WHERE data lives) | `{page_id}/orders/{user_did}` |
| **Validation** | Business rules (what you SHOULD do) | Order state machine |
| **Derivation** | Computed views (aggregated data) | Orders summary from all customers |

### P2P Architecture

- User ↔ Node ↔ User (multi-peer, not fixed roles)
- Roles are **app-defined labels** - not hardcoded
- Capabilities determine permissions, not role names
- P2P sync via QUIC (iroh)

### Connection Handshake

```
User                           Node
  │── Hello + permit ──────────>│  (proves identity)
  │<── Welcome + permit ────────│  (node issues permit)
  │── PermitGrant ─────────────>│  (user issues permit for node)
  │<── Ack ─────────────────────│  (handshake complete)
```

---

## 2. Lua Runtime & APIs

### Core Lua APIs (6 modules)

| Module | Purpose | Context |
|--------|---------|---------|
| `loro:` | Layer access (CRDT operations) | All |
| `permit:` | Identity and context | All |
| `ui:` | UI binding | App runtime |
| `api:` | AI/test exports | App runtime |
| `derivation:` | Node-only transforms | Node only |
| `butler:` | System functions | All |

### loro: Layer Access

```lua
-- Get or create layers
local products = loro:get_or_create_layer(page_id .. "/products", "list")
local settings = loro:get_or_create_layer(page_id .. "/settings", "map")

-- List operations
products:push({ id = uuid(), name = "Widget", price = 99 })
products:get(0)                     -- Get by index (0-based)
products:set(0, updated_product)    -- Update at index
products:delete(0)                  -- Remove at index
products:length()                   -- Count items

-- Map operations
settings:set("theme", "dark")
settings:get("theme")               -- Returns "dark"
settings:delete("theme")
settings:keys()                     -- Returns table of keys
settings:length()                   -- Count keys

-- Layer discovery
local layers = loro:list_layers(page_id .. "/orders/*")
local layer = loro:get_layer(layer_name, "list")  -- Get existing (nil if not found)
```

### permit: Context Information

```lua
local page_id = permit:page_id()      -- Current page ID
local my_did = permit:my_did()        -- My DID (did:key:...)
local role = permit:role()            -- App-defined role (owner/customer/etc)

-- Construct per-user layer name
local my_orders = permit:my_layer("orders")  -- Returns: {page_id}/orders/{my_did}
```

### ui: UI Binding

```lua
-- Set properties (syncs to Slint AppAPI)
ui:set("products", products_array)
ui:set("form_visible", true)

-- Get properties
local current_tab = ui:get("current_tab")

-- Update specific item in array (surgical update)
ui:update("products", index, updated_product)

-- Clear/insert for arrays
ui:clear("products")
ui:insert("products", 0, new_product)
ui:remove("products", index)
```

### api: Function Export for AI/Tests

```lua
-- Export function for AI/test access
api.export("add_product", function(name, price, desc, stock)
    -- Same code path as human UI
    on_modal_action("add_product", "open")
    on_field_changed("product_name", name)
    on_field_changed("product_price", tostring(price))
    on_field_changed("product_desc", desc or "")
    on_field_changed("product_stock", tostring(stock or 10))
    on_modal_action("add_product", "submit")
end)

-- Add description metadata
api.describe("add_product", {
    description = "Add product via UI flow",
    params = {
        {name = "name", type = "string"},
        {name = "price", type = "number"},
    },
    effects = {"products layer updated"},
})

-- Call exported function
api.call("add_product", "Widget", 99)

-- List available functions
local functions = api.list()
```

### derivation: Node-Only Transforms

**Why derivation exists:**
- Each user has their own private layer (e.g., `orders/did:key:alice`)
- Owner needs to see ALL orders aggregated
- Node computes derived views without exposing private data

**Example: E-commerce orders summary** (from `sample_apps/my-shop/shared/init.lua`):

```lua
local page_id = permit:page_id()

derivation:register({
    -- Source pattern: all per-customer order layers
    source = page_id .. "/orders/*",

    -- Target: aggregated summary layer
    target = page_id .. "/derived/orders_summary",

    -- Key function: extract order ID for derived map
    key_fn = function(order)
        return order.id
    end,

    -- Transform: create summary entry from full order
    transform = function(source_layer, order)
        return {
            id = order.id,
            customer = extract_did(source_layer),
            status = order.status,
            total = order.total,
            created_at = order.created_at,
        }
    end,

    -- Filter: only include submitted orders (not drafts)
    filter = function(order)
        return order.status ~= "draft"
    end,
})
```

**Example: Privacy-preserving calendar** (from `sample_apps/my-booking/shared/init.lua`):

```lua
derivation:register({
    source = page_id .. "/bookings/*",
    target = page_id .. "/derived/calendar",

    key_fn = function(booking)
        return booking.date .. "_" .. booking.start_time
    end,

    transform = function(source_layer, booking)
        return {
            date = booking.date,
            start_time = booking.start_time,
            end_time = booking.end_time,
            booked = true
            -- PRIVACY: customer_name, phone, notes are STRIPPED
        }
    end,

    filter = function(booking)
        return booking.status == "confirmed"
    end,
})
```

### butler: System Functions

```lua
-- Send ephemeral message (fire-and-forget, not persisted)
butler:send_ephemeral('{"type":"cursor","x":100,"y":200}')

-- Asset utilities
local asset_url = butler:asset_url(asset_id)
```

### Event Handlers

```lua
function on_init()
    -- App startup - initialize layers, load data
    page_id = permit:page_id()
    products_layer = loro:get_or_create_layer(page_id .. "/products", "list")
    refresh_ui()
end

function on_loro_change(layer_name, change_type)
    -- Layer data changed - update UI
    if layer_name == page_id .. "/products" then
        refresh_products_ui()
    end
end

function on_layer_discovered(layer_name)
    -- New layer found (e.g., customer's orders layer)
    if layer_name:match("/orders/") then
        track_order_layer(layer_name)
    end
end

function on_click(target)
    -- UI button clicked
    if target == "add_product" then
        on_modal_action("add_product", "open")
    end
end

function on_field_changed(field_name, value)
    -- Form input changed
    form_state[field_name] = value
end

function on_modal_action(modal_name, action)
    -- Modal open/close/submit
    if modal_name == "add_product" and action == "submit" then
        do_add_product()
    end
end

function on_ephemeral(user_did, payload)
    -- Real-time event (cursor, typing indicator)
    local data = json.decode(payload)
    update_remote_cursor(user_did, data.x, data.y)
end

function tick()
    -- Game loop (if tick_enabled in manifest)
    update_game_state()
end
```

### Validation (shared/validation.lua)

Validation runs on the node before syncing updates:

```lua
-- Order state machine example (from my-shop)
local ORDER_STATES = {
    draft = {
        writable = {'items', 'quantity', 'notes', 'total'},
        customer_transitions = {'pending'},
        owner_transitions = {}
    },
    pending = {
        writable = {'notes'},
        customer_transitions = {'cancelled'},
        owner_transitions = {'confirmed', 'cancelled'}
    },
    confirmed = {
        writable = {},
        customer_transitions = {},
        owner_transitions = {'shipped'}
    },
    shipped = {
        writable = {},
        customer_transitions = {},
        owner_transitions = {'delivered'}
    },
    delivered = { writable = {}, customer_transitions = {}, owner_transitions = {} },
    cancelled = { writable = {}, customer_transitions = {}, owner_transitions = {} }
}

--- Main validation entry point
function validate_ops(layer_name, ops, from_did, role, page_id)
    -- Dispatch to layer-specific validators
    if layer_name == "products" then
        if role ~= "owner" then
            return false, "Only owner can modify products"
        end
        return true, nil
    end

    if layer_name:match("/orders/") then
        return validate_orders_ops(ops, from_did, role, ...)
    end

    return true, nil  -- Default allow
end
```

---

## 3. App Structure

### Full Directory Structure

```
my-app/
├── shared/                      # Executed based on config
│   ├── manifest.json           # Library metadata
│   ├── validation.lua          # Business rules (runs on node)
│   └── init.lua                # Derivation registration (runs on node)
├── app-owner/                  # Role variant
│   ├── manifest.json           # Entry points
│   ├── app.lua                 # Business logic
│   └── app.slint               # UI definition
├── app-customer/               # Another role
│   ├── manifest.json
│   ├── app.lua
│   └── app.slint
├── permit_template.json        # Page-level permits
└── space_permit_template.json  # Space-level permits
```

### Role Manifest (e.g., `shop-owner/manifest.json`)

```json
{
    "name": "Shop Owner",
    "version": "1.0.0",
    "description": "E-commerce shop owner dashboard",
    "entry_ui": "app.slint",
    "entry_logic": "app.lua",
    "models": ["products", "orders"]
}
```

### Reference Apps

| App | Location | Features |
|-----|----------|----------|
| **my-shop** | `sample_apps/my-shop/` | E-commerce, orders, derivation, multi-role |
| **canvas-app** | `sample_apps/canvas-app/` | Collaboration, shapes, connectors, remote cursors |
| **my-booking** | `sample_apps/my-booking/` | Time slots, scheduling, privacy |
| **photo-gallery** | `sample_apps/photo-gallery/` | Asset handling, galleries |

---

## 4. CRDT Layer System

### Loro CRDT Types

| Type | Behavior | Use Case |
|------|----------|----------|
| **LoroList** | Ordered, interleaving conflicts | Products, orders, messages |
| **LoroMap** | LWW (last writer wins) | Settings, derived summaries |
| **LoroText** | Character-level merge | Documents, chat |
| **LoroTree** | Parent-child relationships | *Future use* |

### Layer Naming Conventions

```
{page_id}/products           → shared data (list)
{page_id}/orders/{user_did}  → per-user partitioned (list)
{page_id}/derived/*          → computed (node-only, map)
{page_id}/drafts             → local-only (sync: false)
{page_id}/assets             → file metadata (map)
app:Shop Owner               → app state layer
```

### Layer Access Patterns

```lua
-- Shared layer (all users see same data)
local products = loro:get_or_create_layer(page_id .. "/products", "list")

-- Per-user layer (each user has their own)
local my_orders = loro:get_or_create_layer(page_id .. "/orders/" .. my_did, "list")

-- Local-only layer (never syncs, for drafts)
local drafts = loro:get_or_create_layer(page_id .. "/drafts", "map")

-- Derived layer (read-only for clients, node writes)
local summary = loro:get_layer(page_id .. "/derived/orders_summary", "map")
```

### Sync Protocol

1. **SyncOffer** - Peer sends state vector + updates
2. **SyncAccept** - Other peer responds with their state vector
3. **SyncAck** - Confirmation that sync completed

---

## 5. Permit System

### UCAN-Based Capabilities

Permits use UCAN tokens with these fields:

| Field | Purpose |
|-------|---------|
| `operations` | Actions allowed (own, read, write, share) |
| `peer_capabilities` | P2P capabilities (relay, share, accept_publish) |
| `layers` | Fixed layer access with type/sync/write |
| `layer_patterns` | Dynamic patterns with wildcards |
| `issue_on` | Delegation templates for child roles |
| `relationship` | Role label (owner, customer, node) |

### Permit Delegation Chain

```
Owner (operations.own: allow)
  └── issues → Node Token (relay: true)
                  └── issues → Customer Token (layer_patterns: {page_id}/orders/{aud})
                  └── issues → Admin Token (write products)
```

### permit_template.json Schema

From `sample_apps/my-shop/permit_template.json`:

```json
{
    "owner_template": {
        "operations": {
            "own": "allow",
            "read": "allow",
            "write": "allow",
            "share": "allow",
            "share_page": "allow"
        },
        "peer_capabilities": {
            "relay": false,
            "share": true,
            "accept_publish": true
        },
        "layers": {
            "{page_id}/products": {
                "type": "list",
                "sync": true,
                "write": true
            },
            "{page_id}/derived/orders_summary": {
                "type": "map",
                "sync": true,
                "write": false
            },
            "{page_id}/drafts": {
                "type": "map",
                "sync": false,
                "create": true
            }
        },
        "layer_patterns": {
            "{page_id}/orders/*": {
                "sync": true,
                "write": true
            }
        },
        "issue_on": {
            "node": {
                "token_type": "page_share",
                "peer_capabilities": { "relay": true },
                "layers": { ... },
                "issue_on": {
                    "viewer": {
                        "layer_patterns": {
                            "{page_id}/orders/{aud}": {
                                "write": true,
                                "sync": true
                            }
                        },
                        "relationship": "customer"
                    }
                }
            }
        }
    }
}
```

**Key pattern:** `{page_id}/orders/{aud}` - `{aud}` expands to the token audience (customer's DID), giving each customer their own layer.

### Adding New Roles

Roles are added via `permit_template.json` - no code changes needed:

```json
"issue_on": {
    "node": { ... },
    "customer": { ... },
    "supplier": {
        "token_type": "page_viewer",
        "layers": {
            "{page_id}/inventory": { "sync": true, "write": true }
        },
        "relationship": "supplier"
    }
}
```

---

## 6. Slint UI Patterns

### AppAPI Global

Every app exposes an `AppAPI` global for Lua ↔ UI binding:

```slint
// Data structures
export struct Product {
    id: string,
    name: string,
    price: float,
    description: string,
    stock: int,
}

export struct Order {
    id: string,
    customer_did: string,
    items: string,
    status: string,
    total: float,
}

// App API - Lua accesses this directly
export global AppAPI {
    // Data arrays (Lua sets via ui:set)
    in-out property<[Product]> products: [];
    in-out property<[Order]> orders: [];

    // Form state (two-way binding)
    in-out property<string> form_product_name: "";
    in-out property<bool> form_modal_visible: false;
    in-out property<int> current_tab: 0;

    // Event callbacks
    callback on_click(string);
    callback on_field_changed(string, string);
    callback on_modal_action(string, string);
}
```

### Two-Way Binding

```slint
export component App inherits Rectangle {
    // Bind to AppAPI (both UI and Lua can modify)
    property<string> new-product-name <=> AppAPI.form_product_name;

    Rectangle {
        TextInput {
            text <=> new-product-name;
            edited => { AppAPI.on_field_changed("product_name", self.text); }
        }
    }
}
```

### Theme Pattern

```slint
global Theme {
    out property<color> bg-primary: #0f172a;
    out property<color> bg-secondary: #1e293b;
    out property<color> text-primary: #f1f5f9;
    out property<color> accent: #3b82f6;
    out property<color> success: #22c55e;
    out property<color> warning: #f59e0b;
    out property<color> danger: #ef4444;
}

global StatusColors {
    pure function get-color(status: string) -> color {
        if status == "pending" { return #f59e0b; }
        if status == "confirmed" { return #3b82f6; }
        if status == "delivered" { return #22c55e; }
        return #64748b;
    }
}
```

---

## 7. AI Automation API

### Socket-Based Control (Primary Method)

AI agents interact with running apps via Unix sockets:

```python
from lib.client import ControlClient

# Connect to running app
client = ControlClient("/tmp/test/owner.sock")

# Execute Lua code and get results
result = client.eval('return get_products_count()')
client.eval('add_product("Widget", 99)')

# App commands
client.signup_or_login("alice", "pass123")
client.create_space_with_pages("./sample_apps/my-shop", "My Shop")
client.list_spaces()
client.open_app(page_id, "Shop Owner")

# Hot reload (changes reflected on all peers)
client.refresh_app("Shop Owner", "./sample_apps/my-shop/shop-owner")
```

### ControlClient API Reference

**Auth:**
```python
client.ping()                           # Check server responding
client.signup(username, passphrase)     # Register new user
client.login(passphrase)                # Login
client.signup_or_login(username, pass)  # Signup if needed, then login
```

**Space/Page/App:**
```python
client.create_space_with_pages(path, name)  # Create space + import pages
client.list_spaces()                        # List all spaces
client.list_pages(space_id)                 # List pages in space
client.list_apps(page_id)                   # List apps in page
client.open_app(page_id, app_name)          # Open app
client.refresh_app(app_name, app_dir)       # Hot reload single app
client.refresh_page(page_dir)               # Hot reload all apps
```

**P2P:**
```python
client.connect_to_node(node)            # Connect owner to node
client.publish_to_node(space_id)        # Publish space
client.get_viewer_link(space_id)        # Get shareable link
client.add_viewer(viewer_client, space_id)  # Add viewer
client.get_connection_string()          # This instance's connection
client.p2p_status()                     # Check P2P ready
```

**Lua Execution:**
```python
client.eval(code)                       # Execute Lua, return result
```

**UI Automation:**
```python
client.ui_click(label)                  # Click by accessible-label
client.ui_type(label, text)             # Type into input
client.ui_get_text(label)               # Read text
client.ui_get_screen()                  # Current screen name
client.ui_list_elements()               # List all UI elements
```

**Debugging:**
```python
client.get_logs(last=100, level=None)   # Get recent logs
client.get_state()                      # State snapshot
client.get_app_status()                 # App loading status
```

**Assets:**
```python
client.upload_asset(page_id, file_path) # Upload file
```

### Exporting Functions from Lua

```lua
-- Export with same code path as human UI
api.export("add_product_via_ui", function(name, price, desc, stock)
    on_modal_action("add_product", "open")
    on_field_changed("product_name", name)
    on_field_changed("product_price", tostring(price))
    on_field_changed("product_desc", desc or "")
    on_field_changed("product_stock", tostring(stock or 10))
    on_modal_action("add_product", "submit")
end)

-- Helper functions for AI
api.export("get_products_count", function()
    return products_layer:length()
end)

api.export("get_first_product_id", function()
    local first = products_layer:get(0)
    return first and first.id or nil
end)
```

---

## 8. Debugging & Testing

### Debugging Order of Operations

1. **Socket first**: Use `client.eval()` to inspect state
2. **Check sync**: Use `wait_for_sync()` to verify peer state
3. **Logs via tmux**: Only if socket doesn't reveal issue

### TmuxManager for Multi-Instance Testing

```python
from lib.tmux import TmuxManager

# Setup instances
tm = TmuxManager(session_name="test", base_dir=Path("/tmp/test"))
tm.add_node("node")
tm.add_shell("owner")
tm.add_shell("viewer")
tm.start()

# Get clients
node = tm.get_client("node")
owner = tm.get_client("owner")
viewer = tm.get_client("viewer")

# Run test flow
owner.signup_or_login("owner")
space = owner.create_space_with_pages("./sample_apps/my-shop")
owner.connect_to_node(node)
owner.publish_to_node(space["id"])
owner.add_viewer(viewer, space["id"])
```

### Wait Helpers

```python
from lib.wait import wait_for_condition, wait_for_sync

# Wait for condition
def check_products():
    return customer.eval("return get_products_count()") >= 1

wait_for_condition(check_products, timeout=30, desc="product sync")

# Wait for data sync between peers
wait_for_sync(
    source=owner,
    target=customer,
    code="return get_products_count()",
    timeout=30
)
```

### tmux Log Inspection

```bash
# Attach to running session
tmux attach -t test

# Switch windows: Ctrl-b + window_number
# Window 0: node logs
# Window 1: owner app
# Window 2: viewer app

# Scroll in pane: Ctrl-b + [
# Exit scroll mode: q
```

### Log Levels

| Level | Use Case | Production |
|-------|----------|------------|
| `trace!` | Wire-level: bytes, serialization | Filtered out |
| `debug!` | Implementation details: cache hits | Filtered out |
| `info!` | Business events: connections, sync | Visible |
| `warn!` | Recoverable issues: retry, fallback | Visible |
| `error!` | Failures requiring attention | Visible |

### Common Issues & Solutions

**1. Permit denied**
- Symptom: Sync fails, "not authorized" errors
- Check: `layer_patterns` in permit_template.json
- Fix: Verify `{page_id}/orders/{aud}` expansion matches layer name

**2. State divergence (peers have different data)**
- Symptom: Data appears on owner but not customer
- Check: `client.eval("return loro:list_layers('*')")` on both sides
- Check: P2P status with `client.p2p_status()`
- Fix: Force re-sync or check validation rejections

**3. Connection issues**
- Symptom: Handshake failures, timeouts
- Check: Node is running and reachable
- Check: Connection string is valid
- Fix: Verify `client.get_connection_string()` works

**4. Validation failed**
- Symptom: Updates rejected on node
- Check: `validation.lua` logic for role/state
- Check: State machine allows transition
- Fix: Ensure state transitions are valid per role

**5. Derivation not running**
- Symptom: Derived layer empty or stale
- Check: `init.lua` registered rules correctly
- Check: Source layer has data
- Fix: `derivation:rebuild("target_layer")` to force rebuild

### Example Test Script

```python
#!/usr/bin/env python3
"""E-commerce sync test - from scripts/test_ecommerce_sync.py"""

from lib.tmux import TmuxManager

SHOP_APP = Path("./sample_apps/my-shop")

def main():
    # Start instances
    tm = TmuxManager(session_name="ecomm_test")
    tm.add_node("node")
    tm.add_shell("owner")
    tm.add_shell("customer")
    tm.start()

    node = tm.get_client("node")
    owner = tm.get_client("owner")
    customer = tm.get_client("customer")

    # Owner setup
    owner.signup_or_login("owner")
    space = owner.create_space_with_pages(str(SHOP_APP))
    page_id = space["pages"][0]["page_id"]

    # Connect and publish
    owner.connect_to_node(node)
    owner.publish_to_node(space["id"])

    # Customer setup
    customer.signup_or_login("customer")
    owner.add_viewer(customer, space["id"])

    # Open apps
    owner.open_app(page_id, "Shop Owner")
    customer.open_app(page_id, "Shop Customer")

    # Owner adds product
    owner.eval('add_product_via_ui("Widget", 99, "A test product", 50)')

    # Wait for sync
    wait_for_condition(
        lambda: customer.eval("return get_products_count()") >= 1,
        timeout=15
    )

    print("Product synced to customer!")
```

---

## 9. Space & Page Hierarchy

### Structure

```
Space (container - grants access to all pages within)
├── parent_space_id (optional, for nesting)
├── space_permit (container-level access)
├── Pages[]
│   ├── Layers[] (CRDT data)
│   ├── page_permit (page-specific access)
│   └── encrypted_key (per-user AES-256)
└── Permits cascade: space_permit → can delegate page_permits
```

### Key Operations

```python
# List root spaces (no parent)
spaces = client.list_spaces()

# List pages in space
pages = client.list_pages(space_id)

# Create space with pages
space = client.create_space_with_pages("./sample_apps/my-shop", "My Shop")
```

### Encryption Overview

- Each page has unique AES-256 key (encrypts ALL its layers)
- Key stored encrypted per-user via X25519 ECIES
- Node re-encrypts when relaying to viewers
- AI doesn't manage keys directly - handled by herald crate

---

## 10. Asset Handling

### Storage Pattern

- Asset ID = Blake3 hash of plaintext
- Stored as `{base_path}/{asset_id}.enc`
- Metadata synced via CRDT layer `{page_id}/assets`

### Sync Flow

1. Owner uploads → encrypts → stores locally
2. Metadata syncs via layer (MapInsert)
3. Node requests missing blobs via `AssetPrepare`
4. Owner decrypts → sends via iroh-blobs
5. Node re-encrypts for viewers → broadcasts

### API

```python
# Upload asset
result = client.upload_asset(page_id, "/path/to/image.jpg")
asset_id = result["asset_id"]

# In Lua, get asset URL
local url = butler:asset_url(asset_id)
```

---

## 11. Ephemeral Events

### Use Cases

- Cursors
- Typing indicators
- Presence

### Characteristics

- Fire-and-forget (QUIC datagram)
- Never persisted
- Opaque payload (app defines format)

### API

```lua
-- Send
butler:send_ephemeral('{"type":"cursor","x":100,"y":200}')

-- Receive
function on_ephemeral(user_did, payload_string)
    local data = json.decode(payload_string)
    if data.type == "cursor" then
        update_remote_cursor(user_did, data.x, data.y)
    end
end
```

---

## 12. Persistence

### Redb Tables

| Table | Key Format | Value |
|-------|------------|-------|
| LAYERS | `{page_id}/{layer}` | Encrypted Loro snapshot |
| VECTORS | `{page_id}/{did}/{device}` | Peer state vectors |
| PERMITS | `{page_id}/{did}` | Permit string |
| SPACES | `{space_id}` | SpaceData (bincode) |
| PAGES | `{page_id}` | PageData (bincode) |

### Flush Cycle

- Every 10s: dirty layers → redb
- Every 30s: reconciliation check

---

## 13. Quick Reference Tables

### Lua API Quick Reference

| API | Method | Returns |
|-----|--------|---------|
| `loro:` | `get_or_create_layer(name, type)` | Layer object |
| `loro:` | `list_layers(pattern)` | Table of names |
| `permit:` | `page_id()` | String |
| `permit:` | `my_did()` | String (did:key:...) |
| `permit:` | `role()` | String (owner/customer/...) |
| `permit:` | `my_layer(type)` | String ({page_id}/{type}/{did}) |
| `ui:` | `set(key, value)` | nil |
| `ui:` | `get(key)` | Value |
| `ui:` | `update(key, index, value)` | nil |
| `api:` | `export(name, fn)` | nil |
| `api:` | `call(name, ...)` | Any |
| `derivation:` | `register({...})` | nil |
| `butler:` | `send_ephemeral(payload)` | nil |

### ControlClient Quick Reference

| Category | Method | Description |
|----------|--------|-------------|
| Auth | `signup_or_login(user, pass)` | Create/login user |
| Space | `create_space_with_pages(path)` | Create space + import |
| Space | `list_spaces()` | List all spaces |
| Space | `list_pages(space_id)` | List pages |
| App | `open_app(page_id, name)` | Open app |
| App | `refresh_app(name, dir)` | Hot reload |
| P2P | `connect_to_node(node)` | Connect to node |
| P2P | `publish_to_node(space_id)` | Publish space |
| P2P | `add_viewer(client, space_id)` | Add viewer |
| Lua | `eval(code)` | Execute Lua |
| UI | `ui_click(label)` | Click element |
| Debug | `get_logs()` | Get recent logs |

### Event Handlers Quick Reference

| Handler | When Called | Parameters |
|---------|-------------|------------|
| `on_init()` | App startup | None |
| `on_loro_change(layer, type)` | Layer data changed | layer_name, change_type |
| `on_layer_discovered(name)` | New layer found | layer_name |
| `on_click(target)` | Button clicked | target_id |
| `on_field_changed(field, val)` | Form input changed | field_name, value |
| `on_modal_action(modal, action)` | Modal event | modal_name, action |
| `on_ephemeral(did, payload)` | Real-time event | user_did, payload_string |
| `tick()` | Game loop | None (if tick_enabled) |
| `validate_ops(...)` | Before sync (node) | layer, ops, from_did, role, page_id |

### Key Files Reference

| Component | File |
|-----------|------|
| Layer wrapper | `butler/src/models/layer.rs` |
| Scribe actor | `butler/src/scribe/mod.rs` |
| Lua runtime | `butler/src/scribe/lua_runtime.rs` |
| Lua bindings | `butler/src/runtime/bindings/` |
| Permit parser | `gurkha/src/parser.rs` |
| Permit service | `gurkha/src/service.rs` |
| Sync protocol | `courier/src/peer_actor/sync.rs` |
| Sample apps | `sample_apps/my-shop/`, `canvas-app/`, `my-booking/` |
| Python client | `scripts/lib/client.py` |
| Python tmux | `scripts/lib/tmux.py` |
| Test examples | `scripts/test_ecommerce_sync.py`, `test_canvas_sync.py` |

---

## Appendix: Minimal App Template

### Directory Structure

```
my-simple-app/
├── shared/
│   └── manifest.json
├── my-app/
│   ├── manifest.json
│   ├── app.lua
│   └── app.slint
└── permit_template.json
```

### shared/manifest.json

```json
{
    "name": "shared",
    "description": "Shared library"
}
```

### my-app/manifest.json

```json
{
    "name": "My App",
    "version": "1.0.0",
    "description": "Simple app",
    "entry_ui": "app.slint",
    "entry_logic": "app.lua",
    "models": ["items"]
}
```

### my-app/app.lua

```lua
local page_id = nil
local items_layer = nil

function on_init()
    page_id = permit:page_id()
    items_layer = loro:get_or_create_layer(page_id .. "/items", "list")
    refresh_ui()
end

function on_loro_change(layer_name, change_type)
    if layer_name == page_id .. "/items" then
        refresh_ui()
    end
end

function refresh_ui()
    local items = {}
    for i = 0, items_layer:length() - 1 do
        table.insert(items, items_layer:get(i))
    end
    ui:set("items", items)
end

function on_click(target)
    if target == "add_item" then
        items_layer:push({
            id = tostring(os.time()),
            name = "New Item",
            created_at = os.date()
        })
    end
end

-- AI/Test exports
api.export("get_items_count", function()
    return items_layer:length()
end)

api.export("add_item", function(name)
    items_layer:push({
        id = tostring(os.time()),
        name = name,
        created_at = os.date()
    })
end)
```

### my-app/app.slint

```slint
export struct Item {
    id: string,
    name: string,
    created_at: string,
}

export global AppAPI {
    in-out property<[Item]> items: [];
    callback on_click(string);
}

export component App inherits Rectangle {
    background: #1e293b;

    VerticalLayout {
        padding: 20px;
        spacing: 10px;

        Text {
            text: "My Simple App";
            color: white;
            font-size: 24px;
        }

        Button {
            text: "Add Item";
            clicked => { AppAPI.on_click("add_item"); }
        }

        for item in AppAPI.items: Rectangle {
            height: 40px;
            background: #334155;
            border-radius: 4px;

            Text {
                text: item.name;
                color: white;
            }
        }
    }
}
```

### permit_template.json (minimal)

```json
{
    "owner_template": {
        "operations": {
            "own": "allow",
            "read": "allow",
            "write": "allow",
            "share": "allow"
        },
        "peer_capabilities": {
            "relay": false,
            "share": true,
            "accept_publish": true
        },
        "layers": {
            "{page_id}/items": {
                "type": "list",
                "sync": true,
                "write": true
            },
            "app:My App": {
                "type": "map",
                "sync": true,
                "write": true
            }
        }
    }
}
```

---

*This guide is designed for AI agent consumption. For human-readable documentation, see the other files in `/docs/`.*
