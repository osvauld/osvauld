# Osvauld Demo Apps

Demo apps for testing the app runtime. Each app demonstrates different features.

**Full AI Development Guide:** See `AI_APP_DEV_GUIDE.md` for comprehensive debugging, logs, crash handling, and manual operations.

## Apps

| App | Directory | Features |
|-----|-----------|----------|
| Snake Game | `snake-game/` | `tick_enabled`, keyboard input (`on_key_pressed`), game loop |
| Math Simulation | `math-sim/` | `tick_enabled`, particle physics, lazy VecModel creation |
| Group Chat | `group-chat/` | Real-time sync, ephemeral events, text input |
| Sthalam Guide | `guide/` | Tab navigation, static content, documentation |

## manifest.json Reference

```json
{
    "name": "App Name",
    "version": "1.0.0",
    "description": "Description",
    "entry_ui": "app.slint",
    "entry_logic": "app.lua",
    "tick_enabled": true,      // Optional: Enable tick() at ~60fps
    "models": ["items"]        // Optional: Pre-declare VecModels
}
```

## Running Demos

```bash
# Run interactively
python scripts/run_demo.py snake
python scripts/run_demo.py math
python scripts/run_demo.py chat

# Attach to tmux session
tmux attach -t demo
```

## Key Patterns

### Game Loop (tick_enabled: true)
```lua
function tick()
    -- Called ~60fps when tick_enabled is true
    update_physics()
    sync_particles_ui()
end
```

### Keyboard Input
```slint
// In AppAPI
callback on_key_pressed(string);

// In component
FocusScope {
    key-pressed(event) => {
        AppAPI.on_key_pressed(event.text);
        accept
    }
}
```

```lua
function on_key_pressed(key)
    if key == "w" then move_up() end
end
```

### VecModel (lazy creation)
```lua
-- VecModels are created automatically on first use
ui:set("particles", particle_array)  -- Creates "particles" model if needed
```

### Datagrams (Real-time Multiplayer)
```lua
-- Send ephemeral data to peers (player movements, cursors)
datagram:send({type = "move", x = 10, y = 20})

-- Receive datagrams
function on_datagram(data)
    if data.type == "move" then
        update_player(data.x, data.y)
    end
end
```

## Debugging

```bash
# Check logs for warnings (permit issues)
grep -i "warn" ~/.local/share/osvauld/logs/*.log

# Check for errors (protocol bugs - report these)
grep -i "error" ~/.local/share/osvauld/logs/*.log

# Clean DB and restart when stuck
rm -rf ~/.local/share/osvauld
python scripts/run_demo.py <app>
```
