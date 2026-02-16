use clap::{Parser, Subcommand};
use tracing::{debug, error, info, warn};

#[cfg(feature = "profiling")]
use std::fs::File;

use base64::{engine::general_purpose, Engine as _};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

// Butler for storage and identity
use butler::{Butler, RedbStore, LayerCache};
use tokio::sync::RwLock;

mod control_server;
mod node_runtime;
mod validation_service;
use control_server::{KunkiControlServer, NodeState};
use node_runtime::NodeRuntimeManager;

// Transport and Courier for P2P
use courier::{Courier, CourierEvent, CourierMode, HandshakeServices};
use transport::{Transport, TransportConfig};

/// Helper function to get passphrase - either from argument or by prompting
fn get_passphrase(
    passphrase_opt: Option<String>,
    prompt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    match passphrase_opt {
        Some(p) => Ok(p),
        None => {
            println!("{}", prompt);
            let passphrase = rpassword::read_password()?;
            if passphrase.is_empty() {
                return Err("Passphrase cannot be empty".into());
            }
            Ok(passphrase)
        }
    }
}

#[derive(Parser)]
#[command(author, version, about = "Osvauld P2P CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Database path (without extension)
    #[arg(short, long, default_value = "cli")]
    db_path: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new user
    Init {
        /// Username for the new user
        #[arg(short, long)]
        username: String,

        /// Passphrase for encryption (optional, will prompt if not provided)
        #[arg(short, long)]
        passphrase: Option<String>,
    },

    /// Start the P2P listener and print connection token
    Start {
        /// Passphrase to unlock the certificate (optional, will prompt if not provided)
        #[arg(short, long)]
        passphrase: Option<String>,

        /// Enable debug server on specified Unix socket path
        #[arg(long)]
        debug_socket: Option<String>,
    },

    /// Generate a folder share token for public viewing
    FolderToken {
        /// Passphrase to unlock the certificate (optional, will prompt if not provided)
        #[arg(short, long)]
        passphrase: Option<String>,

        /// Folder ID to generate token for
        #[arg(short, long)]
        folder_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize profiling tools when feature is enabled
    #[cfg(feature = "profiling")]
    let _flame_guard = setup_profiling();

    // Initialize rich tracing with capture support
    // Check for OSVAULD_LOG_FORMAT=json to output JSON logs (for ai_interface)
    #[cfg(not(feature = "profiling"))]
    let use_json = std::env::var("OSVAULD_LOG_FORMAT")
        .map(|v| v == "json")
        .unwrap_or(false);

    #[cfg(not(feature = "profiling"))]
    let (_guard, capture_handle) = logging_utils::init_rich_tracing_with_capture(logging_utils::LogConfig {
        level: "debug".to_string(),
        log_to_stdout: true,
        use_tree_format: !use_json,
        stdout_json: use_json,
        instance_name: Some("kunki".to_string()),
        ..Default::default()
    })?;

    info!("🚀 Kunki CLI starting");

    let cli = Cli::parse();

    // Initialize Butler's RedbStore for storage
    // Check for STHALAM_DATA_DIR env var (like sthalam does for test automation)
    let (db_path, data_dir): (std::path::PathBuf, std::path::PathBuf) = if let Ok(data_dir_str) = std::env::var("STHALAM_DATA_DIR") {
        let dir = std::path::PathBuf::from(&data_dir_str);
        std::fs::create_dir_all(&dir).expect("Failed to create data directory");
        (dir.join(format!("{}.db", cli.db_path)), dir)
    } else {
        // Default: use db_path directly with .db extension, assets in current dir
        let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        (std::path::PathBuf::from(format!("{}.db", cli.db_path)), current_dir)
    };
    info!("Using database: {:?}", db_path);

    let redb_store = RedbStore::open(&db_path).map_err(|e| -> Box<dyn std::error::Error> {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to open RedbStore: {}", e),
        ))
    })?;
    let redb_store = Arc::new(redb_store);

    match cli.command {
        Commands::Init {
            username,
            passphrase,
        } => {
            let pass = get_passphrase(passphrase, "Enter passphrase:")?;
            handle_init(&username, &pass, redb_store.clone()).await?;
        }
        Commands::Start { passphrase, debug_socket } => {
            let pass = get_passphrase(passphrase, "Enter passphrase to unlock certificate:")?;
            #[cfg(not(feature = "profiling"))]
            handle_start(&pass, redb_store.clone(), debug_socket, &data_dir, Some(capture_handle)).await?;
            #[cfg(feature = "profiling")]
            handle_start(&pass, redb_store.clone(), debug_socket, &data_dir, None).await?;
        }
        Commands::FolderToken {
            passphrase,
            folder_id,
        } => {
            let pass = get_passphrase(passphrase, "Enter passphrase to unlock certificate:")?;
            handle_folder_token(&pass, &folder_id, redb_store.clone())
                .await?;
        }
    }

    Ok(())
}

async fn handle_init(
    username: &str,
    passphrase: &str,
    redb_store: Arc<RedbStore>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if already signed up using Butler
    if butler::is_signed_up(&redb_store)? {
        error!("User already initialized. Use 'start' command to begin.");
        return Ok(());
    }

    info!("Initializing new user: {}", username);

    // Create user using Butler (Herald-based identity)
    let result = butler::signup(&redb_store, username, passphrase)?;

    info!("✔ Identity created");
    info!("✔ DID: {}", result.identity.did());
    info!("⚠️  IMPORTANT: Save your recovery phrase:");
    println!("\n  {}\n", result.mnemonic);

    info!("✔ User '{}' created successfully", username);
    info!("Use 'start' command with your passphrase to begin P2P service");

    Ok(())
}

async fn handle_start(
    passphrase: &str,
    redb_store: Arc<RedbStore>,
    debug_socket: Option<String>,
    data_dir: &std::path::Path,
    capture_handle: Option<logging_utils::CaptureHandle>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if user exists in Butler
    if !butler::is_signed_up(&redb_store)? {
        error!("No user found. Please run 'init' first.");
        return Ok(());
    }

    info!("Loading identity...");

    // Login with Butler (returns Herald Identity)
    let identity = butler::login(&redb_store, passphrase)?;
    let identity_data = butler::get_identity_data(&redb_store)?.ok_or("Identity data not found")?;

    info!("✔ Logged in as: {}", identity_data.username);
    info!("✔ DID: {}", identity.did());

    // Create Butler with LayerCache, AssetStore and set identity
    let layer_cache = Arc::new(RwLock::new(LayerCache::new(redb_store.clone(), 100)));
    let assets_path = data_dir.join("assets");
    let asset_store = Arc::new(butler::AssetStore::new(&assets_path).expect("Failed to create asset store"));

    // Create sync event channel (Scribe → Coordinator)
    // Node mode DOES need EnsureSync - to trigger RefreshSubscriptions when pages are opened
    let (sync_tx, sync_rx) = tokio::sync::mpsc::channel::<butler::SyncEvent>(32);

    // Create page opened channel for node runtime auto-start
    let (page_opened_tx, mut page_opened_rx) = tokio::sync::mpsc::channel::<String>(32);

    let butler = Arc::new(Butler::new(redb_store.clone(), layer_cache, asset_store, Some(sync_tx)));
    butler.set_identity(identity.clone()).await;
    butler.set_page_opened_tx(page_opened_tx).await;

    // Create node runtime manager for derivation
    let node_runtime = Arc::new(NodeRuntimeManager::new(butler.clone()));

    // Spawn task to auto-start node runtime when pages are opened
    //
    // **Context**: App layers (containing init.lua, node.lua) arrive via SyncOffer
    // AFTER the page is opened. First attempt may fail because apps().list() is empty.
    // We retry a few times with delay to wait for app layers to sync.
    let node_runtime_for_task = node_runtime.clone();
    tokio::spawn(async move {
        while let Some(page_id) = page_opened_rx.recv().await {
            info!(page_id = %page_id, "Page opened, starting node runtime");
            let runtime = node_runtime_for_task.clone();
            let pid = page_id.clone();
            tokio::spawn(async move {
                for attempt in 0..10u32 {
                    match runtime.start_node_for_page(&pid).await {
                        Ok(()) => {
                            info!(page_id = %pid, attempt, "Node runtime started");
                            return;
                        }
                        Err(e) => {
                            if attempt < 9 {
                                debug!(page_id = %pid, attempt, error = %e, "Node runtime not ready, retrying");
                                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            } else {
                                warn!(page_id = %pid, error = %e, "Failed to start node runtime after retries");
                            }
                        }
                    }
                }
            });
        }
    });


    // Initialize Transport layer using the device key from Butler
    info!("Initializing transport layer...");
    let device_key = butler.device_key().await
        .map_err(|e| format!("Failed to get device key: {:?}", e))?;
    let transport_config = TransportConfig::new(device_key);
    let (transport, event_rx) = Transport::init(transport_config)
        .await
        .map_err(|e| format!("Failed to init transport: {}", e))?;

    let transport = Arc::new(transport);
    let node_id = transport.node_id();

    info!("✔ Transport initialized");
    info!("✔ Node ID: {}", node_id);

    // Get relay URLs
    let relay_urls = transport.relay_urls();
    if !relay_urls.is_empty() {
        info!("✔ Relay: {}", relay_urls[0]);
    }

    // Start debug server if requested (keep reference for updating state later)
    let debug_server_arc: Option<Arc<KunkiControlServer>> = if let Some(ref socket_path) = debug_socket {
        let socket_path = PathBuf::from(socket_path);
        let debug_server = Arc::new(KunkiControlServer::new(socket_path.clone(), "kunki".to_string(), Some(butler.clone())));

        // Set initial state (connection_string will be updated after generation)
        debug_server.set_state(NodeState {
            node_id: node_id.to_string(),
            did: identity.did().to_string(),
            username: identity_data.username.clone(),
            connected_peers: vec![],
            relay_url: relay_urls.first().cloned(),
            connection_string: None,
        }).await;

        // Set capture handle for event capture commands
        if let Some(ref ch) = capture_handle {
            debug_server.set_capture_handle(ch.clone()).await;
        }

        // Spawn debug server
        let server = debug_server.clone();
        tokio::spawn(async move {
            if let Err(e) = server.start().await {
                error!(error = %e, "Debug server error");
            }
        });

        info!("✔ Debug server enabled on: {:?}", socket_path);
        Some(debug_server)
    } else {
        None
    };

    // Create HandshakeServices with Butler
    let handshake_services = Arc::new(HandshakeServices::new(butler.clone()));

    // Create capture broadcast channel for courier events
    let (capture_tx, _) = tokio::sync::broadcast::channel::<String>(1024);

    // Register capture sources with CaptureHandle
    if let Some(ref ch) = capture_handle {
        ch.register_source(capture_tx.clone()).await;
    }

    // Set capture_tx on butler so ScribeArgs get it
    butler.set_capture_tx(capture_tx.clone()).await;

    // Initialize Courier with services and capture broadcast
    let (courier_handle, mut courier_events, courier) = Courier::init_with_services_and_capture(
        CourierMode::Node,
        transport.clone(),
        Some(handshake_services),
        Some(capture_tx),
    );

    // Note: Router starts accepting connections automatically when Transport::init() is called

    // Spawn Courier event processor
    tokio::spawn(async move {
        courier.run(event_rx).await;
    });

    // Forward sync events to Coordinator (Scribe → Coordinator)
    // This enables EnsureSync to trigger RefreshSubscriptions for connected peers
    let handle_for_sync = courier_handle.clone();
    let mut sync_rx = sync_rx;
    tokio::spawn(async move {
        while let Some(event) = sync_rx.recv().await {
            match event {
                butler::SyncEvent::EnsureSync { user_did } => {
                    tracing::info!(user_did = %user_did, "Node received EnsureSync from Scribe, forwarding to Coordinator");
                    if let Err(e) = handle_for_sync.ensure_sync(&user_did) {
                        tracing::warn!(user_did = %user_did, error = %e, "Failed to forward EnsureSync");
                    } else {
                        tracing::info!(user_did = %user_did, "Node forwarded EnsureSync to Coordinator");
                    }
                }
                butler::SyncEvent::SubscribeLayers { page_id, creator_did, layers } => {
                    tracing::info!(
                        page_id = %page_id,
                        creator_did = %creator_did,
                        count = layers.len(),
                        "Node received SubscribeLayers from Scribe, forwarding to Coordinator"
                    );
                    if let Err(e) = handle_for_sync.subscribe_layers(&page_id, &creator_did, layers) {
                        tracing::warn!(error = %e, "Failed to forward SubscribeLayers");
                    }
                }
            }
        }
    });

    // Note: Node scripts are now started automatically by Scribe when is_node=true
    // Scribe manages the node script lifecycle via node_script_shutdown handle

    // Spawn application event handler (logging only)
    // Note: Node scripts are now started automatically by Scribe when is_node=true
    let _courier_handle = courier_handle.clone(); // Keep handle for future use
    tokio::spawn(async move {
        while let Some(event) = courier_events.recv().await {
            match event {
                CourierEvent::PeerAuthenticated {
                    node_id,
                    did,
                    username,
                } => {
                    info!(
                        "📱 Peer authenticated: {} ({}) - node={}",
                        username, did, node_id
                    );
                }
                CourierEvent::PeerDisconnected { node_id } => {
                    info!("📴 Peer disconnected: {}", node_id);
                }
                CourierEvent::SpacePublished { node_id, space_id } => {
                    info!("📤 Space published to {}: {}", node_id, space_id);
                }
                CourierEvent::PublishFailed {
                    node_id,
                    space_id,
                    error,
                } => {
                    warn!("⚠️ Publish failed to {}: {} - {}", node_id, space_id, error);
                }
                CourierEvent::ViewerSyncComplete {
                    node_id,
                    space_id,
                    pages_synced,
                } => {
                    info!(
                        "📥 Viewer sync complete from {}: {} ({} pages)",
                        node_id, space_id, pages_synced
                    );
                }
                CourierEvent::ShareableLinkReceived {
                    node_id,
                    space_id,
                    permit,
                } => {
                    info!(
                        "🔗 Shareable link generated for {}: {} (permit: {}...)",
                        node_id, space_id, &permit[..permit.len().min(20)]
                    );
                }
                CourierEvent::ConnectRequested { node_id, permit } => {
                    info!(
                        "🔌 Connection requested to {}: permit={}...",
                        node_id, &permit[..permit.len().min(20)]
                    );
                }
                CourierEvent::ViewerSpaceReceived { node_id, space, page_count } => {
                    info!(
                        "📂 Viewer received space {} ({}) from {} ({} pages)",
                        space.id, space.name, node_id, page_count
                    );
                }
                CourierEvent::PageReceived { node_id, page, is_last } => {
                    info!(
                        "📄 Viewer received page {} ({}) from {} for space {} (last: {})",
                        page.id, page.name, node_id, page.space_id, is_last
                    );
                }
                CourierEvent::SyncConsentComplete { node_id, space_id } => {
                    info!(
                        "✅ Sync consent complete for space {} with node {}",
                        space_id, node_id
                    );
                }
                CourierEvent::ConnectionFailed { node_id, error } => {
                    warn!("❌ Connection failed to {}: {}", node_id, error);
                }
            }
        }
    });

    // Print connection string
    println!("\n╔══════════════════════════════════════════╗");
    println!("║     CONNECTION STRING                    ║");
    println!("╚══════════════════════════════════════════╝");

    // Generate connection string using Butler
    let encoded_connection = butler.nodes().generate_connection_string(
        relay_urls.first().map(|s| s.as_str()),
    ).await.map_err(|e| format!("Failed to generate connection string: {:?}", e))?;

    println!("{}", encoded_connection);
    println!("╚══════════════════════════════════════════╝");

    // Update debug server state with connection string
    if let Some(ref debug_server) = debug_server_arc {
        debug_server.set_state(NodeState {
            node_id: node_id.to_string(),
            did: identity.did().to_string(),
            username: identity_data.username.clone(),
            connected_peers: vec![],
            relay_url: relay_urls.first().cloned(),
            connection_string: Some(encoded_connection.clone()),
        }).await;
    }

    println!("\nℹ️  User: {}", identity_data.username);
    println!("ℹ️  DID: {}", identity.did());
    println!("ℹ️  Node ID: {}", node_id);
    println!("╚══════════════════════════════════════════╝\n");

    // Print status
    println!("🟢 SERVICE STATUS: ONLINE");
    println!("🔐 Press Ctrl+C to stop the service");
    println!("🔗 Service is ready to accept connections\n");

    // Setup signal handler for graceful shutdown
    info!("🎯 Service is now running - waiting for shutdown signal");

    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for Ctrl+C");

    info!("🛑 Shutdown signal received");
    info!("🛑 Initiating graceful shutdown...");

    // Note: Node scripts are automatically stopped when Scribe actors stop
    // via their node_script_shutdown handles

    info!("🔄 Cleaning up...");
    info!("✅ P2P service stopped gracefully");
    println!("\n🔴 SERVICE STATUS: OFFLINE");

    Ok(())
}

/// Setup profiling tools (tokio-console and/or tracing-flame)
///
/// **Modes** (based on environment variables):
/// - `FLAME_OUTPUT` + `TOKIO_CONSOLE_PORT` → both layers
/// - `FLAME_OUTPUT` only → flame layer only (no console overhead)
/// - `TOKIO_CONSOLE_PORT` only → console layer only
///
/// **Environment variables**:
/// - `TOKIO_CONSOLE_PORT`: Port for tokio-console (enables console layer)
/// - `FLAME_OUTPUT`: Path for flame graph output file (enables flame layer)
#[cfg(feature = "profiling")]
fn setup_profiling() -> Option<tracing_flame::FlushGuard<std::io::BufWriter<File>>> {
    use tracing_subscriber::prelude::*;

    let console_port: Option<u16> = std::env::var("TOKIO_CONSOLE_PORT")
        .ok()
        .and_then(|p| p.parse().ok());
    let flame_path = std::env::var("FLAME_OUTPUT").ok();

    match (&flame_path, console_port) {
        // Both flame + console
        (Some(path), Some(port)) => {
            let (flame_layer, guard) = match tracing_flame::FlameLayer::with_file(path) {
                Ok((layer, guard)) => (Some(layer), Some(guard)),
                Err(e) => {
                    eprintln!("Failed to create flame layer: {}", e);
                    (None, None)
                }
            };

            let console_layer = console_subscriber::ConsoleLayer::builder()
                .server_addr(([127, 0, 0, 1], port))
                .spawn();

            tracing_subscriber::registry()
                .with(console_layer)
                .with(flame_layer)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true))
                .init();

            tracing::info!(port = port, path = %path, "Profiling: tokio-console + flame");
            guard
        }
        // Flame only (no console overhead)
        // Filter out tokio runtime TRACE spans (normally consumed by console-subscriber)
        (Some(path), None) => {
            let (flame_layer, guard) = match tracing_flame::FlameLayer::with_file(path) {
                Ok((layer, guard)) => (Some(layer), Some(guard)),
                Err(e) => {
                    eprintln!("Failed to create flame layer: {}", e);
                    (None, None)
                }
            };

            let filter = tracing_subscriber::EnvFilter::new(
                "info,tokio=off,runtime=off",
            );

            tracing_subscriber::registry()
                .with(filter)
                .with(flame_layer)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true))
                .init();

            tracing::info!(path = %path, "Profiling: flame only (no console overhead)");
            guard
        }
        // Console only
        (None, Some(port)) => {
            let console_layer = console_subscriber::ConsoleLayer::builder()
                .server_addr(([127, 0, 0, 1], port))
                .spawn();

            tracing_subscriber::registry()
                .with(console_layer)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true))
                .init();

            tracing::info!(port = port, "Profiling: tokio-console only");
            None
        }
        // Neither - just fmt logging
        (None, None) => {
            let filter = tracing_subscriber::EnvFilter::new(
                "info,tokio=off,runtime=off",
            );

            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true))
                .init();

            tracing::info!("Profiling feature enabled but no FLAME_OUTPUT or TOKIO_CONSOLE_PORT set");
            None
        }
    }
}

async fn handle_folder_token(
    passphrase: &str,
    folder_id: &str,
    redb_store: Arc<RedbStore>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if user exists using Butler
    if !butler::is_signed_up(&redb_store)? {
        error!("No user found. Please run 'init' first.");
        return Ok(());
    }

    info!("Loading identity...");

    // Login with Butler (returns Herald Identity)
    let identity = butler::login(&redb_store, passphrase)?;
    let identity_data = butler::get_identity_data(&redb_store)?.ok_or("Identity data not found")?;

    info!("✔ Authenticated as: {}", identity_data.username);

    info!("Generating folder share token for folder: {}", folder_id);

    // Generate space share token using gurkha's stateless function
    let signing_key_bytes = identity.secret_signing_key();
    let (token, _cid) = gurkha::issue_space_viewer_auth(&signing_key_bytes, folder_id)
        .await
        .map_err(|e| format!("Failed to create space permit: {:?}", e))?;
    let pub_key = gurkha::get_public_key(&signing_key_bytes);

    println!("\n╔══════════════════════════════════════════╗");
    println!("║     FOLDER SHARE TOKEN                   ║");
    println!("╚══════════════════════════════════════════╝");
    println!("Folder ID: {}", folder_id);
    println!("╚══════════════════════════════════════════╝");
    println!("Token: {}", token);
    println!("╚══════════════════════════════════════════╝");
    println!("Public Key: {}", pub_key);
    println!("╚══════════════════════════════════════════╝");

    // Create connection string JSON using Herald identity
    let user_pub_key = general_purpose::STANDARD.encode(identity.public_signing_key());
    let device_pub_key = general_purpose::STANDARD.encode(identity.public_device_key());

    let connection_details = json!({
        "user_public_key": user_pub_key,
        "device_public_key": device_pub_key,
        "username": identity_data.username,
        "permit": token,
    });

    // Convert to string and base64 encode
    let connection_json = connection_details.to_string();
    let encoded_connection = general_purpose::STANDARD.encode(connection_json.as_bytes());

    println!("Connection String: {}", encoded_connection);
    println!("╚══════════════════════════════════════════╝\n");

    Ok(())
}
