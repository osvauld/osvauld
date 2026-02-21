//! Authentication callbacks
//!
//! Handles login, signup, and import key flows.
//! Note: P2P initialization is handled by the main binary via the OnLoginSuccess callback.

use std::sync::Arc;

use butler::Butler;
use herald::Identity;
use slint::ComponentHandle;

use crate::Shell;

/// Callback invoked on successful login with the Identity object
/// The caller is responsible for calling butler.set_identity() with this identity
pub type OnLoginSuccess = Arc<dyn Fn(Identity) + Send + Sync + 'static>;

/// Register authentication callbacks on the shell
pub fn register(shell: &Shell, butler: Arc<Butler>, on_login_success: OnLoginSuccess) {
    register_import_key(shell);
    register_login(shell, butler.clone(), on_login_success);
    register_signup(shell, butler);
}

fn register_import_key(shell: &Shell) {
    shell.on_import_key(|| {
        tracing::info!("Import key clicked");
        // TODO: Open file dialog for key import
    });
}

fn register_login(shell: &Shell, butler: Arc<Butler>, on_login_success: OnLoginSuccess) {
    let shell_weak = shell.as_weak();

    shell.on_login(move |passphrase| {
        tracing::info!("Attempting login...");

        match butler.login_sync(&passphrase) {
            Ok(identity) => {
                tracing::info!(did = %identity.did(), "Login successful");

                // Update UI immediately
                if let Some(shell) = shell_weak.upgrade() {
                    shell.set_authenticated(true);
                    shell.set_current_screen("spaces".into());
                }

                // Notify main binary of successful login with the identity
                on_login_success(identity);
            }
            Err(e) => {
                tracing::error!(error = %e, "Login failed");
                // TODO: Set error state on shell
            }
        }
    });
}

fn register_signup(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();

    shell.on_sign_up(move |username, password| {
        tracing::info!(username = %username, "Sign up");

        match butler.signup_sync(&username, &password) {
            Ok(result) => {
                tracing::info!("Signup successful");
                tracing::warn!(mnemonic = %result.mnemonic, "IMPORTANT: Save this seed phrase");

                if let Some(shell) = shell_weak.upgrade() {
                    shell.set_initialized(true);
                    shell.set_current_screen("login".into());
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Signup failed");
                // TODO: Set error state on shell
            }
        }
    });
}
