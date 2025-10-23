# Sthalam

**Infrastructure for sovereign individuals to publish and share content on their own terms**

Sthalam is a decentralized website and content publishing platform built on the Osvauld protocol. It enables individuals to create, publish, and share websites, micro-blogs, newsletters, and interactive content through their own sovereign nodes without relying on centralized platforms.

## Overview

Sthalam empowers creators to maintain full control over their digital presence. Publish websites with forms and comment threads, share micro-blog posts, distribute newsletters, and let subscribers get real-time updates—all while maintaining sovereignty over your data and distribution.

When viewers connect to your content, they receive automatic updates for new posts, websites, and newsletters you publish. No platforms, no intermediaries, just direct sovereign-to-sovereign connections.

---

## Key Features

### Content Publishing
- **HUML-based Website Builder**: Create websites using Human Markup Language (HUML), a declarative YAML-like syntax for building interactive web experiences
- **Multi-page Websites**: Design complex navigation flows with branching logic and screen transitions
- **Micro-blogs with Comments**: Twitter-like posts with real-time collaborative comment threads
- **Newsletters & Posts**: Distribute content directly to connected subscribers
- **Rich Content Blocks**: Text, headings, images, markdown, HTML, modals, and custom styling with CSS

### Interactive Features
- **Forms & Submissions**: Collect user input with validation (text, textarea, email, checkbox, select fields)
- **Real-time Comment Threads**: Collaborative discussions with CRDT-based synchronization
- **Page Navigation**: Create interactive flows with nav buttons and branching logic
- **Live Updates**: Subscribers automatically receive notifications when you publish new content

### Sovereign Infrastructure
- **Publish to Sovereign Nodes**: Deploy content to your own always-on nodes (works on Raspberry Pi)
- **Link-based Sharing**: Generate connection strings with UCAN token-based access control
- **Subscription Model**: When someone connects, they automatically receive updates for all your new content
- **End-to-End Encryption**: All data encrypted in transit and at rest
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
P2P communication infrastructure via Iroh:
- **P2PService**: Manages connections, handshakes, and message routing
- **PeerConnection**: Maintains state for each connected peer
- **ConnectionManager**: Tracks active connections and pending operations
- **Sync Protocols**: Device, user, folder, and resource synchronization
- **Website Handler**: Manages viewer connections and content distribution

### 3. Crypto Layer
Security and authorization infrastructure:
- **Key Management**: PGP keypairs, Ed25519 signing keys, AES-256-GCM encryption
- **Digital Signatures**: PGP-based message signing and verification
- **UCAN Operations**: Token generation, validation, delegation chains
- **Data Encryption**: Hybrid encryption for content and multi-recipient patterns

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
   - **Behavior**: One-way sync from publisher to viewer
   - **Use case**: Website structure, blog posts, newsletter content

2. **thread_comments_doc** (Collaborative Comments)
   - **Viewer permission**: `crud/write` (bidirectional)
   - **Behavior**: Both viewer and publisher sync changes
   - **Use case**: Real-time comment threads and discussions

3. **form_submissions_doc** (Submissions)
   - **Viewer permission**: `crud/append` (append-only)
   - **Behavior**: Viewer sends submissions, no updates back
   - **Use case**: Form responses, feedback collection

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

---

## Getting Started

### Creating Your First Website with HUML

Sthalam uses HUML (Human Markup Language) to define websites. The easiest way to get started is to use any LLM to generate HUML templates:

1. **Upload the HUML Template Guide** to your preferred LLM (ChatGPT, Claude, etc.)
   - File: `HUML_TEMPLATE_GUIDE.md` (in this repository)

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
2. **Connect**: Viewer pastes connection string, establishing P2P connection
3. **Sync**: Initial sync downloads folder and all resources with appropriate permissions
4. **Interact**: View content, submit forms, comment on threads based on UCAN capabilities
5. **Subscribe**: Automatically receive updates when publisher adds new content

### Real-time Updates

When connected:
- **New websites**: Viewer receives notification and can access immediately
- **New posts**: Micro-blog posts appear in subscriber feeds
- **Newsletters**: Direct delivery without email platforms
- **Comments**: Real-time synchronization via Yjs CRDT
- **Form submissions**: Sent to publisher for collection

---

## Use Cases

- **Personal Websites**: Publish without hosting fees or platform lock-in
- **Micro-blogging**: Twitter-like posts with sovereign ownership
- **Newsletters**: Direct distribution to subscribers without intermediaries
- **Community Discussions**: Comment threads without platform moderation
- **Surveys & Forms**: Collect feedback with full data sovereignty
- **Educational Materials**: Share content with granular access control
- **Decentralized Publishing**: Build audiences without algorithmic gatekeepers

---

## Documentation

- **HUML Template Guide**: See `HUML_TEMPLATE_GUIDE.md` for complete syntax reference
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
