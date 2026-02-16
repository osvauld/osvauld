//! Publish protocol tests
//!
//! Tests space/page publishing from owner to node.

use anyhow::Result;

use crate::fixtures::{init_tracing, PAGE_SYNC_TIMEOUT};
use crate::scenario::Scenario;

/// Publish page sync — node receives page after publish
#[tokio::test]
async fn test_publish_page_sync() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .with_tracer()
        .build()
        .await?;

    // Verify tracer captured the publish flow
    s.tracer().assert_contains_sequence(&[
        "Hello",
        "Welcome",
        "PublishSpace",
        "PublishSpaceAck",
        "PageAnnounce",
        "PageAnnounceAck",
    ]);

    // Verify page exists on node
    let page = s.node().butler.pages().get(&s.space().page_id)?;
    assert!(page.is_some(), "Node should have page after publish");

    s.shutdown().await;
    Ok(())
}

/// Space metadata synced — node has correct space name
#[tokio::test]
async fn test_space_metadata_sync() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .space_name("My Chat Space")
        .published()
        .build()
        .await?;

    // Verify space exists on node with correct name
    let space = s.node().butler.spaces().get(&s.space().space_id)?;
    assert!(space.is_some(), "Node should have space");
    assert_eq!(space.unwrap().name, "My Chat Space");

    s.shutdown().await;
    Ok(())
}

/// Page metadata synced — node has correct page name
#[tokio::test]
async fn test_page_metadata_sync() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    // Verify page has correct space relationship
    let page = s
        .node()
        .butler
        .pages()
        .get(&s.space().page_id)?
        .expect("Node should have page");
    assert_eq!(page.meta.space_id, s.space().space_id);

    s.shutdown().await;
    Ok(())
}

/// App files sync — node has app files after publish
#[tokio::test]
async fn test_app_files_sync() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    // Wait for app files to arrive on node
    let files = crate::fixtures::wait_for_app_files(
        &s.node().butler,
        &s.space().page_id,
        "Group Chat",
        PAGE_SYNC_TIMEOUT,
    )
    .await?;

    // Entry logic from manifest.json is "app.lua"
    assert!(
        files.contains_key("app.lua"),
        "Should have app.lua, got keys: {:?}",
        files.keys().collect::<Vec<_>>()
    );

    s.shutdown().await;
    Ok(())
}
