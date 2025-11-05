# Sthalam

**Infrastructure for sovereign individuals to publish and share content on their own terms**

Sthalam is a decentralized website and content publishing platform built on the Osvauld protocol. It enables individuals to create, publish, and share websites, micro-blogs, newsletters, and interactive content through their own sovereign nodes without relying on centralized platforms.

## Overview

Sthalam empowers creators to maintain full control over their digital presence. Publish websites with forms and comment threads, share micro-blog posts, distribute newsletters, and let subscribers get real-time updates—all while maintaining sovereignty over your data and distribution.

When viewers connect to your content, they can pull updates for new posts, websites, and newsletters you publish by actively polling the node. Your sovereign node acts as a passive content hub that viewers poll for updates. No platforms, no intermediaries, just direct sovereign-to-sovereign connections.

---

## Key Features

### Content Publishing
- **HUML-based Website Builder**: Create websites using Human Markup Language (HUML), a declarative YAML-like syntax for building interactive web experiences
- **Multi-page Websites**: Design complex navigation flows with branching logic and screen transitions
- **Micro-blogs with Comments**: Twitter-like posts with multiple independent comment threads where viewers can see and respond to each other's comments in organized discussions
- **Newsletters & Posts**: Distribute content directly to connected subscribers
- **Rich Content Blocks**: Text, headings, images, markdown, HTML, modals, and custom styling with CSS

### Interactive Features
- **Forms & Submissions**: Multiple independent forms per document with field metadata for parsing. Collect user input with validation (text, textarea, email, checkbox, select fields). Each form identified by `form_id` with custom field metadata
- **Real-time Comment Threads**: Multiple independent thread blocks per document with CRDT-based synchronization where multiple viewers can see and respond to each other's comments in organized topic-specific discussions
- **Page Navigation**: Create interactive flows with nav buttons and branching logic
- **Live Updates**: Instant local-first rendering, then background polling for incremental CRDT updates

### Sovereign Infrastructure
- **Publish to Sovereign Nodes**: Deploy content to your own always-on nodes (works on Raspberry Pi)
- **Link-based Sharing**: Generate connection strings with UCAN token-based access control
- **Subscription Model**: When someone connects, they can poll node to receive updates for all your new content
- **Multi-Layer Encryption**: All data encrypted at rest (AES-256-GCM) and in transit (QUIC/TLS). AES keys re-encrypted for node and viewers using PGP
- **Offline-First Architecture**: Create and edit content without internet, sync when available
- **Multi-device Support**: Seamless synchronization across all your devices

---

## Technology Stack

### Frontend
- **Svelte 5** with TypeScript + Vite
- **Yjs CRDT** for conflict-free collaborative editing
- **HUML Parser** for declarative website design
- **Tauri API** for desktop integration

### Backend
- **Rust** with **Tauri 2** (native desktop bridge)
- **Iroh** for P2P networking (QUIC protocol)
- **Sequoia OpenPGP**, **AES-GCM**, **Ed25519** for encryption
- **UCAN tokens** for capability-based authorization
- **SQLite** for local encrypted storage

### Core Dependencies
- **network** - P2P communication layer
- **crypto_utils** - Cryptographic operations
- **services** - Business logic and orchestration
- **persistance** - Database layer

---

## Architecture

Sthalam is built on a three-layer architecture that enables secure, decentralized content publishing:

### 1. Services Layer
Business logic coordinating all application features:
- **Auth Service**: User registration, certificate generation, device management
- **Folder Service**: Content organization with UCAN-based access control
- **Resource Service**: Content lifecycle management (websites, posts, newsletters)
- **Sync Service**: Manifest-based synchronization across devices and peers
- **Node Service**: Sovereign node operations and viewer preparation

### 2. Network Layer
P2P communication infrastructure via Iroh with pull-based architecture:
- **P2PService**: Manages connections, handshakes, and message routing
- **PeerConnection**: Maintains state for each connected peer
- **ConnectionManager**: Tracks active connections and pending operations
- **Sync Protocols**: Device, user, folder, and resource synchronization via polling
- **Website Handler**: Responds to viewer requests for content (node never initiates)

### 3. Crypto Layer
Security and authorization infrastructure:
- **Key Management**: PGP keypairs, Ed25519 signing keys, AES-256-GCM encryption
- **Digital Signatures**: PGP-based message signing and verification
- **UCAN Operations**: Token generation, validation, delegation chains
- **Data Encryption**:
  - AES-256-GCM encryption for all content at rest
  - AES key re-encryption: decrypt with publisher's PGP, re-encrypt with node's PGP
  - AES key re-encryption for viewers: node re-encrypts with each viewer's PGP public key (in later releases, will generate new AES key per viewer, encrypt content with new key, and encrypt new key with viewer's PGP)
  - QUIC connection provides TLS encryption in transit
  - Data encrypted at rest on publisher, node, and viewer devices

---

## UCAN Token Architecture

Sthalam uses **UCAN (User Controlled Authorization Networks)** tokens for fine-grained, capability-based access control. The verbosity of UCAN tokens enables intelligent sync decision-making.

### Token Verbosity for Sync Intelligence

Each UCAN token contains explicit capability strings that describe exactly what the holder can access:

```
sthalam:resource:abc123:blocksuite_doc - crud/read
sthalam:resource:abc123:thread_comments_doc - crud/write
sthalam:resource:abc123:form_submissions_doc - crud/append
```

This verbosity enables viewers to automatically determine:
- **What to sync**: Parse capability strings to extract resource/folder IDs
- **What to merge**: Write permissions enable bidirectional synchronization
- **What to ignore**: Read-only permissions indicate one-way data flow
- **Document-specific behavior**: Different permissions for content, comments, and submissions

### 3-Document Architecture

Resources use a three-document model with distinct permissions:

1. **blocksuite_doc** (Main Content)
   - **Viewer permission**: `crud/read` (read-only)
   - **Behavior**: **Local-first + state vector sync**. Viewer always renders from local copy first (instant load). Publisher writes to local first, syncs via state vectors. Viewer sends state vector to node, receives only missing CRDT diffs
   - **Use case**: Website structure, blog posts, newsletter content

2. **thread_comments_doc** (Collaborative Comments)
   - **Viewer permission**: `crud/write` (bidirectional)
   - **Behavior**: **Local-first + state vector multi-thread**. Multiple independent threads per document with `thread_id`. All participants write to local Yjs doc first (instant UI), then exchange state vectors with node. Both sides send state vectors and respond with only missing CRDT diffs. Filter by `thread_id` for display
   - **Use case**: Real-time comment threads where multiple viewers collaborate across multiple organized discussions

3. **form_submissions_doc** (Submissions)
   - **Viewer permission**: `crud/append` (append-only)
   - **Behavior**: **Local-first + state vector multi-form**. Multiple forms per document with `form_id` and field metadata. Publisher renders from local first. Viewer pushes incremental diffs with `form_id`. Publisher exchanges state vectors with node to receive only missing submission diffs. Filters by `form_id` for display
   - **Use case**: Form responses across multiple forms (contact, feedback, survey), feedback collection with field metadata for parsing

### Token Validation & Extraction

The system validates UCAN tokens throughout the sync flow:

1. **Folder-level tokens** (`network/src/p2p/website_handler.rs:530-555`)
   - Extract folder_id from capabilities: `extract_folder_id_from_ucan()`
   - Verify token matches requested folder

2. **Resource-level tokens** (`network/src/p2p/website_handler.rs:686-710`)
   - Extract resource_id from capabilities: `extract_resource_id_from_ucan()`
   - Support multi-format: `domain:resource:id` and `domain:resource:id:doc_type`

3. **Permission generation** (`services/src/node_service.rs:217-234`)
   - Create resource-specific tokens for each viewer
   - Define granular permissions per document type

This capability-based model eliminates the need for centralized permission servers—the tokens themselves encode all authorization logic, enabling fully decentralized access control.

**Reference**: `crypto_utils/src/ucan_utils.rs`, `network/src/p2p/website_handler.rs`, `services/src/node_service.rs`

### Local-First + State Vector Sync Architecture

Sthalam uses a **local-first rendering with state vector-based CRDT sync** model for optimal performance:

**Initial Sync** (First Time Only):
- New resources send full document for all 3 docs
- Stored locally in encrypted database
- Viewer has complete copy

**Subsequent Access** (Always):
1. **Instant Load**: Website renders from local copy immediately (no network wait)
2. **Write Local First**: All edits/comments written to local Yjs doc first (instant UI update)
3. **State Vector Exchange**: Send state vector to node in background
4. **Receive Missing Diffs**: Node responds with only CRDT diffs you don't have
5. **Send Own Updates**: Send CRDT diffs node doesn't have (based on node's state vector)
6. **Apply Diffs**: Merge missing updates into local copy
7. **Seamless Experience**: User sees instant changes, sync happens in background

**State Vector Protocol**:
- **Bidirectional** (for crud/write): Both sides exchange state vectors and respond with diffs
- **Unidirectional** (for crud/read): Viewer receives diffs via state vector exchange
- **Only missing data**: State vectors determine exactly what diffs each side needs
- **Never full doc**: Only state vectors and CRDT diffs transferred after first sync

**Applies to All 3 Documents**:
- **blocksuite_doc**: Content updates via state vector sync (viewer receives diffs)
- **thread_comments_doc**: Comments via bidirectional state vector exchange (write local first, sync in background)
- **form_submissions_doc**: Submissions via incremental append (publisher receives via state vector sync)

**Benefits**:
- Instant UI updates (write to local first, no network wait)
- Efficient bandwidth usage (only missing diffs, not full docs)
- Tiny state vectors (just version info, not data)
- Offline-capable (works from local copy)
- Smooth user experience (all changes local first, sync in background)
- Scales efficiently (state vectors enable precise diff calculation)

---

## Getting Started

### Creating Your First Website with HUML

Sthalam uses HUML (Human Markup Language) to define websites. The easiest way to get started is to use any LLM to generate HUML templates:

1. **Upload the HUML Template Guide** to your preferred LLM (ChatGPT, Claude, etc.)
   - File: `/docs/specs/HUML_TEMPLATE_GUIDE_ACCURATE.md` (✅ validated against actual implementation)

2. **Describe what you want to build**
   ```
   Example: "I want to create a personal portfolio website with:
   - A landing page with my bio
   - A projects page with a form for contact
   - A blog section with comment threads"
   ```

3. **Get HUML code from the LLM**
   - The LLM will generate HUML syntax based on the template guide

4. **Import into Sthalam**
   - Open Sthalam
   - Create a new website resource
   - Paste the HUML code into the editor
   - Preview and edit the content as needed

5. **Publish**
   - Click publish to make it available on your sovereign node
   - Generate a connection string to share with viewers

### Development Setup

For contributors and developers:

**Prerequisites:**
- **Node.js** (version 18 or higher)
- **pnpm** package manager
- **Rust** (latest stable version)
- **Tauri CLI**: `cargo install tauri-cli`

**Setup:**
```bash
# Clone the repository
git clone https://github.com/osvauld/osvauld.git
cd osvauld/sthalam

# Install frontend dependencies
cd frontend/desktop
pnpm install

# Run development server
cd ../../src-tauri
cargo tauri dev
```

**Build for Production:**
```bash
cd sthalam/src-tauri
cargo tauri build
```

The built application will be in `target/release/bundle`.

---

## How It Works

### Publishing Content

1. **Create Content**: Use the visual builder (Canvas) to design websites with HUML blocks, or write micro-blog posts and newsletters
2. **Design**: Add blocks (text, forms, comment threads, navigation), style with CSS, preview in real-time
3. **Publish**: File the content on your sovereign node's P2P network
4. **Share**: Generate connection strings with UCAN tokens defining viewer permissions

### Viewer Access

1. **Receive Connection String**: Publisher shares base64-encoded token (via email, chat, etc.)
2. **Connect**: Viewer pastes connection string, establishing P2P connection with the sovereign node
3. **Sync**: Viewer pulls initial folder and all resources with appropriate permissions
4. **Interact**: View content, submit forms, comment on threads based on UCAN capabilities
5. **Subscribe**: Actively poll and pull updates when publisher adds new content to the node

### Real-time Updates (Local-First + State Vector Sync)

Viewers experience instant loads with background state vector sync:

**Rendering Flow**:
1. **Instant Load**: Always render from local copy first (no waiting)
2. **Write Local First**: All edits/comments written to local Yjs doc first (instant UI)
3. **Background State Vector Exchange**: Send state vector to node while viewing
4. **Receive Missing Diffs**: Node responds with only CRDT diffs you don't have
5. **Send Own Updates**: Send CRDT diffs node doesn't have
6. **Apply Updates**: Seamlessly merge missing diffs into local doc

**Update Types** (when viewer polls):
- **New websites**: Viewer detects new resources via folder manifest comparison, pulls full resource first time, then state vector sync
- **New posts**: Instant load from local, viewer exchanges state vectors with node for updates
- **Newsletters**: Instant load from local, viewer requests diffs via state vector sync
- **Comments**: All participants write to local first (instant UI), exchange state vectors with node. Each receives only missing CRDT diffs, filters by `thread_id` for display, enabling multi-viewer discussions
- **Form submissions**: Viewers push diffs with `form_id` and field metadata. Publisher renders from local first, exchanges state vectors for submissions

---

## Use Cases

- **Personal Websites**: Publish without hosting fees or platform lock-in
- **Micro-blogging**: Twitter-like posts with sovereign ownership
- **Newsletters**: Direct distribution to subscribers without intermediaries
- **Community Discussions**: Multiple comment threads per document where viewers can discuss with each other in organized topic-specific conversations, without platform moderation
- **Surveys & Forms**: Collect feedback with full data sovereignty
- **Educational Materials**: Share content with granular access control and section-specific discussion threads
- **Decentralized Publishing**: Build audiences without algorithmic gatekeepers

---

## Documentation

- **📚 All Documentation**: See `/docs/` folder for organized documentation
- **HUML Template Guide**: See `/docs/specs/HUML_TEMPLATE_GUIDE_ACCURATE.md` for complete syntax reference (✅ validated against actual code)
- **Implementation Status**: See `/docs/VALIDATION_REPORT.md` for what's actually working
- **Architecture**: See `/docs/architecture/` for system design documents
- **Integration Guides**: See `/docs/guides/` for WASM integration, Loro architecture, and more
- **Osvauld Documentation**: [docs.osvauld.com](https://docs.osvauld.com)
- **Main Repository**: [github.com/osvauld/osvauld](https://github.com/osvauld/osvauld)

---

## Community

Join our community to shape the future of sovereign publishing:

- **Telegram**: [t.me/osvauld](https://t.me/osvauld)
- **Website**: [osvauld.com](https://osvauld.com)

---

## Contributing

We welcome contributions! Whether you're a developer, designer, writer, or enthusiast, there are many ways to contribute to sovereign infrastructure. Join our Telegram and let us know how you'd like to help.

---

## Support

We're never more than an email away:
- **GitHub Issues**: [github.com/osvauld/osvauld/issues](https://github.com/osvauld/osvauld/issues)
- **Email**: [abe@osvauld.com](mailto:abe@osvauld.com)

---

## Security

If you believe you have found a security vulnerability, please responsibly disclose it by emailing [abe@osvauld.com](mailto:abe@osvauld.com) instead of opening a public issue. We will investigate all legitimate reports.

---

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.

---

*Built for the sovereign individual*
