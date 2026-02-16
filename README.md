
<p align="center">
<a href="https://osvauld.com">
  <img src="https://raw.githubusercontent.com/osvauld/osvauld/dev/.github/assets/logo.png" alt="Osvauld Logo" >
</a>
</p>
<p align="center"><strong>Open-source platform for offline-first, E2E encrypted, P2P applications</strong></p>

---

## What is osvauld?

Osvauld is a platform for building truly decentralized applications. Write your app logic in **Lua**, design your UI in **Slint**, and osvauld handles identity, encryption, P2P sync, and offline-first storage automatically.

```
Your App (Lua + Slint/Raylib)
        │
        ▼
┌─────────────────────────────────────┐
│         osvauld Runtime             │
│  ┌─────────┐  ┌─────────────────┐   │
│  │ Identity │  │ Loro CRDT Sync │   │
│  │ (Herald) │  │ (Scribe/Butler)│   │
│  └─────────┘  └─────────────────┘   │
│  ┌─────────────────────────────────┐│
│  │  P2P Network (Courier + QUIC)  ││
│  └─────────────────────────────────┘│
└─────────────────────────────────────┘
```

**No servers required.** Apps sync directly between devices using encrypted P2P connections.

## Core Capabilities

| Capability | Description |
|------------|-------------|
| **Self-Sovereign Identity** | Ed25519 keypairs for signing, X25519 for encryption. Your identity lives on your devices. |
| **UCAN-Based Permits** | Fine-grained, delegatable authorization. Share access without a central authority. |
| **QUIC Transport** | Fast, encrypted connections via iroh. NAT traversal and relay built-in. |
| **Loro CRDT Sync** | Conflict-free data sync. Works offline, merges automatically when peers reconnect. |
| **Dynamic Apps** | Hot-load Lua apps with Slint or Raylib UI. No recompilation needed. |
| **Sovereign Nodes** | Optional always-on nodes (Raspberry Pi, VPS) for relay and offline sync. |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Application Layer                                               │
│  Lua apps, Slint/Raylib UI, business logic                      │
├─────────────────────────────────────────────────────────────────┤
│  Scribe                                                          │
│  CRDT document actors (Loro), per-document sync state            │
├─────────────────────────────────────────────────────────────────┤
│  Butler                                                          │
│  Storage, services, data layer                                   │
├─────────────────────────────────────────────────────────────────┤
│  Courier                                                         │
│  P2P orchestration, handshakes, sync protocol                   │
│  Actor model: Coordinator → PeerActors                          │
├─────────────────────────────────────────────────────────────────┤
│  Gurkha                                                          │
│  Permit parsing, capability extraction, authorization decisions │
├─────────────────────────────────────────────────────────────────┤
│  Transport                                                       │
│  QUIC connections, streams, datagrams, blob transfer            │
│  "Dumb byte pipe" - no protocol knowledge                       │
├─────────────────────────────────────────────────────────────────┤
│  Herald                                                          │
│  Identity, encryption, signing primitives                       │
├─────────────────────────────────────────────────────────────────┤
│  iroh                                                            │
│  QUIC, relay, NAT traversal, blob protocol                      │
└─────────────────────────────────────────────────────────────────┘
```

### Crate Responsibilities

| Crate | Purpose | Key Dependencies |
|-------|---------|------------------|
| `herald` | Identity, Ed25519/X25519 crypto, signing | - |
| `gurkha` | UCAN permit parsing and validation | herald |
| `transport` | QUIC connections, streams, blobs | iroh |
| `courier` | P2P orchestration, sync protocol, actors | transport, butler, gurkha |
| `butler` | Storage, services API, Loro CRDT scribes | herald, gurkha |
| `app_runtime` | Lua VM, Slint bindings, app lifecycle | butler |
| `slint_shell` | Desktop shell for running apps | app_runtime |
| `kunki` | Node runtime (relay, storage, derivations) | courier, butler |

## Sample Apps

| App | Description | Location |
|-----|-------------|----------|
| **My Shop** | E-commerce with owner/customer roles, order management | `sample_apps/my-shop/` |
| **My Booking** | Service booking with provider/customer roles | `sample_apps/my-booking/` |
| **Canvas** | Collaborative whiteboard with shapes, connectors, live cursors | `sample_apps/canvas-app/` |
| **Photo Gallery** | Shared photo albums with blob sync | `sample_apps/photo-gallery/` |
| **Demos** | Sthalam Guide, Snake game, Math sim, Group chat | `sample_apps/osvauld-demos/` |

## Getting Started

### Quick Start

```bash
git clone https://github.com/osvauld/osvauld.git
cd osvauld
cargo run -p slint_shell
```

This launches Sthalam with the **Sthalam Guide** app - an interactive introduction to the Xtended Web. The guide covers:
- What is the Xtended Web
- How Sthalam and Osvauld work
- Architecture overview
- Building your own apps

### Prerequisites

- Rust 1.75+ (with nightly for some features)
- Linux, macOS, or Windows

### Build for Release

```bash
cargo build -p slint_shell --release
./target/release/slint_shell
```

### Run Tests

```bash
# Unit tests
cargo test

# Integration tests (P2P sync scenarios)
cargo test -p integration_tests
```

### Project Structure

```
osvauld/
├── herald/           # Identity and crypto primitives
├── gurkha/           # Permit parsing and validation
├── transport/        # QUIC transport layer
├── courier/          # P2P orchestration
├── butler/           # Storage and services
├── app_runtime/      # Lua VM and Slint bindings
├── slint_shell/      # Desktop application shell
├── kunki/            # Node runtime
├── sample_apps/      # Example applications
├── integration_tests/# P2P sync tests
└── docs/             # Technical documentation
```

## Documentation

- **[Protocol Specification](docs/PROTOCOL.md)** - P2P protocol, message types, sync flows
- **[Architecture](docs/ARCHITECTURE.md)** - System design and crate boundaries
- **[Setup Guide](docs/SETUP.md)** - Build instructions and IDE setup
- **[Data Model](docs/DATA_MODEL.md)** - Space/Page/Layer hierarchy
- **[Permits](docs/PERMITS.md)** - UCAN-based authorization system
- **[App Development](docs/APP_DEVELOPMENT.md)** - Building Lua + Slint apps
- **[Code Policies](CLAUDE.md)** - Logging, comments, and code style

## Contributing

We welcome contributions! See [CLAUDE.md](CLAUDE.md) for code policies and style guidelines.

### Good First Issues

- **Documentation**: Improve examples, add diagrams
- **Sample Apps**: Build new demo apps showcasing features
- **UI Polish**: Improve Slint components and themes

### Intermediate

- **Platform Support**: Mobile (Android/iOS) improvements
- **Performance**: Optimize sync for large datasets
- **Testing**: Expand integration test coverage

### Advanced

- **Protocol**: Implement new sync strategies
- **Crypto**: Additional encryption schemes
- **Network**: Improve NAT traversal reliability

## Community

<p align="center">
  <a href="https://t.me/osvauld">
     <img align="center" src="https://raw.githubusercontent.com/osvauld/osvauld/dev/.github/assets/telegram.png" width="50px"  alt="Telegram community link" >
  </a>
</p>
<p align="center">
Connect with us on Telegram to discuss features and shape the roadmap
</p>

## Support

- **Issues**: [GitHub Issues](https://github.com/osvauld/osvauld/issues)
- **Email**: [osvauld@gmail.com](mailto:osvauld@gmail.com)
- **Security**: For security vulnerabilities, please email us directly instead of opening a public issue.

## Acknowledgments

<p align="center">
  <img align="center" src="https://raw.githubusercontent.com/osvauld/osvauld/dev/.github/assets/community-badge.webp" alt="KSUM logo" >
</p>

This project has received funding from [FLOSS fund](https://floss.fund) and [Innovation grant](https://startupmission.kerala.gov.in/schemes/innovation-grant).

## License

See [LICENSE.txt](LICENSE.txt) for details.
