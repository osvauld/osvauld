//! AI Interface - Standardized interface for AI agents to interact with apps
//!
//! Works with Claude, local LLMs (via kunki), and other AI agents.
//! Spawns app instances, routes commands via debug sockets, aggregates logs.
//!
//! ## Usage
//! ```bash
//! ./ai_interface --instances owner,customer1 --db-dir /tmp/test_dbs
//! ```
//!
//! ## Commands (JSON over stdin)
//! ```json
//! // Eval Lua code in an instance
//! {"target": "owner", "action": "eval", "params": {"code": "add_product({name='Test'})"}}
//!
//! // UI automation
//! {"target": "owner", "action": "ui_click", "params": {"label": "create-space-button"}}
//! {"target": "owner", "action": "ui_type", "params": {"label": "space-name-input", "text": "My Shop"}}
//!
//! // Get state
//! {"target": "owner", "action": "state"}
//!
//! // Get logs
//! {"action": "logs", "params": {"last": 50}}
//!
//! // List instances
//! {"action": "list_instances"}
//! ```

mod controller;
mod protocol;

use std::path::PathBuf;

use clap::Parser;
use controller::{TestConfig, TestController};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "ai_interface")]
#[command(about = "AI interface for app automation - works with Claude, local LLMs, etc.")]
struct Args {
    /// Comma-separated instance names (e.g., "owner,customer1,customer2")
    #[arg(short, long, value_delimiter = ',')]
    instances: Vec<String>,

    /// Spawn kunki node
    #[arg(long)]
    node: bool,

    /// Directory for database files
    #[arg(long, default_value = "/tmp/sthalam_test")]
    db_dir: PathBuf,

    /// Path to slint_shell binary
    #[arg(long)]
    shell_binary: Option<PathBuf>,

    /// Path to kunki binary
    #[arg(long)]
    node_binary: Option<PathBuf>,

    /// JSON mode (for Claude) - default, minimal output
    #[arg(long, default_value = "true")]
    json: bool,

    /// Verbose logging to stderr
    #[arg(short, long)]
    verbose: bool,

    /// Show UI windows (instead of headless testing backend)
    #[arg(long)]
    show_ui: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize logging (to stderr, so stdout is clean for JSON)
    let filter = if args.verbose {
        EnvFilter::new("ui_test_harness=debug,warn")
    } else {
        EnvFilter::new("warn")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    // Resolve binary paths (canonicalize to absolute paths)
    let shell_binary = args.shell_binary
        .unwrap_or_else(|| {
            // Try to find in target/debug
            let cargo_target = std::env::var("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("target"));
            cargo_target.join("debug/slint_shell")
        })
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("slint_shell"));

    let node_binary = args.node_binary
        .or_else(|| {
            let cargo_target = std::env::var("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("target"));
            Some(cargo_target.join("debug/kunki"))
        })
        .and_then(|p| p.canonicalize().ok());

    // Validate instance names
    if args.instances.is_empty() {
        eprintln!("Error: At least one instance name required");
        eprintln!("Example: --instances owner,customer1");
        std::process::exit(1);
    }

    let config = TestConfig {
        instances: args.instances,
        spawn_node: args.node,
        db_dir: args.db_dir,
        shell_binary,
        node_binary,
        show_ui: args.show_ui,
    };

    tracing::info!(
        instances = ?config.instances,
        spawn_node = config.spawn_node,
        db_dir = %config.db_dir.display(),
        "Starting test harness"
    );

    let mut controller = TestController::new(config);

    // Spawn instances
    if let Err(e) = controller.spawn_instances().await {
        eprintln!("{{\"error\": \"Failed to spawn instances: {}\"}}", e);
        std::process::exit(1);
    }

    // Run JSON REPL
    controller.run_json_repl().await;

    // Cleanup
    controller.shutdown().await;

    Ok(())
}
