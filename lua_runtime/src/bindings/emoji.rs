//! Emoji Bindings for Lua
//!
//! Provides emoji lookup by shortcode, name, and search.

use mlua::{UserData, UserDataMethods, Value as LuaValue};

/// Emoji bindings for Lua - lookup emojis by shortcode
///
/// **Usage in Lua**:
/// - `emoji:get("smile")` -> "grinning face with smiling eyes" (returns Unicode emoji string)
/// - `emoji:name("grinning face with smiling eyes")` -> "grinning face with smiling eyes"
///
/// **Note**: Requires emoji-capable fonts installed on system
pub struct EmojiBindings;

impl UserData for EmojiBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // emoji:get(shortcode) -> Unicode emoji string or nil
        methods.add_method(
            "get",
            |lua, _this, shortcode: String| match emojis::get_by_shortcode(&shortcode) {
                Some(emoji) => Ok(LuaValue::String(lua.create_string(emoji.as_str())?)),
                None => Ok(LuaValue::Nil),
            },
        );

        // emoji:name(unicode) -> emoji name string or nil
        methods.add_method("name", |lua, _this, unicode: String| {
            match emojis::get(&unicode) {
                Some(emoji) => Ok(LuaValue::String(lua.create_string(emoji.name())?)),
                None => Ok(LuaValue::Nil),
            }
        });

        // emoji:search(query) -> array of matching emoji objects
        // Returns up to 10 matches: [{ emoji = "grinning face with smiling eyes", name = "...", shortcode = "..." }, ...]
        methods.add_method("search", |lua, _this, query: String| {
            let table = lua.create_table()?;
            let mut count = 0;

            for emoji in emojis::iter() {
                if count >= 10 {
                    break;
                }

                let name_matches = emoji.name().to_lowercase().contains(&query.to_lowercase());
                let shortcode_matches = emoji
                    .shortcode()
                    .map(|s| s.to_lowercase().contains(&query.to_lowercase()))
                    .unwrap_or(false);

                if name_matches || shortcode_matches {
                    let entry = lua.create_table()?;
                    entry.set("emoji", emoji.as_str())?;
                    entry.set("name", emoji.name())?;
                    if let Some(shortcode) = emoji.shortcode() {
                        entry.set("shortcode", shortcode)?;
                    }
                    count += 1;
                    table.set(count, entry)?;
                }
            }

            Ok(LuaValue::Table(table))
        });
    }
}
