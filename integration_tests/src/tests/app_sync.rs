//! App layer sync tests
//!
//! Tests that app files (manifest.json, app.lua, etc.) sync between peers.
//! Uses real sample_apps/ directories and import_page().

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use crate::fixtures::{app_dir, init_tracing, wait_for_app_files, PAGE_SYNC_TIMEOUT};
use crate::scenario::Scenario;

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn make_modified_group_chat_copy(marker: &str) -> Result<(tempfile::TempDir, PathBuf)> {
    let source = app_dir("group-chat").join("group-chat");
    let temp = tempfile::tempdir()?;
    let dest = temp.path().join("group-chat");

    copy_dir_recursive(&source, &dest)?;

    let app_lua_path = dest.join("app.lua");
    let original = fs::read_to_string(&app_lua_path)?;
    let modified = format!("{}\n-- {}\n", original, marker);
    fs::write(&app_lua_path, modified)?;

    Ok((temp, dest))
}

async fn wait_for_app_lua_contains(
    butler: &butler::Butler,
    page_id: &str,
    app_name: &str,
    needle: &str,
) -> Result<String> {
    let deadline = tokio::time::Instant::now() + PAGE_SYNC_TIMEOUT;
    loop {
        if let Ok(files) = butler.apps().get_files(page_id, app_name).await {
            if let Some(app_lua) = files.get("app.lua") {
                if app_lua.contains(needle) {
                    return Ok(app_lua.clone());
                }
            }
        }
        if tokio::time::Instant::now() > deadline {
            return Err(anyhow::anyhow!(
                "Timeout waiting for app.lua to contain marker '{}'",
                needle
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// Owner imports page from real app dir, app layer has files
///
/// **Reproduces**: app:Group Chat layer being empty (data_len=81) on node/viewer.
#[tokio::test]
async fn test_owner_app_layer_has_files() -> Result<()> {
    init_tracing();

    let app_path = app_dir("group-chat");
    assert!(app_path.exists(), "sample_apps/group-chat must exist: {:?}", app_path);

    let mut s = Scenario::builder()
        .app("group-chat")
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
        .app("group-chat")
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
        .app("group-chat")
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

/// Owner refresh propagates updated app files to node via sync.
#[tokio::test]
async fn test_refresh_app_propagates_to_node() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let marker = "refresh-propagates-to-node";
    let (_temp, modified_app_dir) = make_modified_group_chat_copy(marker)?;

    s.owner()
        .butler
        .apps()
        .update(&page_id, &modified_app_dir)
        .await?;

    let node_app_lua =
        wait_for_app_lua_contains(&s.node().butler, &page_id, "Group Chat", marker).await?;
    assert!(node_app_lua.contains(marker), "Node should receive refreshed app.lua content");

    s.shutdown().await;
    Ok(())
}

/// Owner refresh while disconnected syncs to node after reconnect.
#[tokio::test]
async fn test_refresh_app_catches_up_after_node_reconnect() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("group-chat")
        .published()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let marker = "refresh-catches-up-after-reconnect";
    let (_temp, modified_app_dir) = make_modified_group_chat_copy(marker)?;

    s.disconnect_owner_from_node().await?;

    s.owner()
        .butler
        .apps()
        .update(&page_id, &modified_app_dir)
        .await?;

    s.reconnect_owner_to_node().await?;

    let node_app_lua =
        wait_for_app_lua_contains(&s.node().butler, &page_id, "Group Chat", marker).await?;
    assert!(
        node_app_lua.contains(marker),
        "Node should catch up with refreshed app.lua after reconnect"
    );

    s.shutdown().await;
    Ok(())
}
