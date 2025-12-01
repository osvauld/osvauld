use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

use base64::{engine::general_purpose, Engine as _};
use serde_json::json;
use std::sync::Arc;

// Butler for storage and identity
use butler::{Butler, RedbStore, LayerCache};
use tokio::sync::RwLock;

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
    // Initialize rich tracing with tree formatting
    let _guard = logging_utils::init_dev()?;

    info!("🚀 Kunki CLI starting");

    let cli = Cli::parse();

    // Initialize Butler's RedbStore for storage
    let redb_path = format!("{}.redb", cli.db_path);
    let redb_store = RedbStore::open(&redb_path).map_err(|e| -> Box<dyn std::error::Error> {
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
        Commands::Start { passphrase } => {
            let pass = get_passphrase(passphrase, "Enter passphrase to unlock certificate:")?;
            handle_start(&pass, redb_store.clone()).await?;
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

    // Create Butler with LayerCache and set identity
    let layer_cache = Arc::new(RwLock::new(LayerCache::new(redb_store.clone(), 100)));
    let butler = Arc::new(Butler::new(redb_store.clone(), layer_cache));
    butler.set_identity(identity.clone()).await;

    // Generate one-time permit for connection string using Butler
    let (token, _pub_key) = butler.issue_one_time_permit("owner")
        .await
        .map_err(|e| format!("Failed to create permit: {:?}", e))?;

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

    // Create HandshakeServices with Butler
    let handshake_services = Arc::new(HandshakeServices::new(butler.clone()));

    // Initialize Courier with services for auto-processing
    let (courier_handle, mut courier_events, courier) = Courier::init_with_services(
        CourierMode::Node,
        transport.clone(),
        Some(handshake_services),
    );

    // Start accepting connections
    transport.start_accepting();

    // Spawn Courier event processor
    tokio::spawn(async move {
        courier.run(event_rx).await;
    });

    // Spawn application event handler
    let _courier_handle = courier_handle.clone(); // Keep handle for future use
    tokio::spawn(async move {
        while let Some(event) = courier_events.recv().await {
            match event {
                CourierEvent::HelloReceived {
                    node_id,
                    did,
                    username,
                    permit,
                } => {
                    info!(
                        "👋 Hello received from {} ({}) with permit len={}",
                        username, node_id, permit.len()
                    );
                    // TODO: Verify permit, send Welcome back
                }
                CourierEvent::WelcomeReceived {
                    node_id,
                    permit_for_us,
                } => {
                    info!(
                        "🤝 Welcome received from {} with permit len={}",
                        node_id, permit_for_us.len()
                    );
                    // TODO: Store permit, send PermitGrant if first connection
                }
                CourierEvent::PeerAuthenticated {
                    node_id: _,
                    peer_type,
                    username,
                    did,
                } => {
                    info!(
                        "📱 Peer authenticated: {} ({}) - {:?}",
                        username, did, peer_type
                    );
                }
                CourierEvent::PeerDisconnected { node_id } => {
                    info!("📴 Peer disconnected: {}", node_id);
                }
                CourierEvent::SyncRequest {
                    node_id,
                    request_id,
                    resource_id,
                    ..
                } => {
                    info!(
                        "🔄 Sync request from {}: {} (id={})",
                        node_id, resource_id, request_id
                    );
                }
                CourierEvent::SyncPush {
                    node_id,
                    resource_id,
                    updates,
                } => {
                    info!(
                        "📥 Sync push from {}: {} ({} bytes)",
                        node_id,
                        resource_id,
                        updates.len()
                    );
                }
                CourierEvent::FolderRequest {
                    node_id,
                    request_id,
                    folder_id,
                } => {
                    info!(
                        "📁 Folder request from {}: {} (id={})",
                        node_id, folder_id, request_id
                    );
                }
                CourierEvent::Error { message } => {
                    warn!("⚠️  Error: {}", message);
                }
            }
        }
    });

    // Print connection string
    println!("\n╔══════════════════════════════════════════╗");
    println!("║     CONNECTION STRING                    ║");
    println!("╚══════════════════════════════════════════╝");

    // Create connection string with Ed25519 keys
    let user_pub_key = general_purpose::STANDARD.encode(identity.public_signing_key());
    let device_pub_key = general_purpose::STANDARD.encode(identity.public_device_key());

    let connection_details = json!({
        "user_public_key": user_pub_key,
        "device_public_key": device_pub_key,
        "node_id": node_id.to_string(),
        "username": identity_data.username,
        "permit": token,
        "relay": relay_urls.first(),
    });

    // Convert to string and base64 encode
    let connection_json = connection_details.to_string();
    let encoded_connection = general_purpose::STANDARD.encode(connection_json.as_bytes());

    println!("{}", encoded_connection);
    println!("╚══════════════════════════════════════════╝");
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
    info!("🔄 Cleaning up...");
    info!("✅ P2P service stopped gracefully");
    println!("\n🔴 SERVICE STATUS: OFFLINE");

    Ok(())
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

    // Generate folder share token using gurkha's stateless function
    let signing_key_bytes = identity.secret_signing_key();
    let (token, _cid) = gurkha::issue_folder_viewer_auth(&signing_key_bytes, folder_id)
        .await
        .map_err(|e| format!("Failed to create folder permit: {:?}", e))?;
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
