//! Space management callbacks
//!
//! Handles space CRUD operations: request, create, select, delete.

use std::sync::Arc;

use butler::models::Space;
use butler::Butler;
use slint::ComponentHandle;

use crate::Shell;

/// Sort options for spaces list
#[derive(Clone, Copy, Debug)]
enum SpaceSortOption {
    NameAsc,
    NameDesc,
    CreatedDesc,
    CreatedAsc,
    UpdatedDesc,
}

impl SpaceSortOption {
    fn from_index(index: i32) -> Self {
        match index {
            0 => SpaceSortOption::NameAsc,
            1 => SpaceSortOption::NameDesc,
            2 => SpaceSortOption::CreatedDesc,
            3 => SpaceSortOption::CreatedAsc,
            4 => SpaceSortOption::UpdatedDesc,
            _ => SpaceSortOption::NameAsc,
        }
    }
}

fn sort_spaces(spaces: &mut Vec<Space>, option: SpaceSortOption) {
    match option {
        SpaceSortOption::NameAsc => {
            spaces.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        }
        SpaceSortOption::NameDesc => {
            spaces.sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase()))
        }
        SpaceSortOption::CreatedDesc => spaces.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        SpaceSortOption::CreatedAsc => spaces.sort_by(|a, b| a.created_at.cmp(&b.created_at)),
        SpaceSortOption::UpdatedDesc => spaces.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
    }
}

/// Register space management callbacks on the shell
pub fn register(shell: &Shell, butler: Arc<Butler>) {
    register_request_spaces(shell, butler.clone());
    register_create_space(shell, butler.clone());
    register_select_space(shell, butler.clone());
    register_delete_space(shell);
    register_pick_space_template_folder(shell);
}

fn register_request_spaces(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_request_spaces(move |sort_index| {
        let sort_option = SpaceSortOption::from_index(sort_index);
        println!("Requesting spaces with sort: {:?}", sort_option);

        let mut spaces = butler.spaces().list().unwrap_or_default();
        sort_spaces(&mut spaces, sort_option);
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
    shell.on_create_space(move |name, _template_path| {
        println!("Creating space: {}", name);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let butler_inner = butler_create.clone();
        let name_str = name.to_string();
        let result = rt.block_on(async {
            let user_info = butler_inner.user_info().await?;
            butler_inner.spaces().create(name_str, user_info.did).await
        });

        match result {
            Ok(space) => {
                println!("Space created: {} ({})", space.name, space.id);

                if let Some(shell) = shell_weak.upgrade() {
                    let spaces = butler_create.spaces().list().unwrap_or_default();
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
            if let Ok(Some(space)) = butler.spaces().get(&space_id) {
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

fn register_pick_space_template_folder(shell: &Shell) {
    let shell_weak = shell.as_weak();
    shell.on_pick_space_template_folder(move || {
        let folder = rfd::FileDialog::new()
            .set_title("Select App Folder")
            .pick_folder();

        if let Some(folder_path) = folder {
            if let Some(shell) = shell_weak.upgrade() {
                shell.set_space_template_path(folder_path.to_string_lossy().to_string().into());
            }
        }
    });
}
