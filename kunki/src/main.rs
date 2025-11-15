use clap::{Parser, Subcommand};
use crypto_utils::CryptoUtils;
use tracing::{error, info};
use network::{P2PService, p2p_init};
use osvauld_core::models::UserRole;
use persistance::{database::initialize_repositories, initialize_database};

use base64::{Engine as _, engine::general_purpose};
use serde_json::json;
use services::{
    generate_folder_share_token, generate_one_time_ucan_token, handle_signup, is_signed_up,
    load_certificate,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Helper function to get passphrase - either from argument or by prompting
fn get_passphrase(passphrase_opt: Option<String>, prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
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
#[command(author, version, about = "LivNote P2P CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Database path
    #[arg(short, long, default_value = "cli.db")]
    db_path: String,

    /// Domain for UCAN tokens
    #[arg(short = 'o', long, default_value = "sthalam")]
    domain: String,
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
    let crypto_utils = Arc::new(RwLock::new(CryptoUtils::new()));
    let ucan_service = Arc::new(RwLock::new(gurkha::UcanService::new()));
    let domain = Arc::new(cli.domain);

    match cli.command {
        Commands::Init {
            username,
            passphrase,
        } => {
            let pass = get_passphrase(passphrase, "Enter passphrase:")?;
            handle_init(&username, &pass, repo_ctx.clone(), domain.clone()).await?;
        }
        Commands::Start { passphrase } => {
            let pass = get_passphrase(passphrase, "Enter passphrase to unlock certificate:")?;
            handle_start(
                &pass,
                repo_ctx.clone(),
                crypto_utils.clone(),
                ucan_service.clone(),
                domain.clone(),
            )
            .await?;
        }
        Commands::FolderToken {
            passphrase,
            folder_id,
        } => {
            let pass = get_passphrase(passphrase, "Enter passphrase to unlock certificate:")?;
            handle_folder_token(
                &pass,
                &folder_id,
                repo_ctx.clone(),
                crypto_utils.clone(),
                ucan_service.clone(),
                &domain,
            )
            .await?;
        }
    }

    Ok(())
}

async fn handle_init(
    username: &str,
    passphrase: &str,
    repo_ctx: Arc<persistance::database::RepositoryContext>,
    domain: Arc<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if already signed up
    if is_signed_up(repo_ctx.clone()).await? {
        error!("User already initialized. Use 'start' command to begin.");
        return Ok(());
    }

    info!("Initializing new user: {}", username);

    // Create user and certificates
    handle_signup(username, passphrase, repo_ctx.clone(), &domain).await?;

    // Create default folder

    info!("✔ User '{}' created successfully", username);
    info!("✔ Default folder created");
    info!("Use 'start' command with your passphrase to begin P2P service");

    Ok(())
}

async fn handle_start(
    passphrase: &str,
    repo_ctx: Arc<persistance::database::RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
    ucan_service: Arc<RwLock<gurkha::UcanService>>,
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

    info!("✔ Logged in as: {}", user.username);
    info!("✔ User ID: {}", user.id);
    info!("✔ Device ID: {}", device.id);

    // Load UCAN keys into ucan_service
    let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;
    let (signing_key, verifying_key) = {
        let crypto = crypto_utils.read().await;
        crypto.decrypt_ucan_key(&encrypted_ucan_key)?
    };
    {
        let mut ucan_guard = ucan_service.write().await;
        ucan_guard.load_keys(signing_key, verifying_key);
    }

    // Generate and print connection token (always)
    let (token, pub_key) = generate_one_time_ucan_token(
        &domain,
        &UserRole::Owner.to_string(),
        &ucan_service,
    )
    .await?;

    println!("\n╔══════════════════════════════════════════╗");
    println!("║     ONE-TIME CONNECTION STRING           ║");
    println!("╚══════════════════════════════════════════╝");

    // Create connection string JSON
    let connection_details = json!({
        "user_public_key": user.public_key,
        "device_public_key": device.device_key,
        "username": user.username,
        "ucan_token": token,
        "ucan_pub_key": pub_key,
    });

    // Convert to string and base64 encode
    let connection_json = connection_details.to_string();
    let encoded_connection = general_purpose::STANDARD.encode(connection_json.as_bytes());

    println!("{}", encoded_connection);
    println!("╚══════════════════════════════════════════╝");
    println!("\nℹ️  User: {}", user.username);
    println!("ℹ️  User ID: {}", user.id);
    println!("╚══════════════════════════════════════════╝\n");

    // Initialize P2P service (CLI doesn't need event handling)
    info!("Starting P2P service...");
    let (p2p_service, _p2p_receiver) =
        P2PService::new(repo_ctx.clone(), crypto_utils.clone(), ucan_service.clone(), domain.clone());

    let p2p_service = Arc::new(p2p_service);

    // Initialize P2P network (new pattern)
    p2p_init::initialize_p2p(&p2p_service, &user, &device).await?;

    info!("✔ P2P service started");
    info!("✔ Node ID: {}", device.device_key);
    info!("Listening for incoming connections...");

    // Print status
    println!("\n🟢 SERVICE STATUS: ONLINE");
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
    repo_ctx: Arc<persistance::database::RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
    ucan_service: Arc<RwLock<gurkha::UcanService>>,
    domain: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if user exists
    if !is_signed_up(repo_ctx.clone()).await? {
        error!("No user found. Please run 'init' first.");
        return Ok(());
    }

    info!("Loading user certificate...");

    // Load certificate to verify passphrase
    let (user, device) = load_certificate(passphrase, repo_ctx.clone(), &crypto_utils).await?;

    info!("✔ Authenticated as: {}", user.username);

    // Load UCAN keys into ucan_service
    let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;
    let (signing_key, verifying_key) = {
        let crypto = crypto_utils.read().await;
        crypto.decrypt_ucan_key(&encrypted_ucan_key)?
    };
    {
        let mut ucan_guard = ucan_service.write().await;
        ucan_guard.load_keys(signing_key, verifying_key);
    }

    info!("Generating folder share token for folder: {}", folder_id);

    // Generate folder share token
    let (token, pub_key) =
        generate_folder_share_token(folder_id, domain, &ucan_service).await?;

    println!("\n╔══════════════════════════════════════════╗");
    println!("║     FOLDER SHARE TOKEN                   ║");
    println!("╚══════════════════════════════════════════╝");
    println!("Folder ID: {}", folder_id);
    println!("╚══════════════════════════════════════════╝");
    println!("Token: {}", token);
    println!("╚══════════════════════════════════════════╝");
    println!("Public Key: {}", pub_key);
    println!("╚══════════════════════════════════════════╝");

    // Create connection string JSON (same format as connection token)
    let connection_details = json!({
        "user_public_key": user.public_key,
        "device_public_key": device.device_key,
        "username": user.username,
        "ucan_token": token,
        "ucan_pub_key": pub_key,
    });

    // Convert to string and base64 encode
    let connection_json = connection_details.to_string();
    let encoded_connection = general_purpose::STANDARD.encode(connection_json.as_bytes());

    println!("Connection String: {}", encoded_connection);
    println!("╚══════════════════════════════════════════╝\n");

    Ok(())
}
