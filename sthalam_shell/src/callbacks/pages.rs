//! Page management callbacks
//!
//! Handles page operations: request, select, upload, reload.
//! Note: App launching is handled by the main binary, not here.

use std::sync::Arc;

use butler::Butler;
use slint::ComponentHandle;

use crate::Shell;

/// Register page management callbacks on the shell
pub fn register(shell: &Shell, butler: Arc<Butler>) {
    register_request_pages(shell, butler.clone());
    register_select_page(shell, butler.clone());
    register_upload_page(shell, butler.clone());
    register_reload_page(shell, butler.clone());
    register_request_page_apps(shell, butler.clone());
    register_delete_app(shell);
}

/// Register request_pages callback - loads pages for a space
fn register_request_pages(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_request_pages(move |space_id| {
        tracing::info!(space_id = %space_id, "Requesting pages");

        let pages = butler.pages().list(&space_id).unwrap_or_default();
        tracing::debug!(count = pages.len(), "Found pages");

        if let Some(shell) = shell_weak.upgrade() {
            let page_infos: Vec<crate::PageInfo> = pages
                .iter()
                .map(|p| {
                    let app_count = butler
                        .apps()
                        .list(&p.id)
                        .map(|a| a.len() as i32)
                        .unwrap_or(0);
                    crate::PageInfo {
                        id: p.id.clone().into(),
                        name: p.name.clone().into(),
                        app_count,
                    }
                })
                .collect();
            shell.set_pages(slint::ModelRc::new(slint::VecModel::from(page_infos)));
        }
    });
}

/// Register select_page callback - navigates to page view
fn register_select_page(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_select_page(move |page_id| {
        tracing::info!(page_id = %page_id, "Selecting page");

        if let Some(shell) = shell_weak.upgrade() {
            if let Ok(Some(page)) = butler.pages().get(&page_id) {
                shell.set_current_page_name(page.meta.name.into());
            }

            let apps = butler.apps().list(&page_id).unwrap_or_default();
            tracing::debug!(count = apps.len(), page_id = %page_id, "Found apps");

            let app_infos: Vec<crate::AppInfo> = apps
                .iter()
                .map(|app_name| crate::AppInfo {
                    id: app_name.clone().into(),
                    name: app_name.clone().into(),
                    page_id: page_id.clone(),
                })
                .collect();
            shell.set_page_apps(slint::ModelRc::new(slint::VecModel::from(app_infos)));
        }
    });
}

/// Register upload_page callback - opens folder picker to upload page directory
fn register_upload_page(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_upload_page_clicked(move || {
        tracing::info!("Upload page clicked");

        let folder = rfd::FileDialog::new()
            .set_title("Select Page Folder")
            .pick_folder();

        if let Some(folder_path) = folder {
            tracing::info!(path = %folder_path.display(), "Selected folder");

            let current_space_id = if let Some(shell) = shell_weak.upgrade() {
                shell.get_current_space_id().to_string()
            } else {
                tracing::error!("Failed to get current space ID");
                return;
            };

            if current_space_id.is_empty() {
                tracing::warn!("No space selected");
                return;
            }

            let rt = tokio::runtime::Runtime::new().unwrap();
            let butler_inner = butler.clone();

            let page_result = rt.block_on(async {
                butler_inner
                    .apps()
                    .import_page(&current_space_id, &folder_path)
                    .await
            });

            let page = match page_result {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to import page");
                    return;
                }
            };

            tracing::info!(page_name = %page.name, "Page uploaded successfully");

            // Refresh pages list
            if let Some(shell) = shell_weak.upgrade() {
                let pages = butler.pages().list(&current_space_id).unwrap_or_default();
                let page_infos: Vec<crate::PageInfo> = pages
                    .iter()
                    .map(|p| {
                        let app_count = butler
                            .apps()
                            .list(&p.id)
                            .map(|a| a.len() as i32)
                            .unwrap_or(0);
                        crate::PageInfo {
                            id: p.id.clone().into(),
                            name: p.name.clone().into(),
                            app_count,
                        }
                    })
                    .collect();
                shell.set_pages(slint::ModelRc::new(slint::VecModel::from(page_infos)));
            }
        }
    });
}

/// Register reload_page callback - reloads page from filesystem
fn register_reload_page(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_reload_page(move || {
        let current_page_id = if let Some(shell) = shell_weak.upgrade() {
            shell.get_current_page_id().to_string()
        } else {
            return;
        };

        if current_page_id.is_empty() {
            tracing::warn!("No page selected");
            return;
        }

        tracing::info!(page_id = %current_page_id, "Page reload requested");

        // Refresh apps list
        if let Some(shell) = shell_weak.upgrade() {
            let apps = butler.apps().list(&current_page_id).unwrap_or_default();
            let app_infos: Vec<crate::AppInfo> = apps
                .iter()
                .map(|app_name| crate::AppInfo {
                    id: app_name.clone().into(),
                    name: app_name.clone().into(),
                    page_id: current_page_id.clone().into(),
                })
                .collect();
            shell.set_page_apps(slint::ModelRc::new(slint::VecModel::from(app_infos)));
        }
    });
}

/// Register request_page_apps callback - loads apps for a page
fn register_request_page_apps(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_request_page_apps(move |page_id| {
        tracing::info!(page_id = %page_id, "Requesting apps");

        let apps = butler.apps().list(&page_id).unwrap_or_default();
        tracing::debug!(count = apps.len(), "Found apps");

        if let Some(shell) = shell_weak.upgrade() {
            let app_infos: Vec<crate::AppInfo> = apps
                .iter()
                .map(|app_name| crate::AppInfo {
                    id: app_name.clone().into(),
                    name: app_name.clone().into(),
                    page_id: page_id.clone(),
                })
                .collect();
            shell.set_page_apps(slint::ModelRc::new(slint::VecModel::from(app_infos)));
        }
    });
}

/// Register delete_app callback
fn register_delete_app(shell: &Shell) {
    shell.on_delete_app(|app_id| {
        tracing::info!(app_id = %app_id, "Delete app");
        // TODO: Implement app deletion
    });
}
