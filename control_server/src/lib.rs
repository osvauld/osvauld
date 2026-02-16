//! Control Server - Shared infrastructure for Unix socket JSON-RPC
//!
//! This crate provides the base control server implementation used by
//! sthalam and kunki for programmatic control and testing.
//!
//! # Usage
//!
//! ```ignore
//! use control_server::{ControlServer, CommandHandler, Response, types::*};
//!
//! struct MyHandler { /* ... */ }
//!
//! #[async_trait::async_trait]
//! impl CommandHandler for MyHandler {
//!     async fn handle(&self, method: &str, params: Option<serde_json::Value>, id: u64) -> Option<Response> {
//!         match method {
//!             "my_command" => Some(Response::ok_status(id)),
//!             _ => None,
//!         }
//!     }
//! }
//!
//! let server = ControlServer::new(path, "instance".into(), MyHandler::new());
//! server.start().await?;
//! ```

pub mod types;
pub mod server;
pub mod commands;

// Re-export main types
pub use types::{
    Request,
    Response,
    ErrorResponse,
    error_codes,
};

pub use server::{
    ControlServer,
    CommandHandler,
};

// Re-export async_trait for convenience
pub use async_trait::async_trait;
