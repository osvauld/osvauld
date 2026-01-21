# AI Interface & Testing Automation

This directory contains example test scenarios for automated UI testing with AI agents.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     AI Interface Binary                         │
│                    (ai_interface crate)                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐           │
│  │    owner     │  │   customer   │  │    kunki    │           │
│  │  (Slint UI)  │  │  (Slint UI)  │  │  (headless) │           │
│  │      +       │  │      +       │  │      +      │           │
│  │DebugServer   │  │DebugServer   │  │DebugServer  │           │
│  └──────┬───────┘  └──────┬───────┘  └──────┬──────┘           │
│         │                 │                 │                   │
│         └─────────────────┴─────────────────┘                   │
│                           │                                     │
│                    Unix Sockets                                 │
└───────────────────────────┼─────────────────────────────────────┘
                            │
                         Claude
```

## Running Tests

### 1. Build the AI interface

```bash
cargo build -p ai_interface
```

### 2. Start the AI interface with instances

```bash
./target/debug/ai_interface \
  --instances owner,customer \
  --node \
  --db-dir /tmp/test_dbs
```

### 3. Send commands via JSON (stdin)

```json
{"target": "owner", "action": "ui_click", "params": {"label": "new-user-button"}, "id": 1}
{"target": "owner", "action": "eval", "params": {"code": "permit:role()"}, "id": 2}
{"action": "logs", "params": {"last": 50}, "id": 3}
```

## Debug Commands

### UI Automation

| Command | Description |
|---------|-------------|
| `ui_click` | Click element by accessible-label |
| `ui_type` | Type text into element |
| `ui_get_text` | Get text from element |

### Lua Evaluation

| Command | Description |
|---------|-------------|
| `eval` | Execute Lua code and return result |
| `state` | Get current state snapshot |
| `reload` | Reload app code from disk |

### Logs

| Command | Description |
|---------|-------------|
| `logs` | Get recent logs with filtering |
| `subscribe_logs` | Stream logs in real-time |

### Meta Commands

| Command | Description |
|---------|-------------|
| `list_instances` | List all connected instances |
| `ping` | Health check |

## Accessible Labels

All interactive UI elements have `accessible-label` properties for automation:

### Auth Screens
- `new-user-button` - "I am new here" button
- `import-key-button` - "I already have key" button
- `username-input` - Username text input
- `password-input` - Password text input
- `signup-submit-button` - Sign up submit
- `passphrase-input` - Login passphrase input
- `login-submit-button` - Login submit

### Spaces Dashboard
- `create-space-button` - Create new space
- `add-website-button` - Add website button
- `settings-button` - Settings button

### Modals
- `space-name-input` - Space name input
- `create-space-submit` - Create space confirm

## Debug Console (Manual)

For manual debugging, use the egui debug console:

```bash
./target/debug/debug_console --connect /tmp/test_dbs/owner.sock
```

Features:
- Log viewer with filtering
- State tree inspector
- Lua REPL with history
