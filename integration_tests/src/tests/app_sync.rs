//! App layer sync tests
//!
//! Tests that app files (manifest.json, app.lua, etc.) sync between peers.
//! Uses real sample_apps/ directories and import_page().

use anyhow::Result;

use crate::fixtures::{init_tracing, wait_for_app_files, app_dir, PAGE_SYNC_TIMEOUT};
use crate::scenario::Scenario;

/// Owner imports page from real app dir, app layer has files
///
/// **Reproduces**: app:Group Chat layer being empty (data_len=81) on node/viewer.
#[tokio::test]
async fn test_owner_app_layer_has_files() -> Result<()> {
    init_tracing();

    let app_path = app_dir("osvauld-demos");
    assert!(app_path.exists(), "sample_apps/osvauld-demos must exist: {:?}", app_path);

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();

    // Wait for app files on owner via import
    let files = wait_for_app_files(
        &s.owner().butler,
        &page_id,
        "Group Chat",
        PAGE_SYNC_TIMEOUT,
    ).await?;

    assert!(!files.is_empty(), "App layer should have files");
    assert!(files.contains_key("manifest.json"), "Should have manifest.json");
    assert!(files.contains_key("app.lua"), "Should have app.lua");

    s.shutdown().await;
    Ok(())
}

/// App layer syncs from owner to node after publish
///
/// **Bug**: data_len=81 means empty LoroDoc arrived instead of files
#[tokio::test]
async fn test_app_layer_syncs_to_node() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();

    // Wait for app files to arrive on node
    let node_files = wait_for_app_files(
        &s.node().butler,
        &page_id,
        "Group Chat",
        PAGE_SYNC_TIMEOUT,
    ).await?;

    assert!(!node_files.is_empty(), "Node should have app files");
    assert!(node_files.contains_key("manifest.json"), "Node should have manifest.json");
    assert!(node_files.contains_key("app.lua"), "Node should have app.lua");

    // Verify content is non-trivial (not empty LoroDoc)
    let manifest = node_files.get("manifest.json").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(manifest)?;
    assert_eq!(
        parsed.get("name").and_then(|v| v.as_str()),
        Some("Group Chat"),
        "Manifest name should be 'Group Chat'"
    );

    // Verify app.lua content
    let lua_content = node_files.get("app.lua").unwrap();
    assert!(lua_content.contains("on_init"), "app.lua should contain on_init function");

    s.shutdown().await;
    Ok(())
}

/// App layer syncs from owner → node → viewer
///
/// **Bug coverage**: Viewer must receive app files to render the UI.
/// Without this, viewer gets empty hash and no app loads.
#[tokio::test]
async fn test_app_layer_syncs_to_viewer() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();

    // Wait for app files to reach node first
    let node_files = wait_for_app_files(
        &s.node().butler,
        &page_id,
        "Group Chat",
        PAGE_SYNC_TIMEOUT,
    ).await?;
    assert!(!node_files.is_empty(), "Node should have app files");

    // Connect viewer
    let viewer_link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &viewer_link, &page_id).await?;

    // Wait for app files to arrive on viewer
    let viewer_files = wait_for_app_files(
        &s.viewer(0).butler,
        &page_id,
        "Group Chat",
        PAGE_SYNC_TIMEOUT,
    ).await?;

    assert!(!viewer_files.is_empty(), "Viewer should have app files");
    assert!(viewer_files.contains_key("manifest.json"), "Viewer should have manifest.json");
    assert!(viewer_files.contains_key("app.lua"), "Viewer should have app.lua");

    // Verify manifest content
    let manifest = viewer_files.get("manifest.json").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(manifest)?;
    assert_eq!(
        parsed.get("name").and_then(|v| v.as_str()),
        Some("Group Chat"),
        "Manifest name should be 'Group Chat'"
    );

    s.shutdown().await;
    Ok(())
}
