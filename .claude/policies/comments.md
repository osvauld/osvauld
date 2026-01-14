# Comment Policy

## Purpose

Comments and logs exist for **Claude and the author** to understand complex sync flows during debugging. This is an experimental approach - prioritize clarity over brevity.

## Function Documentation Format

Use domain-specific terminology (their/our, peer, permit). Comments should be understandable by both Claude and human developers.

### Full Example

```rust
/// Handles first connection from a new owner.
///
/// **Context**: Called when we (Node) receive Hello from someone claiming to be owner,
/// and we don't have an owner yet.
///
/// **Peer sends**: Hello message with first_connection permit from connection string
/// **We verify**: Permit is valid first_connection type, signature proves identity
/// **We store**: OwnerInfo (their DID, username, public_key)
/// **We issue**: Long-lived permit_for_owner
/// **We send**: Welcome message containing permit_for_owner
///
/// After this, peer should send PermitGrant with permit_for_node.
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

## Key Elements

Use these sections as appropriate:

| Section | Use When | Example |
|---------|----------|---------|
| **Context** | Always - when/why function is called | "Called when we receive Hello from peer" |
| **Peer sends** | Message handlers | "Hello with first_connection permit" |
| **We verify** | Validation logic | "Permit validity, signature" |
| **We store** | Persistence operations | "OwnerInfo to database" |
| **We issue** | Creating tokens/permits | "Long-lived permit_for_owner" |
| **We send** | Response messages | "Welcome with permit_for_owner" |

## Perspective

Use "we" and "our" for the local node, "peer" or "they/their" for remote:

```rust
/// **Context**: Called when we (Node) receive Hello...
/// **Peer sends**: Hello message with permit
/// **We verify**: Their permit is valid
/// **We store**: Their identity info
/// **We send**: Our welcome response
```

## Inline Comments

Use sparingly, only for non-obvious logic:

```rust
// Permit must be first_connection type - subsequent connections use stored permits
if !permit.is_first_connection() {
    return Err("Expected first_connection permit".into());
}
```

## What NOT to Comment

### Obvious code
```rust
// Bad - obvious
let len = data.len(); // Get the length

// Good - no comment needed
let len = data.len();
```

### What code does (should be clear from code itself)
```rust
// Bad - describes what code does
// Serialize the message to bytes
let serialized = bincode::serialize(msg)?;

// Good - no comment, code is self-documenting
let serialized = bincode::serialize(msg)?;
```

### Commented-out code
```rust
// Bad - delete instead
// let old_impl = do_old_thing();
let new_impl = do_new_thing();

// Good - just the new code
let new_impl = do_new_thing();
```

## Module-Level Documentation

Use `//!` for module docs at the top of files:

```rust
//! Transport Layer for Osvauld P2P Network
//!
//! This crate provides the pure P2P transport layer using iroh.
//! It handles connections, streams, and message framing.
//! Business logic lives in the Courier layer.
//!
//! # Architecture
//!
//! - **Transport**: Main API, owns endpoint and connection pool
//! - **ConnectionPool**: Manages active connections
//! - **ConnectionHandle**: Lightweight reference for sending messages
```

## Struct and Enum Documentation

Document public types with purpose and usage:

```rust
/// Handle to a peer connection
///
/// This is a lightweight reference that Courier can store and use to send messages.
/// The actual connection is owned by Transport's ConnectionPool.
#[derive(Clone)]
pub struct ConnectionHandle {
    inner: Arc<PeerConnection>,
    node_id: NodeId,
}
```
