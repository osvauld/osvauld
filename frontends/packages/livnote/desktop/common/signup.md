
Component Structure Signup

Signup.svelte (Main orchestrator)
├── WelcomeView.svelte (Welcome screen)
├── FlowContainer.svelte (Layout wrapper)
│   ├── BackButton.svelte (Navigation)
│   └── [Flow Step Components]
│       ├── BaseImportPvtKey.svelte
│       ├── NewPassword.svelte
│       ├── CollectUsername.svelte
│       └── ProvidePrivateKey.svelte



