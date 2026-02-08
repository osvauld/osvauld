//! Protocol-level subscription tests using MockConnection
//!
//! Tests subscription behavior during publish and sync.
//!
//! **Note**: Layer subscriptions happen in the Scribe/sync protocol.
//! These tests focus on page-level subscription during publish.

use anyhow::Result;

use crate::scenario::{init_tracing, MockScenario, PAGE_LAYERS};

/// Test that pages are accessible after publish
///
/// **Flow**:
/// 1. Owner-node handshake
/// 2. Owner creates space with page
/// 3. Publish to node
/// 4. Node has page
#[tokio::test]
async fn test_mock_page_accessible_after_publish() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    let space_info = s.setup_owner("Test Space").await?;
    tracing::info!("Space created with page: {}", space_info.page_id);

    // Publish to node
    s.publish_space(&space_info.id).await?;

    // Wait for page to sync
    s.wait_for_page(s.node(), &space_info.page_id).await?;
    tracing::info!("Page synced to node");

    // Verify node has the page
    let node_page = s.node().butler.pages().get(&space_info.page_id)?;
    assert!(node_page.is_some(), "Node should have page");

    s.shutdown().await;

    Ok(())
}

/// Test space metadata is accessible on node after publish
///
/// **Flow**:
/// 1. Create space with name
/// 2. Publish to node
/// 3. Node can see space metadata
#[tokio::test]
async fn test_mock_space_accessible_after_publish() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    let space_info = s.setup_owner("My Test Space").await?;
    s.publish_space(&space_info.id).await?;
    s.wait_for_page(s.node(), &space_info.page_id).await?;

    // Verify node has space
    let node_space = s.node().butler.spaces().get(&space_info.id)?;
    assert!(node_space.is_some(), "Node should have space");

    let space = node_space.unwrap();
    assert_eq!(space.name, "My Test Space", "Space name should match");

    tracing::info!("Space accessible on node");

    s.shutdown().await;

    Ok(())
}

/// Test page belongs to correct space
///
/// **Flow**:
/// 1. Create space and page
/// 2. Publish
/// 3. Page on node references correct space
#[tokio::test]
async fn test_mock_page_space_relationship() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    let space_info = s.setup_owner("Test Space").await?;
    s.publish_space(&space_info.id).await?;
    s.wait_for_page(s.node(), &space_info.page_id).await?;

    // Verify page belongs to space
    let node_page = s.node().butler.pages().get(&space_info.page_id)?
        .expect("Node should have page");

    assert_eq!(node_page.meta.space_id, space_info.id, "Page should belong to space");

    tracing::info!("Page-space relationship verified");

    s.shutdown().await;

    Ok(())
}

/// Test multiple spaces are separate
///
/// **Flow**:
/// 1. Create two spaces
/// 2. Publish both
/// 3. Pages are in correct spaces
#[tokio::test]
async fn test_mock_multiple_spaces_separate() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    // Create first space with page
    let space_info1 = s.setup_owner("Space One").await?;
    s.publish_space(&space_info1.id).await?;

    // Create second space with page
    let owner_info = s.owner().butler.user_info().await?;
    let space2 = s.owner().butler.spaces()
        .create("Space Two".to_string(), owner_info.did, crate::scenario::SPACE_TEMPLATE)
        .await?;

    let layer_names: Vec<String> = PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
    let page2 = s.owner().butler.pages()
        .create(&space2.id, "Page Two", layer_names, crate::scenario::PAGE_TEMPLATE)
        .await?;

    s.publish_space(&space2.id).await?;

    // Wait for both pages
    s.wait_for_page(s.node(), &space_info1.page_id).await?;
    s.wait_for_page(s.node(), &page2.id).await?;

    // Verify pages are in correct spaces
    let node_page1 = s.node().butler.pages().get(&space_info1.page_id)?
        .expect("Should have page1");
    let node_page2 = s.node().butler.pages().get(&page2.id)?
        .expect("Should have page2");

    assert_eq!(node_page1.meta.space_id, space_info1.id);
    assert_eq!(node_page2.meta.space_id, space2.id);
    assert_ne!(node_page1.meta.space_id, node_page2.meta.space_id);

    tracing::info!("Multiple spaces correctly separated");

    s.shutdown().await;

    Ok(())
}

/// Test page name preserved after publish
///
/// **Flow**:
/// 1. Create page with specific name
/// 2. Publish
/// 3. Node has page with same name
#[tokio::test]
async fn test_mock_page_name_preserved() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    // Create space with custom page name
    let owner_info = s.owner().butler.user_info().await?;
    let space = s.owner().butler.spaces()
        .create("Test Space".to_string(), owner_info.did, crate::scenario::SPACE_TEMPLATE)
        .await?;

    let layer_names: Vec<String> = PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
    let page = s.owner().butler.pages()
        .create(&space.id, "Custom Page Name", layer_names, crate::scenario::PAGE_TEMPLATE)
        .await?;

    s.publish_space(&space.id).await?;
    s.wait_for_page(s.node(), &page.id).await?;

    // Verify name preserved
    let node_page = s.node().butler.pages().get(&page.id)?
        .expect("Should have page");

    assert_eq!(node_page.meta.name, "Custom Page Name");

    tracing::info!("Page name preserved after sync");

    s.shutdown().await;

    Ok(())
}
