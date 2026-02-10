//! Test fixtures — helpers, paths, wait functions
//!
//! No hardcoded page templates. Page templates come from sample app dirs
//! via `butler.apps().import_page()`. Space template is the only fixture JSON.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

/// Timeout for mock protocol operations (fast, in-memory)
pub const MOCK_TIMEOUT: Duration = Duration::from_millis(500);

/// Timeout for page sync to complete
pub const PAGE_SYNC_TIMEOUT: Duration = Duration::from_secs(10);

/// Information about a created space + page
pub struct SpaceInfo {
    pub space_id: String,
    pub page_id: String,
}

/// Resolve workspace root (parent of protocol_tests/)
pub fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    manifest_dir.parent().unwrap_or(&manifest_dir).to_path_buf()
}

/// Get sample app directory path
///
/// Example: `app_dir("osvauld-demos")` -> `<workspace>/sample_apps/osvauld-demos`
pub fn app_dir(name: &str) -> PathBuf {
    workspace_root().join("sample_apps").join(name)
}

/// Read space template JSON from fixtures
pub fn space_template() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("space_template.json");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read space_template.json at {:?}: {}", path, e))
}

/// Initialize tracing for tests (idempotent)
pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap()),
        )
        .try_init();
}

/// Poll until a condition returns Some(T), with timeout
pub async fn wait_until<F, T>(desc: &str, timeout: Duration, f: F) -> Result<T>
where
    F: Fn() -> Option<T>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(val) = f() {
            return Ok(val);
        }
        if tokio::time::Instant::now() > deadline {
            return Err(anyhow::anyhow!("Timeout waiting for: {}", desc));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Wait for a layer to have data on a peer's butler (raw bytes)
pub async fn wait_for_layer_data(
    butler: &butler::Butler,
    page_id: &str,
    layer_name: &str,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let page_id = page_id.to_string();
    let layer_name = layer_name.to_string();
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        if let Ok(Some(data)) = butler.store().get_layer(&page_id, &layer_name) {
            if !data.is_empty() {
                return Ok(data);
            }
        }
        if tokio::time::Instant::now() > deadline {
            return Err(anyhow::anyhow!(
                "Timeout waiting for layer data: page={}, layer={}",
                page_id,
                layer_name
            ));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Wait for app files to sync to a peer
pub async fn wait_for_app_files(
    butler: &butler::Butler,
    page_id: &str,
    app_name: &str,
    timeout: Duration,
) -> Result<HashMap<String, String>> {
    let page_id = page_id.to_string();
    let app_name = app_name.to_string();
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        if let Ok(files) = butler.apps().get_files(&page_id, &app_name).await {
            if !files.is_empty() {
                return Ok(files);
            }
        }
        if tokio::time::Instant::now() > deadline {
            return Err(anyhow::anyhow!(
                "Timeout waiting for app files: page={}, app={}",
                page_id,
                app_name
            ));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
