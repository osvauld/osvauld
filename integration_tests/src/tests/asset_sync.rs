//! Asset sync integration tests
//!
//! Tests the full P2P asset sync lifecycle:
//! - Asset syncs during publish (owner → node)
//! - Live asset sync after initial publish
//! - Multiple assets sync correctly
//! - Node broadcasts to viewers (owner → node → viewer)
//!
//! Asset Sync Flow:
//! 1. Owner uploads asset: butler.upload_asset_async() → encrypts & stores in AssetStore
//! 2. Metadata syncs via MapInsert to {page_id}/assets Loro layer
//! 3. Layer syncs via SyncOffer → SyncAccept → SyncAck
//! 4. On SyncAck for /assets layer → trigger_asset_sync_after_layer_sync()
//! 5. Node: find_missing_assets_from_layer() → sends AssetPrepare
//! 6. Owner: on_asset_prepare() → decrypt → add to MockBlobStore → send AssetReady
//! 7. Node: on_asset_ready() → download from MockBlobStore → verify → re-encrypt → store → send AssetAck
//!
//! 3-Party Broadcast Flow (Test 4):
//! 1. Owner uploads asset, syncs to node (steps 1-7 above)
//! 2. Node broadcasts SyncOffer for assets layer to subscribed viewer
//! 3. Viewer receives metadata, sends AssetPrepare to node
//! 4. Node: on_asset_prepare() → decrypt → add to MockBlobStore → send AssetReady
//! 5. Viewer: on_asset_ready() → download → verify → re-encrypt → store → send AssetAck

use crate::fixtures::{
    ASSET_SYNC_DELAY, TEST_PAGE_LAYERS, TEST_PAGE_TEMPLATE, TEST_SPACE_TEMPLATE,
};
use crate::helpers::{
    assert_asset_synced, generate_test_image, init_tracing, setup_with_handshake,
    setup_with_viewer, upload_test_asset, wait_for_asset_blob,
};
use courier::coordinator::CoordinatorMessage;
use tracing::info;

/// Test 1: Asset syncs when page is published
///
/// **Setup**: Owner + node with completed handshake
/// **Action**: Create space/page, upload asset, publish space
/// **Expected**: Asset blob arrives on node, content matches
#[tokio::test]
async fn test_asset_sync_during_publish() {
    init_tracing();
    info!("=== test_asset_sync_during_publish ===");

    // Setup: owner + node with handshake
    let peers = setup_with_handshake("owner", "node").await.unwrap();

    // Create space and page
    let user_info = peers.owner_butler.user_info().await.unwrap();
    let space = peers
        .owner_butler
        .create_space(
            "Asset Test Space".to_string(),
            user_info.did.clone(),
            TEST_SPACE_TEMPLATE,
        )
        .await
        .unwrap();

    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
    let page = peers
        .owner_butler
        .create_page(&space.id, "Asset Test Page", layer_names, TEST_PAGE_TEMPLATE)
        .await
        .unwrap();

    // Upload test asset before publish
    let test_data = generate_test_image(42, 1024);
    let asset = upload_test_asset(&peers.owner_butler, &page.id, "test_image.png", &test_data)
        .await
        .unwrap();

    info!(
        hash = %asset.hash,
        filename = %asset.filename,
        "Asset uploaded, now publishing space"
    );

    // Publish space (triggers page sync + asset sync)
    let node_node_id = peers.node_id();
    peers
        .owner_coordinator()
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .unwrap();

    // Wait for page to arrive on node first
    peers
        .harness
        .wait_for_page(&peers.node_butler, &page.id)
        .await
        .unwrap();

    info!("Page synced to node, waiting for asset blob");

    // Wait for asset blob to arrive on node
    wait_for_asset_blob(&peers.node_butler, &asset.hash, ASSET_SYNC_DELAY)
        .await
        .unwrap();

    // Verify asset content matches
    assert_asset_synced(
        &peers.owner_butler,
        &peers.node_butler,
        &page.id,
        &asset.hash,
    )
    .await
    .unwrap();

    info!("test_asset_sync_during_publish PASSED");
    peers.shutdown().await;
}

/// Test 2: Live asset sync after initial publish
///
/// **Setup**: Owner + node with already-published page
/// **Action**: Upload new asset to existing page (live sync)
/// **Expected**: Asset blob arrives on node via layer sync trigger
#[tokio::test]
async fn test_live_asset_sync() {
    init_tracing();
    info!("=== test_live_asset_sync ===");

    // Setup: owner + node with handshake
    let peers = setup_with_handshake("owner", "node").await.unwrap();

    // Create and publish space/page first (no assets yet)
    let user_info = peers.owner_butler.user_info().await.unwrap();
    let space = peers
        .owner_butler
        .create_space(
            "Live Asset Test Space".to_string(),
            user_info.did.clone(),
            TEST_SPACE_TEMPLATE,
        )
        .await
        .unwrap();

    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
    let page = peers
        .owner_butler
        .create_page(
            &space.id,
            "Live Asset Test Page",
            layer_names,
            TEST_PAGE_TEMPLATE,
        )
        .await
        .unwrap();

    // Publish space first (establishes sync relationship)
    let node_node_id = peers.node_id();
    peers
        .owner_coordinator()
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .unwrap();

    // Wait for page to sync
    peers
        .harness
        .wait_for_page(&peers.node_butler, &page.id)
        .await
        .unwrap();

    info!("Initial publish complete, now uploading asset for live sync");

    // Now upload asset to the already-synced page (triggers live asset sync)
    let test_data = generate_test_image(99, 2048);
    let asset = upload_test_asset(&peers.owner_butler, &page.id, "live_image.png", &test_data)
        .await
        .unwrap();

    info!(
        hash = %asset.hash,
        "Live asset uploaded, waiting for sync"
    );

    // Wait for asset blob to arrive via live sync
    // Flow: upload → assets layer MapInsert → SyncOffer → SyncAccept → SyncAck
    //       → trigger_asset_sync_after_layer_sync → AssetPrepare → AssetReady → AssetAck
    wait_for_asset_blob(&peers.node_butler, &asset.hash, ASSET_SYNC_DELAY)
        .await
        .unwrap();

    // Verify content
    assert_asset_synced(
        &peers.owner_butler,
        &peers.node_butler,
        &page.id,
        &asset.hash,
    )
    .await
    .unwrap();

    info!("test_live_asset_sync PASSED");
    peers.shutdown().await;
}

/// Test 3: Multiple assets sync correctly
///
/// **Setup**: Owner + node with completed handshake
/// **Action**: Upload 3 assets, then publish
/// **Expected**: All 3 asset blobs arrive on node, all content matches
#[tokio::test]
async fn test_multiple_assets_sync() {
    init_tracing();
    info!("=== test_multiple_assets_sync ===");

    // Setup: owner + node with handshake
    let peers = setup_with_handshake("owner", "node").await.unwrap();

    // Create space and page
    let user_info = peers.owner_butler.user_info().await.unwrap();
    let space = peers
        .owner_butler
        .create_space(
            "Multi-Asset Test Space".to_string(),
            user_info.did.clone(),
            TEST_SPACE_TEMPLATE,
        )
        .await
        .unwrap();

    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
    let page = peers
        .owner_butler
        .create_page(
            &space.id,
            "Multi-Asset Test Page",
            layer_names,
            TEST_PAGE_TEMPLATE,
        )
        .await
        .unwrap();

    // Upload multiple assets
    let data1 = generate_test_image(1, 512);
    let data2 = generate_test_image(2, 1024);
    let data3 = generate_test_image(3, 768);

    let asset1 = upload_test_asset(&peers.owner_butler, &page.id, "img1.png", &data1)
        .await
        .unwrap();
    let asset2 = upload_test_asset(&peers.owner_butler, &page.id, "img2.png", &data2)
        .await
        .unwrap();
    let asset3 = upload_test_asset(&peers.owner_butler, &page.id, "doc.pdf", &data3)
        .await
        .unwrap();

    info!(
        assets = ?[&asset1.hash[..8], &asset2.hash[..8], &asset3.hash[..8]],
        "3 assets uploaded, now publishing"
    );

    // Publish space
    let node_node_id = peers.node_id();
    peers
        .owner_coordinator()
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .unwrap();

    // Wait for page
    peers
        .harness
        .wait_for_page(&peers.node_butler, &page.id)
        .await
        .unwrap();

    info!("Page synced, waiting for all asset blobs");

    // Wait for all 3 asset blobs
    wait_for_asset_blob(&peers.node_butler, &asset1.hash, ASSET_SYNC_DELAY)
        .await
        .unwrap();
    wait_for_asset_blob(&peers.node_butler, &asset2.hash, ASSET_SYNC_DELAY)
        .await
        .unwrap();
    wait_for_asset_blob(&peers.node_butler, &asset3.hash, ASSET_SYNC_DELAY)
        .await
        .unwrap();

    // Verify all content matches
    assert_asset_synced(
        &peers.owner_butler,
        &peers.node_butler,
        &page.id,
        &asset1.hash,
    )
    .await
    .unwrap();

    assert_asset_synced(
        &peers.owner_butler,
        &peers.node_butler,
        &page.id,
        &asset2.hash,
    )
    .await
    .unwrap();

    assert_asset_synced(
        &peers.owner_butler,
        &peers.node_butler,
        &page.id,
        &asset3.hash,
    )
    .await
    .unwrap();

    info!("test_multiple_assets_sync PASSED");
    peers.shutdown().await;
}

/// Test 4: Node broadcasts asset to viewer (3-party flow)
///
/// **Setup**: Owner + node + viewer with published page
/// **Action**: Owner uploads new asset (live sync to node, then broadcast to viewer)
/// **Expected**: Asset blob arrives on both node AND viewer, content matches
///
/// **Flow**:
/// 1. Owner uploads asset → metadata to {page_id}/assets layer
/// 2. Owner's Scribe broadcasts SyncOffer to node
/// 3. Node receives, applies, sends SyncAck → triggers asset fetch from owner
/// 4. Node stores asset, then broadcasts SyncOffer to subscribed viewer
/// 5. Viewer receives metadata, sends AssetPrepare to node
/// 6. Node responds with AssetReady, viewer downloads and stores
#[tokio::test]
async fn test_asset_broadcast_to_viewer() {
    init_tracing();
    info!("=== test_asset_broadcast_to_viewer ===");

    // Setup: owner + node + viewer with published page (no assets yet)
    let ctx = setup_with_viewer("owner", "node", "viewer").await.unwrap();

    info!(
        page_id = %ctx.page_id,
        "3-party setup complete, uploading asset for broadcast test"
    );

    // Owner uploads asset to the already-synced page
    let test_data = generate_test_image(777, 1536);
    let asset = upload_test_asset(&ctx.owner_butler, &ctx.page_id, "broadcast_image.png", &test_data)
        .await
        .unwrap();

    info!(
        hash = %asset.hash,
        "Asset uploaded by owner, waiting for sync to node"
    );

    // Wait for asset to arrive on node first (owner → node)
    wait_for_asset_blob(&ctx.node_butler, &asset.hash, ASSET_SYNC_DELAY)
        .await
        .unwrap();

    info!("Asset arrived on node, waiting for broadcast to viewer");

    // Wait for asset to arrive on viewer (node → viewer broadcast)
    wait_for_asset_blob(&ctx.viewer.butler, &asset.hash, ASSET_SYNC_DELAY)
        .await
        .unwrap();

    // Verify content matches on node
    assert_asset_synced(
        &ctx.owner_butler,
        &ctx.node_butler,
        &ctx.page_id,
        &asset.hash,
    )
    .await
    .unwrap();

    // Verify content matches on viewer
    assert_asset_synced(
        &ctx.owner_butler,
        &ctx.viewer.butler,
        &ctx.page_id,
        &asset.hash,
    )
    .await
    .unwrap();

    info!("test_asset_broadcast_to_viewer PASSED");
    ctx.shutdown().await;
}
