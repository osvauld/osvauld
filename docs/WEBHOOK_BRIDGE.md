# Webhook Bridge

A lightweight osvauld peer that bridges external HTTP webhooks into the P2P mesh. It receives webhook POSTs from services like GitHub, Stripe, Jira, etc., and forwards them to handler functions in osvauld apps running on shared pages.

## Core Concept

The webhook bridge is **an independent user** — not a node, not an extension of kunki. It has its own identity (DID), connects to a kunki node as a regular peer (`CourierMode::User`), and participates in pages it's been invited to. It is intentionally lightweight because it's just another participant.

It ships with a **built-in Lua app** (the router) that handles all incoming HTTP traffic. Routes are configured via CRDT layers from a separate sthalam UI app. When a webhook arrives, the router looks up the config, then **forwards the payload to a named handler function in another page's Lua runtime**.

## Architecture

```
GitHub/Stripe/etc
        |
        | HTTP POST
        v
┌──────────────────────────────────────────────────┐
│  webhook_bridge binary                           │
│                                                  │
│  ┌────────────┐   ┌──────────────────────────┐   │
│  │ Axum HTTP  │   │ Router Lua Runtime       │   │
│  │ catch-all  │──>│ (built-in, webhook page) │   │
│  │            │   │                          │   │
│  │ Parses     │   │ on_webhook(req)          │   │
│  │ body to    │   │   lookup route config    │   │
│  │ Lua table  │   │   return routing decision│   │
│  └────────────┘   └───────────┬──────────────┘   │
│                               │                  │
│                    routing decision:              │
│                    page_id, handler_func, payload │
│                               │                  │
│                  ┌────────────v───────────────┐   │
│                  │ Rust Dispatch Layer        │   │
│                  │                            │   │
│                  │ Looks up Lua runtime for   │   │
│                  │ target page_id             │   │
│                  │ Sends LuaCommand to it     │   │
│                  └────────────┬───────────────┘   │
│                               │                  │
│                  ┌────────────v───────────────┐   │
│                  │ Target Page Lua Runtime    │   │
│                  │ (e.g., group-chat app)     │   │
│                  │                            │   │
│                  │ handle_pr(payload)         │   │
│                  │   format message           │   │
│                  │   scribe:map(...):set(...) │   │
│                  └───────────────────────────┘   │
│                                                  │
│  ┌─────────┐  ┌──────────┐  ┌────────────────┐  │
│  │ Herald  │  │ Butler   │  │ Courier        │  │
│  │Identity │  │ Storage  │  │ P2P (User mode)│  │
│  └─────────┘  └──────────┘  └────────────────┘  │
└──────────────────────────────────────────────────┘
        |                           |
        | QUIC/P2P                  | sync
        v                           v
   [kunki node] <────sync────> [sthalam on laptop]
                                    |
                               Webhook Config app
                               (Slint UI for routes)
```

## Two Apps, Loosely Coupled via CRDT

### 1. Router App (built into the webhook_bridge binary)

The bridge binary embeds its own Lua app. This app:
- Reads route config from the `config/routes` CRDT layer
- Receives all HTTP requests via the `on_webhook` callback
- Matches the request path against configured routes
- Returns a routing decision: which page, which handler function, what payload
- Writes to `webhooks/log` for observability

This app runs against the **webhook config page's** scribe. It does not load Lua from the page's `app:` layers — its code is baked into the binary.

### 2. Webhook Config App (standard Lua+Slint app for sthalam)

A regular osvauld app installed in sthalam. Provides a Slint UI for:
- Managing routes (add, edit, delete, enable/disable)
- Viewing webhook logs
- Selecting target pages and handler functions from dropdowns
- Managing webhook secrets

This app writes to the same `config/routes` CRDT layer that the router reads. Changes sync automatically — no restart of the bridge needed.

## User Workflow

### Initial Setup

1. **Initialize the bridge:**
   ```
   $ webhook_bridge init --username "github-bot"
     Generated identity: did:key:z6MkWn...
     Stored in ./webhook_data/
   ```

2. **In sthalam — create a webhook config page:**
   - Install the "Webhook Config" app
   - Create a new page using the webhook permit template
   - This page holds route config and webhook logs

3. **In sthalam — invite the bridge:**
   - Share the webhook config page with `did:key:z6MkWn...` as a collaborator
   - Also share any target pages (e.g., a group-chat page) with the bridge's DID

4. **Start the bridge:**
   ```
   $ webhook_bridge start --node <connection_string> --http-port 9090
     Connected to node
     Discovered 2 pages (webhook-config, team-chat)
     Router running on webhook-config page
     Loaded app for team-chat page (2 webhook handlers available)
     Listening on 0.0.0.0:9090
   ```

5. **In sthalam — configure routes:**
   - Open the webhook config page
   - Add a route:
     - Path: `/github`
     - Source: `github`
     - Secret: `whsec_abc...`
     - Target page: Team Chat
     - Handler: `handle_pr`

6. **Configure GitHub:**
   - Webhook URL: `http://your-server:9090/github`
   - Secret: `whsec_abc...`
   - Events: Pull requests

### Steady State

- PR opened on GitHub
- GitHub POSTs to `http://your-server:9090/github`
- Bridge router Lua matches path → route config
- Bridge Rust layer dispatches to team-chat page's Lua runtime
- `handle_pr(payload)` runs, writes a message to `channels/dev/messages`
- Message syncs to all peers — appears in everyone's group-chat

## HTTP Handling

### Thread Model

The binary runs two thread types:

```
Tokio async runtime              Lua OS threads (one per page)
├── Transport (QUIC)             ├── Router runtime (built-in app)
├── Courier (P2P sync)           ├── Page A runtime (loaded from app: layers)
├── Scribe actors (ractor)       └── Page B runtime (loaded from app: layers)
├── Axum HTTP server
│     │
│     │ LuaCommand::WebhookReceived (via mpsc)
│     └──────────────────────────────────────────> Router Lua thread
│                                                       │
│     LuaCommand::WebhookDispatch (via mpsc)            │ routing decision
│     <─────────────────────────────────────────────────┘
│     │
│     └──────────────────────────────────────────> Target page Lua thread
│
│     oneshot response
│     <──────────────────────────────────────────  Target page Lua thread
│
└── Returns HTTP response
```

### Axum Setup

Axum does **zero routing**. All routing logic lives in Lua. Axum is a dumb pipe:

```rust
let app = axum::Router::new()
    .fallback(forward_to_lua);
```

The catch-all handler:
1. Extracts path, method, headers, body
2. Parses body as `serde_json::Value`
3. Sends `LuaCommand::WebhookReceived` to the router Lua runtime
4. Awaits oneshot response
5. Returns HTTP response

### Body Handling

The HTTP body is parsed from JSON to `serde_json::Value` on the Rust side, then converted to a native Lua table via the existing `json_to_lua()` conversion. **No JSON binding is needed in Lua** — the payload arrives as a regular table:

```lua
function on_webhook(req)
    -- req.body is already a Lua table, not a string
    local pr = req.body.pull_request
    local title = pr.title  -- direct access
end
```

## Request Lifecycle (Detailed)

```
1. HTTP POST /github arrives

2. Axum handler (tokio):
   - Extract: path="/github", method="POST", headers={...}
   - Parse body: serde_json::from_slice(bytes) → Value
   - Create oneshot channel (response_tx, response_rx)
   - Send LuaCommand::WebhookReceived {
       path, method, headers,
       body: Value,
       response_tx,
     } to router's cmd_tx
   - Await response_rx

3. Router Lua thread (step() loop picks up command):
   - Rust converts Value → Lua table via json_to_lua()
   - Builds request table: { path="/github", method="POST", headers={...}, body={...} }
   - Calls on_webhook(request_table)
   - Lua returns: {
       forward = true,
       page_id = "chat_abc",
       handler = "handle_pr",
       payload = req.body,
     }
     OR: { status = 404, body = "no matching route" }
   - If forward=true, Rust receives routing decision

4. Rust dispatch layer:
   - Looks up Lua runtime cmd_tx for page "chat_abc"
   - Creates new oneshot channel
   - Sends LuaCommand::WebhookDispatch {
       handler_name: "handle_pr",
       payload: lua_to_json(payload) → Value,
       response_tx,
     }
   - Awaits response

5. Target page Lua thread (step() picks up command):
   - Rust converts Value → Lua table
   - Looks up _G["handle_pr"]
   - Calls handle_pr(payload_table)
   - Handler writes to scribe, returns result
   - Sends response back via response_tx

6. Rust receives response, forwards to original axum oneshot
7. Axum returns HTTP response to the webhook sender
```

## Route Configuration

### CRDT Layer: `config/routes`

A LoroMap where each key is a route ID and each value is a route config:

```lua
-- Written by the sthalam Webhook Config app
-- Read by the bridge's built-in router app
{
    ["route_001"] = {
        path = "/github",
        source = "github",              -- source type (for signature verification)
        secret = "whsec_abc...",         -- webhook secret
        target_page_id = "page_abc123",  -- which page to forward to
        handler_func = "handle_pr",      -- function name in that page's app
        enabled = true,
    },
    ["route_002"] = {
        path = "/stripe/payments",
        source = "stripe",
        secret = "whsec_def...",
        target_page_id = "page_def456",
        handler_func = "handle_payment",
        enabled = true,
    },
}
```

### Sthalam Config UI

```
Routes:
┌──────────────────┬──────────┬─────────────────┬─────────────────┬────────┐
│ Path             │ Source   │ Target Page     │ Handler         │ Status │
├──────────────────┼──────────┼─────────────────┼─────────────────┼────────┤
│ /github          │ GitHub   │ Team Chat       │ handle_pr       │ active │
│ /stripe/payments │ Stripe   │ Finance Feed    │ handle_payment  │ active │
│ /jira            │ Generic  │ Project Board   │ handle_generic  │ paused │
└──────────────────┴──────────┴─────────────────┴─────────────────┴────────┘
                                                          [+ Add Route]

Add Route:
  Path:     [/github                ]
  Source:   [GitHub              ▼]
  Secret:   [•••••••••••••••••••  ]
  Target:   [Team Chat Page      ▼]   ← pages the bridge is invited to
  Handler:  [handle_pr           ▼]   ← handlers that page declares
  [Save]
```

## Handler Discovery

Target apps declare webhook-compatible handler functions so the config UI knows what to show in the handler dropdown. Three approaches (not mutually exclusive):

### Option A: Manifest Declaration

The app's `manifest.json` declares available webhook handlers:

```json
{
    "name": "group-chat",
    "webhook_handlers": [
        { "name": "handle_pr", "description": "GitHub pull request events" },
        { "name": "handle_issue", "description": "GitHub issue events" },
        { "name": "handle_generic", "description": "Raw webhook payload" }
    ]
}
```

### Option B: Convention

Any Lua function prefixed with `webhook_` is treated as a webhook handler:

```lua
function webhook_handle_pr(payload)  -- discoverable
function webhook_handle_issue(payload)  -- discoverable
function internal_helper(x)  -- not discoverable
```

### Option C: CRDT Registration

On startup, the page's Lua runtime writes its available handlers to a well-known layer:

```lua
function on_init()
    local meta = scribe:map("__webhook_handlers")
    meta:set("handle_pr", { description = "GitHub PR events" })
    meta:set("handle_issue", { description = "GitHub issue events" })
end
```

The config UI reads this layer to populate the handler dropdown.

## Target App Integration

Any osvauld app can become webhook-compatible by defining handler functions. Example for group-chat:

```lua
-- In group-chat app code (loaded by the bridge from app: layers)

function handle_pr(payload)
    local action = payload.action
    local pr = payload.pull_request

    local text = ""
    if action == "opened" then
        text = string.format("%s opened PR #%d: %s\n%s",
            pr.user.login, pr.number, pr.title, pr.html_url)
    elseif action == "closed" and pr.merged then
        text = string.format("PR #%d merged: %s", pr.number, pr.title)
    elseif action == "closed" then
        text = string.format("PR #%d closed: %s", pr.number, pr.title)
    end

    if text ~= "" then
        send_message("dev", text)
    end
end

function handle_issue(payload)
    local action = payload.action
    local issue = payload.issue

    if action == "opened" then
        local text = string.format("New issue #%d: %s\n%s",
            issue.number, issue.title, issue.html_url)
        send_message("general", text)
    end
end

-- Helper (used by handlers, same as regular chat message sending)
function send_message(channel, text)
    local period = clock:day()
    local layer_name = "channels/" .. channel .. "/messages/" .. period
    local messages = scribe:map(layer_name)
    local msg_id = generate_id()

    messages:set(msg_id, {
        id = msg_id,
        sender_did = scribe:my_did(),
        sender_name = scribe:my_name() or "Webhook Bot",
        text = text,
        timestamp = clock:count(),
        deleted = false,
        edited = false,
        thread_parent_id = "",
        thread_reply_count = 0,
        reply_to = "",
        reply_preview = "",
        reactions = {},
        attachment_hash = "",
        attachment_name = "",
    })
end
```

## Built-in Router App

Embedded in the webhook_bridge binary. Runs against the webhook config page's scribe.

```lua
-- Built into the binary. This IS the router.

local routes = {}

function on_init()
    load_routes()
end

function load_routes()
    routes = {}
    local config = scribe:map("config/routes")
    for _, key in ipairs(config:keys()) do
        local r = config:get(key)
        if r.enabled then
            routes[r.path] = r
        end
    end
end

-- Called whenever the config layer changes (user edited routes in sthalam)
-- The bridge should reload routes on layer change events.

function on_webhook(req)
    local route = routes[req.path]
    if not route then
        log_webhook(req, nil, "no_match")
        return { status = 404, body = "no matching route" }
    end

    -- Signature verification
    if route.secret and route.secret ~= "" then
        if not verify_signature(req, route) then
            log_webhook(req, route, "bad_signature")
            return { status = 401, body = "signature verification failed" }
        end
    end

    -- Forward to target page
    log_webhook(req, route, "forwarded")
    return {
        forward = true,
        page_id = route.target_page_id,
        handler = route.handler_func,
        payload = req.body,
    }
end

function verify_signature(req, route)
    if route.source == "github" then
        local sig = req.headers["x-hub-signature-256"]
        if not sig then return false end
        local expected = "sha256=" .. hmac.sha256(route.secret, req.raw_body)
        return sig == expected
    elseif route.source == "stripe" then
        -- Stripe signature verification
        return true  -- TODO
    end
    return true  -- generic: no verification
end

function log_webhook(req, route, status)
    local log = scribe:map("webhooks/log")
    local entry_id = generate_id()
    log:set(entry_id, {
        timestamp = clock:count(),
        path = req.path,
        method = req.method,
        route_id = route and route.id or "",
        status = status,
        source_ip = req.headers["x-forwarded-for"] or "",
    })
end
```

## Permit Template

### Webhook Config Page

```json
{
    "owner_template": {
        "layers": [
            {
                "name": "config/routes",
                "type": "map",
                "sync": true,
                "write": true
            },
            {
                "name": "config/general",
                "type": "map",
                "sync": true,
                "write": true
            },
            {
                "name": "webhooks/log",
                "type": "map",
                "sync": true,
                "write": true
            }
        ],
        "issue_on": {
            "collaborator": {
                "layers": [
                    {
                        "name": "config/routes",
                        "permissions": ["read"]
                    },
                    {
                        "name": "webhooks/log",
                        "permissions": ["read", "write"]
                    }
                ]
            }
        }
    }
}
```

Target pages (e.g., group-chat) use their own existing permit templates. The bridge is invited as a collaborator with write access to the relevant layers.

## New Crate: `webhook_bridge`

### Dependencies

```
webhook_bridge
├── herald        (identity)
├── butler        (minimal redb storage)
├── transport     (QUIC via iroh)
├── courier       (P2P sync, CourierMode::User)
├── scribe        (CRDT read/write)
├── lua_runtime   (headless Lua, ui_enabled: false)
├── axum          (HTTP listener — near-zero new deps, hyper already in tree)
├── clap          (CLI)
└── tokio         (async runtime)
```

### Binary Structure

```
webhook_bridge/
├── Cargo.toml
└── src/
    ├── main.rs           # CLI: init / start subcommands
    ├── config.rs         # Startup config (port, data dir, node connection)
    ├── bootstrap.rs      # Peer bootstrap (identity, butler, transport, courier)
    ├── server.rs         # Axum catch-all → LuaCommand bridge
    ├── dispatch.rs       # Cross-page Lua runtime dispatch (the complex part)
    ├── runtime_manager.rs # Manages Lua runtimes per discovered page
    └── router_app.lua    # Embedded via include_str!()
```

### CLI

```
webhook_bridge init --username <name> [--data-dir <path>]
    Generate identity, store in redb.
    Prints DID for sharing.

webhook_bridge start --node <connection_string> --http-port <port> [--data-dir <path>]
    Connect to node, discover pages, start router, listen for HTTP.
```

## Changes to `lua_runtime` Crate

### New LuaCommand Variants

```rust
/// Sent by axum to the router Lua runtime
LuaCommand::WebhookReceived {
    path: String,
    method: String,
    headers: HashMap<String, String>,
    body: serde_json::Value,    // parsed JSON, converted to Lua table
    raw_body: String,           // raw bytes for signature verification
    response_tx: oneshot::Sender<WebhookResponse>,
}

/// Sent by the dispatch layer to a target page's Lua runtime
LuaCommand::WebhookDispatch {
    handler_name: String,           // e.g., "handle_pr"
    payload: serde_json::Value,     // forwarded from the router
    response_tx: oneshot::Sender<WebhookResponse>,
}
```

### New Recognized Callback

Add `on_webhook` to the `HandlerCache` (same pattern as `on_ephemeral`, `on_peer_joined`):

```rust
// In HandlerCache
has_on_webhook: bool,
```

### New Lua Binding: `hmac`

For webhook signature verification:

```lua
local sig = hmac.sha256(secret_key, message_body)
-- Returns hex-encoded HMAC-SHA256 digest
```

Implemented as a Rust binding wrapping the `hmac` + `sha2` crates.

## Complexity Map

| Component | Complexity | Notes |
|-----------|-----------|-------|
| Peer bootstrap | Low | Follows kunki pattern exactly |
| Axum catch-all | Low | ~30 lines, dumb pipe |
| `LuaCommand::WebhookReceived` | Low | One new variant, standard pattern |
| `on_webhook` callback | Low | Same as existing callbacks |
| `hmac` Lua binding | Low | Thin wrapper over hmac crate |
| Router Lua app (built-in) | Low | Config lookup + signature check |
| Sthalam config app (UI) | Medium | Standard Slint app, route CRUD |
| **Loading page app code** | **Medium** | Bridge syncs `app:` layers, loads Lua from shared pages |
| **Cross-page Lua dispatch** | **High** | Multiple runtime management, routing decisions, response plumbing |
| **Handler discovery** | **Medium** | Apps declaring handlers, surfacing to config UI |

The dispatch layer (`dispatch.rs` + `runtime_manager.rs`) is the core complexity. Everything else follows established patterns in the codebase.
