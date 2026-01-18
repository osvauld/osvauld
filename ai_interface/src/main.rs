//! AI Interface - Orchestrator for AI agents to control apps
//!
//! All paths derived from session name: /tmp/sthalam/{name}/
//!
//! ## Usage
//! ```bash
//! # Start session (you run this):
//! ./ai_interface --name my_test --instances owner,customer --node --show-ui
//!
//! # Claude sends commands via socket:
//! echo '{"target":"owner","action":"login","params":{"passphrase":"test123"}}' | nc -U /tmp/sthalam/my_test/ai.sock
//! ```

mod controller;
mod protocol;

use std::path::PathBuf;

use clap::Parser;
use controller::{SessionConfig, TestController};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "ai_interface")]
#[command(about = "AI interface for app orchestration - spawns apps in tmux, accepts commands via socket")]
struct Args {
    /// Session name - used for tmux session, directory, and socket names
    /// All paths derived: /tmp/sthalam/{name}/
    #[arg(short, long)]
    name: String,

    /// Comma-separated instance names (e.g., "owner,customer1,customer2")
    #[arg(short, long, value_delimiter = ',')]
    instances: Vec<String>,

    /// Spawn kunki node
    #[arg(long)]
    node: bool,

    /// Path to slint_shell binary (default: target/debug/slint_shell)
    #[arg(long)]
    shell_binary: Option<PathBuf>,

    /// Path to kunki binary (default: target/debug/kunki)
    #[arg(long)]
    node_binary: Option<PathBuf>,

    /// Show UI windows (instead of headless testing backend)
    #[arg(long)]
    show_ui: bool,

    /// Use stdin/stdout instead of socket (for testing)
    #[arg(long)]
    stdin: bool,

    /// Verbose logging to stderr
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize logging (to stderr, so stdout is clean for JSON)
    let filter = if args.verbose {
        EnvFilter::new("ai_interface=debug,warn")
    } else {
        EnvFilter::new("ai_interface=info,warn")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    // Resolve binary paths
    let shell_binary = args
        .shell_binary
        .unwrap_or_else(|| {
            let cargo_target = std::env::var("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("target"));
            cargo_target.join("debug/slint_shell")
        })
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("slint_shell"));

    let node_binary = args
        .node_binary
        .or_else(|| {
            let cargo_target = std::env::var("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("target"));
            Some(cargo_target.join("debug/kunki"))
        })
        .and_then(|p| p.canonicalize().ok());

    // Validate
    if args.instances.is_empty() {
        eprintln!("Error: At least one instance name required");
        eprintln!("Example: --name my_test --instances owner,customer");
        std::process::exit(1);
    }

    let config = SessionConfig {
        name: args.name.clone(),
        instances: args.instances,
        spawn_node: args.node,
        shell_binary,
        node_binary,
        show_ui: args.show_ui,
    };

    tracing::info!(
        session = %config.name,
        base_dir = %config.base_dir().display(),
        ai_socket = %config.ai_socket().display(),
        instances = ?config.instances,
        spawn_node = config.spawn_node,
        show_ui = config.show_ui,
        "Starting AI interface"
    );

    let mut controller = TestController::new(config);

    // Spawn instances in tmux
    if let Err(e) = controller.spawn_instances().await {
        eprintln!("{{\"error\": \"Failed to spawn instances: {}\"}}", e);
        std::process::exit(1);
    }

    // Run either socket server or stdin REPL
    if args.stdin {
        controller.run_stdin_repl().await;
    } else {
        if let Err(e) = controller.run_socket_server().await {
            eprintln!("{{\"error\": \"Socket server failed: {}\"}}", e);
            std::process::exit(1);
        }
    }

    controller.shutdown().await;

    Ok(())
}
