//! UI Automation - Process UI commands on Slint main thread
//!
//! Uses Slint's testing API to find elements by accessible-label and interact with them.

use tokio::sync::mpsc;

use crate::debug_server::{ElementInfo, UiCommand};
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
        }
    }
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
