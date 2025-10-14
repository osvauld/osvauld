import { YjsManager, type YjsManagerConfig } from "./yjsManager";
import type { Block, Viewport, UserInfo } from "../types/blocksuite.types";
import * as Y from "yjs";

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
  private resourceType: string = 'website'; // Track resource type for split docs

  constructor(config: BlocksuiteCoordinatorConfig) {
    console.log("🔨 BlocksuiteCoordinator constructor called with userInfo:", config.userInfo);
    this.config = config;

    const yjsConfig: YjsManagerConfig = {
      clientId: config.userInfo.id,
      onUpdate: async (update: Uint8Array, origin: any, docType?: string) => {
        // NEW: docType parameter for routing updates
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
   * Supports multiple document keys: blocksuite_doc, form_doc, thread_doc + thread_comments_doc, main_doc (legacy)
   * NEW: Handles split docs for noticeboards (thread_doc + thread_comments_doc)
   * OPTIMIZED: Async to handle deferred loading of large documents
   */
  async loadBlocksuite(data: any): Promise<void> {
    if (!data) {
      console.warn("⚠️ No data provided to loadBlocksuite");
      return;
    }

    try {
      console.log("🔄 Loading blocksuite data...");

      // Step 1: Detect resource type
      this.resourceType = this.detectResourceType(data);
      console.log("🔍 Detected resource type:", this.resourceType);

      // Step 2: Reinitialize Yjs documents with resource type (destroys old, creates fresh)
      const docs = this.yjsManager.initialize(this.resourceType);
      console.log("✅ Yjs documents reinitialized");

      // Step 3: Set user info in awareness
      this.yjsManager.setUserInfo(this.config.userInfo);

      // Step 4: Set up one-time listener for when document is ready
      docs.mainDoc.once('afterAllTransactions', () => {
        console.log("✅ Document transactions complete - blocksuite ready");

        // Dispatch event so UI knows the document is ready
        document.dispatchEvent(new CustomEvent('blocksuite-ready', {
          detail: {
            resourceId: this.config.userInfo.id
          }
        }));
      });

      // Step 5: Handle split docs for noticeboards and forms
      if (this.resourceType === 'noticeboard') {
        // Load thread_doc (main post)
        if (data.thread_doc) {
          const threadUpdates = Array.isArray(data.thread_doc)
            ? new Uint8Array(data.thread_doc)
            : (data.thread_doc.updates ? new Uint8Array(data.thread_doc.updates) : null);

          if (threadUpdates && threadUpdates.length > 0) {
            console.log(`📥 Applying thread_doc updates (${threadUpdates.length} bytes)`);
            await this.yjsManager.applyUpdate(threadUpdates, "loading", "thread_doc");
          }
        }

        // Load thread_comments_doc (comments)
        if (data.thread_comments_doc) {
          const commentsUpdates = Array.isArray(data.thread_comments_doc)
            ? new Uint8Array(data.thread_comments_doc)
            : (data.thread_comments_doc.updates ? new Uint8Array(data.thread_comments_doc.updates) : null);

          if (commentsUpdates && commentsUpdates.length > 0) {
            console.log(`📥 Applying thread_comments_doc updates (${commentsUpdates.length} bytes)`);
            await this.yjsManager.applyUpdate(commentsUpdates, "loading", "thread_comments_doc");
          }
        }

        this.currentDocKey = 'thread_doc';
      } else if (this.resourceType === 'form') {
        // Load form_doc (form definition)
        if (data.form_doc) {
          const formUpdates = Array.isArray(data.form_doc)
            ? new Uint8Array(data.form_doc)
            : (data.form_doc.updates ? new Uint8Array(data.form_doc.updates) : null);

          if (formUpdates && formUpdates.length > 0) {
            console.log(`📥 Applying form_doc updates (${formUpdates.length} bytes)`);
            await this.yjsManager.applyUpdate(formUpdates, "loading", "form_doc");
          }
        }

        // Load form_submissions_doc (submissions)
        if (data.form_submissions_doc) {
          const submissionsUpdates = Array.isArray(data.form_submissions_doc)
            ? new Uint8Array(data.form_submissions_doc)
            : (data.form_submissions_doc.updates ? new Uint8Array(data.form_submissions_doc.updates) : null);

          if (submissionsUpdates && submissionsUpdates.length > 0) {
            console.log(`📥 Applying form_submissions_doc updates (${submissionsUpdates.length} bytes)`);
            await this.yjsManager.applyUpdate(submissionsUpdates, "loading", "form_submissions_doc");
          }
        }

        this.currentDocKey = 'form_doc';
      } else {
        // Step 6: Standard single-doc loading (website, form)
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
          await this.yjsManager.applyUpdate(updates, "loading");
          console.log("✅ Updates applied");
        } else {
          console.warn("⚠️ No document updates found in data");
        }
      }
    } catch (error) {
      console.error("❌ Error loading blocksuite:", error);
    }
  }

  /**
   * NEW: Detect resource type from data
   */
  private detectResourceType(data: any): string {
    if (data.thread_doc && data.thread_comments_doc) return 'noticeboard';
    if (data.thread_doc) return 'noticeboard'; // Legacy single-doc noticeboard
    if (data.form_doc && data.form_submissions_doc) return 'form'; // Split-doc form
    if (data.form_doc) return 'form'; // Legacy single-doc form
    if (data.blocksuite_doc) return 'website';
    return 'website'; // Default
  }

  /**
   * Save blocksuite data to backend
   * Uses the correct document key based on resource type (form_doc, blocksuite_doc, etc.)
   * NEW: Saves both docs for noticeboards (thread_doc + thread_comments_doc)
   */
  saveBlocksuite(): any {
    console.log("💾 saveBlocksuite called, resourceType:", this.resourceType, "currentDocKey:", this.currentDocKey);

    const docs = this.yjsManager.getDocuments();
    if (!docs) {
      console.error("❌ No documents available in yjsManager");
      return null;
    }

    // Handle split docs for noticeboards and forms
    if (this.resourceType === 'noticeboard' && docs.secondaryDoc) {
      const threadUpdates = Y.encodeStateAsUpdateV2(docs.mainDoc);
      const commentsUpdates = Y.encodeStateAsUpdateV2(docs.secondaryDoc);

      console.log("📤 Saving split docs:", {
        thread_doc: threadUpdates.length,
        thread_comments_doc: commentsUpdates.length
      });

      return {
        thread_doc: Array.from(threadUpdates),
        thread_comments_doc: Array.from(commentsUpdates),
        last_modified: Date.now()
      };
    } else if (this.resourceType === 'form' && docs.secondaryDoc) {
      const formUpdates = Y.encodeStateAsUpdateV2(docs.mainDoc);
      const submissionsUpdates = Y.encodeStateAsUpdateV2(docs.secondaryDoc);

      console.log("📤 Saving split docs:", {
        form_doc: formUpdates.length,
        form_submissions_doc: submissionsUpdates.length
      });

      return {
        form_doc: Array.from(formUpdates),
        form_submissions_doc: Array.from(submissionsUpdates),
        last_modified: Date.now()
      };
    } else {
      // Standard single-doc save
      const mainUpdates = this.yjsManager.getStateAsUpdate();
      console.log("📦 Main updates length:", mainUpdates.length);

      const result = {
        [this.currentDocKey]: Array.from(mainUpdates),
        last_modified: Date.now()
      };

      console.log("📤 Returning save data:", {
        docKey: this.currentDocKey,
        updatesLength: mainUpdates.length
      });

      return result;
    }
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
