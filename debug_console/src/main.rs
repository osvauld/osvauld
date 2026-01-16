//! Debug Console - egui-based debugger for Sthalam apps
//!
//! Connects to the same Unix socket as the test harness,
//! providing a Chrome DevTools-like experience for manual debugging.
//!
//! ## Usage
//! ```bash
//! ./debug_console --connect /tmp/sthalam_test/owner.sock
//! ```

mod connection;
mod panels;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use eframe::egui;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

use connection::DebugConnection;
use panels::{LogPanel, StatePanel, ReplPanel};

#[derive(Parser)]
#[command(name = "debug_console")]
#[command(about = "Debug console for Sthalam apps")]
struct Args {
    /// Unix socket path to connect to
    #[arg(short, long)]
    connect: PathBuf,

    /// Instance name (for display)
    #[arg(short, long, default_value = "debug")]
    name: String,
}

fn main() -> Result<(), eframe::Error> {
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("debug_console=debug,warn"))
        .init();

    tracing::info!(
        socket = %args.connect.display(),
        "Starting debug console"
    );

    // Create async runtime for background tasks
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title(format!("Debug Console - {}", args.name)),
        ..Default::default()
    };

    eframe::run_native(
        "Debug Console",
        options,
        Box::new(|_cc| {
            // Create shared connection state
            let connection = Arc::new(RwLock::new(None));
            let logs: Arc<RwLock<Vec<LogEntry>>> = Arc::new(RwLock::new(Vec::new()));
            let state: Arc<RwLock<Option<serde_json::Value>>> = Arc::new(RwLock::new(None));

            // Spawn connection task
            let conn_clone = connection.clone();
            let socket_path = args.connect.clone();

            rt.spawn(async move {
                match DebugConnection::connect(socket_path).await {
                    Ok(conn) => {
                        *conn_clone.write().await = Some(conn);
                        tracing::info!("Connected to debug server");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to connect to debug server");
                    }
                }
            });

            Ok(Box::new(DebugConsoleApp {
                rt: Arc::new(rt),
                connection,
                logs,
                state,
                instance_name: args.name,
                log_panel: LogPanel::default(),
                state_panel: StatePanel::default(),
                repl_panel: ReplPanel::default(),
                active_tab: Tab::Logs,
            }))
        }),
    )
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LogEntry {
    pub ts: String,
    pub level: String,
    pub target: String,
    pub msg: String,
    #[serde(default)]
    pub instance: Option<String>,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Logs,
    State,
    Repl,
}

struct DebugConsoleApp {
    rt: Arc<tokio::runtime::Runtime>,
    connection: Arc<RwLock<Option<DebugConnection>>>,
    logs: Arc<RwLock<Vec<LogEntry>>>,
    state: Arc<RwLock<Option<serde_json::Value>>>,
    instance_name: String,
    log_panel: LogPanel,
    state_panel: StatePanel,
    repl_panel: ReplPanel,
    active_tab: Tab,
}

impl eframe::App for DebugConsoleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top panel with tabs
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Logs, "Logs");
                ui.selectable_value(&mut self.active_tab, Tab::State, "State");
                ui.selectable_value(&mut self.active_tab, Tab::Repl, "Lua REPL");

                ui.separator();

                // Connection status
                let connected = self.rt.block_on(async {
                    self.connection.read().await.is_some()
                });

                if connected {
                    ui.colored_label(egui::Color32::GREEN, format!("● Connected: {}", self.instance_name));
                } else {
                    ui.colored_label(egui::Color32::RED, "● Disconnected");
                }

                // Refresh button
                if ui.button("Refresh").clicked() {
                    self.refresh_data();
                }
            });
        });

        // Main panel
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                Tab::Logs => {
                    let logs = self.rt.block_on(async {
                        self.logs.read().await.clone()
                    });
                    self.log_panel.ui(ui, &logs);
                }
                Tab::State => {
                    let state = self.rt.block_on(async {
                        self.state.read().await.clone()
                    });
                    self.state_panel.ui(ui, &state);
                }
                Tab::Repl => {
                    self.repl_panel.ui(ui, &self.rt, &self.connection);
                }
            }
        });

        // Request repaint for live updates
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

impl DebugConsoleApp {
    fn refresh_data(&mut self) {
        let connection = self.connection.clone();
        let logs = self.logs.clone();
        let state = self.state.clone();

        self.rt.spawn(async move {
            let mut conn_guard = connection.write().await;
            if let Some(conn) = conn_guard.as_mut() {
                // Fetch logs
                if let Ok(response) = conn.send_command(r#"{"method": "logs", "params": {"last": 100}}"#).await {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                        if let Some(result) = parsed.get("result") {
                            if let Some(log_entries) = result.get("logs") {
                                if let Ok(entries) = serde_json::from_value::<Vec<LogEntry>>(log_entries.clone()) {
                                    *logs.write().await = entries;
                                }
                            }
                        }
                    }
                }

                // Fetch state
                if let Ok(response) = conn.send_command(r#"{"method": "state"}"#).await {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                        if let Some(result) = parsed.get("result") {
                            *state.write().await = Some(result.clone());
                        }
                    }
                }
            }
        });
    }
}
