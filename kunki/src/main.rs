use clap::{Parser, Subcommand};
use crypto_utils::CryptoUtils;
use log::{error, info};
use network::P2PService;
use osvauld_core::models::{device::Device, user::User};
use persistance::{DbConnection, database::initialize_repositories, initialize_database};
use services::{generate_one_time_ucan_token, handle_signup, is_signed_up, load_certificate};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(author, version, about = "LivNote P2P CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Database path
    #[arg(short, long, default_value = "cli.db")]
    db_path: String,

    /// Domain for UCAN tokens
    #[arg(short = 'o', long, default_value = "livnote")]
    domain: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new user
    Init {
        /// Username for the new user
        #[arg(short, long)]
        username: String,

        /// Passphrase for encryption
        #[arg(short, long)]
        passphrase: String,
    },

    /// Start the P2P listener and print connection token
    Start {
        /// Passphrase to unlock the certificate
        #[arg(short, long)]
        passphrase: String,

        /// Generate and print a one-time connection token
        #[arg(short = 't', long)]
        print_token: bool,
    },

    /// Generate a connection token without starting the listener
    Token {
        /// Passphrase to unlock the certificate
        #[arg(short, long)]
        passphrase: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    // Initialize database
    let db_connection =
        initialize_database(&cli.db_path)
            .await
            .map_err(|e| -> Box<dyn std::error::Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;
    let repo_ctx = Arc::new(initialize_repositories(db_connection.clone()));
    let crypto_utils = Arc::new(Mutex::new(CryptoUtils::new()));
    let domain = Arc::new(cli.domain);

    match cli.command {
        Commands::Init {
            username,
            passphrase,
        } => {
            handle_init(&username, &passphrase, repo_ctx.clone()).await?;
        }
        Commands::Start {
            passphrase,
            print_token,
        } => {
            handle_start(
                &passphrase,
                print_token,
                repo_ctx.clone(),
                crypto_utils.clone(),
                domain.clone(),
            )
            .await?;
        }
        Commands::Token { passphrase } => {
            handle_token(&passphrase, repo_ctx.clone(), crypto_utils.clone(), &domain).await?;
        }
    }

    Ok(())
}

async fn handle_init(
    username: &str,
    passphrase: &str,
    repo_ctx: Arc<persistance::database::RepositoryContext>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if already signed up
    if is_signed_up(repo_ctx.clone()).await? {
        error!("User already initialized. Use 'start' command to begin.");
        return Ok(());
    }

    info!("Initializing new user: {}", username);

    // Create user and certificates
    handle_signup(username, passphrase, repo_ctx.clone()).await?;

    // Create default folder
    services::create_default_folder(repo_ctx.clone()).await?;

    info!("✓ User '{}' created successfully", username);
    info!("✓ Default folder created");
    info!("Use 'start' command with your passphrase to begin P2P service");

    Ok(())
}

async fn handle_start(
    passphrase: &str,
    print_token: bool,
    repo_ctx: Arc<persistance::database::RepositoryContext>,
    crypto_utils: Arc<Mutex<CryptoUtils>>,
    domain: Arc<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if user exists
    if !is_signed_up(repo_ctx.clone()).await? {
        error!("No user found. Please run 'init' first.");
        return Ok(());
    }

    info!("Loading user certificate...");

    // Load certificate and get user/device
    let (user, device) = load_certificate(passphrase, repo_ctx.clone(), &crypto_utils).await?;

    info!("✓ Logged in as: {}", user.username);
    info!("✓ User ID: {}", user.id);
    info!("✓ Device ID: {}", device.id);

    // Generate and print connection token if requested
    if print_token {
        let (token, pub_key) =
            generate_one_time_ucan_token(&domain, &crypto_utils, repo_ctx.clone()).await?;

        println!("\n═══════════════════════════════════════════");
        println!("ONE-TIME CONNECTION TOKEN");
        println!("═══════════════════════════════════════════");
        println!("{}", token);
        println!("═══════════════════════════════════════════");
        println!("Public Key: {}", pub_key);
        println!("═══════════════════════════════════════════\n");
    }

    // Initialize P2P service
    info!("Starting P2P service...");
    let (p2p_service, mut p2p_receiver, _p2p_sender, mut incoming_receiver) =
        P2PService::new(repo_ctx.clone(), crypto_utils.clone(), domain.clone());

    let p2p_service = Arc::new(p2p_service);

    // Start P2P service
    p2p_service.start_p2p_service(&device, &user).await?;

    info!("✓ P2P service started");
    info!("✓ Node ID: {}", device.device_key);
    info!("Listening for incoming connections...");

    // Create tasks for handling events
    let p2p_service_clone = p2p_service.clone();
    let incoming_task = tokio::spawn(async move {
        let p2p_service_clone = p2p_service.clone();
        P2PService::start_processing_incoming_events(
            (*p2p_service_clone).clone(),
            incoming_receiver,
        );
    });

    // Handle P2P events
    let event_task = tokio::spawn(async move {
        while let Some(event) = p2p_receiver.recv().await {
            use network::p2p::P2PEvent;
            match event {
                P2PEvent::Connected => {
                    info!("📡 Peer connected");
                }
                P2PEvent::Disconnected => {
                    info!("📡 Peer disconnected");
                }
                P2PEvent::HandshakeFailed { error } => {
                    error!("❌ Handshake failed: {}", error);
                }
                P2PEvent::SyncComplete => {
                    info!("✓ Sync completed");
                }
                P2PEvent::ShareComplete => {
                    info!("✓ Share completed");
                }
                P2PEvent::Error { message, source } => {
                    error!("❌ P2P error from {}: {}", source, message);
                }
                P2PEvent::LiveEditConnected { connection_id } => {
                    info!("✏️ Live edit connected: {}", connection_id);
                }
                P2PEvent::ResourceAdded { resource_id } => {
                    info!("📄 Resource added: {}", resource_id);
                }
                _ => {
                    // Handle other events as needed
                }
            }
        }
    });

    // Setup signal handler for graceful shutdown
    let shutdown_signal = tokio::signal::ctrl_c();

    tokio::select! {
        _ = shutdown_signal => {
            info!("\n✓ Shutting down P2P service...");
        }
        _ = incoming_task => {
            info!("Incoming event processor stopped");
        }
        _ = event_task => {
            info!("P2P event handler stopped");
        }
    }

    info!("✓ P2P service stopped");
    Ok(())
}

async fn handle_token(
    passphrase: &str,
    repo_ctx: Arc<persistance::database::RepositoryContext>,
    crypto_utils: Arc<Mutex<CryptoUtils>>,
    domain: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if user exists
    if !is_signed_up(repo_ctx.clone()).await? {
        error!("No user found. Please run 'init' first.");
        return Ok(());
    }

    info!("Loading user certificate...");

    // Load certificate to verify passphrase
    let (user, _device) = load_certificate(passphrase, repo_ctx.clone(), &crypto_utils).await?;

    info!("✓ Authenticated as: {}", user.username);

    // Generate connection token
    let (token, pub_key) =
        generate_one_time_ucan_token(domain, &crypto_utils, repo_ctx.clone()).await?;

    println!("\n═══════════════════════════════════════════");
    println!("ONE-TIME CONNECTION TOKEN");
    println!("═══════════════════════════════════════════");
    println!("{}", token);
    println!("═══════════════════════════════════════════");
    println!("Public Key: {}", pub_key);
    println!("User ID: {}", user.id);
    println!("Username: {}", user.username);
    println!("═══════════════════════════════════════════\n");

    Ok(())
}
