//! Protocol-level sync tests using MockConnection
//!
//! Tests sync protocol flows at the handshake/publish level.
//!
//! **Note**: Direct layer editing is done through Scribe (Lua runtime).
//! These tests focus on the protocol-level sync that happens during publish.

use anyhow::Result;

use crate::scenario::{init_tracing, MockScenario};

/// Test page sync during publish using MockConnection
///
/// **Flow**:
/// 1. Owner and node connect (mock)
/// 2. Owner creates space/page
/// 3. Publish to node
/// 4. Page appears on node
#[tokio::test]
async fn test_mock_publish_sync() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;
    tracing::info!("Owner-Node connected");

    // Setup owner with space
    let space_info = s.setup_owner("Test Space").await?;
    tracing::info!("Space: {}, Page: {}", space_info.id, space_info.page_id);

    // Publish to node
    s.publish_space(&space_info.id).await?;

    // Wait for page to sync
    s.wait_for_page(s.node(), &space_info.page_id).await?;
    tracing::info!("Page synced to node");

    // Verify node has the page
    let node_page = s.node().butler.pages().get(&space_info.page_id)?;
    assert!(node_page.is_some(), "Node should have page after publish");

    s.shutdown().await;

    Ok(())
}

/// Test multiple pages sync correctly
///
/// **Flow**:
/// 1. Create space with multiple pages
/// 2. Publish
/// 3. All pages sync to node
#[tokio::test]
async fn test_mock_multiple_pages_sync() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    // Create space
    let owner_info = s.owner().butler.user_info().await?;
    let space = s.owner().butler.spaces()
        .create("Multi Page Space".to_string(), owner_info.did.clone(), crate::scenario::SPACE_TEMPLATE)
        .await?;

    // Create multiple pages
    let layer_names: Vec<String> = crate::scenario::PAGE_LAYERS.iter().map(|s| s.to_string()).collect();

    let page1 = s.owner().butler.pages()
        .create(&space.id, "Page 1", layer_names.clone(), crate::scenario::PAGE_TEMPLATE)
        .await?;

    let page2 = s.owner().butler.pages()
        .create(&space.id, "Page 2", layer_names.clone(), crate::scenario::PAGE_TEMPLATE)
        .await?;

    tracing::info!("Created pages: {}, {}", page1.id, page2.id);

    // Publish
    s.publish_space(&space.id).await?;

    // Wait for both pages to sync
    s.wait_for_page(s.node(), &page1.id).await?;
    s.wait_for_page(s.node(), &page2.id).await?;

    tracing::info!("Both pages synced to node");

    // Verify both pages exist on node
    assert!(s.node().butler.pages().get(&page1.id)?.is_some());
    assert!(s.node().butler.pages().get(&page2.id)?.is_some());

    s.shutdown().await;

    Ok(())
}

/// Test space metadata syncs correctly
///
/// **Flow**:
/// 1. Create space with specific metadata
/// 2. Publish
/// 3. Node has space metadata
#[tokio::test]
async fn test_mock_space_metadata_sync() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    let space_info = s.setup_owner("My Named Space").await?;
    s.publish_space(&space_info.id).await?;
    s.wait_for_page(s.node(), &space_info.page_id).await?;

    // Verify node has space with correct name
    let node_space = s.node().butler.spaces().get(&space_info.id)?;
    assert!(node_space.is_some(), "Node should have space");
    assert_eq!(node_space.unwrap().name, "My Named Space");

    tracing::info!("Space metadata synced correctly");

    s.shutdown().await;

    Ok(())
}

/// Test page metadata syncs correctly
///
/// **Flow**:
/// 1. Create page with specific metadata
/// 2. Publish
/// 3. Node has page metadata
#[tokio::test]
async fn test_mock_page_metadata_sync() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    let space_info = s.setup_owner("Test Space").await?;
    s.publish_space(&space_info.id).await?;
    s.wait_for_page(s.node(), &space_info.page_id).await?;

    // Verify node has page with correct metadata
    let node_page = s.node().butler.pages().get(&space_info.page_id)?
        .expect("Node should have page");

    assert_eq!(node_page.meta.name, "Test Page");
    assert_eq!(node_page.meta.space_id, space_info.id);

    tracing::info!("Page metadata synced correctly");

    s.shutdown().await;

    Ok(())
}

/// Test rapid republish doesn't cause issues
///
/// **Flow**:
/// 1. Publish
/// 2. Publish again immediately
/// 3. No errors, page still synced
#[tokio::test]
async fn test_mock_rapid_republish() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    let space_info = s.setup_owner("Test Space").await?;

    // Rapid republish
    s.publish_space(&space_info.id).await?;
    s.publish_space(&space_info.id).await?;
    s.publish_space(&space_info.id).await?;

    // Wait for sync
    s.wait_for_page(s.node(), &space_info.page_id).await?;

    // Verify page exists
    assert!(s.node().butler.pages().get(&space_info.page_id)?.is_some());

    tracing::info!("Rapid republish handled correctly");

    s.shutdown().await;

    Ok(())
}
