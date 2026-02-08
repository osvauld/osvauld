//! Page Runtime - Generate browser shell that wraps app components
//!
//! Apps are pure components (no Window inheritance) with an `export global AppAPI`
//! that defines their public interface. The shell wraps the app in a Window with
//! tab bar. Runtime accesses app via global API (set_global_callback, set_global_property).

use std::path::Path;

/// App tab information
#[derive(Clone, Debug)]
pub struct AppTab {
    pub name: String,
    pub display_name: String,
}

/// Parsed exported types from app.slint
#[derive(Default)]
pub struct ExportedTypes {
    /// Struct names (e.g., Product, Order)
    pub structs: Vec<String>,
    /// Global names (e.g., AppAPI)
    pub globals: Vec<String>,
}

/// Parse exported types (structs and globals) from app.slint source
///
/// Looks for lines like: `export struct Product {` or `export global AppAPI {`
pub fn parse_exported_types(slint_source: &str) -> ExportedTypes {
    let mut result = ExportedTypes::default();

    for line in slint_source.lines() {
        let trimmed = line.trim();

        // Parse: export struct Name {
        if trimmed.starts_with("export struct ") {
            if let Some(rest) = trimmed.strip_prefix("export struct ") {
                let name = rest
                    .split(|c: char| c == '{' || c.is_whitespace())
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    result.structs.push(name);
                }
            }
        }

        // Parse: export global Name {
        if trimmed.starts_with("export global ") {
            if let Some(rest) = trimmed.strip_prefix("export global ") {
                let name = rest
                    .split(|c: char| c == '{' || c.is_whitespace())
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    result.globals.push(name);
                }
            }
        }
    }

    result
}

/// Generate browser shell that imports and wraps the app
///
/// **Architecture:**
/// - App = pure component with `export global AppAPI` for public interface
/// - Shell = Window + TabBar + imports App
/// - Runtime accesses app via globals (no forwarding needed)
/// - Shell re-exports globals so interpreter can access them
///
/// **Returns:** Slint source for the shell
pub fn generate_page_shell(
    app_slint_path: &Path,
    tabs: &[AppTab],
    current_app: &str,
    page_name: &str,
    version: Option<&str>,
) -> String {
    // Read app source to parse exported types
    let app_source = std::fs::read_to_string(app_slint_path).unwrap_or_default();
    let exported_types = parse_exported_types(&app_source);

    let app_path = app_slint_path.to_string_lossy();
    let tab_bar = generate_tab_bar(tabs, current_app);

    // Build import statement with App, structs, and globals
    let mut imports = vec!["App as UserApp".to_string()];
    imports.extend(exported_types.structs.iter().cloned());
    imports.extend(exported_types.globals.iter().cloned());
    let import_list = imports.join(", ");

    // Re-export globals so interpreter can access them via set_global_callback
    let global_exports = if exported_types.globals.is_empty() {
        String::new()
    } else {
        format!("\nexport {{ {} }}", exported_types.globals.join(", "))
    };

    // Generate version footer if version is provided
    let version_footer = if let Some(v) = version {
        format!(
            r#"
        // Version footer
        Rectangle {{
            height: 20px;
            background: #0d0d1a;

            Text {{
                text: "v{version}";
                color: #555;
                font-size: 10px;
                horizontal-alignment: right;
                vertical-alignment: center;
                x: parent.width - self.width - 10px;
            }}
        }}"#,
            version = v
        )
    } else {
        String::new()
    };

    format!(
        r#"import {{ VerticalBox, HorizontalBox, Button }} from "std-widgets.slint";
import {{ {import_list} }} from "{app_path}";
{global_exports}

export component App inherits Window {{
    title: "{page_name}";
    background: #1a1a2e;
    min-width: 800px;
    min-height: 600px;

    // Tab callbacks (handled by Rust)
    callback select-tab(string);
    callback add-app();

    VerticalLayout {{
{tab_bar}

        // App content - no forwarding needed, runtime uses AppAPI global
        UserApp {{
            width: 100%;
            vertical-stretch: 1;
        }}
{version_footer}
    }}
}}"#,
        import_list = import_list,
        app_path = app_path,
        global_exports = global_exports,
        page_name = page_name,
        tab_bar = tab_bar,
        version_footer = version_footer,
    )
}

/// Generate Slint code for the tab bar
fn generate_tab_bar(tabs: &[AppTab], current_app: &str) -> String {
    let tab_items: Vec<String> = tabs
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let is_active = tab.name == current_app;
            format!(
                r#"                Rectangle {{
                    width: tab{i}-text.preferred-width + 16px;
                    height: 26px;
                    background: {active} ? #3a3a5a : (tab{i}-touch.has-hover ? #2a2a4a : transparent);
                    border-radius: 4px;

                    tab{i}-text := Text {{
                        text: "{display}";
                        color: {active} ? #eee : #888;
                        font-size: 12px;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }}

                    tab{i}-touch := TouchArea {{
                        clicked => {{ root.select-tab("{name}"); }}
                    }}
                }}"#,
                i = i,
                display = tab.display_name,
                name = tab.name,
                active = is_active,
            )
        })
        .collect();

    format!(
        r#"        // Tab bar
        Rectangle {{
            height: 36px;
            background: #0d0d1a;

            HorizontalLayout {{
                padding: 5px;
                padding-left: 10px;
                spacing: 4px;

{tabs}

                // Add app button
                Rectangle {{
                    width: 26px;
                    height: 26px;
                    background: add-touch.has-hover ? #2a2a4a : transparent;
                    border-radius: 4px;

                    Text {{
                        text: "+";
                        color: #666;
                        font-size: 14px;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }}

                    add-touch := TouchArea {{
                        clicked => {{ root.add-app(); }}
                    }}
                }}

                Rectangle {{ horizontal-stretch: 1; }}
            }}
        }}

        // Separator
        Rectangle {{
            height: 1px;
            background: #252538;
        }}"#,
        tabs = tab_items.join("\n\n")
    )
}

/// Write generated shell to temp directory
pub fn write_shell_slint(
    temp_dir: &Path,
    source: &str,
) -> std::io::Result<std::path::PathBuf> {
    let path = temp_dir.join("_shell.slint");
    std::fs::write(&path, source)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_generate_shell() {
        let tabs = vec![
            AppTab { name: "signup".to_string(), display_name: "Signup".to_string() },
            AppTab { name: "admin".to_string(), display_name: "Admin".to_string() },
        ];

        let shell = generate_page_shell(
            &PathBuf::from("/tmp/app.slint"),
            &tabs,
            "signup",
            "Waitlist",
            Some("1.0.0-abcd1234"),
        );

        assert!(shell.contains("import { App as UserApp"));
        assert!(shell.contains("UserApp {"));
        assert!(shell.contains("select-tab"));
        assert!(shell.contains("Signup"));
        assert!(shell.contains("Admin"));
        assert!(shell.contains("v1.0.0-abcd1234"));
    }

    #[test]
    fn test_parse_exported_types() {
        let source = r#"
export struct Product {
    id: string,
}

export struct Order {
    id: string,
}

export global AppAPI {
    in-out property<[Product]> products: [];
}

export component App inherits Rectangle {
}
"#;

        let types = parse_exported_types(source);
        assert!(types.structs.contains(&"Product".to_string()));
        assert!(types.structs.contains(&"Order".to_string()));
        assert!(types.globals.contains(&"AppAPI".to_string()));
    }
}
