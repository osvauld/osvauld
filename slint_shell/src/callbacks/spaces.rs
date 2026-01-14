//! Space management callbacks
//!
//! Handles space CRUD operations: request, create, select, delete.

use std::sync::Arc;

use butler::Butler;
use slint::ComponentHandle;

use crate::templates::SPACE_TEMPLATE;
use crate::Shell;

/// Register space management callbacks on the shell
pub fn register(shell: &Shell, butler: Arc<Butler>) {
    register_request_spaces(shell, butler.clone());
    register_create_space(shell, butler.clone());
    register_select_space(shell, butler.clone());
    register_delete_space(shell);
}

fn register_request_spaces(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_request_spaces(move || {
        println!("Requesting spaces...");

        let spaces = butler.list_spaces().unwrap_or_default();
        println!("Found {} spaces", spaces.len());

        if let Some(shell) = shell_weak.upgrade() {
            let space_infos: Vec<crate::SpaceInfo> = spaces
                .iter()
                .map(|s| crate::SpaceInfo {
                    id: s.id.clone().into(),
                    name: s.name.clone().into(),
                    app_count: 0,
                })
                .collect();
            shell.set_spaces(slint::ModelRc::new(slint::VecModel::from(space_infos)));
        }
    });
}

fn register_create_space(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    let butler_create = butler.clone();
    shell.on_create_space(move |name| {
        println!("Creating space: {}", name);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let butler_inner = butler_create.clone();
        let name_str = name.to_string();

        let result = rt.block_on(async {
            let user_info = butler_inner.user_info().await?;
            butler_inner
                .create_space(name_str, user_info.did, SPACE_TEMPLATE)
                .await
        });

        match result {
            Ok(space) => {
                println!("Space created: {} ({})", space.name, space.id);

                if let Some(shell) = shell_weak.upgrade() {
                    let spaces = butler_create.list_spaces().unwrap_or_default();
                    let space_infos: Vec<crate::SpaceInfo> = spaces
                        .iter()
                        .map(|s| crate::SpaceInfo {
                            id: s.id.clone().into(),
                            name: s.name.clone().into(),
                            app_count: 0,
                        })
                        .collect();
                    shell.set_spaces(slint::ModelRc::new(slint::VecModel::from(space_infos)));
                }
            }
            Err(e) => {
                println!("Failed to create space: {}", e);
            }
        }
    });
}

fn register_select_space(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_select_space(move |space_id| {
        println!("Selected space: {}", space_id);

        if let Some(shell) = shell_weak.upgrade() {
            if let Ok(Some(space)) = butler.get_space(&space_id) {
                shell.set_current_space_name(space.name.into());
            }
        }
    });
}

fn register_delete_space(shell: &Shell) {
    shell.on_delete_space(|space_id| {
        println!("Delete space: {}", space_id);
        // TODO: Implement space deletion
    });
}
