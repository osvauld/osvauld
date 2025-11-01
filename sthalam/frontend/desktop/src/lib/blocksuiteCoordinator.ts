import { YjsManager, type YjsManagerConfig } from "./yjsManager";
import type { Block, Viewport, UserInfo } from "../types/blocksuite.types";
import * as Y from "yjs";
import { SubmissionsStore } from "./submissionsStore";
import { ThreadCommentsStore } from "./threadCommentsStore";
import { BlocksuiteStore } from "./blocksuiteStore";
import { observeYjsDocuments } from "./templateState.svelte";

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
  private resourceType: string = 'website'; // Track resource type
  private hasThreadBlocks: boolean = false; // Track if resource has thread blocks
  private hasFormBlocks: boolean = false; // Track if resource has form blocks

  // Stores for managing different document types
  private submissionsStore: SubmissionsStore;
  private threadCommentsStore: ThreadCommentsStore;
  private blocksuiteStore: BlocksuiteStore;

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

    // Initialize stores
    console.log("🔨 Creating stores...");
    this.submissionsStore = new SubmissionsStore();
    this.threadCommentsStore = new ThreadCommentsStore();
    this.blocksuiteStore = new BlocksuiteStore();

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
    const loadStartTime = performance.now();
    console.log("🔧 [LOAD START] loadBlocksuite called at", loadStartTime);

    if (!data) {
      console.warn("⚠️ No data provided to loadBlocksuite");
      return;
    }

    try {
      console.log("🔄 Loading blocksuite data...");

      // Step 1: Detect resource type
      console.log("⏱️ [LOAD] Step 1: Detecting resource type at", performance.now() - loadStartTime, "ms");
      this.resourceType = this.detectResourceType(data);
      console.log("🔍 Detected resource type:", this.resourceType);

      // Step 2: Reinitialize Yjs documents with resource type (destroys old, creates fresh)
      console.log("⏱️ [LOAD] Step 2: Reinitializing Yjs documents at", performance.now() - loadStartTime, "ms");
      this.yjsManager.initialize(this.resourceType);
      console.log("⏱️ [LOAD] Yjs documents reinitialized at", performance.now() - loadStartTime, "ms");
      console.log("✅ Yjs documents reinitialized");

      // Step 3: Set user info in awareness
      console.log("⏱️ [LOAD] Step 3: Setting user info at", performance.now() - loadStartTime, "ms");
      this.yjsManager.setUserInfo(this.config.userInfo);

      // Step 4: Load blocksuite document
      console.log("⏱️ [LOAD] Step 4: Preparing blocksuite document at", performance.now() - loadStartTime, "ms");
      let blocksuiteUpdates: Uint8Array | null = null;

      if (data.blocksuite_doc) {
        this.currentDocKey = 'blocksuite_doc';
        blocksuiteUpdates = Array.isArray(data.blocksuite_doc)
          ? new Uint8Array(data.blocksuite_doc)
          : (data.blocksuite_doc.updates ? new Uint8Array(data.blocksuite_doc.updates) : null);
      } else if (data.thread_doc) {
        this.currentDocKey = 'thread_doc';
        blocksuiteUpdates = Array.isArray(data.thread_doc)
          ? new Uint8Array(data.thread_doc)
          : (data.thread_doc.updates ? new Uint8Array(data.thread_doc.updates) : null);
      } else if (data.form_doc) {
        this.currentDocKey = 'form_doc';
        blocksuiteUpdates = Array.isArray(data.form_doc)
          ? new Uint8Array(data.form_doc)
          : (data.form_doc.updates ? new Uint8Array(data.form_doc.updates) : null);
      }

      if (blocksuiteUpdates && blocksuiteUpdates.length > 0) {
        console.log(`📥 Applying ${this.currentDocKey} updates (${blocksuiteUpdates.length} bytes)`);
        console.log("⏱️ [LOAD] Applying blocksuite doc updates at", performance.now() - loadStartTime, "ms");
        await this.yjsManager.applyUpdate(blocksuiteUpdates, "loading", this.currentDocKey);
        console.log("⏱️ [LOAD] Blocksuite doc updates applied at", performance.now() - loadStartTime, "ms");
      }

      // Step 5: Load comments doc (if exists and has thread blocks)
      if (this.hasThreadBlocks && data.thread_comments_doc) {
        console.log("⏱️ [LOAD] Step 5: Loading comments doc at", performance.now() - loadStartTime, "ms");
        const commentsUpdates = Array.isArray(data.thread_comments_doc)
          ? new Uint8Array(data.thread_comments_doc)
          : (data.thread_comments_doc.updates ? new Uint8Array(data.thread_comments_doc.updates) : null);

        if (commentsUpdates && commentsUpdates.length > 0) {
          console.log(`📥 Applying thread_comments_doc updates (${commentsUpdates.length} bytes)`);
          await this.yjsManager.applyUpdate(commentsUpdates, "loading", "thread_comments_doc");
          console.log("⏱️ [LOAD] Comments doc updates applied at", performance.now() - loadStartTime, "ms");
        }
      }

      // Step 6: Load submissions doc (if exists and has form blocks)
      if (this.hasFormBlocks && data.form_submissions_doc) {
        console.log("⏱️ [LOAD] Step 6: Loading submissions doc at", performance.now() - loadStartTime, "ms");
        const submissionsUpdates = Array.isArray(data.form_submissions_doc)
          ? new Uint8Array(data.form_submissions_doc)
          : (data.form_submissions_doc.updates ? new Uint8Array(data.form_submissions_doc.updates) : null);

        if (submissionsUpdates && submissionsUpdates.length > 0) {
          console.log(`📥 [LOAD] Applying form_submissions_doc updates from backend: ${submissionsUpdates.length} bytes`);
          await this.yjsManager.applyUpdate(submissionsUpdates, "loading", "form_submissions_doc");
          console.log("⏱️ [LOAD] Submissions doc updates applied at", performance.now() - loadStartTime, "ms");

          // Log the final size after loading
          const docs = this.yjsManager.getDocuments();
          if (docs?.submissionsDoc) {
            const finalSize = Y.encodeStateAsUpdateV2(docs.submissionsDoc).length;
            console.log(`📊 [LOAD] form_submissions_doc total size after loading: ${finalSize} bytes`);
          }
        }
      }

      // Step 6.5: Load content doc (template state - publisher content)
      if (data.content_doc) {
        console.log("⏱️ [LOAD] Step 6.5: Loading content doc at", performance.now() - loadStartTime, "ms");
        const contentUpdates = Array.isArray(data.content_doc)
          ? new Uint8Array(data.content_doc)
          : (data.content_doc.updates ? new Uint8Array(data.content_doc.updates) : null);

        if (contentUpdates && contentUpdates.length > 0) {
          console.log(`📥 [LOAD] Applying content_doc updates from backend: ${contentUpdates.length} bytes`);
          await this.yjsManager.applyUpdate(contentUpdates, "loading", "content_doc");
          console.log("⏱️ [LOAD] Content doc updates applied at", performance.now() - loadStartTime, "ms");
        }
      }

      // Step 6.6: Load user_content doc (user-specific state)
      if (data.user_content_doc) {
        console.log("⏱️ [LOAD] Step 6.6: Loading user_content doc at", performance.now() - loadStartTime, "ms");
        const userContentUpdates = Array.isArray(data.user_content_doc)
          ? new Uint8Array(data.user_content_doc)
          : (data.user_content_doc.updates ? new Uint8Array(data.user_content_doc.updates) : null);

        if (userContentUpdates && userContentUpdates.length > 0) {
          console.log(`📥 [LOAD] Applying user_content_doc updates from backend: ${userContentUpdates.length} bytes`);
          await this.yjsManager.applyUpdate(userContentUpdates, "loading", "user_content_doc");
          console.log("⏱️ [LOAD] User content doc updates applied at", performance.now() - loadStartTime, "ms");
        }
      }

      // Step 7: Set up stores with Yjs maps
      console.log("⏱️ [LOAD] Step 7: Setting up stores at", performance.now() - loadStartTime, "ms");
      const docs = this.yjsManager.getDocuments();
      if (docs) {
        // Set up blocksuite store (main doc blocks) - always available
        this.blocksuiteStore.setBlocksMap(docs.blocks);

        // Set up thread comments store (if available for this resource type)
        if (docs.commentsBlocks && docs.commentsDoc) {
          this.threadCommentsStore.setCommentsMap(docs.commentsBlocks, docs.commentsDoc);
          console.log("✅ Thread comments store initialized");
        } else {
          console.log("ℹ️ Thread comments store not needed for this resource type");
        }

        // Set up submissions store (if available for this resource type)
        if (docs.submissionsBlocks && docs.submissionsDoc) {
          this.submissionsStore.setSubmissionsMap(docs.submissionsBlocks, docs.submissionsDoc);
          console.log("✅ Submissions store initialized");
        } else {
          console.log("ℹ️ Submissions store not needed for this resource type");
        }
      }

      // Step 8: Dispatch store-ready events (like livnote's pattern)
      console.log("⏱️ [LOAD] Step 8: Dispatching store-ready events at", performance.now() - loadStartTime, "ms");
      document.dispatchEvent(new CustomEvent('blocksuite-store-ready', {
        detail: { blocksuiteStore: this.blocksuiteStore }
      }));
      document.dispatchEvent(new CustomEvent('submissions-store-ready', {
        detail: { submissionsStore: this.submissionsStore }
      }));
      document.dispatchEvent(new CustomEvent('thread-comments-store-ready', {
        detail: { threadCommentsStore: this.threadCommentsStore }
      }));

      // Step 8.5: Set up template state observation of content and user_content docs
      console.log("⏱️ [LOAD] Step 8.5: Setting up template state observation at", performance.now() - loadStartTime, "ms");
      observeYjsDocuments(docs?.content, docs?.userContent);
      console.log("🔗 Template state observing content and user_content docs");

      // Step 9: Dispatch blocksuite-ready event after all loading is complete
      // This ensures the event fires even if there were no updates or they completed instantly
      console.log("⏱️ [LOAD] Step 9: All loading complete, dispatching blocksuite-ready event at", performance.now() - loadStartTime, "ms");
      console.log("✅ Blocksuite loading complete - dispatching ready event");
      console.log("🎉 [EVENT DISPATCH] Dispatching blocksuite-ready event NOW");
      document.dispatchEvent(new CustomEvent('blocksuite-ready', {
        detail: {
          resourceId: this.config.userInfo.id
        }
      }));
      console.log("🎉 [EVENT DISPATCH] blocksuite-ready event dispatched at", performance.now() - loadStartTime, "ms");
    } catch (error) {
      console.error("❌ Error loading blocksuite:", error);
      console.log("⏱️ [LOAD] Error occurred at", performance.now() - loadStartTime, "ms");
      // Dispatch ready event even on error to prevent infinite loading state
      console.log("🎉 [EVENT DISPATCH] Dispatching blocksuite-ready event (error case)");
      document.dispatchEvent(new CustomEvent('blocksuite-ready', {
        detail: {
          resourceId: this.config.userInfo.id,
          error: true
        }
      }));
      console.log("🎉 [EVENT DISPATCH] blocksuite-ready event dispatched (error) at", performance.now() - loadStartTime, "ms");
    }
  }

  /**
   * Detect resource type and check for thread/form blocks
   */
  private detectResourceType(data: any): string {
    // Reset flags
    this.hasThreadBlocks = false;
    this.hasFormBlocks = false;

    // Check for blocksuite_doc (website with potential thread/form blocks)
    if (data.blocksuite_doc) {
      const blockCheck = this.checkForThreadOrFormBlocks(data.blocksuite_doc);
      this.hasThreadBlocks = blockCheck.hasThread;
      this.hasFormBlocks = blockCheck.hasForm;
      console.log('🔍 Website resource detected:', { hasThreadBlocks: this.hasThreadBlocks, hasFormBlocks: this.hasFormBlocks });
      return 'website';
    }

    // Default to website (legacy resource types removed)
    return 'website';
  }

  /**
   * Check if document data contains thread or form blocks
   * This helps determine if we need a secondaryDoc
   */
  private checkForThreadOrFormBlocks(docData: any): { hasThread: boolean; hasForm: boolean } {
    try {
      // Decode the Yjs update to peek at blocks
      const updates = Array.isArray(docData)
        ? new Uint8Array(docData)
        : (docData.updates ? new Uint8Array(docData.updates) : null);

      if (!updates || updates.length === 0) {
        return { hasThread: false, hasForm: false };
      }

      // Create temporary doc to decode
      const tempDoc = new Y.Doc();
      Y.applyUpdateV2(tempDoc, updates);
      const blocks = tempDoc.getMap('blocks');

      let hasThread = false;
      let hasForm = false;

      blocks.forEach((block: any) => {
        if (block.type === 'thread') hasThread = true;

        // Check for form-related blocks
        if (block.type === 'form-submit-button') hasForm = true;
        if (block.type === 'form') hasForm = true; // Form metadata blocks
        if (block.type?.startsWith('form-field-')) hasForm = true; // Form field blocks
        if (block.type === 'nav-button' && block.formId) hasForm = true; // Nav buttons with forms (MODE 2 & 3)
      });

      tempDoc.destroy();

      console.log('🔍 Block detection:', { hasThread, hasForm });
      return { hasThread, hasForm };
    } catch (error) {
      console.warn('⚠️ Error checking for thread/form blocks:', error);
      return { hasThread: false, hasForm: false };
    }
  }

  /**
   * Save blocksuite data to backend
   * Saves all 3 documents when applicable: main + comments + submissions
   */
  saveBlocksuite(): any {
    console.log("💾 saveBlocksuite called", {
      resourceType: this.resourceType,
      currentDocKey: this.currentDocKey,
      hasThreadBlocks: this.hasThreadBlocks,
      hasFormBlocks: this.hasFormBlocks
    });

    const docs = this.yjsManager.getDocuments();
    if (!docs) {
      console.error("❌ No documents available in yjsManager");
      return null;
    }

    const result: any = {
      last_modified: Date.now()
    };

    // Save blocksuite document
    const blocksuiteUpdates = Y.encodeStateAsUpdateV2(docs.blocksuiteDoc);
    result[this.currentDocKey] = Array.from(blocksuiteUpdates);
    console.log(`📤 Saving ${this.currentDocKey}: ${blocksuiteUpdates.length} bytes`);

    // Save content doc (template state - publisher content)
    if (docs.content) {
      const contentUpdates = Y.encodeStateAsUpdateV2(docs.contentDoc!);
      result.content_doc = Array.from(contentUpdates);
      console.log(`📤 Saving content_doc: ${contentUpdates.length} bytes`);
    }

    // Save user_content doc (user-specific state)
    if (docs.userContent) {
      const userContentUpdates = Y.encodeStateAsUpdateV2(docs.userContentDoc!);
      result.user_content_doc = Array.from(userContentUpdates);
      console.log(`📤 Saving user_content_doc: ${userContentUpdates.length} bytes`);
    }

    // Save comments doc if exists
    if (docs.commentsDoc) {
      const commentsUpdates = Y.encodeStateAsUpdateV2(docs.commentsDoc);
      result.thread_comments_doc = Array.from(commentsUpdates);
      console.log(`📤 Saving thread_comments_doc: ${commentsUpdates.length} bytes`);
    }

    // Save submissions doc if exists
    if (docs.submissionsDoc) {
      const submissionsUpdates = Y.encodeStateAsUpdateV2(docs.submissionsDoc);
      result.form_submissions_doc = Array.from(submissionsUpdates);
      console.log(`📤 Saving form_submissions_doc: ${submissionsUpdates.length} bytes`);
    }

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
  applyRemoteUpdate(update: Uint8Array | number[], senderId: number, docType?: string): void {
    if (senderId === this.config.userInfo.id) return;
    this.yjsManager.applyUpdate(update, 'sync', docType);
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
   * Get the submissions store
   */
  getSubmissionsStore(): SubmissionsStore {
    return this.submissionsStore;
  }

  /**
   * Get the thread comments store
   */
  getThreadCommentsStore(): ThreadCommentsStore {
    return this.threadCommentsStore;
  }

  /**
   * Get the blocksuite store
   */
  getBlocksuiteStore(): BlocksuiteStore {
    return this.blocksuiteStore;
  }

  /**
   * Clean up
   */
  destroy(): void {
    // Destroy stores
    this.submissionsStore.destroy();
    this.threadCommentsStore.destroy();
    this.blocksuiteStore.destroy();

    // Destroy Yjs manager
    this.yjsManager.destroy();
  }
}
