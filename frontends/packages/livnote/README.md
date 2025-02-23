# Livnote

Meet **Osvauld Livnote** – a collaborative document editor application that brings robust, peer-to-peer connectivity and end-to-end encryption to your document workflow. Edit, share, and collaborate on your documents in real time, all while staying truly independent of cloud infrastructure.

---

## 🔒 Features & Security

- **Real-Time Secure Collaboration:**  
  Enjoy seamless, real-time document editing with a peer-to-peer connection that ensures data is transmitted directly between devices—minimizing reliance on intermediaries.

- **End-to-End Encryption:**  
  All your documents and metadata are encrypted locally. Only you and those you explicitly share with have the keys to decrypt your content, strictly adhering to a zero-knowledge architecture.

- **Open-PGP & Seqouia-PGP:**  
  Securely share documents with team members using the trusted Open-PGP protocol, enhanced by Rust-based Seqouia-PGP, which emphasizes safety and correctness.

- **Robust Cryptographic Algorithms:**  
  Our platform employs ECC Curve25519 and AES-256, following the OpenPGP RFC 4880 standard to keep your documents secure.

---

## 🍙 Offline First

- **Work Anywhere, Anytime:**  
  Create and edit documents offline with full functionality. Once you're back online, osvauld livnote seamlessly syncs your changes, ensuring your work is always up to date without compromising security.

---

## 📝 Collaborative Editing

- **Real-Time Editing:**  
  Experience simultaneous editing with your team, complete with conflict resolution and version history to track changes and revert if necessary.

- **Effortless Sharing:**  
  Invite collaborators with ease and manage permissions securely, ensuring that every edit is as private as it is productive.

---

## 🛠️ Tech

- **Modern UI & Cross-Platform:**  
  The user interface is built with Svelte and TypeScript, ensuring a fast, responsive experience across devices.

- **Tauri & SQLite:**  
  Enjoy native performance on Android, iOS, Linux, Windows, and macOS with cross-platform builds powered by Tauri and secure local storage managed by SQLite.

- **Peer-to-Peer Enabled with Iroh:**  
  Direct, encrypted connections between devices are made possible by Iroh, providing a fast and reliable p2p experience with fallback to relay servers when needed.

- **ProseMirror**
  A toolkit for building rich-text editors on the web.

---

## 🛠️ Development & Testing
### Prerequisites
- **Node.js & pnpm:** Ensure you have Node.js (v14+ recommended) and pnpm installed.
- **Rust & Tauri CLI:** Install the Rust toolchain and Tauri CLI for building desktop applications.
- **SQLite:** (Optional) For local database management if needed during development.

### Setup
1. **Clone the Repository:**
   ```bash
   git clone https://github.com/osvauld/osvauld.git

2. **Install dependecies:**
   ```bash
   cd /frontends/packages/livnote/desktop
   pnpm install

3. **Run development server:**
   ```bash
   cd /tauri/src-tauri
   cargo tauri dev

## 🤝 Contributing

We welcome contributions! Please follow these steps:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

### Code Style

- Follow the established Rust/TypeScript style guide
- Use Prettier/Rust-analyser for code formatting
- Ensure all tests pass before submitting PRs
- Document new features and changes

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Tauri team for the amazing framework
- Iroh team for p2p capabilities
- Sequoia-PGP team for Rust implementation
- All contributors and community members

## 📞 Support & Contact

- GitHub Issues: [Create an issue](https://github.com/osvauld/osvauld/issues)
- Email: support@osvauld.com

## 🔄 Version History

See [CHANGELOG.md](CHANGELOG.md) for a list of changes and version updates.


## 🚢 Onboard osvauld livnote

Available on:
- Android
- iOS
- Linux
- Windows
- macOS

---

## ❤️ Community

Join the growing community of osvauld users!  
- **GitHub Discussions:** Connect, ask questions, and share your projects.  
- **Discord:** Chat with community members in real time.  

All community interactions are guided by our Code of Conduct to ensure a respectful and collaborative environment.

---

Start collaborating securely with **osvauld livnote** – where your documents remain yours, and every edit is protected.
