# Setup Guide

Build instructions, prerequisites, and IDE setup for osvauld development.

## Prerequisites

### Required

- **Rust 1.75+** with nightly toolchain
  ```bash
  rustup install stable
  rustup install nightly
  rustup default stable
  ```

- **Git** for cloning and contributing

### Platform-Specific

#### Linux

```bash
# Debian/Ubuntu
sudo apt install build-essential pkg-config libssl-dev libfontconfig1-dev

# Fedora
sudo dnf install gcc pkg-config openssl-devel fontconfig-devel

# Arch
sudo pacman -S base-devel pkg-config openssl fontconfig
```

#### macOS

```bash
# Xcode command line tools
xcode-select --install

# Homebrew dependencies (optional, usually not needed)
brew install pkg-config openssl
```

#### Windows

- Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- Select "Desktop development with C++"

## Building

### Clone Repository

```bash
git clone https://github.com/osvauld/osvauld.git
cd osvauld
```

### Build All Crates

```bash
# Debug build (faster compilation)
cargo build

# Release build (optimized)
cargo build --release
```

### Build Specific Crates

```bash
# Desktop shell
cargo build -p sthalam

# Node runtime
cargo build -p kunki

# Just the core crates
cargo build -p herald -p gurkha -p transport -p courier -p butler
```

## Running

### Desktop Shell

```bash
# Debug mode
cargo run -p sthalam

# Release mode
cargo run -p sthalam --release
```

### Node (Kunki)

```bash
# Start a local node
cargo run -p kunki -- --config node-config.toml
```

### With Logging

```bash
# See all logs
RUST_LOG=debug cargo run -p sthalam

# Filter by crate
RUST_LOG=courier=debug,transport=trace cargo run -p sthalam

# Recommended for development
RUST_LOG=info,courier=debug cargo run -p sthalam
```

## Testing

### Unit Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p courier
cargo test -p butler

# With output
cargo test -- --nocapture
```

### Integration Tests

```bash
# P2P sync tests (requires multiple instances)
cargo test -p integration_tests

# Specific test
cargo test -p integration_tests test_owner_node_sync
```

### Test Coverage

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Generate coverage
cargo llvm-cov --html
open target/llvm-cov/html/index.html
```

## IDE Setup

### VS Code

1. Install extensions:
   - [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
   - [Slint](https://marketplace.visualstudio.com/items?itemName=Slint.slint)
   - [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml)

2. Recommended settings (`.vscode/settings.json`):
   ```json
   {
     "rust-analyzer.cargo.features": "all",
     "rust-analyzer.check.command": "clippy",
     "rust-analyzer.diagnostics.experimental.enable": true,
     "editor.formatOnSave": true,
     "[rust]": {
       "editor.defaultFormatter": "rust-lang.rust-analyzer"
     }
   }
   ```

### RustRover / IntelliJ

1. Open project folder
2. Wait for indexing to complete
3. Enable Clippy: Settings → Languages & Frameworks → Rust → Use Clippy

### Neovim

With [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig):

```lua
require('lspconfig').rust_analyzer.setup {
  settings = {
    ['rust-analyzer'] = {
      cargo = { features = 'all' },
      checkOnSave = { command = 'clippy' },
    }
  }
}
```

## Project Structure

```
osvauld/
├── herald/               # Identity and crypto
│   ├── src/
│   │   ├── lib.rs
│   │   ├── identity.rs
│   │   └── crypto.rs
│   └── Cargo.toml
│
├── gurkha/               # Permit validation
│   ├── src/
│   │   ├── lib.rs
│   │   ├── permit.rs
│   │   └── capabilities.rs
│   └── Cargo.toml
│
├── transport/            # QUIC networking
│   ├── src/
│   │   ├── lib.rs
│   │   ├── connection.rs
│   │   └── stream.rs
│   └── Cargo.toml
│
├── courier/              # P2P orchestration
│   ├── src/
│   │   ├── lib.rs
│   │   ├── coordinator.rs
│   │   ├── peer_actor.rs
│   │   └── sync.rs
│   └── Cargo.toml
│
├── butler/               # Storage services
│   ├── src/
│   │   ├── lib.rs
│   │   ├── services/
│   │   │   ├── page_service.rs
│   │   │   └── blob_service.rs
│   │   └── scribe.rs
│   └── Cargo.toml
│
├── lua_runtime/          # Lua scripting runtime
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
│
├── scribe/               # Document sync engine
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
│
├── domains/              # Domain logic
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
│
├── sthalam/              # App shell library
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
│
├── sthalam_shell/        # Desktop app entry point
│   ├── src/
│   │   └── main.rs
│   └── Cargo.toml
│
├── renderer_slint/       # Slint renderer
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
│
├── renderer_raylib/      # Raylib renderer
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
│
├── kunki/                # Node runtime
│   ├── src/
│   │   └── main.rs
│   └── Cargo.toml
│
├── sample_apps/          # Example apps
│   ├── my-shop/
│   ├── my-booking/
│   ├── canvas-app/
│   └── photo-gallery/
│
├── integration_tests/    # P2P tests
├── docs/                 # Documentation
├── Cargo.toml            # Workspace manifest
└── Cargo.lock
```

## Common Issues

### Build Fails with OpenSSL Error

```bash
# Linux
sudo apt install libssl-dev

# macOS
brew install openssl
export OPENSSL_DIR=$(brew --prefix openssl)
```

### Slint Compilation Errors

Ensure you have the Slint VS Code extension for syntax checking. Common issues:
- Missing semicolons after property declarations
- Using `=` instead of `:` for property binding

### Cargo Watch

For auto-rebuild during development:

```bash
cargo install cargo-watch
cargo watch -x 'build -p sthalam'
```

### Clean Build

If you encounter strange errors:

```bash
cargo clean
cargo build
```

## Development Workflow

### Making Changes

1. Create a feature branch:
   ```bash
   git checkout -b feature/my-feature
   ```

2. Make changes, run tests:
   ```bash
   cargo test
   cargo clippy
   ```

3. Format code:
   ```bash
   cargo fmt
   ```

4. Commit with descriptive message:
   ```bash
   git commit -m "feat: add new capability to permits"
   ```

### Running Sample Apps

1. Build the shell:
   ```bash
   cargo build -p sthalam
   ```

2. Launch and select an app from `sample_apps/`

3. For multi-peer testing, run multiple instances:
   ```bash
   # Terminal 1 - Owner
   cargo run -p sthalam

   # Terminal 2 - Node
   cargo run -p kunki

   # Terminal 3 - Viewer
   cargo run -p sthalam
   ```

## Debugging

### Logging

Use `tracing` macros:

```rust
use tracing::{trace, debug, info, warn, error};

trace!("wire-level detail");
debug!("implementation detail");
info!("business event");
warn!("recoverable issue");
error!("failure requiring attention");
```

### Inspecting Loro Documents

```rust
// In test or debug code
let doc = scribe.get_document();
println!("{:?}", doc.export_snapshot());
```

### Network Debugging

```bash
# See all network traffic
RUST_LOG=transport=trace cargo run -p sthalam
```
