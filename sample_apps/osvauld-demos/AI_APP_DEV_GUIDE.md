# AI App Development Guide

This guide is for AI assistants (Claude) building Sthalam apps. It covers the development workflow, debugging, and common issues.

## Key Directories

```
osvauld/
├── scripts/                    # Development & production scripts
│   ├── run_demo.py            # Main script to run apps (USE THIS)
│   ├── run_production.py      # Production node runner
│   └── setup_production.sh    # Systemd setup for always-on
│
├── sample_apps/osvauld-demos/ # All demo apps live here
│   ├── snake-game/            # Game with tick loop
│   ├── math-sim/              # Particle simulation
│   ├── group-chat/            # Real-time chat
│   ├── guide/                 # Documentation app
│   ├── permit_template.json   # Defines app permissions
│   ├── CLAUDE.md              # Quick reference for AI
│   └── AI_APP_DEV_GUIDE.md    # This file
```

## Quick Start

```bash
# Run an app
python scripts/run_demo.py snake
python scripts/run_demo.py math
python scripts/run_demo.py chat
python scripts/run_demo.py guide

# Run with specific app directory
python scripts/run_demo.py /path/to/my-app
```

## Development Feedback Loop

AI should follow this automated loop:

1. **Write/Edit** app files (manifest.json, app.slint, app.lua)
2. **Run** the app with `python scripts/run_demo.py <app>`
3. **Check output** for errors (Slint compile errors, Lua errors)
4. **Check logs** for runtime issues
5. **Fix and repeat** until app works

### Checking Logs

Logs are critical for debugging. Check these locations:

```bash
# App runtime logs (most useful)
cat ~/.local/share/osvauld/logs/*.log | tail -100

# Filter for warnings (permit issues show here)
grep -i "warn" ~/.local/share/osvauld/logs/*.log

# Filter for errors
grep -i "error" ~/.local/share/osvauld/logs/*.log

# Production node logs
cat ~/.local/share/osvauld-production/logs/*.log | tail -100
```

### Common Log Patterns

**Permit Issues (WARN level):**
```
WARN gurkha: permit validation failed
WARN courier: unauthorized layer access
WARN butler: layer write denied
```
→ Fix: Check permit_template.json has the app layer defined

**Protocol Errors (ERROR level):**
```
ERROR courier: connection failed
ERROR transport: QUIC error
ERROR butler: sync failed
```
→ Report these - they indicate protocol-level bugs

## Handling App Crashes

When the app crashes during development:

### 1. Check the Error Output
```bash
# If running in tmux, capture the output
tmux capture-pane -t demo -p | tail -50

# Or check the log files
tail -100 ~/.local/share/osvauld/logs/*.log
```

### 2. Common Crash Causes

**Slint Compile Error:**
```
Error: Could not compile app.slint
```
→ Fix syntax in app.slint (check brackets, semicolons, property types)

**Lua Runtime Error:**
```
Lua error: attempt to index nil value
```
→ Fix Lua code (check variable names, function calls)

**VecModel Error:**
```
Error: VecModel not found
```
→ Ensure arrays are set with ui:set() before accessing

### 3. Restart After Crash

```bash
# Kill any stuck processes
pkill -f slint_shell

# Clean and restart
python scripts/run_demo.py <app>
```

## Manual Operations

### Starting Apps Manually

```bash
# Without the script (for debugging)
cd /home/abe/osvauld
cargo run -p slint_shell -- --app-dir sample_apps/osvauld-demos/snake-game
```

### Using tmux for Development

```bash
# Create a dev session
tmux new -s dev

# In tmux, run the app
python scripts/run_demo.py snake

# Detach: Ctrl+B then D
# Reattach: tmux attach -t dev

# Kill session
tmux kill-session -t dev
```

### Cleaning Databases

When state gets corrupted or you need a fresh start:

```bash
# Clean development DB
rm -rf ~/.local/share/osvauld
# App will recreate on next run

# Clean production DB
rm -rf ~/.local/share/osvauld-production
rm -rf /tmp/osvauld-production

# Then restart
python scripts/run_demo.py <app>
# or
python scripts/run_production.py
```

## App Structure

```
my-app/
  manifest.json   # Required: app metadata
  app.slint       # Required: UI definition
  app.lua         # Required: logic
```

### manifest.json

```json
{
  "name": "My App",
  "version": "1.0.0",
  "description": "What it does",
  "entry_ui": "app.slint",
  "entry_logic": "app.lua",
  "tick_enabled": false
}
```

- `tick_enabled: true` - For games/animations, calls `tick()` at ~60fps

### app.slint

```slint
// Define the API global - this is how Lua talks to UI
export global AppAPI {
    in-out property<string> message: "Hello";
    in-out property<int> count: 0;
    in-out property<[MyStruct]> items: [];

    callback on_click(string);
    callback on_key_pressed(string);
}

// Define data structures
export struct MyStruct {
    name: string,
    value: int,
}

// Main component must be named App
export component App inherits Rectangle {
    background: #1e293b;

    Text {
        text: AppAPI.message;
        color: white;
    }

    TouchArea {
        clicked => { AppAPI.on_click("button1"); }
    }
}
```

### app.lua

```lua
-- Called once when app loads
function on_init()
    ui:set("message", "Started!")
end

-- Called when UI triggers on_click callback
function on_click(target)
    if target == "button1" then
        local count = ui:get("count")
        ui:set("count", count + 1)
    end
end

-- Called when UI triggers on_key_pressed callback
function on_key_pressed(key)
    print("Key: " .. key)
end

-- Called ~60fps if tick_enabled: true
function tick()
    -- Update game state
    update_physics()
    -- Sync to UI
    ui:set("particles", particle_array)
end

-- Synced layer callbacks
function on_layer_update(layer_name, key, value)
    -- Called when remote peer updates a layer
    print("Layer updated: " .. layer_name)
end
```

## Lua API Reference

### UI Functions

```lua
ui:set("property", value)    -- Set UI property
ui:get("property")           -- Get UI property value

-- Arrays become VecModels automatically
ui:set("items", {{name="a", value=1}, {name="b", value=2}})
```

### Layer Functions (CRDT Sync)

```lua
-- Map layers (persistent, synced)
layer:set("key", value)      -- Write to synced storage
layer:get("key")             -- Read from synced storage

-- List layers (persistent, synced)
layer:list_push(value)       -- Append to synced list
layer:list_get()             -- Get all items
```

### Datagram Functions (Real-time Multiplayer)

Datagrams are for **ephemeral, real-time data** like player movements in multiplayer games. Unlike layers, datagrams are NOT persisted - they're fire-and-forget for live updates.

```lua
-- Send datagram to all connected peers
datagram:send({
    type = "player_move",
    x = player.x,
    y = player.y,
    timestamp = os.time()
})

-- Receive datagrams (called automatically)
function on_datagram(data)
    if data.type == "player_move" then
        -- Update other player's position
        other_players[data.player_id] = {x = data.x, y = data.y}
    end
end
```

**Use cases:**
- Player position/movement in real-time games
- Cursor positions in collaborative apps
- Typing indicators
- Any high-frequency, ephemeral updates

**When to use Datagram vs Layer:**
| Datagram | Layer |
|----------|-------|
| Real-time, ephemeral | Persistent, synced |
| Fire-and-forget | CRDT merge |
| Player movements | Scores, chat history |
| No history needed | History preserved |

### Utility Functions

```lua
print("debug message")       -- Logs to console
json.encode(table)           -- Table to JSON string
json.decode(string)          -- JSON string to table
```

## Adding New Apps to Production

When creating a new app, you must add it to permit_template.json:

```json
// In owner_template.layers, add:
"app:My App Name": {
    "type": "map",
    "sync": true,
    "write": true
}

// Also add to these sections (with write: false for viewers):
// - issue_on.node.layers
// - issue_on.node.issue_on.viewer.layers
// - consent_template.layers
```

Then clean the DB and restart to create a new space with the app.

## Protocol Errors to Report

If you see these errors, report them as they indicate bugs:

```
ERROR courier: handshake failed unexpectedly
ERROR transport: connection reset during sync
ERROR butler: CRDT merge conflict unresolved
ERROR gurkha: permit chain validation failed
```

These are not normal and should be investigated.

## Reference Apps

Study these for patterns:

| App | Features |
|-----|----------|
| `snake-game/` | tick(), keyboard input, game loop, scores list |
| `math-sim/` | tick(), particle arrays, settings sync |
| `group-chat/` | messages list, reactions map, text input |
| `guide/` | tabs, static content, no sync needed |

## Tips for AI Development

1. **Always check logs** after running - warnings reveal permit issues
2. **Use tmux** - keeps sessions alive, captures output
3. **Clean DB** when stuck - fresh state often fixes issues
4. **Study reference apps** - copy patterns that work
5. **Report protocol errors** - these are bugs, not user errors
6. **Test incrementally** - small changes, run, verify, repeat
