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

  constructor(config: BlocksuiteCoordinatorConfig) {
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

    this.yjsManager = new YjsManager(yjsConfig);
  }

  /**
   * Initialize the coordinator and Yjs documents
   */
  initialize(): void {
    const docs = this.yjsManager.initialize();
    this.yjsManager.setUserInfo(this.config.userInfo);
  }

  /**
   * Load blocksuite data from saved state
   * Following livnote's loadNote pattern: reinitialize docs, apply updates, wait for ready
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

      // Step 4: Apply main document updates with 'loading' origin
      if (data.main_doc && data.main_doc.updates) {
        const updates = new Uint8Array(data.main_doc.updates);
        console.log(`📥 Applying ${updates.length} bytes of updates...`);
        this.yjsManager.applyUpdate(updates, "loading");
        console.log("✅ Updates applied");
      } else {
        console.warn("⚠️ No main_doc.updates found in data");
      }
    } catch (error) {
      console.error("❌ Error loading blocksuite:", error);
    }
  }

  /**
   * Save blocksuite data to backend
   */
  saveBlocksuite(): any {
    const docs = this.yjsManager.getDocuments();
    if (!docs) return null;

    const mainUpdates = this.yjsManager.getStateAsUpdate();

    return {
      main_doc: {
        updates: Array.from(mainUpdates),
        state_vector: Array.from(this.yjsManager.getStateVector())
      },
      last_modified: Date.now()
    };
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
   * Set cached data state for collaborator updates
   */
  setCachedDataState(dataState: any): void {
    this.yjsManager.setCachedDataState(dataState);
  }

  /**
   * Clean up
   */
  destroy(): void {
    this.yjsManager.destroy();
  }
}
