# UCAN Service Migration Notes (TEMP - TO BE MERGED)

**Date:** 2025-11-15
**Status:** ✅ Complete
**Migration:** `services::ucan_service` → `gurkha` library

---

## Overview

This document captures the migration of UCAN functionality from the `services` crate to a standalone `gurkha` library, and the integration of `ucan_service` throughout the codebase following the same pattern as `crypto_utils`.

## Background

Previously, UCAN logic was spread across:
- `services/src/ucan_service.rs` - Business logic for token generation
- `crypto_utils/src/ucan_utils.rs` - Utility functions
- Mixed responsibilities with infrastructure dependencies

**Goal:** Create a pure domain logic library (`gurkha`) with zero infrastructure dependencies, similar to how `crypto_utils` is organized.

---

## Architecture Changes

### 1. UcanService Structure

```rust
// gurkha/src/service.rs
pub struct UcanService {
    signing_key: Option<SigningKey>,
    verifying_key: Option<VerifyingKey>,
}

impl UcanService {
    pub fn new() -> Self {
        Self {
            signing_key: None,
            verifying_key: None,
        }
    }

    pub fn load_keys(&mut self, signing_key: SigningKey, verifying_key: VerifyingKey) {
        self.signing_key = Some(signing_key);
        self.verifying_key = Some(verifying_key);
    }

    fn get_keys(&self) -> ServiceResult<(&SigningKey, &VerifyingKey)> {
        match (&self.signing_key, &self.verifying_key) {
            (Some(sk), Some(vk)) => Ok((sk, vk)),
            _ => Err(ServiceError::KeysNotLoaded),
        }
    }
}
```

**Key Points:**
- Keys are `Option<T>` and loaded during login via `load_keys()`
- Private `get_keys()` validates keys are loaded before any operation
- Follows the same pattern as `CryptoUtils` for consistency

### 2. Thread-Safe Wrapping

**Pattern used throughout:**
```rust
Arc<RwLock<gurkha::UcanService>>
```

**Important:** Use `tokio::sync::RwLock`, NOT `std::sync::RwLock`

**Reason:** Tauri commands and async contexts require types to be `Send + Sync`. The `std::sync::RwLockReadGuard` is NOT `Send`, causing compilation errors in async contexts.

**Example Error (if using std::sync::RwLock):**
```
error[E0277]: `std::sync::RwLockReadGuard<'_, UcanService>` cannot be sent between threads safely
```

---

## Migration Checklist

### ✅ Core Library (gurkha)

- [x] Move UCAN domain logic to `gurkha/src/`
- [x] Implement `UcanService` with optional keys
- [x] Add `get_keys()` validation to all token generation methods
- [x] Add `KeysNotLoaded` error variant
- [x] Create factory function for convenience

### ✅ Services Layer

Updated all functions to accept `&Arc<RwLock<UcanService>>`:

**Auth Service:**
- [x] `generate_one_time_ucan_token`
- [x] `generate_folder_share_token`
- [x] `parse_and_validate_handshake_token`

**Folder Service:**
- [x] `create_folder`
- [x] `share_folder`

**Resource Service:**
- [x] `create_resource` (crud.rs)
- [x] `share_resource` (crud.rs)
- [x] `prepare_resource_transfer` (sync.rs)
- [x] `accept_resource_from_peer` (sync.rs)
- [x] `get_resource_ucans_for_sync` (sync.rs)
- [x] `prepare_resource_for_peer` (sync.rs)
- [x] `prepare_resource_sync_request` (sync.rs)

**Pattern for service functions:**
```rust
pub async fn some_service_function(
    // ... other params
    ucan_service: &Arc<RwLock<gurkha::UcanService>>,
) -> ServiceResult<T> {
    let ucan_service_guard = ucan_service.read().await;
    let result = ucan_service_guard.some_method().await?;
    drop(ucan_service_guard);  // Optional, but good practice
    // ... continue
}
```

### ✅ Network Layer

- [x] Add `ucan_service` field to `PeerConnection`
- [x] Pass `ucan_service` to `PeerConnection::new()`
- [x] Update all P2P message handlers to use `peer_conn.ucan_service`
- [x] Add gurkha dependency to network/Cargo.toml

### ✅ Application Layer

**kunki (CLI):**
- [x] Add gurkha dependency
- [x] Create `ucan_service` at startup
- [x] Update `handle_start`, `handle_token`, `handle_folder_token` signatures
- [x] Pass `ucan_service` to all service calls

**sthalam (Tauri):**
- [x] Add gurkha dependency
- [x] Create `ucan_service` at startup
- [x] Manage `ucan_service` as Tauri state
- [x] Update all Tauri command handlers

**tauri_handlers:**
- [x] Add gurkha dependency
- [x] Add `ucan_service: State<'_, Arc<RwLock<gurkha::UcanService>>>` to commands
- [x] Pass `&ucan_service` to service calls

---

## Code Patterns

### 1. Application Initialization

```rust
// sthalam/src-tauri/src/lib.rs
use tokio::sync::RwLock;  // IMPORTANT: tokio, not std!

let crypto_utils = Arc::new(RwLock::new(CryptoUtils::new()));
let ucan_service = Arc::new(RwLock::new(gurkha::UcanService::new()));

// Pass to P2P service
let (p2p_service, p2p_receiver) = P2PService::new(
    repo_ctx.clone(),
    crypto_utils.clone(),
    ucan_service.clone(),  // Add this
    domain,
);

// Manage as Tauri state
app.manage(ucan_service);
```

### 2. Tauri Command Handlers

```rust
#[tauri::command]
pub async fn handle_add_folder(
    input: AddFolderInput,
    config: State<'_, HandlerConfig>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    ucan_service: State<'_, Arc<RwLock<gurkha::UcanService>>>,  // Add this
    user_state: State<'_, UserState>,
) -> Result<BaseCryptoResponse, String> {
    let user = user_state.get_user().await?;
    let folder = create_folder(
        input.name,
        Some(input.description),
        input.folder_template_json,
        repo_ctx.inner().clone(),
        &crypto_utils,
        &config.domain,
        &user,
        &ucan_service,  // Add this
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(BaseCryptoResponse::FolderCreated(folder))
}
```

### 3. Service Function Implementation

```rust
pub async fn create_folder(
    name: String,
    description: Option<String>,
    folder_template_json: String,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    domain: &str,
    user: &User,
    ucan_service: &Arc<RwLock<gurkha::UcanService>>,  // Add this parameter
) -> ServiceResult<Folder> {
    // ... validation

    let mut folder = Folder::new(name, description, false, String::new());

    // Lock, use, and release
    let ucan_service_guard = ucan_service.read().await;
    let (folder_root_ucan_key, ucan_cid) = ucan_service_guard
        .issue_folder_owner_token(&folder.id, domain, &folder_template_json)
        .await?;
    // Guard automatically dropped here

    folder.ucan = folder_root_ucan_key.clone();
    // ... rest of function
}
```

### 4. P2P Message Handlers

```rust
pub async fn handle_resource_data_sync(
    payload: &ResourceDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    let domain = &peer_conn.domain;

    services::accept_resource_from_peer(
        &payload.resource,
        &payload.share_records,
        &payload.folder_ucan,
        domain,
        repo_ctx,
        &peer_conn.ucan_service,  // Use from peer_conn
    )
    .await
    .map_err(|e| {
        error!("Failed to accept resource from peer: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to accept resource: {}", e))
    })?;

    Ok(())
}
```

---

## Common Pitfalls & Solutions

### ❌ Problem: Threading Error

```
error[E0277]: `std::sync::RwLockReadGuard<'_, UcanService>` cannot be sent between threads safely
```

**Cause:** Using `std::sync::RwLock` instead of `tokio::sync::RwLock`

**Solution:**
```rust
// ❌ WRONG
use std::sync::RwLock;

// ✅ CORRECT
use tokio::sync::RwLock;
```

### ❌ Problem: Keys Not Loaded Error

```
ServiceError::KeysNotLoaded
```

**Cause:** Trying to use UcanService before calling `load_keys()`

**Solution:** Ensure keys are loaded during login:
```rust
// In auth/login flow
let (signing_key, verifying_key) = crypto.decrypt_ucan_key(&certificate.private_key)?;
let mut ucan_guard = ucan_service.write().await;
ucan_guard.load_keys(signing_key, verifying_key);
```

### ❌ Problem: Missing Parameter in Function Call

```
error[E0061]: this function takes 8 arguments but 7 arguments were supplied
```

**Cause:** Forgot to add `&ucan_service` parameter after refactoring

**Solution:** Add `&ucan_service` (or `&peer_conn.ucan_service` in P2P context)

---

## Dependency Updates

Add `gurkha` to these Cargo.toml files:

```toml
# kunki/Cargo.toml
# tauri_handlers/Cargo.toml
# sthalam/src-tauri/Cargo.toml
# network/Cargo.toml

[dependencies]
gurkha = { path = "../gurkha" }  # Or "../../gurkha" depending on location
```

---

## Testing Considerations

1. **Login Flow:** Verify keys are loaded correctly after successful login
2. **Token Generation:** Test all token generation paths (one-time, folder, resource, etc.)
3. **P2P Handshake:** Ensure peer connections can exchange and validate UCANs
4. **Folder/Resource Creation:** Verify owner tokens are generated and stored correctly
5. **Folder/Resource Sharing:** Verify delegated tokens are created with proper permissions

---

## Future Improvements

1. **Key Lifecycle Management:**
   - Consider auto-clearing keys on logout
   - Add key rotation support
   - Implement key backup/recovery

2. **Error Handling:**
   - Add more specific error types for different UCAN validation failures
   - Improve error messages for debugging

3. **Performance:**
   - Consider caching frequently used tokens
   - Profile RwLock contention in high-concurrency scenarios

4. **Security:**
   - Audit all UCAN delegation chains
   - Add token expiration enforcement
   - Implement token revocation mechanism

---

## Related Documentation

- See `UCAN_AUTHORIZATION.md` for UCAN structure and validation details
- See `SERVICE_LAYER.md` for service architecture patterns
- See `NETWORK_LAYER.md` for P2P integration details
- See `SYNC_PROTOCOL.md` for folder/resource synchronization flow

---

## Migration Verification

✅ All crates compile successfully:
```bash
cargo check
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.06s
```

✅ No threading errors
✅ All function signatures updated consistently
✅ All call sites updated with `&ucan_service` parameter

---

**Note:** This document should be integrated into the main documentation and then removed. Key sections to merge:
- Add "UcanService Management" section to SERVICE_LAYER.md
- Update "Thread Safety" section in NETWORK_LAYER.md
- Add "Key Loading Pattern" to UCAN_AUTHORIZATION.md
