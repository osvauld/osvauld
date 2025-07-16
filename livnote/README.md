# LivNote

**Real-time collaborative document editor powered by Osvauld**

LivNote is a privacy-first collaborative document editor that works entirely peer-to-peer. Built on the Osvauld framework, it enables real-time collaboration without servers, ensuring your documents remain private and under your control.

## ✨ Features

- **📝 Real-time Collaborative Editing**: Multiple users can edit documents simultaneously with instant synchronization
- **📋 Markdown Support**: Write and edit documents using Markdown syntax
- **💬 Comments & Annotations**: Add comments and feedback directly within documents
- **🔒 End-to-End Encrypted**: All document content is encrypted in transit and at rest
- **🔐 Encrypted at Rest**: Local documents are stored encrypted on your device
- **🌐 Offline-First**: Work seamlessly without internet connectivity, sync when available
- **🚫 No Servers Required**: Direct peer-to-peer communication - no data leaves your control
- **⚡ Rich Text Editing**: Powered by ProseMirror for a smooth, responsive editing experience
- **🔄 Conflict-Free Synchronization**: Uses Yjs CRDT for seamless collaborative editing without conflicts

## 🛠️ Technology Stack

- **Editor**: [ProseMirror](https://prosemirror.net/) - Rich text editor
- **CRDT**: [Yjs](https://github.com/yjs/yjs) - Conflict-free replicated data types
- **Framework**: [Osvauld](https://github.com/osvauld/osvauld) - P2P application framework
- **Frontend**: Svelte with TypeScript
- **Backend**: Rust with Tauri

## 🚀 Getting Started

### Prerequisites

- **Node.js** (version 18 or higher)
- **pnpm** package manager
- **Rust** (latest stable version)
- **Tauri CLI** (`cargo install tauri-cli`)

### Development Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/osvauld/osvauld.git
   cd osvauld
   ```

   > LivNote is included as part of the main Osvauld repository

2. **Install frontend dependencies**
   ```bash
   cd livnote/frontend/desktop
   pnpm install
   ```

3. **Run the development server**
   ```bash
   cd ../../src-tauri
   cargo tauri dev
   ```

### Building for Production

```bash
cd livnote/src-tauri
cargo tauri build
```

The built application will be available in the `target/release/bundle` directory.

## 🌟 How It Works

LivNote leverages the Osvauld framework to create a truly decentralized collaborative editing experience:

1. **Direct P2P Connections**: Documents are shared directly between devices without intermediary servers
2. **Cryptographic Security**: All data is encrypted using self-sovereign identity principles
3. **Conflict Resolution**: Yjs CRDT ensures all collaborators see consistent document state
4. **Offline Resilience**: Work continues seamlessly even when disconnected

## 🎯 Use Cases

- **Team Collaboration**: Work together on documents without relying on cloud services
- **Privacy-Sensitive Writing**: Keep confidential documents completely private
- **Markdown Documentation**: Create technical documentation with Markdown support
- **Offline Documentation**: Create and edit documents in environments with limited connectivity
- **Decentralized Note-Taking**: Build a personal knowledge base that you fully control

## 📖 About Osvauld

LivNote is built on the [Osvauld framework](https://github.com/osvauld/osvauld), which provides the P2P networking, encryption, and synchronization capabilities that make server-free collaboration possible. Osvauld handles the complex infrastructure so applications like LivNote can focus on user experience.

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](../CONTRIBUTING.md) for details on how to get started.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.

## 🔗 Links

- **Main Repository**: [Osvauld Framework](https://github.com/osvauld/osvauld)
- **Website**: [osvauld.com](https://osvauld.com)
- **Discord**: [Join our community](https://discord.gg/BVQtV6gS2c)

---

*Built with ❤️ using the Osvauld framework*
