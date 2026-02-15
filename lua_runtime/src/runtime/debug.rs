use mlua::{Lua, Value};

const LUA_BUILTINS: &[&str] = &[
    "_G",
    "_VERSION",
    "_timers",
    "assert",
    "collectgarbage",
    "dofile",
    "error",
    "getmetatable",
    "ipairs",
    "load",
    "loadfile",
    "next",
    "pairs",
    "pcall",
    "print",
    "rawequal",
    "rawget",
    "rawlen",
    "rawset",
    "require",
    "select",
    "setmetatable",
    "tonumber",
    "tostring",
    "type",
    "warn",
    "xpcall",
    "coroutine",
    "debug",
    "io",
    "math",
    "os",
    "package",
    "string",
    "table",
    "utf8",
    "scribe",
    "ui",
    "page",
    "permit",
    "peers",
    "derivation",
    "layout",
    "emoji",
    "datetime",
    "timer",
    "api",
    "binding",
    "presence_lib",
];

/// Collect non-builtin Lua global names for debug introspection.
pub(super) fn collect_lua_globals(lua: &Lua) -> Vec<String> {
    let mut globals = Vec::new();
    if let Ok(pairs) = lua
        .globals()
        .pairs::<String, Value>()
        .collect::<Result<Vec<_>, _>>()
    {
        for (name, _) in pairs {
            if !LUA_BUILTINS.contains(&name.as_str()) {
                globals.push(name);
            }
        }
    }
    globals.sort();
    globals
}
