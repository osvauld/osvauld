import * as Y from "yjs";
import { applyAwarenessUpdate, Awareness, encodeAwarenessUpdate } from "y-protocols/awareness";
import type { UserInfo, Collaborator } from "../types/blocksuite.types";

export interface YjsDocuments {
  mainDoc: Y.Doc;
  blocks: Y.Map<any>;
  viewport: Y.Map<any>;
  awareness: Awareness;

  // NEW: Secondary document for thread comments
  secondaryDoc?: Y.Doc;
  secondaryBlocks?: Y.Map<any>;
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

    const mainDoc = new Y.Doc({
      gc: true,
      gcFilter: () => false,
    });

    const blocks = mainDoc.getMap("blocks");
    const viewport = mainDoc.getMap("viewport");
    const awareness = new Awareness(mainDoc);

    // Initialize viewport if empty
    if (viewport.size === 0) {
      viewport.set("x", 0);
      viewport.set("y", 0);
      viewport.set("zoom", 1);
    }

    // Set up update listener for main doc
    if (this.config.onUpdate) {
      mainDoc.on("updateV2", (update: Uint8Array, origin: any) => {
        if (origin !== "sync" && origin !== "loading") {
          this.config.onUpdate!(update, origin, "main");
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

    // NEW: Create secondary doc for thread comments or form submissions
    let secondaryDoc: Y.Doc | undefined;
    let secondaryBlocks: Y.Map<any> | undefined;

    if (resourceType === 'noticeboard' || resourceType === 'form') {
      const docTypeName = resourceType === 'noticeboard' ? 'thread comments' : 'form submissions';
      const docTypeKey = resourceType === 'noticeboard' ? 'thread_comments_doc' : 'form_submissions_doc';

      console.log(`🔧 Creating secondary doc for ${docTypeName}`);
      secondaryDoc = new Y.Doc({
        gc: true,
        gcFilter: () => false,
      });

      secondaryBlocks = secondaryDoc.getMap("blocks");

      // Set up update listener for secondary doc
      if (this.config.onUpdate) {
        secondaryDoc.on("updateV2", (update: Uint8Array, origin: any) => {
          if (origin !== "sync" && origin !== "loading") {
            this.config.onUpdate!(update, origin, docTypeKey);
          }
        });
      }

      console.log(`✅ Secondary doc created for ${docTypeName}`);
    }

    this.documents = {
      mainDoc,
      blocks,
      viewport,
      awareness,
      secondaryDoc,
      secondaryBlocks
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
    return Y.encodeStateVector(this.documents.mainDoc);
  }

  /**
   * Apply update from remote
   * NEW: Supports docType parameter to route updates to the correct document
   */
  applyUpdate(update: Uint8Array | number[], origin: any = 'sync', docType?: string): void {
    if (!this.documents) return;
    const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);

    // Route to secondary doc if docType is thread_comments_doc or form_submissions_doc
    if ((docType === 'thread_comments_doc' || docType === 'form_submissions_doc')
        && this.documents.secondaryDoc) {
      Y.applyUpdateV2(this.documents.secondaryDoc, updateArray, origin);
    } else {
      // Default: apply to main doc
      Y.applyUpdateV2(this.documents.mainDoc, updateArray, origin);
    }
  }

  /**
   * Get state as update for initial sync
   */
  getStateAsUpdate(): Uint8Array {
    if (!this.documents) return new Uint8Array();
    return Y.encodeStateAsUpdateV2(this.documents.mainDoc);
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
   * NEW: Also destroys secondary doc if it exists
   */
  destroy(): void {
    if (this.documents) {
      this.documents.awareness.destroy();
      this.documents.mainDoc.destroy();

      // NEW: Destroy secondary doc if it exists
      if (this.documents.secondaryDoc) {
        this.documents.secondaryDoc.destroy();
      }

      this.documents = null;
    }
  }
}
