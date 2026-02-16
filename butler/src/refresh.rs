//! App refresh handler
//!
//! **Context**: Owner edited files on disk, wants to reload into Scribe.
//! Reads files, compares with current layer, updates LoroMap, commits.

use std::path::Path;

use tracing::{info, instrument};
use walkdir::WalkDir;

use scribe::LayerUnit;
use scribe::ScribeState;

/// Refresh app from filesystem (owner only)
///
/// **Context**: Owner edited files on disk, wants to reload into Scribe
/// **We do**: Read files, compare with current layer, update LoroMap, commit
/// **Observer**: Loro observer broadcasts to peers + emits PageUpdate
#[instrument(skip(state, app_dir), fields(app_name = %app_name))]
pub async fn handle_refresh_app(
    state: &mut ScribeState,
    app_name: &str,
    app_dir: &Path,
) -> std::result::Result<Vec<String>, String> {
    // Use bare layer name (Scribe uses bare names, no page_id/ prefix)
    let layer_name = format!("app:{}", app_name);

    info!(
        page_id = %state.page_id,
        app_name = %app_name,
        app_dir = %app_dir.display(),
        "Refreshing app from directory"
    );

    // 1. Collect new files from disk
    let new_files =
        collect_app_files(app_dir).map_err(|e| format!("Failed to read app files: {}", e))?;

    // 2. Get or create the app layer unit
    let unit = state
        .units
        .entry(layer_name.clone())
        .or_insert_with(LayerUnit::new_empty);

    // 3. Get current files to compare
    let old_files = unit.layer().get_all_files();

    // 4. Find changed files
    let mut changed_files = Vec::new();
    for (path, content) in &new_files {
        if old_files.get(path) != Some(content) {
            changed_files.push(path.clone());
        }
    }
    // Track deleted files
    for path in old_files.keys() {
        if !new_files.contains_key(path.as_str()) {
            changed_files.push(path.clone());
        }
    }

    if changed_files.is_empty() {
        info!(
            page_id = %state.page_id,
            app_name = %app_name,
            "No changes detected"
        );
        return Ok(changed_files);
    }

    // 5. Update layer with new files
    unit.layer()
        .set_all_files(&new_files)
        .map_err(|e| format!("Failed to update app layer: {}", e))?;

    // 6. Commit to trigger Loro observer (broadcasts to peers + emits PageUpdate)
    unit.layer().commit();

    // 7. Mark as dirty for persistence
    unit.mark_dirty();

    info!(
        page_id = %state.page_id,
        app_name = %app_name,
        changed_count = changed_files.len(),
        files = ?changed_files,
        "App refreshed, observer will broadcast"
    );

    Ok(changed_files)
}

/// Collect app files from a directory
///
/// Returns HashMap of relative_path -> content for text files.
#[instrument(skip_all)]
fn collect_app_files(
    app_dir: &Path,
) -> std::result::Result<std::collections::HashMap<String, String>, String> {
    let mut files = std::collections::HashMap::new();

    for entry in WalkDir::new(app_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !["slint", "lua", "json"].contains(&ext) {
            continue;
        }

        if let Ok(relative_path) = path.strip_prefix(app_dir) {
            let relative_str = relative_path.to_string_lossy().to_string();
            if let Ok(content) = std::fs::read_to_string(path) {
                files.insert(relative_str, content);
            }
        }
    }

    Ok(files)
}
