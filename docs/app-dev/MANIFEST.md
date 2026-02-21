# Manifest Reference

Every app directory needs a `manifest.json` that defines its metadata and entry points.

## Field Reference

All manifests are deserialized as `domains::AppManifest` (see `domains/src/manifest.rs`).

| Field | Type | Required | Default | Used By | Description |
|-------|------|----------|---------|---------|-------------|
| `name` | string | Yes | - | All | Display name |
| `version` | string | Yes | - | All | Semver version |
| `entry_logic` | string | Yes | - | User apps | Path to app.lua |
| `renderer` | string | No | `"slint"` | User apps | `"slint"` or `"raylib"` |
| `entry_ui` | string | Slint | - | Slint apps | Path to .slint UI file |
| `models` | string[] | No | [] | Slint apps | VecModel names for incremental updates |
| `width` | number | No | 800 | Raylib apps | Window width in pixels |
| `height` | number | No | 600 | Raylib apps | Window height in pixels |
| `target_fps` | number | No | 60 | Raylib apps | Target frame rate |
| `entry_node` | string | No | - | Node runtime (kunki) | Path to node.lua (derivation, game loop) |

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

### Slint App with Timers

For periodic logic in Slint apps, use timer APIs in Lua (`timer.setInterval`, `timer.setTimeout`).

### Node Runtime Manifest

Library used by the headless node runtime (kunki) for derivation, validation, or server-side game loops. Has no UI:

```json
{
    "name": "shared",
    "version": "1.0.0",
    "entry_logic": "init.lua",
    "entry_node": "node.lua"
}
```

The `entry_node` field points to a Lua script executed by kunki. This is used for:
- **Derivation registration**: `derivation:register()` for aggregating layers
- **Server-side game loops**: Running multiplayer game logic (e.g., tank-game node)
- **Validation**: Business rules enforced on the node

## Unified Manifest Type

All manifest.json files are deserialized into the same `domains::AppManifest` type. This type is shared across:
- **sthalam** (desktop app)
- **kunki** (headless node runtime)
- **renderer_slint** (Slint UI renderer)
- **renderer_raylib** (Raylib graphics renderer)

Legacy per-crate manifest structs have been removed (renderer_raylib::GameManifest, kunki local manifest). The `renderer_slint::AppManifest` is now a type alias of `domains::AppManifest`.

## Default Behavior

- **`renderer`**: Defaults to `"slint"` if omitted
- **`width`**: Defaults to 800 (Raylib window width)
- **`height`**: Defaults to 600 (Raylib window height)
- **`target_fps`**: Defaults to 60 (Raylib target FPS)

## Gotchas

- **Raylib apps don't use `entry_ui`** -- there's no .slint file
- **`models` is Slint-only** -- pre-declares VecModels for incremental updates. VecModels can also be created lazily via `ui:set()`, so this field is optional
- **Raylib requires `--features raylib`** compile flag when building sthalam
- **`entry_node`** is for the node runtime (kunki) -- used for derivation, validation, and server-side game loops
- **New apps need an `"app:App Name"` layer** in app policy declarations (`app.osv`, or legacy templates during migration) for all roles (see [PERMITS.md](PERMITS.md))
