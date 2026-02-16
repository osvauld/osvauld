use crate::{generate_page_shell, write_shell_slint, AppTab, AppVersion, Manifest, PreparedPage};
use butler::{Butler, ScribeMessage};
use ractor::ActorRef;
use std::collections::HashMap;

/// Get app files from Scribe's in-memory layer.
///
/// **Why**: Scribe has the latest synced content; storage may be stale.
pub async fn get_app_files_from_scribe(
    scribe: &ActorRef<ScribeMessage>,
    app_name: &str,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    scribe.cast(ScribeMessage::GetAppFiles {
        app_name: app_name.to_string(),
        reply: tx,
    })?;
    let result = rx.await??;
    Ok(result)
}

/// Get app manifest from Scribe.
///
/// **Why**: Need to check renderer field before deciding which runtime to use.
pub async fn get_app_manifest(
    scribe: &ActorRef<ScribeMessage>,
    app_name: &str,
) -> Result<Manifest, Box<dyn std::error::Error + Send + Sync>> {
    let files = get_app_files_from_scribe(scribe, app_name)
        .await
        .map_err(|e| format!("Failed to get app files: {}", e))?;

    let manifest_content = files
        .get("manifest.json")
        .ok_or_else(|| format!("manifest.json not found for app {}", app_name))?;

    let manifest: Manifest = serde_json::from_str(manifest_content)
        .map_err(|e| format!("Failed to parse manifest.json: {}", e))?;

    Ok(manifest)
}

/// Extract app and generate browser shell with tabs.
pub async fn prepare_page(
    butler: &Butler,
    page_id: &str,
    app_name: Option<&str>,
    scribe: &ActorRef<ScribeMessage>,
) -> Result<PreparedPage, Box<dyn std::error::Error>> {
    use std::fs;

    let page = butler
        .pages()
        .get(page_id)?
        .ok_or_else(|| format!("Page {} not found", page_id))?;
    let page_name = page.meta.name.clone();

    let all_apps = butler.apps().list(page_id)?;
    if all_apps.is_empty() {
        return Err(format!("No apps found in page {}", page_id).into());
    }

    let app_name = match app_name {
        Some(name) => name.to_string(),
        None => all_apps.first().unwrap().clone(),
    };

    let files = get_app_files_from_scribe(scribe, &app_name).await?;

    tracing::debug!(
        page_id = %page_id,
        app_name = %app_name,
        file_count = files.len(),
        all_apps = ?all_apps,
        "Preparing page with tabs"
    );

    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path().to_path_buf();

    for (file_path, content) in &files {
        let full_path = temp_path.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, content)?;
    }

    let data_layers = butler.apps().list_data_layers(page_id).unwrap_or_default();
    tracing::debug!(
        page_id = %page_id,
        data_layers = ?data_layers,
        "Retrieved data layers from page"
    );

    let lua_path = temp_path.join("app.lua");
    let manifest_path = temp_path.join("manifest.json");
    let (models, semantic_version) = if manifest_path.exists() {
        match fs::read_to_string(&manifest_path) {
            Ok(content) => match serde_json::from_str::<Manifest>(&content) {
                Ok(manifest) => (manifest.models, manifest.version),
                Err(e) => {
                    tracing::warn!(
                        page_id = %page_id,
                        app_name = %app_name,
                        error = %e,
                        "Failed to parse manifest.json, using defaults"
                    );
                    (vec![], "0.0.0".to_string())
                }
            },
            Err(e) => {
                tracing::warn!(
                    page_id = %page_id,
                    app_name = %app_name,
                    error = %e,
                    "Failed to read manifest.json, using defaults"
                );
                (vec![], "0.0.0".to_string())
            }
        }
    } else {
        (vec![], "0.0.0".to_string())
    };

    let version = AppVersion::new(&semantic_version, &files);
    tracing::info!(app_name = %app_name, version = %version.display, "App version computed");

    let tabs: Vec<AppTab> = all_apps
        .iter()
        .map(|name| AppTab {
            name: name.clone(),
            display_name: name.clone(),
        })
        .collect();

    let app_slint_path = temp_path.join("app.slint");
    let shell_source = generate_page_shell(
        &app_slint_path,
        &tabs,
        &app_name,
        &page_name,
        Some(&version.display),
    );
    let shell_path = write_shell_slint(&temp_path, &shell_source)?;

    tracing::info!(
        page_id = %page_id,
        page_name = %page_name,
        app_name = %app_name,
        shell_path = %shell_path.display(),
        tabs_count = tabs.len(),
        models = ?models,
        "Generated browser shell with tabs"
    );

    let _ = temp_dir.keep();

    Ok(PreparedPage {
        shell_path,
        lua_path,
        app_name,
        page_id: page_id.to_string(),
        page_name,
        all_apps: tabs,
        data_layers,
        models,
        temp_dir: temp_path,
        restore_geometry: None,
        version,
    })
}
