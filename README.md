<br />
<p align="center">
<a href="https://osvauld.com">
  <img src="https://www.osvauld.com/assets/logo.png" alt="Osvauld Logo" width="300" height="300" >
</a>
</p>
<h1 align="center"><b>Own Your Data, Leave No Footprint</b></h1>

**Osvauld** is a framework for building peer-to-peer applications that prioritize privacy, security, and user control. Built entirely in Rust, Osvauld provides everything you need to create modern, privacy-respecting applications that work without centralized servers or data collection.

## 🚀 Current Applications

### LivNote - Collaborative Document Editor
A real-time collaborative document editor that works entirely peer-to-peer. [Explore LivNote →](./livnote)

### Password Manager *(Coming Soon)*
A secure credential management system with team sharing capabilities.

## 🔒 Core Framework Features

- **Server-Free P2P Networking**: Direct device-to-device communication - no servers needed to host or maintain
- **End-to-End Encryption**: All data encrypted in transit and at rest following zero-knowledge architecture
- **Self-Sovereign Identity**: Digital signatures and certificate-based identity management
- **QUIC Protocol**: Fast, authenticated, and encrypted connections for reliable P2P communication
- **Offline-First Architecture**: Full functionality without internet connectivity, syncing when available
- **CRDT Integration**: Conflict-free replicated data types for seamless collaborative editing
- **Cross-Platform Support**: Works on Android, iOS, Linux, Windows, and macOS

## 📦 Libraries

🔧 [`crypto_utils`](./crypto_utils) - Cryptographic operations library powered by Sequoia-PGP for identity management, encryption, and digital signatures.

🌐 [`network`](./network) - Peer-to-peer networking layer powered by Iroh for direct device connections and data exchange.

🗄️ [`persistance`](./persistance) - SQLite implementation of repository interfaces for local data storage and management.

⚙️ [`services`](./services) - Synchronization and authentication logic for coordinating P2P operations.

🏗️ [`core`](./core) - Core domain types and business logic foundations for Osvauld applications.

## 🍙 Offline-First Philosophy

Osvauld applications work seamlessly offline, syncing changes when connectivity returns. This approach ensures:
- Complete user control over data
- Reduced security risks through local-first design
- Strong local encryption
- No dependency on external servers or services

## 🏗️ Building with Osvauld

Osvauld provides the infrastructure for creating privacy-focused, peer-to-peer applications. Whether you're building collaborative tools, secure messaging, or data sharing applications, Osvauld handles the complex networking, encryption, and synchronization so you can focus on your app's unique features.

## ❤️ Community

Join our community to discuss ideas, ask questions, and share your projects:

- **GitHub Discussions**: Ask questions and share ideas
- **Discord**: [Join our server](https://discord.gg/BVQtV6gS2c) for real-time chat
- **Code of Conduct**: [Read our community guidelines](./CODE_OF_CONDUCT.md)

## 🙏 Acknowledgments

This project has received funding from the **Kerala Startup Mission**.

---

*Ready to build the next generation of privacy-first applications? Start with Osvauld.*
