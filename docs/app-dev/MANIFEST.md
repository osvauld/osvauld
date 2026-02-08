# Manifest Reference

Every app directory needs a `manifest.json` that defines its metadata and entry points.

## Field Reference

| Field | Type | Required | Default | Used By | Description |
|-------|------|----------|---------|---------|-------------|
| `name` | string | Yes | - | All | Display name |
| `version` | string | No | - | All | Semver version |
| `description` | string | No | - | All | Human-readable description |
| `entry_ui` | string | Slint | - | Slint apps | Path to .slint UI file |
| `entry_logic` | string | Yes | - | User apps | Path to app.lua |
| `entry_node` | string | No | - | Shared libs | Path to node.lua (derivation) |
| `renderer` | string | No | `"slint"` | User apps | `"slint"` or `"raylib"` |
| `tick_enabled` | bool | No | false | Slint apps | Enables `tick()` at ~60fps |
| `models` | string[] | No | [] | Slint apps | VecModel names for incremental updates |
| `width` | number | No | 800 | Raylib apps | Window width in pixels |
| `height` | number | No | 600 | Raylib apps | Window height in pixels |
| `target_fps` | number | No | 60 | Raylib apps | Target frame rate |

## Manifest Types

### User App Manifest (Slint)

Standard app with UI and logic:

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

### User App Manifest (Raylib)

Game or simulation with Raylib renderer:

```json
{
    "name": "Snake Raylib",
    "version": "1.0.0",
    "description": "Snake game with Raylib",
    "entry_logic": "app.lua",
    "renderer": "raylib",
    "width": 800,
    "height": 600,
    "target_fps": 60
}
```

### Slint Game with Tick

App that needs a ~60fps game loop via Slint:

```json
{
    "name": "Snake Game",
    "version": "1.0.0",
    "description": "Classic snake game",
    "entry_ui": "app.slint",
    "entry_logic": "app.lua",
    "tick_enabled": true
}
```

### Shared Library Manifest

Library used by multiple apps (e.g., validation, derivation). Has no UI:

```json
{
    "name": "shared",
    "description": "Shared validation and derivation logic",
    "entry_node": "node.lua"
}
```

A minimal shared manifest with just the name:

```json
{
    "name": "shared",
    "description": "Shared library"
}
```

## Gotchas

- **`renderer` defaults to `"slint"` if omitted** -- only set it to `"raylib"` when needed
- **Raylib apps don't use `entry_ui`** -- there's no .slint file
- **`models` is Slint-only** -- pre-declares VecModels for incremental updates. VecModels can also be created lazily via `ui:set()`, so this field is optional
- **`tick_enabled` is Slint-only** -- Raylib always has a game loop
- **Raylib requires `--features raylib`** compile flag when building sthalam
- **`entry_node`** is for shared libraries that run on the node (derivation, game loop)
- **New apps need an `"app:App Name"` layer** in permit_template.json for all roles (see [PERMITS.md](PERMITS.md))
