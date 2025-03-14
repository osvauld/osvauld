use tokio::time::Duration;

// Constants shared across the p2p module
pub const ALPN_PROTOCOL: &[u8] = b"n0/iroh/examples/magic/0";
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(60);
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);
pub const MAX_HANDSHAKE_SIZE: usize = 32768; // 32KB max size for handshake messages
