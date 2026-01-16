# AI Interface - Test Orchestration for Claude

## Overview

ai_interface is a test harness that allows Claude to:
- Spawn multiple app instances (slint_shell, kunki)
- Execute UI automation (click, type, get_text)
- Run Lua code to inspect/modify app state
- Access protocol-level logs from all libs (courier, butler, gurkha, transport)
- Orchestrate multi-instance scenarios (owner + customer sync)

## Setup

### Option 1: Pre-configured Databases (Recommended)

Use the setup tool to create databases with full e-commerce scenario already configured:

```bash
# Build everything with debug info (required for UI automation)
SLINT_EMIT_DEBUG_INFO=1 cargo build -p ai_interface -p slint_shell -p kunki -p integration_tests

# Create databases with full setup (owner + node + customer, app synced)
cargo run -p integration_tests --bin setup_test_dbs -- --db-dir /tmp/ai_test --passphrase test123

# Output:
#   /tmp/ai_test/shop_owner.db  - owner with space/app
#   /tmp/ai_test/kunki.db       - node with synced data
#   /tmp/ai_test/customer.db    - customer subscribed to app
```

Then use with ai_interface:

```bash
./target/debug/ai_interface \
  --instances shop_owner,customer \
  --node \
  --db-dir /tmp/ai_test
```

### Option 2: Fresh Databases

Start with fresh databases and set up via UI automation (more complex):

```bash
./target/debug/ai_interface --instances owner,customer --node --db-dir /tmp/ai_test_fresh
```

## Workflow: Login and Test (Pre-configured DBs)

When using pre-configured databases from `setup_test_dbs`:

```
1. ai_interface spawns instances
   ├─ shop_owner: shows "login" screen (already signed up)
   ├─ customer: shows "login" screen
   └─ kunki: already initialized with synced data

2. Claude logs in each instance (using direct login command):
   {"target": "shop_owner", "action": "login", "params": {"passphrase": "test123"}}
   → shop_owner now on "spaces" screen with My Shop space

3. Claude opens the app:
   {"target": "shop_owner", "action": "ui_click", "params": {"label": "my-shop-space"}}
   → app opens, ready to test

4. Subscribe to sync logs:
   {"action": "subscribe_logs", "params": {"target": "courier::.*", "level": "info"}}

5. Run tests via eval commands:
   {"target": "shop_owner", "action": "eval", "params": {"code": "add_product({name='Test'})"}}
   → watch logs for sync to customer
```

## Workflow: Modify App and Retest

App code is stored as Loro docs in the database. To test new code, delete DBs and re-run setup:

```bash
# 1. Claude modifies app code
#    (edits sample_apps/my-shop/*.lua)

# 2. Delete old databases and re-run setup with fresh code
rm -rf /tmp/ai_test/*
cargo run -p integration_tests --bin setup_test_dbs -- --db-dir /tmp/ai_test --passphrase test123

# 3. Restart ai_interface (it will use fresh DBs with new app code)
./target/debug/ai_interface --instances shop_owner,customer --node --db-dir /tmp/ai_test

# 4. Claude logs in and tests new behavior
```

This ensures:
- Fresh databases with new app code imported
- Clean sync state between owner/node/customer
- Same passphrase (test123) for login

## Command Format

All commands use JSON-RPC style format:

```json
{"target": "<instance>", "action": "<command>", "params": {...}, "id": <number>}
```

- `target`: Instance name ("owner", "customer", "kunki") or "all" for broadcast, omit for meta commands
- `action`: Command to execute
- `params`: Command-specific parameters (optional)
- `id`: Request ID for correlation (optional)

## Commands

### Meta Commands (no target)

#### List Instances
```json
{"action": "list_instances", "id": 1}
```
Response:
```json
{"result": {"instances": ["owner", "customer", "kunki"]}, "id": 1}
```

#### Get Logs (Historical)
```json
{"action": "get_logs", "params": {"count": 50}, "id": 2}
{"action": "get_logs", "params": {"count": 100, "level": "warn"}, "id": 3}
{"action": "get_logs", "params": {"target": "courier::.*", "message": "handshake"}, "id": 4}
```
Response:
```json
{"result": [{"ts": "...", "level": "INFO", "target": "courier::handshake", "msg": "...", "instance": "owner"}], "id": 2}
```

#### Subscribe to Log Stream
Subscribe to real-time filtered logs:
```json
{"action": "subscribe_logs", "params": {
  "instances": ["owner", "kunki"],
  "level": "info",
  "target": "courier::.*",
  "message": "handshake|permit"
}, "id": 5}
```
Response:
```json
{"result": {"subscribed": true}, "id": 5}
```

Logs stream as they arrive:
```json
{"log": {"ts": "...", "level": "INFO", "target": "courier::handshake", "msg": "peer connected", "instance": "owner"}}
{"log": {"ts": "...", "level": "INFO", "target": "courier::permit", "msg": "permit verified", "instance": "kunki"}}
```

Filter parameters (all optional):
- `instances`: Array of instance names to filter
- `level`: Log level ("trace", "debug", "info", "warn", "error")
- `target`: Regex pattern for module path (e.g., "courier::.*")
- `message`: Regex pattern for message content

#### Unsubscribe from Log Stream
```json
{"action": "unsubscribe_logs", "id": 6}
```

### Instance Commands

#### UI Automation

Get current screen:
```json
{"target": "owner", "action": "ui_get_screen", "id": 10}
```

Click a UI element:
```json
{"target": "owner", "action": "ui_click", "params": {"label": "new-user-button"}, "id": 11}
```

Type text into an input:
```json
{"target": "owner", "action": "ui_type", "params": {"label": "username-input", "text": "shop_owner"}, "id": 12}
```

Get text from an element:
```json
{"target": "owner", "action": "ui_get_text", "params": {"label": "error-message"}, "id": 13}
```

List all accessible UI elements (for debugging):
```json
{"target": "owner", "action": "ui_list_elements", "id": 14}
```

#### Direct Commands

Login (bypasses UI automation - recommended for reliable login):
```json
{"target": "owner", "action": "login", "params": {"passphrase": "test123"}, "id": 15}
```
Note: The `ui_type` command has a known limitation where `set_accessible_value` doesn't update Slint's TextInput binding. Use `login` command for reliable login.

#### Lua Evaluation

Execute Lua code in the instance:
```json
{"target": "owner", "action": "eval", "params": {"code": "return butler:get_spaces()"}, "id": 20}
```

#### Node Commands

Get connection string from kunki:
```json
{"target": "kunki", "action": "get_connection_string", "id": 30}
```

## Common Workflows

### 1. New User Signup Flow

```json
// Check we're on initiation screen
{"target": "owner", "action": "ui_get_screen", "id": 1}
// Response: {"result": {"screen": "initiation"}, "instance": "owner", "id": 1}

// Click new user button
{"target": "owner", "action": "ui_click", "params": {"label": "new-user-button"}, "id": 2}

// Enter username
{"target": "owner", "action": "ui_type", "params": {"label": "username-input", "text": "shop_owner"}, "id": 3}

// Enter passphrase
{"target": "owner", "action": "ui_type", "params": {"label": "passphrase-input", "text": "secure_pass_123"}, "id": 4}

// Click signup
{"target": "owner", "action": "ui_click", "params": {"label": "signup-button"}, "id": 5}

// Verify we reached spaces screen
{"target": "owner", "action": "ui_get_screen", "id": 6}
// Response: {"result": {"screen": "spaces"}, "instance": "owner", "id": 6}
```

### 2. Login Flow

```json
// Should be on login screen
{"target": "owner", "action": "ui_get_screen", "id": 1}
// Response: {"result": {"screen": "login"}, "instance": "owner", "id": 1}

// Use direct login command (recommended - bypasses UI automation limitation)
{"target": "owner", "action": "login", "params": {"passphrase": "secure_pass_123"}, "id": 2}
// Response: {"result": {"success": true, "did": "Login successful"}, "instance": "owner", "id": 2}

// Verify success - screen should now be "spaces"
{"target": "owner", "action": "ui_get_screen", "id": 3}
// Response: {"result": {"screen": "spaces"}, "instance": "owner", "id": 3}
```

### 3. Add Sovereign Node

```json
// Get connection string from kunki
{"target": "kunki", "action": "get_connection_string", "id": 1}
// Response: {"result": {"connection_string": "base64..."}, "instance": "kunki", "id": 1}

// Navigate to nodes screen (assuming logged in)
{"target": "owner", "action": "ui_click", "params": {"label": "nodes-tab"}, "id": 2}

// Click add node
{"target": "owner", "action": "ui_click", "params": {"label": "add-node-button"}, "id": 3}

// Paste connection string
{"target": "owner", "action": "ui_type", "params": {"label": "connection-string-input", "text": "<connection_string>"}, "id": 4}

// Submit
{"target": "owner", "action": "ui_click", "params": {"label": "add-node-submit"}, "id": 5}
```

### 4. Multi-Instance Sync Test

```json
// Subscribe to sync-related logs
{"action": "subscribe_logs", "params": {"target": "courier::sync", "level": "info"}, "id": 1}

// Owner creates a space
{"target": "owner", "action": "eval", "params": {"code": "return butler:create_space('My Shop')"}, "id": 2}

// Customer connects to owner
{"target": "customer", "action": "eval", "params": {"code": "return butler:connect_to_peer('<owner_did>')"}, "id": 3}

// Watch logs for sync events
// {"log": {"ts": "...", "level": "INFO", "target": "courier::sync", "msg": "space synced", "instance": "customer"}}

// Verify customer received the space
{"target": "customer", "action": "eval", "params": {"code": "return butler:get_spaces()"}, "id": 4}
```

## Log Filtering

Filter logs by:
- **level**: "trace", "debug", "info", "warn", "error"
- **target**: Regex on module path (e.g., "courier::handshake", "butler::.*")
- **message**: Regex on message content (e.g., "handshake|permit", "error")
- **instances**: Array of instance names (e.g., ["owner", "kunki"])

Examples:
```json
// All courier logs
{"action": "subscribe_logs", "params": {"target": "courier::.*"}, "id": 1}

// Warnings and errors from all instances
{"action": "subscribe_logs", "params": {"level": "warn"}, "id": 2}

// Handshake events from owner only
{"action": "subscribe_logs", "params": {"instances": ["owner"], "message": "handshake"}, "id": 3}
```

## Sending Commands (Named Pipe Mode)

```bash
# Send a command
echo '{"target": "owner", "action": "ui_get_screen", "id": 1}' > /tmp/ai_cmd

# Read the response
tail -1 /tmp/ai_out.log
```

## Tips

1. **Always check screen state** before UI interactions to avoid invalid operations
2. **Use log subscription** when debugging sync issues - watch for courier events
3. **Increase log count** if you need more history: `{"action": "get_logs", "params": {"count": 500}}`
4. **Filter by instance** when running multiple apps to reduce noise
5. **Use regex patterns** to match related log messages (e.g., "connect|disconnect|error")
