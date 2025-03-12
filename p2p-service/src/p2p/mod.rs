pub mod constants;

pub mod handshake;
pub mod messaging;
pub mod service;
pub mod share;
pub mod sync;
pub mod emitter

// Re-export the service
pub use emitter::{P2PEvent, P2PEventEmitter};
pub use service::P2PService;
