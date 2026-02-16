# Slint UI Patterns

Complete guide to building Slint UIs for osvauld apps.

## Overview

Slint is a declarative UI toolkit used for osvauld desktop apps. Apps define their UI in `.slint` files and connect to Lua logic through the `AppAPI` global.

## When to Use Slint

| Use Slint | Use Raylib Instead |
|-----------|-------------------|
| Forms, dashboards, data-heavy apps | Games, simulations, custom rendering |
| Collaboration tools, chat | Anything needing pixel-level control |
| Standard widget layouts | Real-time animation at high FPS |

---

## AppAPI Global

Every Slint app must have an `AppAPI` global. This is the bridge between Lua and Slint:

```slint
export global AppAPI {
    // Data (Lua writes, Slint reads)
    in-out property<[Product]> products: [];

    // UI state (two-way binding)
    in-out property<string> input_text: "";
    in-out property<bool> modal_visible: false;
    in-out property<int> current_tab: 0;

    // Callbacks (Slint calls, Lua handles)
    callback on_click(string);
    callback on_field_changed(string, string);  // field, value
    callback on_modal_action(string, string);   // modal, action
    callback on_key_pressed(string);
}
```

---

## Component Naming

The main component **must** be named `App`:

```slint
export component App inherits Rectangle {
    background: #1e293b;
    // ...
}
```

---

## Structs

Define structs for complex data passed between Lua and Slint:

```slint
export struct Product {
    id: string,
    name: string,
    price: float,
    stock: int,
}
```

---

## Two-Way Binding

```slint
export component App inherits Rectangle {
    property<string> new-name <=> AppAPI.input_text;

    TextInput {
        text <=> new-name;
        edited => { AppAPI.on_field_changed("name", self.text); }
    }
}
```

---

## Theme Global

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
```

---

## Keyboard Input

Requires a `FocusScope` to capture key events:

```slint
export component App inherits Rectangle {
    FocusScope {
        key-pressed(event) => {
            AppAPI.on_key_pressed(event.text);
            accept
        }
    }
}
```

Special key names from Slint:
- Arrow keys: `"↑"`, `"↓"`, `"←"`, `"→"`
- Space: `" "`
- Enter: `"\n"`
- Escape: `"\u{1b}"`
- Letters: `"a"`, `"A"`, etc.

---

## VecModel Pattern

Arrays set via `ui:set` become VecModels. Use `for` loops in Slint to render them:

```slint
for item in AppAPI.products: Rectangle {
    height: 40px;
    Text { text: item.name; }
    TouchArea {
        clicked => { AppAPI.on_click(item.id); }
    }
}
```

---

## Event Bus Pattern

Route all UI events through typed callbacks:

```slint
// Button click
TouchArea {
    clicked => { AppAPI.on_click("add_item"); }
}

// Form input
TextInput {
    edited => { AppAPI.on_field_changed("name", self.text); }
}

// Modal action
TouchArea {
    clicked => { AppAPI.on_modal_action("add_dialog", "submit"); }
}
```

```lua
function on_click(target)
    if target == "add_item" then add_item() end
end

function on_field_changed(field, value)
    form_state[field] = value
end

function on_modal_action(modal, action)
    if modal == "add_dialog" and action == "submit" then do_add() end
end
```

---

## Complete Slint Example

```slint
export struct Item {
    id: string,
    name: string,
    done: bool,
}

export global AppAPI {
    in-out property<[Item]> items: [];
    in-out property<string> input_text: "";
    callback on_click(string);
    callback on_field_changed(string, string);
}

global Theme {
    out property<color> bg: #1e293b;
    out property<color> text: #f1f5f9;
    out property<color> accent: #3b82f6;
}

export component App inherits Rectangle {
    background: Theme.bg;

    VerticalLayout {
        padding: 20px;
        spacing: 10px;

        Text {
            text: "Todo List";
            color: Theme.text;
            font-size: 24px;
        }

        HorizontalLayout {
            spacing: 8px;
            TextInput {
                text <=> AppAPI.input_text;
                edited => { AppAPI.on_field_changed("input", self.text); }
            }
            TouchArea {
                clicked => { AppAPI.on_click("add"); }
                Rectangle {
                    background: Theme.accent;
                    Text { text: "Add"; color: white; }
                }
            }
        }

        for item in AppAPI.items: Rectangle {
            height: 40px;
            background: #334155;
            border-radius: 4px;
            HorizontalLayout {
                padding: 10px;
                Text { text: item.name; color: Theme.text; }
            }
            TouchArea {
                clicked => { AppAPI.on_click(item.id); }
            }
        }
    }
}
```

---

## Slint Gotchas

- Component **must** be named `App`
- **Must** have `export global AppAPI`
- VecModels can be declared in manifest `models` field OR created lazily via `ui:set`
- Keyboard input requires a `FocusScope` wrapping the area

---

## Emoji Support

For Slint apps using emoji, use emoji-capable fonts:

```slint
Text {
    text: "Hello 😊";
    font-family: "Noto Color Emoji, sans-serif";
}
```

**Linux**: Install emoji fonts: `sudo pacman -S noto-fonts-emoji` (Arch) or `sudo apt install fonts-noto-color-emoji` (Debian/Ubuntu)

**macOS/Windows**: Emoji fonts included by default.

---

## See Also

- [RENDERERS.md](app-dev/RENDERERS.md) - Full renderer documentation (Slint + Raylib)
- [LUA_API.md](app-dev/LUA_API.md) - Lua API for UI bindings (`ui:set`, `ui:push`, etc.)
- [SCRIBE_API.md](app-dev/SCRIBE_API.md) - Declarative data binding with `scribe:bind()`
- [MANIFEST.md](app-dev/MANIFEST.md) - manifest.json reference for Slint apps
