//! UI Automation - Process UI commands on Slint main thread
//!
//! Simplified version without element-level interaction (click/type/getText).
//! Uses shell callback invocation for all commands.

use slint::Model;
use sthalam_shell::Shell;
use tokio::sync::mpsc;

use crate::control_server::UiCommand;

/// Process UI commands from the debug server
///
/// Called periodically from the Slint event loop via a Timer.
pub fn process_ui_commands(shell: &Shell, ui_rx: &mut mpsc::Receiver<UiCommand>) {
    while let Ok(cmd) = ui_rx.try_recv() {
        match cmd {
            UiCommand::DirectLogin { passphrase, response_tx } => {
                shell.invoke_login(passphrase.as_str().into());
                let screen = shell.get_current_screen().to_string();
                if screen == "spaces" {
                    let _ = response_tx.send(Ok("Login successful".to_string()));
                } else {
                    let _ = response_tx.send(Err("Login may have failed - screen is still login".to_string()));
                }
            }
            UiCommand::SignUp { username, passphrase, response_tx } => {
                shell.invoke_sign_up(username.as_str().into(), passphrase.as_str().into());
                let screen = shell.get_current_screen().to_string();
                if screen == "spaces" {
                    let _ = response_tx.send(Ok("Signup successful".to_string()));
                } else if screen == "login" {
                    let _ = response_tx.send(Ok("Signup successful - please login".to_string()));
                } else {
                    let _ = response_tx.send(Err(format!("Signup may have failed - screen is {}", screen)));
                }
            }
            UiCommand::OpenApp { page_id, app_name, response_tx } => {
                shell.set_current_page_id(page_id.as_str().into());
                shell.set_current_screen("page-view".into());
                shell.invoke_select_page(page_id.as_str().into());
                shell.invoke_select_app(app_name.as_str().into());
                let _ = response_tx.send(Ok(format!("App '{}' loading started", app_name)));
            }
            UiCommand::CreateSpace { name, template_path, response_tx } => {
                shell.invoke_create_space(name.as_str().into(), template_path.as_str().into());

                let spaces = shell.get_spaces();
                let space_id = (0..spaces.row_count())
                    .filter_map(|i| spaces.row_data(i))
                    .find(|s| s.name.as_str() == name)
                    .map(|s| s.id.to_string());

                match space_id {
                    Some(id) => {
                        let _ = response_tx.send(Ok((id, name)));
                    }
                    None => {
                        let _ = response_tx.send(Err(format!("Space '{}' was not created", name)));
                    }
                }
            }
            UiCommand::AddNode { connection_string, response_tx } => {
                shell.invoke_add_node(connection_string.as_str().into());
                let _ = response_tx.send(Ok("triggered".to_string()));
            }
            UiCommand::AddWebsite { connection_string, response_tx } => {
                shell.invoke_add_website(connection_string.as_str().into());
                let _ = response_tx.send(Ok("triggered".to_string()));
            }
            UiCommand::RefreshApp { app_name, app_dir, response_tx } => {
                shell.invoke_refresh_app(app_name.as_str().into(), app_dir.as_str().into());
                let _ = response_tx.send(Ok(vec!["refresh_triggered".to_string()]));
            }
            UiCommand::RefreshPage { page_dir, response_tx } => {
                let result = handle_refresh_page(shell, &page_dir);
                let _ = response_tx.send(result);
            }
        }
    }
}

/// Refresh all apps in a page directory
///
/// Scans the page_dir for subdirectories with manifest.json and refreshes each app
fn handle_refresh_page(shell: &Shell, page_dir: &str) -> Result<Vec<String>, String> {
    use std::path::Path;

    let page_path = Path::new(page_dir);
    if !page_path.exists() {
        return Err(format!("Page directory not found: {}", page_dir));
    }

    let mut refreshed_apps = Vec::new();

    let entries = std::fs::read_dir(page_path)
        .map_err(|e| format!("Failed to read page directory: {}", e))?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        let manifest_content = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read manifest: {}", e))?;

        let manifest: serde_json::Value = serde_json::from_str(&manifest_content)
            .map_err(|e| format!("Invalid manifest: {}", e))?;

        let app_name = manifest.get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| format!("manifest.json in {:?} missing 'name' field", path))?;

        let app_dir_str = path.to_string_lossy().to_string();
        shell.invoke_refresh_app(app_name.into(), app_dir_str.as_str().into());
        refreshed_apps.push(app_name.to_string());
    }

    if refreshed_apps.is_empty() {
        return Err("No apps found in page directory (subdirectories with manifest.json)".to_string());
    }

    Ok(refreshed_apps)
}
