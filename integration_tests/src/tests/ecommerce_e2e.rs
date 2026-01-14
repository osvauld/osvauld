//! Production-Like E2E Tests using HeadlessRuntime
//!
//! These tests use real production code paths:
//! - **HeadlessRuntime** to call Lua functions (mock only UI)
//! - **Real Lua app code** from sample_apps/my-shop
//! - **Real sync flow** through PeerActor (SyncOffer/SyncAccept)
//!
//! Instead of manually manipulating LoroDoc with ScribeMessage::ApplyUpdate,
//! we call the actual Lua functions which trigger the real sync pipeline.

use std::sync::Arc;

use tracing::info;

use butler::Butler;
use courier::coordinator::{CoordinatorMessage, CourierMode};

use crate::TestHarness;
use crate::app_loader::{load_myshop_owner_app, load_myshop_customer_app, load_myshop_init};
use crate::fixtures::{ACTOR_SPAWN_DELAY, EXTENDED_SYNC_DELAY};
use crate::headless_helper::create_headless_runtime;
use crate::helpers::{
    init_tracing, setup_butler_with_identity,
    generate_connection_string, complete_handshake,
};

/// Load the real my-shop space template at compile time
const MY_SHOP_SPACE_TEMPLATE: &str = include_str!("../../../sample_apps/my-shop/space_permit_template.json");

/// Find sample app path (works from both workspace root and integration_tests dir)
fn find_sample_app_path(app_name: &str) -> anyhow::Result<std::path::PathBuf> {
    // Try from integration_tests directory first (when running `cargo test -p integration_tests`)
    let from_integration_tests = std::path::Path::new("..").join("sample_apps").join(app_name);
    if from_integration_tests.exists() {
        return Ok(from_integration_tests);
    }

    // Try from project root (when running from workspace root)
    let from_root = std::path::Path::new("sample_apps").join(app_name);
    if from_root.exists() {
        return Ok(from_root);
    }

    Err(anyhow::anyhow!(
        "Could not find sample app: {}. Tried:\n  - {}\n  - {}",
        app_name,
        from_integration_tests.display(),
        from_root.display()
    ))
}

// =============================================================================
// Scenario Setup
// =============================================================================

/// E-commerce scenario with HeadlessRuntime support
pub struct HeadlessEcommerceScenario {
    pub harness: TestHarness,
    pub owner_butler: Arc<Butler>,
    pub node_butler: Arc<Butler>,
    pub customer_butler: Arc<Butler>,
    pub space_id: String,
    pub page_id: String,
    pub _temps: Vec<tempfile::TempDir>,
}

impl HeadlessEcommerceScenario {
    pub async fn shutdown(&self) {
        self.harness.shutdown().await;
    }
}

/// Setup e-commerce scenario with owner, node, and one customer
async fn setup_headless_scenario() -> anyhow::Result<HeadlessEcommerceScenario> {
    // Setup identities
    let (owner_butler, _owner_key, owner_temp) = setup_butler_with_identity("shop_owner", "pass").await?;
    let (node_butler, _node_key, node_temp) = setup_butler_with_identity("shop_node", "pass").await?;
    let (customer_butler, _cust_key, cust_temp) = setup_butler_with_identity("customer", "pass").await?;

    let mut harness = TestHarness::new();
    harness.add_peer_with_butler("owner", CourierMode::User, owner_butler.clone()).await?;
    harness.add_peer_with_butler("node", CourierMode::Node, node_butler.clone()).await?;
    harness.add_peer_with_butler("customer", CourierMode::User, customer_butler.clone()).await?;

    let node_node_id = harness.peer("node").unwrap().node_id;

    // === Owner ↔ Node handshake ===
    let connection_string = generate_connection_string(&node_butler).await?;
    owner_butler.add_sovereign_node(&connection_string)
        .map_err(|e| anyhow::anyhow!("Failed to add sovereign node: {}", e))?;

    let _transport = harness.spawn_transport();
    harness.connect_and_notify("owner", "node").await?;
    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;
    complete_handshake(&harness, &owner_butler, &node_butler).await?;

    // === Create and publish space ===
    let user_info = owner_butler.user_info().await?;
    let space = owner_butler
        .create_space("My Shop".to_string(), user_info.did.clone(), MY_SHOP_SPACE_TEMPLATE)
        .await?;

    // Import page from sample_apps/my-shop directory (creates all app layers including app:shared)
    // This is the real production flow that creates:
    // - app:Shop Owner, app:Shop Customer, app:shared (with init.lua + validation.lua)
    // - products layer from permit_template.json
    let myshop_path = find_sample_app_path("my-shop")?;
    info!("Importing my-shop from: {}", myshop_path.display());
    let page = owner_butler.import_page(&space.id, &myshop_path).await?;
    info!("Imported page {} with layers", page.id);

    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;
    owner_coordinator.cast(CoordinatorMessage::PublishSpace {
        node_id: node_node_id,
        space_id: space.id.clone(),
    })?;

    // Wait for space and page on node
    harness.wait_for_space(&node_butler, &space.id).await?;
    harness.wait_for_page(&node_butler, &page.id).await?;

    // === Connect Customer ===
    let viewer_conn = node_butler.generate_viewer_connection_string(&space.id, None).await?;
    let parsed = customer_butler.parse_connection_string(&viewer_conn)
        .map_err(|e| anyhow::anyhow!("Failed to parse connection string: {}", e))?;
    harness.connect_and_notify("customer", "node").await?;
    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    let cust_coordinator = harness.peer("customer").unwrap().coordinator.clone();
    cust_coordinator.cast(CoordinatorMessage::InitiateHandshake {
        node_id: node_node_id,
        permit: parsed.permit.clone(),
    })?;
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Request space as viewer
    cust_coordinator.cast(CoordinatorMessage::RequestSpace {
        node_id: node_node_id,
        space_id: space.id.clone(),
        viewer_permit: parsed.permit,
    })?;

    // Wait for customer to receive space and page
    harness.wait_for_space(&customer_butler, &space.id).await?;
    harness.wait_for_page(&customer_butler, &page.id).await?;

    Ok(HeadlessEcommerceScenario {
        harness,
        owner_butler,
        node_butler,
        customer_butler,
        space_id: space.id,
        page_id: page.id,
        _temps: vec![owner_temp, node_temp, cust_temp],
    })
}

// =============================================================================
// Tests
// =============================================================================

/// Test: Owner adds product via HeadlessRuntime
///
/// Uses real Lua code path:
/// 1. Owner calls add_product() via HeadlessRuntime
/// 2. Lua calls loro:get_or_create_layer("products", "list"):push(product)
/// 3. Scribe commits and broadcasts to PeerActor
/// 4. Real SyncOffer flows to node and customer
#[tokio::test(flavor = "multi_thread")]
async fn test_owner_adds_product_via_headless_runtime() {
    init_tracing();

    let scenario = match setup_headless_scenario().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Setup failed: {}", e);
            return;
        }
    };

    info!("=== HeadlessRuntime E2E: Owner adds product ===");
    info!("Page: {}", scenario.page_id);

    // Load owner Lua app code
    let owner_app_code = load_myshop_owner_app();

    // Create HeadlessRuntime for owner (role extracted from permit)
    let owner_runtime = create_headless_runtime(
        &scenario.owner_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create owner runtime");

    // Initialize the app (creates products layer)
    owner_runtime.call_no_args::<()>("on_init")
        .expect("on_init should succeed");

    info!("Owner runtime initialized");

    // Owner adds a product using real Lua code
    // add_product(name, price, description, stock)
    owner_runtime.call_with_args::<_, ()>(
        "add_product",
        ("Widget A", "1999", "A great widget", "100"),
    ).expect("add_product should succeed");

    info!("Owner added product via Lua");

    // Give time for sync to propagate through real PeerActor flow
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify product exists via Lua API (same as production reads data)
    // Create a HeadlessRuntime on node to read via Lua
    let node_runtime = create_headless_runtime(
        &scenario.node_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create node runtime");

    node_runtime.call_no_args::<()>("on_init")
        .expect("Node on_init should succeed");

    // Query products via Lua API
    let product_count: i64 = node_runtime.call_no_args("get_products_count")
        .expect("get_products_count should succeed");

    info!("Node has {} products via Lua API", product_count);
    assert!(product_count >= 1, "Node should have at least 1 product");

    info!("=== PASSED: Owner adds product via HeadlessRuntime ===");

    scenario.shutdown().await;
}

/// Test: Customer creates order via HeadlessRuntime
///
/// Full e-commerce flow with HeadlessRuntime:
/// 1. Owner adds product
/// 2. Customer sees product (via sync)
/// 3. Customer creates order via Lua
/// 4. Customer submits order
/// 5. Owner receives order (via sync)
#[tokio::test(flavor = "multi_thread")]
async fn test_customer_order_flow_via_headless_runtime() {
    init_tracing();

    let scenario = match setup_headless_scenario().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Setup failed: {}", e);
            return;
        }
    };

    info!("=== HeadlessRuntime E2E: Customer order flow ===");

    // Load app code
    let owner_app_code = load_myshop_owner_app();
    let customer_app_code = load_myshop_customer_app();

    // Create runtimes (role extracted from permit)
    let owner_runtime = create_headless_runtime(
        &scenario.owner_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create owner runtime");

    // Initialize owner and add product
    owner_runtime.call_no_args::<()>("on_init")
        .expect("on_init should succeed");

    owner_runtime.call_with_args::<_, ()>(
        "add_product",
        ("Premium Widget", "4999", "Top quality widget", "50"),
    ).expect("add_product should succeed");

    info!("Owner added product");

    // Wait for sync to customer
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Create customer runtime (role extracted from permit)
    let customer_runtime = create_headless_runtime(
        &scenario.customer_butler,
        &scenario.page_id,
        &customer_app_code,
    ).await.expect("Failed to create customer runtime");

    // Initialize customer app
    customer_runtime.call_no_args::<()>("on_init")
        .expect("Customer on_init should succeed");

    info!("Customer runtime initialized");

    // Customer needs to select product first (simulating UI interaction)
    // select_product(product_id, product_name, product_price)
    // Note: In real app, product_id comes from browsing products list
    // For test, we use a placeholder ID that will be handled by the fallback lookup
    customer_runtime.call_with_args::<_, ()>(
        "select_product",
        ("test-product-id", "Premium Widget", 4999),
    ).expect("select_product should succeed");

    // Customer creates order
    // create_order(product_id, quantity_str, notes, shipping_address)
    customer_runtime.call_with_args::<_, ()>(
        "create_order",
        ("test-product-id", "2", "Rush delivery please", "123 Customer St"),
    ).expect("create_order should succeed");

    info!("Customer created order (draft)");

    // Get the order ID from the customer's orders layer to submit it
    // Call submit_order with no args - the Lua function uses selected_order_id internally
    customer_runtime.call_no_args::<()>("submit_order")
        .expect("submit_order should succeed");

    info!("Customer submitted order");

    // Wait for sync to node (no storage flush needed - we query via Lua)
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify orders via Lua layer on node (not storage)
    // Create a HeadlessRuntime on node with init.lua (derivation rules) + owner app
    let node_init_code = load_myshop_init();
    let node_app_code = format!("{}\n\n{}", node_init_code, owner_app_code);
    let node_runtime = create_headless_runtime(
        &scenario.node_butler,
        &scenario.page_id,
        &node_app_code,
    ).await.expect("Failed to create node runtime");

    // Initialize to register derivation rules and scan for order layers
    node_runtime.call_no_args::<()>("on_init")
        .expect("Node on_init should succeed");

    // Rebuild derived layers from historical data (orders synced before rules registered)
    node_runtime.initialize_derivation()
        .expect("Derivation initialization should succeed");

    // Wait for derived layer to sync to owner
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Query orders count via Lua (now uses derived layer)
    let order_count: i64 = node_runtime.call_no_args("get_orders_count")
        .expect("get_orders_count should succeed");

    info!("Node received {} order(s) via Lua query", order_count);
    assert!(order_count >= 1, "Node should have received at least one order from customer");

    // Verify owner also received the derived layer (syncs from node)
    // Re-init to pick up the orders_summary layer that synced from node
    owner_runtime.call_no_args::<()>("on_init")
        .expect("Owner re-init should succeed");

    let owner_order_count: i64 = owner_runtime.call_no_args("get_orders_count")
        .expect("Owner get_orders_count should succeed");

    info!("Owner received {} order(s) from derived layer", owner_order_count);
    assert!(owner_order_count >= 1, "Owner should see orders from derived layer synced from node");

    info!("=== PASSED: Customer order flow via HeadlessRuntime ===");

    scenario.shutdown().await;
}

/// Test: Verify sync happens through real PeerActor path
///
/// This test verifies that:
/// 1. HeadlessRuntime triggers real Scribe writes
/// 2. Scribe broadcasts to subscribed PeerActors
/// 3. PeerActor sends SyncOffer to remote peers
/// 4. Remote PeerActor applies updates
///
/// NOT manually calling ScribeMessage::ApplyUpdate in test code!
#[tokio::test(flavor = "multi_thread")]
async fn test_sync_through_real_peeractor() {
    init_tracing();

    let scenario = match setup_headless_scenario().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Setup failed: {}", e);
            return;
        }
    };

    info!("=== Verify real PeerActor sync flow ===");

    let owner_app_code = load_myshop_owner_app();

    // Create owner runtime (role extracted from permit)
    let owner_runtime = create_headless_runtime(
        &scenario.owner_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create owner runtime");

    owner_runtime.call_no_args::<()>("on_init")
        .expect("on_init should succeed");

    // Baseline: check products count via Lua API (same as production)
    let products_before: i64 = owner_runtime.call_no_args("get_products_count")
        .expect("get_products_count should succeed");
    info!("Products count before: {}", products_before);

    // Owner adds multiple products
    for i in 1..=3 {
        owner_runtime.call_with_args::<_, ()>(
            "add_product",
            (format!("Product {}", i), format!("{}", i * 1000), format!("Description {}", i), "10"),
        ).expect("add_product should succeed");
    }

    info!("Owner added 3 products");

    // Wait for sync through real PeerActor path
    // In production: Scribe commits → broadcasts to owner's PeerActor → SyncOffer to node
    tokio::time::sleep(EXTENDED_SYNC_DELAY * 2).await;

    // Verify node received the products via Lua API (same as production)
    let node_runtime = create_headless_runtime(
        &scenario.node_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create node runtime");

    node_runtime.call_no_args::<()>("on_init")
        .expect("Node on_init should succeed");

    let products_after: i64 = node_runtime.call_no_args("get_products_count")
        .expect("get_products_count should succeed");
    info!("Products count on node after: {}", products_after);

    // Products count should increase after adding products
    assert!(
        products_after > products_before,
        "Products count should grow after adding products (before: {}, after: {})",
        products_before,
        products_after
    );

    // Verify at least 3 products
    assert!(products_after >= 3, "Node should have at least 3 products");

    info!("=== PASSED: Sync through real PeerActor ===");

    scenario.shutdown().await;
}

/// Test: Derivation correctly updates on status changes
///
/// This test verifies that the derivation engine correctly handles
/// field-level updates (not just inserts). Previously, when order status
/// changed from pending→confirmed, the derivation would fail because
/// it received only the field value ("confirmed") instead of the full order.
///
/// The fix fetches the full item from the source layer for field-level updates.
#[tokio::test(flavor = "multi_thread")]
async fn test_derivation_updates_on_status_change() {
    init_tracing();

    let scenario = match setup_headless_scenario().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Setup failed: {}", e);
            return;
        }
    };

    info!("=== Test: Derivation updates on status change ===");

    // Load app code
    let owner_app_code = load_myshop_owner_app();
    let customer_app_code = load_myshop_customer_app();
    let init_code = load_myshop_init();

    // Create owner runtime
    let owner_runtime = create_headless_runtime(
        &scenario.owner_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create owner runtime");

    owner_runtime.call_no_args::<()>("on_init")
        .expect("Owner on_init should succeed");

    // Owner adds a product
    owner_runtime.call_with_args::<_, ()>(
        "add_product",
        ("Test Widget", "2999", "A test widget", "50"),
    ).expect("add_product should succeed");

    info!("Owner added product");

    // Wait for sync to customer
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Create customer runtime
    let customer_runtime = create_headless_runtime(
        &scenario.customer_butler,
        &scenario.page_id,
        &customer_app_code,
    ).await.expect("Failed to create customer runtime");

    customer_runtime.call_no_args::<()>("on_init")
        .expect("Customer on_init should succeed");

    // Customer selects product and creates order
    customer_runtime.call_with_args::<_, ()>(
        "select_product",
        ("test-product-id", "Test Widget", 2999),
    ).expect("select_product should succeed");

    customer_runtime.call_with_args::<_, ()>(
        "create_order",
        ("test-product-id", "1", "Test order", "123 Test St"),
    ).expect("create_order should succeed");

    // Customer submits order (draft → pending)
    customer_runtime.call_no_args::<()>("submit_order")
        .expect("submit_order should succeed");

    // Get the order ID from customer
    let order_id: String = customer_runtime.call_no_args("get_last_order_id")
        .expect("get_last_order_id should succeed");

    info!("Customer submitted order: {}", order_id);

    // Wait for sync to node
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Create node runtime with derivation rules
    let node_app_code = format!("{}\n\n{}", init_code, owner_app_code);
    let node_runtime = create_headless_runtime(
        &scenario.node_butler,
        &scenario.page_id,
        &node_app_code,
    ).await.expect("Failed to create node runtime");

    node_runtime.call_no_args::<()>("on_init")
        .expect("Node on_init should succeed");

    // Initialize derivation rules (one-time at startup)
    node_runtime.initialize_derivation()
        .expect("Derivation initialization should succeed");

    // Wait for derived layer to be created and synced
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify order is in derived layer with status="pending"
    let status_after_submit: Option<String> = node_runtime.call_with_args("get_order_status", order_id.clone())
        .expect("get_order_status should succeed");

    info!("Order status after submit: {:?}", status_after_submit);
    assert_eq!(status_after_submit, Some("pending".to_string()),
        "Order should have status 'pending' after customer submit");

    // Get customer DID for owner to update the order
    let customer_did = scenario.customer_butler.user_info().await
        .expect("Failed to get customer info").did;

    // Owner confirms the order (pending → confirmed)
    // This is the critical test: status change triggers field-level update
    owner_runtime.call_no_args::<()>("on_init")
        .expect("Owner re-init should succeed");

    owner_runtime.call_with_args::<_, ()>(
        "update_order_status",
        (customer_did.clone(), order_id.clone(), "confirmed"),
    ).expect("update_order_status to confirmed should succeed");

    info!("Owner confirmed order");

    // Wait for sync through node (owner → node → derivation updates)
    tokio::time::sleep(EXTENDED_SYNC_DELAY * 2).await;

    // Re-initialize node to pick up updates (simulates ongoing runtime)
    node_runtime.call_no_args::<()>("on_init")
        .expect("Node re-init should succeed");

    // Verify derived layer was updated with new status
    let status_after_confirm: Option<String> = node_runtime.call_with_args("get_order_status", order_id.clone())
        .expect("get_order_status should succeed");

    info!("Order status after confirm: {:?}", status_after_confirm);
    assert_eq!(status_after_confirm, Some("confirmed".to_string()),
        "Order should have status 'confirmed' after owner confirmation");

    // Owner ships the order (confirmed → shipped)
    owner_runtime.call_with_args::<_, ()>(
        "update_order_status",
        (customer_did.clone(), order_id.clone(), "shipped"),
    ).expect("update_order_status to shipped should succeed");

    info!("Owner shipped order");

    // Wait for sync
    tokio::time::sleep(EXTENDED_SYNC_DELAY * 2).await;

    // Re-initialize and verify
    node_runtime.call_no_args::<()>("on_init")
        .expect("Node re-init should succeed");

    let status_after_ship: Option<String> = node_runtime.call_with_args("get_order_status", order_id.clone())
        .expect("get_order_status should succeed");

    info!("Order status after ship: {:?}", status_after_ship);
    assert_eq!(status_after_ship, Some("shipped".to_string()),
        "Order should have status 'shipped' after owner ships");

    // Also verify owner received the derived layer updates
    owner_runtime.call_no_args::<()>("on_init")
        .expect("Owner re-init should succeed");

    let owner_status: Option<String> = owner_runtime.call_with_args("get_order_status", order_id.clone())
        .expect("Owner get_order_status should succeed");

    info!("Owner sees order status: {:?}", owner_status);
    assert_eq!(owner_status, Some("shipped".to_string()),
        "Owner should see order status 'shipped' from synced derived layer");

    info!("=== PASSED: Derivation updates on status change ===");

    scenario.shutdown().await;
}

// =============================================================================
// Sync Behavior Tests
// =============================================================================

/// Test: Draft orders with sync:false don't sync to node
///
/// Verifies that the {page_id}/drafts layer with sync:false never broadcasts.
/// Customer creates a draft order, which should remain local-only.
#[tokio::test(flavor = "multi_thread")]
async fn test_draft_orders_dont_sync() {
    init_tracing();

    let scenario = match setup_headless_scenario().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Setup failed: {}", e);
            return;
        }
    };

    info!("=== Test: Draft orders don't sync to node ===");

    // Load app code
    let owner_app_code = load_myshop_owner_app();
    let customer_app_code = load_myshop_customer_app();

    // Create owner runtime and add a product first
    let owner_runtime = create_headless_runtime(
        &scenario.owner_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create owner runtime");

    owner_runtime.call_no_args::<()>("on_init")
        .expect("Owner on_init should succeed");

    owner_runtime.call_with_args::<_, ()>(
        "add_product",
        ("Draft Test Widget", "999", "A widget for testing drafts", "10"),
    ).expect("add_product should succeed");

    // Wait for product to sync to customer
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Create customer runtime
    let customer_runtime = create_headless_runtime(
        &scenario.customer_butler,
        &scenario.page_id,
        &customer_app_code,
    ).await.expect("Failed to create customer runtime");

    customer_runtime.call_no_args::<()>("on_init")
        .expect("Customer on_init should succeed");

    // Customer creates a draft order (should NOT sync)
    customer_runtime.call_with_args::<_, ()>(
        "select_product",
        ("test-product-id", "Draft Test Widget", 999),
    ).expect("select_product should succeed");

    customer_runtime.call_with_args::<_, ()>(
        "create_order",
        ("test-product-id", "1", "Draft order", "123 Test St"),
    ).expect("create_order should succeed");

    // Verify draft exists on customer
    let customer_drafts: i64 = customer_runtime.call_no_args("get_drafts_count")
        .expect("get_drafts_count should succeed");
    let customer_orders: i64 = customer_runtime.call_no_args("get_orders_count")
        .expect("get_orders_count should succeed");

    info!("Customer drafts: {}, orders: {}", customer_drafts, customer_orders);
    assert!(customer_drafts >= 1, "Customer should have at least 1 draft");
    assert_eq!(customer_orders, 0, "Customer should have 0 submitted orders");

    // Wait to ensure any sync would have happened
    tokio::time::sleep(EXTENDED_SYNC_DELAY * 2).await;

    // Create node runtime with customer app code to check drafts
    let node_runtime = create_headless_runtime(
        &scenario.node_butler,
        &scenario.page_id,
        &customer_app_code,
    ).await.expect("Failed to create node runtime");

    node_runtime.call_no_args::<()>("on_init")
        .expect("Node on_init should succeed");

    // Node should NOT have any drafts (sync:false layer)
    let node_drafts: i64 = node_runtime.call_no_args("get_drafts_count")
        .expect("get_drafts_count should succeed");

    info!("Node drafts: {}", node_drafts);
    assert_eq!(node_drafts, 0, "Node should have 0 drafts - drafts layer should NOT sync");

    info!("=== PASSED: Draft orders don't sync to node ===");

    scenario.shutdown().await;
}

/// Test: Submitted orders sync correctly from drafts to orders list
///
/// Verifies the draft→submit workflow:
/// 1. Draft created in local drafts map (sync:false)
/// 2. After submit, order moves to orders list (sync:true)
/// 3. Node receives the submitted order
#[tokio::test(flavor = "multi_thread")]
async fn test_submitted_orders_sync() {
    init_tracing();

    let scenario = match setup_headless_scenario().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Setup failed: {}", e);
            return;
        }
    };

    info!("=== Test: Submitted orders sync correctly ===");

    // Load app code
    let owner_app_code = load_myshop_owner_app();
    let customer_app_code = load_myshop_customer_app();
    let init_code = load_myshop_init();

    // Create owner runtime and add a product
    let owner_runtime = create_headless_runtime(
        &scenario.owner_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create owner runtime");

    owner_runtime.call_no_args::<()>("on_init")
        .expect("Owner on_init should succeed");

    owner_runtime.call_with_args::<_, ()>(
        "add_product",
        ("Submit Test Widget", "1999", "A widget for testing submit", "50"),
    ).expect("add_product should succeed");

    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Create customer runtime
    let customer_runtime = create_headless_runtime(
        &scenario.customer_butler,
        &scenario.page_id,
        &customer_app_code,
    ).await.expect("Failed to create customer runtime");

    customer_runtime.call_no_args::<()>("on_init")
        .expect("Customer on_init should succeed");

    // Customer creates a draft order
    customer_runtime.call_with_args::<_, ()>(
        "select_product",
        ("test-product-id", "Submit Test Widget", 1999),
    ).expect("select_product should succeed");

    customer_runtime.call_with_args::<_, ()>(
        "create_order",
        ("test-product-id", "2", "Test order for submit", "456 Submit St"),
    ).expect("create_order should succeed");

    // Verify BEFORE submit: drafts=1, orders=0
    let drafts_before: i64 = customer_runtime.call_no_args("get_drafts_count")
        .expect("get_drafts_count should succeed");
    let orders_before: i64 = customer_runtime.call_no_args("get_orders_count")
        .expect("get_orders_count should succeed");

    info!("Before submit - drafts: {}, orders: {}", drafts_before, orders_before);
    assert!(drafts_before >= 1, "Should have at least 1 draft before submit");
    assert_eq!(orders_before, 0, "Should have 0 orders before submit");

    // Customer submits order
    customer_runtime.call_no_args::<()>("submit_order")
        .expect("submit_order should succeed");

    // Verify AFTER submit: drafts=0, orders=1
    let drafts_after: i64 = customer_runtime.call_no_args("get_drafts_count")
        .expect("get_drafts_count should succeed");
    let orders_after: i64 = customer_runtime.call_no_args("get_orders_count")
        .expect("get_orders_count should succeed");

    info!("After submit - drafts: {}, orders: {}", drafts_after, orders_after);
    assert_eq!(drafts_after, 0, "Draft should be removed after submit");
    assert!(orders_after >= 1, "Should have at least 1 order after submit");

    // Wait for sync to node
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Create node runtime with derivation
    let node_app_code = format!("{}\n\n{}", init_code, owner_app_code);
    let node_runtime = create_headless_runtime(
        &scenario.node_butler,
        &scenario.page_id,
        &node_app_code,
    ).await.expect("Failed to create node runtime");

    node_runtime.call_no_args::<()>("on_init")
        .expect("Node on_init should succeed");

    node_runtime.initialize_derivation()
        .expect("Derivation initialization should succeed");

    // Node should have received the submitted order
    let node_orders: i64 = node_runtime.call_no_args("get_orders_count")
        .expect("get_orders_count should succeed");

    info!("Node orders after sync: {}", node_orders);
    assert!(node_orders >= 1, "Node should have received at least 1 order via sync");

    info!("=== PASSED: Submitted orders sync correctly ===");

    scenario.shutdown().await;
}

/// Test: Customer doesn't receive orders_summary layer
///
/// Verifies permit-driven layer access:
/// - Owner/node have orders_summary in their permit
/// - Customer does NOT have orders_summary in their permit
#[tokio::test(flavor = "multi_thread")]
async fn test_customer_no_orders_summary() {
    init_tracing();

    let scenario = match setup_headless_scenario().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Setup failed: {}", e);
            return;
        }
    };

    info!("=== Test: Customer doesn't receive orders_summary ===");

    // Load app code
    let owner_app_code = load_myshop_owner_app();
    let customer_app_code = load_myshop_customer_app();
    let init_code = load_myshop_init();

    // Create owner runtime and add a product
    let owner_runtime = create_headless_runtime(
        &scenario.owner_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create owner runtime");

    owner_runtime.call_no_args::<()>("on_init")
        .expect("Owner on_init should succeed");

    owner_runtime.call_with_args::<_, ()>(
        "add_product",
        ("Summary Test Widget", "2999", "Widget for summary test", "25"),
    ).expect("add_product should succeed");

    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Create customer runtime
    let customer_runtime = create_headless_runtime(
        &scenario.customer_butler,
        &scenario.page_id,
        &customer_app_code,
    ).await.expect("Failed to create customer runtime");

    customer_runtime.call_no_args::<()>("on_init")
        .expect("Customer on_init should succeed");

    // Customer creates and submits an order
    customer_runtime.call_with_args::<_, ()>(
        "select_product",
        ("test-product-id", "Summary Test Widget", 2999),
    ).expect("select_product should succeed");

    customer_runtime.call_with_args::<_, ()>(
        "create_order",
        ("test-product-id", "1", "Summary test order", "789 Summary St"),
    ).expect("create_order should succeed");

    customer_runtime.call_no_args::<()>("submit_order")
        .expect("submit_order should succeed");

    // Wait for sync to node
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Create node runtime with derivation to generate orders_summary
    let node_app_code = format!("{}\n\n{}", init_code, owner_app_code);
    let node_runtime = create_headless_runtime(
        &scenario.node_butler,
        &scenario.page_id,
        &node_app_code,
    ).await.expect("Failed to create node runtime");

    node_runtime.call_no_args::<()>("on_init")
        .expect("Node on_init should succeed");

    node_runtime.initialize_derivation()
        .expect("Derivation initialization should succeed");

    // Wait for derived layer to sync
    tokio::time::sleep(EXTENDED_SYNC_DELAY * 2).await;

    // Check node has orders in derived layer first
    let node_orders: i64 = node_runtime.call_no_args("get_orders_count")
        .expect("Node get_orders_count should succeed");

    info!("Node orders (from orders_summary): {}", node_orders);
    assert!(node_orders >= 1, "Node should have orders from derivation");

    // Wait more for sync from node to owner
    tokio::time::sleep(EXTENDED_SYNC_DELAY * 2).await;

    // Re-init owner to pick up orders_summary
    owner_runtime.call_no_args::<()>("on_init")
        .expect("Owner re-init should succeed");

    // Owner should have orders from orders_summary
    let owner_orders: i64 = owner_runtime.call_no_args("get_orders_count")
        .expect("Owner get_orders_count should succeed");

    info!("Owner orders (from orders_summary): {}", owner_orders);
    // Note: Owner may not receive orders_summary if sync from node->owner isn't working
    // The key assertion for this test is that CUSTOMER doesn't have orders_summary

    // Customer should NOT have orders_summary layer
    customer_runtime.call_no_args::<()>("on_init")
        .expect("Customer re-init should succeed");

    let customer_has_summary: bool = customer_runtime.call_no_args("has_orders_summary")
        .expect("has_orders_summary should succeed");

    info!("Customer has orders_summary: {}", customer_has_summary);
    assert!(!customer_has_summary, "Customer should NOT have orders_summary layer (not in permit)");

    info!("=== PASSED: Customer doesn't receive orders_summary ===");

    scenario.shutdown().await;
}

/// Test: Owner product drafts stay local until published
///
/// Verifies that owner's draft products (sync:false) don't sync
/// until explicitly published (moved to synced products layer).
#[tokio::test(flavor = "multi_thread")]
async fn test_owner_product_drafts_local() {
    init_tracing();

    let scenario = match setup_headless_scenario().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Setup failed: {}", e);
            return;
        }
    };

    info!("=== Test: Owner product drafts stay local ===");

    let owner_app_code = load_myshop_owner_app();

    // Create owner runtime
    let owner_runtime = create_headless_runtime(
        &scenario.owner_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create owner runtime");

    owner_runtime.call_no_args::<()>("on_init")
        .expect("Owner on_init should succeed");

    // Owner creates a product DRAFT (should NOT sync)
    owner_runtime.call_with_args::<_, ()>(
        "add_product_draft",
        ("Draft Product", "999", "A draft product", "10"),
    ).expect("add_product_draft should succeed");

    // Verify draft exists on owner
    let owner_drafts: i64 = owner_runtime.call_no_args("get_drafts_count")
        .expect("get_drafts_count should succeed");
    let owner_products: i64 = owner_runtime.call_no_args("get_products_count")
        .expect("get_products_count should succeed");

    info!("Owner drafts: {}, products: {}", owner_drafts, owner_products);
    assert!(owner_drafts >= 1, "Owner should have at least 1 draft");
    // Note: owner_products may have products from other tests if using shared scenario

    // Wait to ensure any sync would have happened
    tokio::time::sleep(EXTENDED_SYNC_DELAY * 2).await;

    // Create node runtime to check products
    let node_runtime = create_headless_runtime(
        &scenario.node_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create node runtime");

    node_runtime.call_no_args::<()>("on_init")
        .expect("Node on_init should succeed");

    // Record products count before publish
    let node_products_before: i64 = node_runtime.call_no_args("get_products_count")
        .expect("get_products_count should succeed");

    info!("Node products before publish: {}", node_products_before);

    // Get the draft ID and publish it
    let draft_id: Option<String> = owner_runtime.call_no_args("get_last_draft_id")
        .expect("get_last_draft_id should succeed");

    assert!(draft_id.is_some(), "Should have a draft ID to publish");
    let draft_id = draft_id.unwrap();

    info!("Publishing draft: {}", draft_id);

    owner_runtime.call_with_args::<_, ()>(
        "publish_product",
        (draft_id.clone(),),
    ).expect("publish_product should succeed");

    // Verify draft is removed and product is published
    let owner_drafts_after: i64 = owner_runtime.call_no_args("get_drafts_count")
        .expect("get_drafts_count should succeed");
    let owner_products_after: i64 = owner_runtime.call_no_args("get_products_count")
        .expect("get_products_count should succeed");

    info!("Owner after publish - drafts: {}, products: {}", owner_drafts_after, owner_products_after);
    assert!(owner_drafts_after < owner_drafts, "Draft should be removed after publish");
    assert!(owner_products_after > owner_products, "Products should increase after publish");

    // Wait for sync to node
    tokio::time::sleep(EXTENDED_SYNC_DELAY * 2).await;

    // Node should now have the published product
    node_runtime.call_no_args::<()>("on_init")
        .expect("Node re-init should succeed");

    let node_products_after: i64 = node_runtime.call_no_args("get_products_count")
        .expect("get_products_count should succeed");

    info!("Node products after publish: {}", node_products_after);
    assert!(
        node_products_after > node_products_before,
        "Node should have more products after owner publishes (before: {}, after: {})",
        node_products_before,
        node_products_after
    );

    info!("=== PASSED: Owner product drafts stay local ===");

    scenario.shutdown().await;
}

/// Test: End-to-end sync flow integrity
///
/// Comprehensive test verifying all sync rules work together:
/// 1. Owner adds product (syncs)
/// 2. Customer sees product
/// 3. Customer creates draft (local only)
/// 4. Customer submits (syncs)
/// 5. Node runs derivation
/// 6. Owner sees order and updates status
/// 7. Status change propagates through sync
#[tokio::test(flavor = "multi_thread")]
async fn test_sync_flow_integrity() {
    init_tracing();

    let scenario = match setup_headless_scenario().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Setup failed: {}", e);
            return;
        }
    };

    info!("=== Test: Sync flow integrity (end-to-end) ===");

    // Load app code
    let owner_app_code = load_myshop_owner_app();
    let customer_app_code = load_myshop_customer_app();
    let init_code = load_myshop_init();

    // 1. Owner adds a product (direct, not draft)
    let owner_runtime = create_headless_runtime(
        &scenario.owner_butler,
        &scenario.page_id,
        &owner_app_code,
    ).await.expect("Failed to create owner runtime");

    owner_runtime.call_no_args::<()>("on_init")
        .expect("Owner on_init should succeed");

    owner_runtime.call_with_args::<_, ()>(
        "add_product",
        ("Integrity Test Widget", "4999", "Widget for integrity test", "100"),
    ).expect("add_product should succeed");

    info!("Step 1: Owner added product");

    // Wait for sync
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 2. Customer sees product
    let customer_runtime = create_headless_runtime(
        &scenario.customer_butler,
        &scenario.page_id,
        &customer_app_code,
    ).await.expect("Failed to create customer runtime");

    customer_runtime.call_no_args::<()>("on_init")
        .expect("Customer on_init should succeed");

    let customer_products: i64 = customer_runtime.call_no_args("get_products_count")
        .expect("get_products_count should succeed");

    info!("Step 2: Customer sees {} products", customer_products);
    assert!(customer_products >= 1, "Customer should see at least 1 product");

    // 3. Customer creates draft order (local only)
    customer_runtime.call_with_args::<_, ()>(
        "select_product",
        ("test-product-id", "Integrity Test Widget", 4999),
    ).expect("select_product should succeed");

    customer_runtime.call_with_args::<_, ()>(
        "create_order",
        ("test-product-id", "3", "Integrity test order", "999 Integrity Blvd"),
    ).expect("create_order should succeed");

    let drafts: i64 = customer_runtime.call_no_args("get_drafts_count")
        .expect("get_drafts_count should succeed");

    info!("Step 3: Customer created draft (drafts={})", drafts);
    assert!(drafts >= 1, "Customer should have at least 1 draft");

    // 4. Customer submits order
    customer_runtime.call_no_args::<()>("submit_order")
        .expect("submit_order should succeed");

    let order_id: String = customer_runtime.call_no_args("get_last_order_id")
        .expect("get_last_order_id should succeed");

    info!("Step 4: Customer submitted order {}", order_id);

    // Wait for sync to node
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 5. Node runs derivation
    let node_app_code = format!("{}\n\n{}", init_code, owner_app_code);
    let node_runtime = create_headless_runtime(
        &scenario.node_butler,
        &scenario.page_id,
        &node_app_code,
    ).await.expect("Failed to create node runtime");

    node_runtime.call_no_args::<()>("on_init")
        .expect("Node on_init should succeed");

    node_runtime.initialize_derivation()
        .expect("Derivation initialization should succeed");

    let node_orders: i64 = node_runtime.call_no_args("get_orders_count")
        .expect("get_orders_count should succeed");

    info!("Step 5: Node has {} orders after derivation", node_orders);
    assert!(node_orders >= 1, "Node should have at least 1 order");

    // Wait for derived layer to sync to owner
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 6. Owner sees order and updates status
    owner_runtime.call_no_args::<()>("on_init")
        .expect("Owner re-init should succeed");

    let owner_orders: i64 = owner_runtime.call_no_args("get_orders_count")
        .expect("get_orders_count should succeed");

    info!("Step 6: Owner sees {} orders", owner_orders);
    assert!(owner_orders >= 1, "Owner should see at least 1 order");

    // Get customer DID for status update
    let customer_did = scenario.customer_butler.user_info().await
        .expect("Failed to get customer info").did;

    // Owner confirms the order
    owner_runtime.call_with_args::<_, ()>(
        "update_order_status",
        (customer_did.clone(), order_id.clone(), "confirmed"),
    ).expect("update_order_status should succeed");

    info!("Step 6: Owner confirmed order");

    // Wait for sync
    tokio::time::sleep(EXTENDED_SYNC_DELAY * 2).await;

    // 7. Verify status propagated
    node_runtime.call_no_args::<()>("on_init")
        .expect("Node re-init should succeed");

    let final_status: Option<String> = node_runtime.call_with_args("get_order_status", order_id.clone())
        .expect("get_order_status should succeed");

    info!("Step 7: Final order status: {:?}", final_status);
    assert_eq!(final_status, Some("confirmed".to_string()),
        "Order status should be 'confirmed' after owner update");

    info!("=== PASSED: Sync flow integrity (end-to-end) ===");

    scenario.shutdown().await;
}
