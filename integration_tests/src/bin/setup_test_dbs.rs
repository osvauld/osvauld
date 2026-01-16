//! Setup Test Databases
//!
//! Creates persistent databases with full e-commerce scenario setup:
//! - Owner: signed up, has space with my-shop app, published to node, has products
//! - Node (kunki): initialized, has space/page synced from owner with products
//! - Customer: signed up, subscribed to space as viewer, sees products
//!
//! Usage:
//! ```bash
//! # Create databases in /tmp/ai_test
//! cargo run -p integration_tests --bin setup_test_dbs -- --db-dir /tmp/ai_test
//!
//! # Then use with ai_interface:
//! ./target/debug/ai_interface --instances owner,customer --node --db-dir /tmp/ai_test
//! ```

use clap::Parser;
use tracing::info;

use courier::coordinator::{CoordinatorMessage, CourierMode};

use integration_tests::TestHarness;
use integration_tests::app_loader::load_myshop_owner_app;
use integration_tests::fixtures::{ACTOR_SPAWN_DELAY, EXTENDED_SYNC_DELAY};
use integration_tests::headless_helper::create_headless_runtime;
use integration_tests::helpers::{
    setup_butler_persistent, generate_connection_string, complete_handshake,
};

/// Load the real my-shop space template at compile time
const MY_SHOP_SPACE_TEMPLATE: &str = include_str!("../../../sample_apps/my-shop/space_permit_template.json");

#[derive(Parser)]
#[command(name = "setup_test_dbs")]
#[command(about = "Setup persistent test databases for ai_interface")]
struct Args {
    /// Directory to store databases
    #[arg(long, default_value = "/tmp/ai_test")]
    db_dir: String,

    /// Passphrase for all users (owner, node, customer)
    #[arg(long, default_value = "test123")]
    passphrase: String,
}

/// Find sample app path
fn find_sample_app_path(app_name: &str) -> anyhow::Result<std::path::PathBuf> {
    let from_integration_tests = std::path::Path::new("..").join("sample_apps").join(app_name);
    if from_integration_tests.exists() {
        return Ok(from_integration_tests);
    }

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           SETTING UP TEST DATABASES                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Database directory: {}", args.db_dir);
    println!("Passphrase: {}", args.passphrase);
    println!();

    // Create persistent databases
    info!("Creating owner database...");
    let (owner_butler, _owner_key, _owner_holder) = setup_butler_persistent(
        &args.db_dir,
        "shop_owner",
        &args.passphrase,
    ).await?;

    info!("Creating node database...");
    let (node_butler, _node_key, _node_holder) = setup_butler_persistent(
        &args.db_dir,
        "kunki",
        &args.passphrase,
    ).await?;

    info!("Creating customer database...");
    let (customer_butler, _cust_key, _cust_holder) = setup_butler_persistent(
        &args.db_dir,
        "customer",
        &args.passphrase,
    ).await?;

    // Setup test harness with MockTransport
    let mut harness = TestHarness::new();
    harness.add_peer_with_butler("owner", CourierMode::User, owner_butler.clone()).await?;
    harness.add_peer_with_butler("node", CourierMode::Node, node_butler.clone()).await?;
    harness.add_peer_with_butler("customer", CourierMode::User, customer_butler.clone()).await?;

    let node_node_id = harness.peer("node").unwrap().node_id;

    // === Owner ↔ Node handshake ===
    info!("Setting up owner ↔ node handshake...");
    let connection_string = generate_connection_string(&node_butler).await?;
    owner_butler.add_sovereign_node(&connection_string)
        .map_err(|e| anyhow::anyhow!("Failed to add sovereign node: {}", e))?;

    let _transport = harness.spawn_transport();
    harness.connect_and_notify("owner", "node").await?;
    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;
    complete_handshake(&harness, &owner_butler, &node_butler).await?;
    info!("✔ Owner ↔ Node handshake complete");

    // === Create and publish space ===
    info!("Creating space and importing app...");
    let user_info = owner_butler.user_info().await?;
    let space = owner_butler
        .create_space("My Shop".to_string(), user_info.did.clone(), MY_SHOP_SPACE_TEMPLATE)
        .await?;

    // Import page from sample_apps/my-shop
    let myshop_path = find_sample_app_path("my-shop")?;
    info!("Importing my-shop from: {}", myshop_path.display());
    let page = owner_butler.import_page(&space.id, &myshop_path).await?;
    info!("✔ Space created: {} (page: {})", space.id, page.id);

    // Publish to node
    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;
    owner_coordinator.cast(CoordinatorMessage::PublishSpace {
        node_id: node_node_id,
        space_id: space.id.clone(),
    })?;

    // Wait for sync
    harness.wait_for_space(&node_butler, &space.id).await?;
    harness.wait_for_page(&node_butler, &page.id).await?;
    info!("✔ Space published to node");

    // === Add Products ===
    info!("Adding products via headless runtime...");
    let owner_app_code = load_myshop_owner_app();
    let owner_runtime = create_headless_runtime(&owner_butler, &page.id, &owner_app_code)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create owner runtime: {}", e))?;

    // Initialize the app (sets up products_layer)
    owner_runtime.call_no_args::<()>("on_init")
        .map_err(|e| anyhow::anyhow!("Failed to call on_init: {}", e))?;

    // Add products using call_with_args
    owner_runtime.call_with_args::<_, ()>("add_product", ("Widget", 29.99, "A useful widget", 100))
        .map_err(|e| anyhow::anyhow!("Failed to add product 1: {}", e))?;
    owner_runtime.call_with_args::<_, ()>("add_product", ("Gadget", 49.99, "A cool gadget", 50))
        .map_err(|e| anyhow::anyhow!("Failed to add product 2: {}", e))?;
    owner_runtime.call_with_args::<_, ()>("add_product", ("Gizmo", 19.99, "A handy gizmo", 200))
        .map_err(|e| anyhow::anyhow!("Failed to add product 3: {}", e))?;

    let product_count: i32 = owner_runtime.call_no_args("get_products_count")
        .map_err(|e| anyhow::anyhow!("Failed to get product count: {}", e))?;
    info!("✔ Added {} products", product_count);

    // Close page to flush dirty layers to disk before sync
    owner_butler.close_page(&page.id).await?;
    info!("✔ Flushed owner page to disk");

    // Wait for products to sync to node
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // === Connect Customer ===
    info!("Setting up customer as viewer...");
    let viewer_conn = node_butler.generate_viewer_connection_string(&space.id, None).await?;
    let parsed = customer_butler.parse_connection_string(&viewer_conn)
        .map_err(|e| anyhow::anyhow!("Failed to parse connection string: {}", e))?;

    // Store node as contact so customer can resolve device info for sync
    let node_did = parsed.node_did()
        .map_err(|e| anyhow::anyhow!("Failed to get node DID: {}", e))?;
    customer_butler.add_node_contact(
        &node_did,
        &parsed.node_encryption_key,
        &parsed.name,
        &node_node_id.to_string(),
        &parsed.permit,
    ).map_err(|e| anyhow::anyhow!("Failed to add node contact: {}", e))?;
    info!("Stored node contact for customer");

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
    info!("✔ Customer subscribed to space");

    // Shutdown harness
    harness.shutdown().await;

    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    SETUP COMPLETE                            ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Databases created:");
    println!("  • {}/shop_owner.db  (owner)", args.db_dir);
    println!("  • {}/kunki.db       (node)", args.db_dir);
    println!("  • {}/customer.db    (customer)", args.db_dir);
    println!();
    println!("Login credentials:");
    println!("  • Passphrase: {}", args.passphrase);
    println!();
    println!("Space: {} ({})", space.name, space.id);
    println!("Page:  {} ({})", page.name, page.id);
    println!("Products: {} items pre-loaded", product_count);
    println!();
    println!("To use with ai_interface:");
    println!("  ./target/debug/ai_interface \\");
    println!("    --instances shop_owner,customer \\");
    println!("    --node \\");
    println!("    --db-dir {}", args.db_dir);
    println!();

    Ok(())
}
