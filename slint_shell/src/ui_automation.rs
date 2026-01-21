//! UI Automation - Process UI commands on Slint main thread
//!
//! Uses Slint's testing API to find elements by accessible-label and interact with them.

use slint::Model;
use tokio::sync::mpsc;

use crate::control_server::{ElementInfo, UiCommand};
use crate::Shell;

/// Process UI commands from the debug server
///
/// This function should be called periodically from the Slint event loop
/// (e.g., via a Timer) to process queued UI commands.
pub fn process_ui_commands(shell: &Shell, ui_rx: &mut mpsc::Receiver<UiCommand>) {
    // Process all pending commands (non-blocking)
    while let Ok(cmd) = ui_rx.try_recv() {
        match cmd {
            UiCommand::Click { label, response_tx } => {
                let result = handle_click(shell, &label);
                let _ = response_tx.send(result);
            }
            UiCommand::Type { label, text, response_tx } => {
                let result = handle_type(shell, &label, &text);
                let _ = response_tx.send(result);
            }
            UiCommand::GetText { label, response_tx } => {
                let result = handle_get_text(shell, &label);
                let _ = response_tx.send(result);
            }
            UiCommand::GetScreen { response_tx } => {
                let screen = shell.get_current_screen().to_string();
                let _ = response_tx.send(screen);
            }
            UiCommand::ListElements { response_tx } => {
                let elements = list_all_elements(shell);
                let _ = response_tx.send(elements);
            }
            UiCommand::DirectLogin { passphrase, response_tx } => {
                // Invoke the login callback directly - this triggers the registered on_login handler
                // which calls butler.login_sync() and updates the UI state on success
                shell.invoke_login(passphrase.as_str().into());
                // Return success - actual login result will be reflected in UI state
                // Check the screen after a moment to see if login succeeded
                let screen = shell.get_current_screen().to_string();
                if screen == "spaces" {
                    let _ = response_tx.send(Ok("Login successful".to_string()));
                } else {
                    let _ = response_tx.send(Err("Login may have failed - screen is still login".to_string()));
                }
            }
            UiCommand::OpenApp { page_id, app_name, response_tx } => {
                // Set the current page_id (required by select_app callback)
                shell.set_current_page_id(page_id.as_str().into());
                // Navigate the UI to page-view screen
                shell.set_current_screen("page-view".into());
                // Invoke the select_page callback to load page data
                shell.invoke_select_page(page_id.as_str().into());
                // Invoke the select_app callback - this triggers the registered on_select_app handler
                // which loads the app via butler and starts the LuaWorker
                shell.invoke_select_app(app_name.as_str().into());
                // Return success - the app loading happens asynchronously
                let _ = response_tx.send(Ok(format!("App '{}' loading started", app_name)));
            }
            UiCommand::SignUp { username, passphrase, response_tx } => {
                // Invoke the sign_up callback - this triggers the registered on_sign_up handler
                shell.invoke_sign_up(username.as_str().into(), passphrase.as_str().into());
                // Check screen to see if signup succeeded
                let screen = shell.get_current_screen().to_string();
                if screen == "spaces" {
                    let _ = response_tx.send(Ok("Signup successful".to_string()));
                } else if screen == "login" {
                    let _ = response_tx.send(Ok("Signup successful - please login".to_string()));
                } else {
                    let _ = response_tx.send(Err(format!("Signup may have failed - screen is {}", screen)));
                }
            }
            UiCommand::RefreshApp { app_name, app_dir, response_tx } => {
                // RefreshApp is handled via the shared refresh channel
                // The app runtime timer processes these requests
                // For now, we invoke the shell callback which triggers the handler
                shell.invoke_refresh_app(app_name.as_str().into(), app_dir.as_str().into());
                // Return immediately - actual refresh happens asynchronously
                // The callback handler will send the result via a different mechanism
                let _ = response_tx.send(Ok(vec!["refresh_triggered".to_string()]));
            }
            UiCommand::RefreshPage { page_dir, response_tx } => {
                // RefreshPage iterates through all app subdirectories and refreshes each
                let result = handle_refresh_page(shell, &page_dir);
                let _ = response_tx.send(result);
            }
            UiCommand::CreateSpace { name, template_path, response_tx } => {
                // Invoke the create_space callback - this triggers the registered on_create_space handler
                // which calls butler.create_space() and updates the UI's spaces list
                shell.invoke_create_space(name.as_str().into(), template_path.as_str().into());

                // The callback runs synchronously (via block_on) and updates shell.spaces
                // Find the created space by name to get its actual ID
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
            UiCommand::AddWebsite { connection_string, response_tx } => {
                // Invoke the add_website callback - this triggers the registered on_add_website handler
                // which connects to the node and requests the space as viewer, updating the UI
                shell.invoke_add_website(connection_string.as_str().into());
                // Return success - actual connection happens asynchronously
                let _ = response_tx.send(Ok("triggered".to_string()));
            }
            UiCommand::AddNode { connection_string, response_tx } => {
                // Invoke the add_node callback - this triggers the registered on_add_node handler
                // which adds the node via butler and connects via courier, updating the UI
                shell.invoke_add_node(connection_string.as_str().into());
                // Return success - actual connection happens asynchronously
                let _ = response_tx.send(Ok("triggered".to_string()));
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

    // Scan for app subdirectories (directories containing manifest.json)
    let entries = std::fs::read_dir(page_path)
        .map_err(|e| format!("Failed to read page directory: {}", e))?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Check if this subdirectory has a manifest.json
        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        // Read manifest to get app name
        let manifest_content = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read manifest: {}", e))?;

        let manifest: serde_json::Value = serde_json::from_str(&manifest_content)
            .map_err(|e| format!("Invalid manifest: {}", e))?;

        let app_name = manifest.get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| format!("manifest.json in {:?} missing 'name' field", path))?;

        // Invoke refresh_app for this app
        let app_dir_str = path.to_string_lossy().to_string();
        shell.invoke_refresh_app(app_name.into(), app_dir_str.as_str().into());
        refreshed_apps.push(app_name.to_string());
    }

    if refreshed_apps.is_empty() {
        return Err("No apps found in page directory (subdirectories with manifest.json)".to_string());
    }

    Ok(refreshed_apps)
}

fn handle_click(shell: &Shell, label: &str) -> Result<(), String> {
    // Use Slint's testing API to find element by accessible-label
    use i_slint_backend_testing::ElementHandle;

    // Find element by accessible label
    let mut elements = ElementHandle::find_by_accessible_label(shell, label);
    if let Some(element) = elements.next() {
        // Invoke the default accessible action (click for buttons)
        element.invoke_accessible_default_action();
        Ok(())
    } else {
        Err(format!("Element not found: {}", label))
    }
}

fn handle_type(shell: &Shell, label: &str, text: &str) -> Result<(), String> {
    use i_slint_backend_testing::ElementHandle;

    let mut elements = ElementHandle::find_by_accessible_label(shell, label);
    if let Some(element) = elements.next() {
        // Set the accessible value (text for inputs)
        element.set_accessible_value(text);
        Ok(())
    } else {
        Err(format!("Element not found: {}", label))
    }
}

fn handle_get_text(shell: &Shell, label: &str) -> Result<String, String> {
    use i_slint_backend_testing::ElementHandle;

    let mut elements = ElementHandle::find_by_accessible_label(shell, label);
    if let Some(element) = elements.next() {
        // Get the accessible value
        Ok(element.accessible_value().unwrap_or_default().to_string())
    } else {
        Err(format!("Element not found: {}", label))
    }
}

/// List all accessible elements in the UI tree for debugging
fn list_all_elements(shell: &Shell) -> Vec<ElementInfo> {
    use i_slint_backend_testing::ElementHandle;

    let mut elements = Vec::new();

    // Try different methods to find elements
    // Method 1: Find by element type name
    for type_name in ["TextInput", "TouchArea", "Button", "Text", "Rectangle"] {
        for element in ElementHandle::find_by_element_type_name(shell, type_name) {
            let accessible_label = element.accessible_label().map(|s| s.to_string());
            let accessible_role = element
                .accessible_role()
                .map(|r| format!("{:?}", r))
                .unwrap_or_else(|| "none".to_string());

            elements.push(ElementInfo {
                id: element.id().unwrap_or_default().to_string(),
                type_name: type_name.to_string(),
                accessible_label,
                accessible_role,
            });
        }
    }

    // Method 2: Also try find_by_accessible_label for known labels
    for label in ["passphrase-input", "login-submit-button", "new-user-button"] {
        for element in ElementHandle::find_by_accessible_label(shell, label) {
            let accessible_role = element
                .accessible_role()
                .map(|r| format!("{:?}", r))
                .unwrap_or_else(|| "none".to_string());

            elements.push(ElementInfo {
                id: format!("found_by_label_{}", label),
                type_name: element.type_name().unwrap_or_default().to_string(),
                accessible_label: Some(label.to_string()),
                accessible_role,
            });
        }
    }

    elements
}
