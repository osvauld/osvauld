# Code Structure Policy

## Separate Functions for Different Flows

Each logical flow gets its own function. This makes debugging clearer and enables focused `#[instrument]` tracing.

### Bad - Mixed Flows

```rust
async fn process_hello(&self, node_id: NodeId, ...) {
    let is_first = permit.is_first_connection();
    if is_first {
        // 50 lines of first connection logic
        // Hard to trace, hard to document
    } else {
        // 50 lines of reconnection logic
        // Different flow buried in else branch
    }
}
```

### Good - Dispatch to Separate Functions

```rust
/// Routes Hello message to appropriate handler based on permit type.
#[instrument]
async fn process_hello(&self, node_id: NodeId, ...) -> Result<PeerInfo, String> {
    let parsed = Permit::from_token(permit)?;

    if parsed.is_first_connection() {
        self.handle_first_connection(node_id, did, username, public_key, permit).await
    } else {
        self.handle_reconnection(node_id, did, permit).await
    }
}

/// Handles first connection from a new owner.
/// [detailed doc comment]
#[instrument]
async fn handle_first_connection(&self, ...) -> Result<PeerInfo, String> {
    // Clear, focused logic for first connection only
}

/// Handles reconnection from existing owner.
/// [detailed doc comment]
#[instrument]
async fn handle_reconnection(&self, ...) -> Result<PeerInfo, String> {
    // Clear, focused logic for reconnection only
}
```

### Benefits

- Each function has single responsibility
- `#[instrument]` shows clear call tree in logs
- Doc comments are focused on one flow
- Easier to test individually
- Easier for Claude to understand context

## Nesting Limits

**Maximum 2 levels of nesting.** Beyond that, extract to a function.

### Bad - Deep Nesting

```rust
async fn process(&self, msg: Message) {
    match msg {
        Message::Hello { permit, .. } => {
            if permit.is_valid() {
                match permit.permit_type() {
                    PermitType::FirstConnection => {
                        if self.has_owner() {
                            // 5 levels deep - hard to follow
                        }
                    }
                }
            }
        }
    }
}
```

### Good - Extract to Functions

```rust
async fn process(&self, msg: Message) {
    match msg {
        Message::Hello { permit, .. } => self.on_hello(permit, ...).await,
        Message::Welcome { .. } => self.on_welcome(..).await,
    }
}

async fn on_hello(&self, permit: &str, ..) -> Result<()> {
    let parsed = Permit::from_token(permit)?;

    match parsed.permit_type() {
        PermitType::FirstConnection => self.handle_first_connection(..).await,
        PermitType::Reconnection => self.handle_reconnection(..).await,
    }
}
```

## Match Arm Length

Match arms with **more than 3 lines** should be extracted to functions.

### Bad

```rust
match msg {
    Message::Hello { did, username, public_key, permit } => {
        let parsed = Permit::from_token(permit)?;
        let verified = self.verify_permit(&parsed)?;
        let owner_info = OwnerInfo::new(did, username, public_key);
        self.store.set_owner(owner_info)?;
        let welcome_permit = self.issue_permit_for_owner()?;
        self.send_welcome(welcome_permit).await?;
    }
}
```

### Good

```rust
match msg {
    Message::Hello { did, username, public_key, permit } => {
        self.on_hello(node_id, did, username, public_key, permit).await?
    }
}
```

## Handler Naming

| Prefix | Use For | Example |
|--------|---------|---------|
| `on_*` | Incoming message handlers | `on_hello`, `on_welcome`, `on_permit_grant` |
| `handle_*` | Processing logic | `handle_first_connection`, `handle_reconnection` |
| `process_*` | Router/dispatch functions | `process_message`, `process_hello` |
| `send_*` | Outgoing operations | `send_welcome`, `send_ack` |

## Crate Boundaries

Context should not leak across crate boundaries. Each crate has specific responsibilities:

| Crate | Responsibility | Accesses |
|-------|---------------|----------|
| `transport` | QUIC connections, message framing | iroh |
| `courier` | P2P orchestration, handshakes | transport, butler, gurkha |
| `butler` | Services API, storage coordination | herald, gurkha, repositories |
| `gurkha` | Permit parse + validate only | herald (for crypto) |
| `herald` | Identity, encryption, signing | (standalone) |
| `tauri_handlers` | App commands | butler, courier |

### Dependency Flow

```
tauri_handlers / kunki
        │
        ▼
    courier ◄──── butler
        │            │
        ▼            ▼
   transport     gurkha ◄── herald
```

### Service APIs

Butler exposes **services only**, not direct store access:

```rust
// Good - use service API
node_service.set_owner_permit_from_owner(permit)?;

// Bad - direct store access from outside butler
store.owner.set_permit(permit)?;
```

## Rust Style

### Error Handling

Use idiomatic Rust:

```rust
// Good - ? operator
let permit = Permit::from_token(token)?;

// Good - combinators for transforms
let node_id = node_id_str.parse().map_err(|e| format!("Invalid node_id: {}", e))?;

// Each crate wraps dependency errors
pub enum CourierError {
    Transport(TransportError),
    Butler(ButlerError),
    // ...
}
```

### Imports

Import at the top of the file, no full paths in code:

```rust
// Good
use transport::{Message, ConnectionHandle};

fn send(&self, msg: Message) { ... }

// Bad
fn send(&self, msg: transport::protocol::Message) { ... }
```

### Async Patterns

Use `tokio::spawn` for async event loops:

```rust
// Spawn event consumer
tokio::spawn(async move {
    while let Some(event) = event_rx.recv().await {
        handle_event(event).await;
    }
});
```

Use `&self` with `Arc` internals for services:

```rust
pub struct Courier {
    transport: Arc<Transport>,
    services: Arc<HandshakeServices>,
}
```
