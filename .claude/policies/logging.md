# Logging Policy

## Purpose

Logs exist for **Claude and the author** to understand complex sync flows during debugging. This is an experimental approach - prioritize visibility over minimal logging.

## Log Levels

| Level | Use Case | Production | Examples |
|-------|----------|------------|----------|
| `trace!` | Wire-level: bytes, serialization, raw data | Filtered out | Message send/receive bytes |
| `debug!` | Implementation details: cache hits, skipped operations | Filtered out | Skipping storage on reconnection |
| `info!` | Business events: connections, handshakes, sync completion | **Visible** | "Connected to peer", "Handshake complete" |
| `warn!` | Recoverable issues: retry, fallback, degraded state | **Visible** | "Retry connection", "Using fallback" |
| `error!` | Failures requiring attention | **Visible** | "Failed to connect", "Invalid permit" |

## Instrumentation

Use `#[instrument]` on ALL async functions:

```rust
#[instrument]
async fn handle_first_connection(
    &self,
    node_id: NodeId,
    did: &str,
    username: &str,
    public_key: &[u8],
    permit: &str,
) -> Result<PeerInfo, String> {
    // ...
}
```

**Current stance**: Don't skip parameters. We're experimenting with full visibility.

## Wire-Level Logging (Transport Layer)

Transport layer logs send/receive at trace level using Debug derive:

```rust
pub async fn send(&self, msg: &Message) -> Result<()> {
    let serialized = bincode::serialize(msg)?;
    let len = serialized.len();

    trace!("─→ SEND ({} bytes): {:?}", len, msg);

    // ... send logic
}
```

Receive side:
```rust
trace!("←─ RECV ({} bytes) from {}: {:?}", len, node_id, message);
```

## Business Logic Logging (Courier Layer)

Courier does NOT repeat wire logs. Log business events at info:

```rust
info!("Handshake complete with owner (first connection)");
info!("Reconnection successful");
```

Use debug for implementation details:
```rust
debug!("Reconnection - permit already stored, skipping storage");
```

## What NOT to Log

- Don't duplicate wire logs in business logic
- Don't log at info for internal operations
- Don't use if statements just for different log messages:

**Bad:**
```rust
if is_reconnection {
    info!("Reconnection handshake complete");
} else {
    info!("First connection handshake complete");
}
```

**Good:**
```rust
let conn_type = if is_reconnection { "reconnection" } else { "first connection" };
info!("Handshake complete ({})", conn_type);
```

## Message Logging

Use `{:?}` format with Debug derive for all message types:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    Hello { ... },
    Welcome { ... },
    // ...
}
```

This provides zero-cost formatting in production (trace filtered out) while giving full visibility during development.
