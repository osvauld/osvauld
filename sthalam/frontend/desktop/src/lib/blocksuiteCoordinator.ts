import { YjsManager, type YjsManagerConfig } from "./yjsManager";
import type { Block, Viewport, UserInfo } from "../types/blocksuite.types";

export interface BlocksuiteCoordinatorConfig {
  onCollaborationUpdate: (update: Uint8Array) => Promise<void>;
  onAwarenessUpdate: (update: Uint8Array) => Promise<void>;
  userInfo: UserInfo;
}

/**
 * Coordinates between YjsManager and UI components
 * Follows livnote's NotesCoordinator pattern
 */
export class BlocksuiteCoordinator {
  private yjsManager: YjsManager;
  private config: BlocksuiteCoordinatorConfig;
  private currentDocKey: string = 'main_doc'; // Track which doc type we're using

  constructor(config: BlocksuiteCoordinatorConfig) {
    console.log("🔨 BlocksuiteCoordinator constructor called with userInfo:", config.userInfo);
    this.config = config;

    const yjsConfig: YjsManagerConfig = {
      clientId: config.userInfo.id,
      onUpdate: async (update: Uint8Array, origin: any) => {
        await config.onCollaborationUpdate(update);
      },
      onAwarenessChange: async (encodedUpdate: Uint8Array, origin: string) => {
        await config.onAwarenessUpdate(encodedUpdate);
      }
    };

    console.log("🔨 Creating YjsManager...");
    this.yjsManager = new YjsManager(yjsConfig);
    console.log("✅ BlocksuiteCoordinator constructor complete");
  }

  /**
   * Initialize the coordinator and Yjs documents
   */
  initialize(): void {
    console.log("🔧 BlocksuiteCoordinator.initialize() called");
    const docs = this.yjsManager.initialize();
    this.yjsManager.setUserInfo(this.config.userInfo);
    console.log("✅ BlocksuiteCoordinator initialized with documents:", !!docs);
  }

  /**
   * Load blocksuite data from saved state
   * Following livnote's loadNote pattern: reinitialize docs, apply updates, wait for ready
   * Supports multiple document keys: blocksuite_doc, form_doc, thread_doc, main_doc (legacy)
   */
  loadBlocksuite(data: any): void {
    if (!data) {
      console.warn("⚠️ No data provided to loadBlocksuite");
      return;
    }

    try {
      console.log("🔄 Loading blocksuite data...");

      // Step 1: Reinitialize Yjs documents (destroys old, creates fresh)
      const docs = this.yjsManager.initialize();
      console.log("✅ Yjs documents reinitialized");

      // Step 2: Set user info in awareness
      this.yjsManager.setUserInfo(this.config.userInfo);

      // Step 3: Set up one-time listener for when document is ready
      docs.mainDoc.once('afterAllTransactions', () => {
        console.log("✅ Document transactions complete - blocksuite ready");

        // Dispatch event so UI knows the document is ready
        document.dispatchEvent(new CustomEvent('blocksuite-ready', {
          detail: {
            resourceId: this.config.userInfo.id
          }
        }));
      });

      // Step 4: Detect and apply document updates based on resource type
      // Check for different document keys: blocksuite_doc, form_doc, thread_doc, main_doc (legacy)
      let docKey: string | null = null;
      let updates: Uint8Array | null = null;

      if (data.blocksuite_doc) {
        docKey = 'blocksuite_doc';
        updates = Array.isArray(data.blocksuite_doc)
          ? new Uint8Array(data.blocksuite_doc)
          : (data.blocksuite_doc.updates ? new Uint8Array(data.blocksuite_doc.updates) : null);
      } else if (data.form_doc) {
        docKey = 'form_doc';
        updates = Array.isArray(data.form_doc)
          ? new Uint8Array(data.form_doc)
          : (data.form_doc.updates ? new Uint8Array(data.form_doc.updates) : null);
      } else if (data.thread_doc) {
        docKey = 'thread_doc';
        updates = Array.isArray(data.thread_doc)
          ? new Uint8Array(data.thread_doc)
          : (data.thread_doc.updates ? new Uint8Array(data.thread_doc.updates) : null);
      } else if (data.main_doc) {
        docKey = 'main_doc';  // Legacy support
        updates = Array.isArray(data.main_doc)
          ? new Uint8Array(data.main_doc)
          : (data.main_doc.updates ? new Uint8Array(data.main_doc.updates) : null);
      }

      // Store the current doc key for saving
      if (docKey) {
        this.currentDocKey = docKey;
      }

      if (updates && updates.length > 0) {
        console.log(`📥 Applying ${updates.length} bytes from ${docKey}...`);
        this.yjsManager.applyUpdate(updates, "loading");
        console.log("✅ Updates applied");
      } else {
        console.warn("⚠️ No document updates found in data");
      }
    } catch (error) {
      console.error("❌ Error loading blocksuite:", error);
    }
  }

  /**
   * Save blocksuite data to backend
   * Uses the correct document key based on resource type (form_doc, blocksuite_doc, etc.)
   */
  saveBlocksuite(): any {
    console.log("💾 saveBlocksuite called, currentDocKey:", this.currentDocKey);

    const docs = this.yjsManager.getDocuments();
    if (!docs) {
      console.error("❌ No documents available in yjsManager");
      return null;
    }
    console.log("✅ Documents available:", !!docs.mainDoc, !!docs.viewport);

    const mainUpdates = this.yjsManager.getStateAsUpdate();
    console.log("📦 Main updates length:", mainUpdates.length);

    const result = {
      [this.currentDocKey]: Array.from(mainUpdates),
      last_modified: Date.now()
    };

    console.log("📤 Returning save data:", {
      docKey: this.currentDocKey,
      updatesLength: mainUpdates.length,
      result
    });

    return result;
  }

  /**
   * Get state vector for sync
   */
  getStateVector(): any {
    return {
      main: Array.from(this.yjsManager.getStateVector())
    };
  }

  /**
   * Apply remote update from sync
   */
  applyRemoteUpdate(update: Uint8Array, senderId: number): void {
    if (senderId === this.config.userInfo.id) return;
    this.yjsManager.applyUpdate(update, 'sync');
  }

  /**
   * Apply remote awareness update
   */
  applyAwarenessUpdate(update: Uint8Array, senderId: number): void {
    this.yjsManager.applyAwarenessUpdate(update, senderId);
  }

  /**
   * Get Yjs documents for direct access
   */
  getDocuments() {
    return this.yjsManager.getDocuments();
  }


  /**
   * Clean up
   */
  destroy(): void {
    this.yjsManager.destroy();
  }
}
