import * as Y from "yjs";
import { applyAwarenessUpdate, Awareness, encodeAwarenessUpdate } from "y-protocols/awareness";
import type { UserInfo, Collaborator } from "../types/blocksuite.types";

export interface YjsDocuments {
  blocksuiteDoc: Y.Doc;
  blocks: Y.Map<any>;
  viewport: Y.Map<any>;
  awareness: Awareness;

  // Content document (publisher-owned, shared across users)
  contentDoc?: Y.Doc;
  content?: Y.Map<any>;

  // User content document (per-user state, mutable)
  userContentDoc?: Y.Doc;
  userContent?: Y.Map<any>;

  // Thread comments document (collaborative - all participants can read/write)
  commentsDoc?: Y.Doc;
  commentsBlocks?: Y.Map<any>;

  // Form submissions document (viewers append, owner receives)
  submissionsDoc?: Y.Doc;
  submissionsBlocks?: Y.Map<any>;
}

export interface YjsManagerConfig {
  clientId: number;
  onUpdate?: (update: Uint8Array, origin: any, docType?: string) => void;
  onAwarenessChange?: (changes: any, origin: string) => void;
}

/**
 * Manages YJS document lifecycle and synchronization for blocksuite
 * Separated from UI concerns - follows livnote pattern
 */
export class YjsManager {
  private documents: YjsDocuments | null = null;
  private config: YjsManagerConfig;

  constructor(config: YjsManagerConfig) {
    this.config = config;
  }

  /**
   * Initialize YJS documents and structures
   * IMPORTANT: Always destroys old documents first to ensure clean slate (livnote pattern)
   * NEW: Supports resourceType parameter to create secondary docs for threads
   */
  initialize(resourceType?: string): YjsDocuments {
    // Destroy old documents first - critical for note switching
    this.destroy();

    const blocksuiteDoc = new Y.Doc({
      gc: true,
      gcFilter: () => false,
    });

    const blocks = blocksuiteDoc.getMap("blocks");
    const viewport = blocksuiteDoc.getMap("viewport");
    const awareness = new Awareness(blocksuiteDoc);

    // Initialize viewport if empty
    if (viewport.size === 0) {
      viewport.set("x", 0);
      viewport.set("y", 0);
      viewport.set("zoom", 1);
    }

    // Set up update listener for blocksuite doc
    if (this.config.onUpdate) {
      blocksuiteDoc.on("updateV2", (update: Uint8Array, origin: any) => {
        if (origin !== "sync" && origin !== "loading") {
          this.config.onUpdate!(update, origin, "blocksuite_doc");
        }
      });
    }

    // Set up awareness listener
    if (this.config.onAwarenessChange) {
      awareness.on('change', async (changes: { added: number[], updated: number[], removed: number[] }, origin: string) => {
        if (origin === 'local') {
          try {
            const clients = [...changes.added, ...changes.updated, ...changes.removed];
            if (clients.length === 0) {
              return;
            }
            const encodedUpdate = encodeAwarenessUpdate(awareness, clients);
            this.config.onAwarenessChange!(encodedUpdate, origin);
          } catch (error) {
            console.error("Error processing awareness update in YjsManager:", error);
          }
        }
      });
    }

    // Create content doc (publisher-owned data)
    console.log('🔧 Creating contentDoc for publisher content');
    const contentDoc = new Y.Doc({
      gc: true,
      gcFilter: () => false,
    });
    const content = contentDoc.getMap("content");

    // Set up update listener for content doc
    if (this.config.onUpdate) {
      contentDoc.on("updateV2", (update: Uint8Array, origin: any) => {
        if (origin !== "sync" && origin !== "loading") {
          console.log(`📤 [content_doc] Update triggered, size: ${update.byteLength} bytes, origin:`, origin);
          this.config.onUpdate!(update, origin, "content_doc");
        }
      });
    }
    console.log('✅ contentDoc created');

    // Create user content doc (per-user state)
    console.log('🔧 Creating userContentDoc for user state');
    const userContentDoc = new Y.Doc({
      gc: true,
      gcFilter: () => false,
    });
    const userContent = userContentDoc.getMap("user_content");

    // Set up update listener for user content doc
    if (this.config.onUpdate) {
      userContentDoc.on("updateV2", (update: Uint8Array, origin: any) => {
        if (origin !== "sync" && origin !== "loading") {
          this.config.onUpdate!(update, origin, "user_content_doc");
        }
      });
    }
    console.log('✅ userContentDoc created');

    // Create comments doc if needed (for thread blocks)
    let commentsDoc: Y.Doc | undefined;
    let commentsBlocks: Y.Map<any> | undefined;

    if (resourceType === 'noticeboard' || resourceType === 'website') {
      console.log('🔧 Creating commentsDoc for thread blocks');
      commentsDoc = new Y.Doc({
        gc: true,
        gcFilter: () => false,
      });

      commentsBlocks = commentsDoc.getMap("blocks");

      // Set up update listener for comments doc
      if (this.config.onUpdate) {
        commentsDoc.on("updateV2", (update: Uint8Array, origin: any) => {
          if (origin !== "sync" && origin !== "loading") {
            this.config.onUpdate!(update, origin, "thread_comments_doc");
          }
        });
      }

      console.log('✅ commentsDoc created');
    }

    // Create submissions doc if needed (for form blocks)
    let submissionsDoc: Y.Doc | undefined;
    let submissionsBlocks: Y.Map<any> | undefined;

    if (resourceType === 'form' || resourceType === 'website') {
      console.log('🔧 Creating submissionsDoc for form blocks');
      submissionsDoc = new Y.Doc({
        gc: true,
        gcFilter: () => false,
      });

      submissionsBlocks = submissionsDoc.getMap("blocks");

      // Set up update listener for submissions doc
      if (this.config.onUpdate) {
        submissionsDoc.on("updateV2", (update: Uint8Array, origin: any) => {
          if (origin !== "sync" && origin !== "loading") {
            this.config.onUpdate!(update, origin, "form_submissions_doc");
          }
        });
      }

      console.log('✅ submissionsDoc created');
    }

    this.documents = {
      blocksuiteDoc,
      blocks,
      viewport,
      awareness,
      contentDoc,
      content,
      userContentDoc,
      userContent,
      commentsDoc,
      commentsBlocks,
      submissionsDoc,
      submissionsBlocks
    };

    return this.documents;
  }

  /**
   * Get current documents
   */
  getDocuments(): YjsDocuments | null {
    return this.documents;
  }

  /**
   * Update client ID and reinitialize if needed
   */
  updateClientId(clientId: number): void {
    this.config.clientId = clientId;
    if (this.documents) {
      this.initialize();
    }
  }

  /**
   * Set user info in awareness
   */
  setUserInfo(userInfo: UserInfo): void {
    if (!this.documents) return;
    this.documents.awareness.setLocalState({
      user: userInfo
    });
  }

  /**
   * Get state vector for sync
   */
  getStateVector(): Uint8Array {
    if (!this.documents) return new Uint8Array();
    return Y.encodeStateVector(this.documents.blocksuiteDoc);
  }

  /**
   * Apply update from remote
   * Routes updates to the correct document based on docType
   * OPTIMIZED: Returns Promise for async handling of large updates
   */
  applyUpdate(update: Uint8Array | number[], origin: any = 'sync', docType?: string): Promise<void> {
    if (!this.documents) return Promise.resolve();
    const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);

    // Route to correct document based on docType
    let targetDoc: Y.Doc = this.documents.blocksuiteDoc;

    if (docType === 'content_doc' && this.documents.contentDoc) {
      targetDoc = this.documents.contentDoc;
    } else if (docType === 'user_content_doc' && this.documents.userContentDoc) {
      targetDoc = this.documents.userContentDoc;
    } else if (docType === 'thread_comments_doc' && this.documents.commentsDoc) {
      targetDoc = this.documents.commentsDoc;
    } else if (docType === 'form_submissions_doc' && this.documents.submissionsDoc) {
      targetDoc = this.documents.submissionsDoc;
    } else if (docType === 'blocksuite_doc') {
      targetDoc = this.documents.blocksuiteDoc;
    }

    // Track byte size changes for form_submissions_doc
    let beforeSize = 0;
    let afterSize = 0;
    if (docType === 'form_submissions_doc' && this.documents.submissionsDoc) {
      beforeSize = Y.encodeStateAsUpdateV2(targetDoc).length;
      console.log(`📊 [YjsManager] form_submissions_doc BEFORE applying update: ${beforeSize} bytes`);
    }

    // For loading large documents (>50KB), defer to next frame to keep UI responsive
    if (origin === 'loading' && updateArray.length > 50000) {
      console.log(`⚡ Deferring large update (${(updateArray.length / 1024).toFixed(1)}KB) to next frame...`);
      return new Promise((resolve) => {
        requestAnimationFrame(() => {
          console.log(`📥 Applying deferred update to ${docType || 'blocksuiteDoc'}...`);
          targetDoc.transact(() => {
            Y.applyUpdateV2(targetDoc, updateArray, origin);
          }, origin);
          console.log(`✅ Deferred update applied`);

          // Log size change for form_submissions_doc
          if (docType === 'form_submissions_doc' && this.documents?.submissionsDoc) {
            afterSize = Y.encodeStateAsUpdateV2(targetDoc).length;
            console.log(`📊 [YjsManager] form_submissions_doc AFTER applying update: ${afterSize} bytes (diff: ${afterSize - beforeSize})`);
          }

          resolve();
        });
      });
    } else {
      // Small updates or sync updates - apply immediately
      Y.applyUpdateV2(targetDoc, updateArray, origin);

      // Log size change for form_submissions_doc
      if (docType === 'form_submissions_doc' && this.documents.submissionsDoc) {
        afterSize = Y.encodeStateAsUpdateV2(targetDoc).length;
        console.log(`📊 [YjsManager] form_submissions_doc AFTER applying update: ${afterSize} bytes (diff: ${afterSize - beforeSize})`);
      }

      return Promise.resolve();
    }
  }

  /**
   * Get state as update for initial sync
   */
  getStateAsUpdate(): Uint8Array {
    if (!this.documents) return new Uint8Array();
    return Y.encodeStateAsUpdateV2(this.documents.blocksuiteDoc);
  }

  /**
   * Apply awareness update from remote clients
   */
  async applyAwarenessUpdate(update: Uint8Array | number[], sender: number): Promise<void> {
    if (!this.documents || sender === this.config.clientId) {
      return;
    }

    try {
      const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);
      if (updateArray.length === 0) {
        console.warn("⚠️ Received empty awareness update");
        return;
      }
      applyAwarenessUpdate(this.documents.awareness, updateArray, 'remote');
    } catch (error) {
      console.error("❌ Error applying awareness update:", error);
    }
  }


  /**
   * Check if documents are initialized
   */
  isInitialized(): boolean {
    return this.documents !== null;
  }

  /**
   * Clean up and destroy documents
   */
  destroy(): void {
    if (this.documents) {
      this.documents.awareness.destroy();
      this.documents.blocksuiteDoc.destroy();

      if (this.documents.contentDoc) {
        this.documents.contentDoc.destroy();
      }

      if (this.documents.userContentDoc) {
        this.documents.userContentDoc.destroy();
      }

      if (this.documents.commentsDoc) {
        this.documents.commentsDoc.destroy();
      }

      if (this.documents.submissionsDoc) {
        this.documents.submissionsDoc.destroy();
      }

      this.documents = null;
    }
  }
}
