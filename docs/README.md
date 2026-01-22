# Osvauld Documentation

Technical documentation for contributors and developers building on osvauld.

## Implementation Status

### Core Infrastructure

| Component | Status | Notes |
|-----------|--------|-------|
| **Herald** (Identity) | Complete | Ed25519 signing, X25519 encryption |
| **Gurkha** (Permits) | Complete | UCAN parsing, capability validation |
| **Transport** (QUIC) | Complete | Streams, datagrams, blob transfer |
| **Courier** (P2P) | Complete | Handshakes, sync protocol, actors |
| **Butler** (Storage) | Complete | Services API, Loro CRDT scribes |
| **App Runtime** | Complete | Lua VM, Slint bindings |

### P2P Protocol

| Feature | Status | Notes |
|---------|--------|-------|
| Connection handshake | Complete | Hello/Welcome/PermitGrant flow |
| Space/Page publishing | Complete | Owner → Node |
| Shareable links | Complete | aud:* wildcard permits |
| Viewer flow | Complete | SpaceRequest → SpaceData → SyncConsent |
| 3-step sync | Complete | SyncOffer → SyncAccept → SyncAck |
| Asset sync (blobs) | Complete | iroh-blobs protocol |
| Ephemeral datagrams | Complete | Cursors, typing indicators |
| Multi-viewer | In Progress | Concurrent viewer scenarios |

### Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| Linux | Complete | Full support |
| macOS | Complete | Full support |
| Windows | Complete | Full support |
| Android | In Progress | Tauri mobile |
| iOS | Planned | Tauri mobile |

## Quick Links

| Document | Description |
|----------|-------------|
| [PROTOCOL.md](PROTOCOL.md) | P2P protocol specification, message types, sync flows |
| [ARCHITECTURE.md](ARCHITECTURE.md) | System architecture, crate boundaries, data flow |
| [SETUP.md](SETUP.md) | Build instructions, prerequisites, IDE setup |
| [DATA_MODEL.md](DATA_MODEL.md) | Space/Page/Layer hierarchy, encryption model |
| [PERMITS.md](PERMITS.md) | UCAN-based authorization, permit types, delegation |
| [APP_DEVELOPMENT.md](APP_DEVELOPMENT.md) | Building Lua + Slint apps, API reference |

## Crate Reference

```
┌─────────────────────────────────────────────────────────────────┐
│                     Application Layer                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ slint_shell │  │   kunki     │  │  tauri_app  │             │
│  │  (Desktop)  │  │   (Node)    │  │  (Mobile)   │             │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │
│         │                │                │                     │
│         └────────────────┴────────────────┘                     │
│                          │                                      │
│                  ┌───────┴───────┐                              │
│                  │  app_runtime  │                              │
│                  │  (Lua + Slint)│                              │
│                  └───────┬───────┘                              │
├──────────────────────────┼──────────────────────────────────────┤
│                     Core Layer                                  │
│         ┌────────────────┼────────────────┐                     │
│         │                │                │                     │
│  ┌──────┴──────┐  ┌──────┴──────┐  ┌──────┴──────┐             │
│  │   courier   │  │   butler    │  │   gurkha    │             │
│  │  (P2P Sync) │  │  (Storage)  │  │  (Permits)  │             │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │
│         │                │                │                     │
│         └────────────────┴────────────────┘                     │
│                          │                                      │
│                  ┌───────┴───────┐                              │
│                  │   transport   │                              │
│                  │    (QUIC)     │                              │
│                  └───────┬───────┘                              │
├──────────────────────────┼──────────────────────────────────────┤
│                   Foundation Layer                              │
│                  ┌───────┴───────┐                              │
│                  │    herald     │                              │
│                  │  (Identity)   │                              │
│                  └───────────────┘                              │
└─────────────────────────────────────────────────────────────────┘
```

### Crate Details

| Crate | Purpose | Public API |
|-------|---------|------------|
| **herald** | Cryptographic identity | `Identity`, `KeyPair`, `encrypt()`, `sign()` |
| **gurkha** | Permit validation | `Permit`, `PermitValidator`, `Capabilities` |
| **transport** | QUIC networking | `Transport`, `Connection`, `Stream` |
| **courier** | P2P protocol | `Coordinator`, `PeerActor`, `SyncManager` |
| **butler** | Storage services | `Butler`, `Scribe`, `PageService` |
| **app_runtime** | App execution | `LuaRuntime`, `SlintBindings`, `AppContext` |

## Where to Start

### Building an App

1. Read [APP_DEVELOPMENT.md](APP_DEVELOPMENT.md) for the Lua + Slint app model
2. Look at `sample_apps/` for working examples
3. Start with a simple app (landing, snake-game) before complex ones

### Understanding the Protocol

1. Read [PROTOCOL.md](PROTOCOL.md) for message flows
2. See [PERMITS.md](PERMITS.md) for authorization model
3. Check [DATA_MODEL.md](DATA_MODEL.md) for storage hierarchy

### Contributing to Core

1. Read [ARCHITECTURE.md](ARCHITECTURE.md) for crate boundaries
2. Follow [CLAUDE.md](../CLAUDE.md) for code style
3. Start with the crate most relevant to your contribution

## Code Style

See [CLAUDE.md](../CLAUDE.md) for:
- Logging levels (trace/debug/info/warn/error)
- Comment style (domain terminology, inline comments)
- Code structure (max nesting, handler naming)
- Crate boundaries (what can depend on what)
