//! Setup Booking Test Database
//!
//! Creates persistent databases for the service booking app:
//! - Provider: signed up, has space with my-booking app, published to node, has schedule
//! - Node (kunki): initialized, has space/page synced from provider
//! - Customer: signed up, subscribed to space as customer, can book appointments
//!
//! Usage:
//! ```bash
//! # Create databases in /tmp/booking_test
//! cargo run -p integration_tests --bin setup_booking_db -- --db-dir /tmp/booking_test
//!
//! # Then use with ai_interface:
//! ./target/debug/ai_interface --instances provider,customer --node --db-dir /tmp/booking_test
//! ```

use clap::Parser;
use tracing::info;

use courier::coordinator::{CoordinatorMessage, CourierMode};

use integration_tests::TestHarness;
use integration_tests::app_loader::load_booking_provider_app;
use integration_tests::fixtures::{ACTOR_SPAWN_DELAY, EXTENDED_SYNC_DELAY};
use integration_tests::headless_helper::create_headless_runtime;
use integration_tests::helpers::{
    setup_butler_persistent, generate_connection_string, complete_handshake,
};

/// Load the my-booking space template at compile time
const BOOKING_SPACE_TEMPLATE: &str = include_str!("../../../sample_apps/my-booking/space_permit_template.json");

#[derive(Parser)]
#[command(name = "setup_booking_db")]
#[command(about = "Setup persistent test databases for booking ai_interface")]
struct Args {
    /// Directory to store databases
    #[arg(long, default_value = "/tmp/booking_test")]
    db_dir: String,

    /// Passphrase for all users (provider, node, customer)
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
    println!("║           SETTING UP BOOKING TEST DATABASES                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Database directory: {}", args.db_dir);
    println!("Passphrase: {}", args.passphrase);
    println!();

    // Create persistent databases
    info!("Creating provider database...");
    let (provider_butler, _provider_key, _provider_holder) = setup_butler_persistent(
        &args.db_dir,
        "provider",
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
    // Note: Use "owner" as peer name since complete_handshake expects it
    let mut harness = TestHarness::new();
    harness.add_peer_with_butler("owner", CourierMode::User, provider_butler.clone()).await?;
    harness.add_peer_with_butler("node", CourierMode::Node, node_butler.clone()).await?;
    harness.add_peer_with_butler("customer", CourierMode::User, customer_butler.clone()).await?;

    let node_node_id = harness.peer("node").unwrap().node_id;

    // === Provider ↔ Node handshake ===
    info!("Setting up provider ↔ node handshake...");
    let connection_string = generate_connection_string(&node_butler).await?;
    provider_butler.add_sovereign_node(&connection_string)
        .map_err(|e| anyhow::anyhow!("Failed to add sovereign node: {}", e))?;

    let _transport = harness.spawn_transport();
    harness.connect_and_notify("owner", "node").await?;
    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;
    complete_handshake(&harness, &provider_butler, &node_butler).await?;
    info!("✔ Provider ↔ Node handshake complete");

    // === Create and publish space ===
    info!("Creating space and importing app...");
    let user_info = provider_butler.user_info().await?;
    let space = provider_butler
        .create_space("Booking Service".to_string(), user_info.did.clone(), BOOKING_SPACE_TEMPLATE)
        .await?;

    // Import page from sample_apps/my-booking
    let booking_path = find_sample_app_path("my-booking")?;
    info!("Importing my-booking from: {}", booking_path.display());
    let page = provider_butler.import_page(&space.id, &booking_path).await?;
    info!("✔ Space created: {} (page: {})", space.id, page.id);

    // Publish to node
    let provider_coordinator = &harness.peer("owner").unwrap().coordinator;
    provider_coordinator.cast(CoordinatorMessage::PublishSpace {
        node_id: node_node_id,
        space_id: space.id.clone(),
    })?;

    // Wait for sync
    harness.wait_for_space(&node_butler, &space.id).await?;
    harness.wait_for_page(&node_butler, &page.id).await?;
    info!("✔ Space published to node");

    // === Set up weekly schedule ===
    info!("Setting up weekly schedule via headless runtime...");
    let provider_app_code = load_booking_provider_app();
    let provider_runtime = create_headless_runtime(&provider_butler, &page.id, &provider_app_code)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create provider runtime: {}", e))?;

    // Initialize the app
    provider_runtime.call_no_args::<()>("on_init")
        .map_err(|e| anyhow::anyhow!("Failed to call on_init: {}", e))?;

    // Set up Mon-Fri 9:00-17:00, 60 minute slots
    for day in &["monday", "tuesday", "wednesday", "thursday", "friday"] {
        provider_runtime.call_with_args::<_, ()>("set_schedule_via_ui", (*day, "09:00", "17:00", 60))
            .map_err(|e| anyhow::anyhow!("Failed to set schedule for {}: {}", day, e))?;
        info!("  Set schedule for {}", day);
    }

    info!("✔ Weekly schedule configured (Mon-Fri 9:00-17:00, 60 min slots)");

    // Close page to flush dirty layers to disk before sync
    provider_butler.close_page(&page.id).await?;
    info!("✔ Flushed provider page to disk");

    // Wait for schedule to sync to node
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // === Connect Customer ===
    info!("Setting up customer...");
    let customer_conn = node_butler.generate_viewer_connection_string(&space.id, None).await?;
    let parsed = customer_butler.parse_connection_string(&customer_conn)
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

    // Request space as customer
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
    println!("  • {}/provider.db  (service provider)", args.db_dir);
    println!("  • {}/kunki.db     (node)", args.db_dir);
    println!("  • {}/customer.db  (customer)", args.db_dir);
    println!();
    println!("Login credentials:");
    println!("  • Passphrase: {}", args.passphrase);
    println!();
    println!("Space: {} ({})", space.name, space.id);
    println!("Page:  {} ({})", page.name, page.id);
    println!("Schedule: Mon-Fri 9:00-17:00, 60 min slots");
    println!();
    println!("To use with ai_interface:");
    println!("  ./target/debug/ai_interface \\");
    println!("    --instances provider,customer \\");
    println!("    --node \\");
    println!("    --db-dir {}", args.db_dir);
    println!();

    Ok(())
}
