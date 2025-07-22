import * as Y from "yjs";
import { Awareness, encodeAwarenessUpdate } from "y-protocols/awareness";
import type {
  CommentThread,
  ImageMetadata,
  ImageAsset,
  UserInfo
} from "../../../types/notes.types";

export interface YjsDocuments {
  mainDoc: Y.Doc;
  imageDoc: Y.Doc;
  type: Y.XmlFragment;
  commentsMap: Y.Map<CommentThread>;
  imagesMap: Y.Map<ImageAsset>;
  metadata: Y.Map<any>;
  awareness: Awareness;
}

export interface YjsManagerConfig {
  clientId: number;
  onUpdate?: (update: Uint8Array, origin: any, docType: 'main' | 'images') => void;
  onAwarenessChange?: (changes: any, origin: string) => void;
}

/**
 * Manages YJS document lifecycle and synchronization
 * Separated from ProseMirror concerns
 */
export class YjsManager {
  private documents: YjsDocuments | null = null;
  private config: YjsManagerConfig;
  private afterTransactionsHandler: (() => void) | null = null;
  constructor(config: YjsManagerConfig) {
    this.config = config;
  }

  /**
   * Initialize YJS documents and structures
   */
  initialize(): YjsDocuments {
    // Clean up any existing documents
    this.destroy();

    // Create new documents
    const mainDoc = new Y.Doc();
    const imageDoc = new Y.Doc();

    mainDoc.clientID = this.config.clientId;

    // Initialize YJS structures
    const type = mainDoc.getXmlFragment("prosemirror");
    const commentsMap = mainDoc.getMap<CommentThread>("comments");
    const metadata = mainDoc.getMap("metadata");
    const imagesMap = imageDoc.getMap<ImageAsset>("images");
    const awareness = new Awareness(mainDoc);

    // Set up update handlers
    if (this.config.onUpdate) {
      mainDoc.on("update", (update: Uint8Array, origin: any) => {
        if (origin !== "sync" && origin !== "loading") {
          this.config.onUpdate!(update, origin, "main");
        }
      });
      imageDoc.on("update", (update: Uint8Array, origin: any) => {
        if (origin !== "sync" && origin !== "loading") {
          this.config.onUpdate!(update, origin, "images");
        }
      });
    }

    // Set up awareness handler
    if (this.config.onAwarenessChange) {
      awareness.on('change', async (changes: { added: number[], updated: number[], removed: number[] }, origin: string) => {
        console.log("YJS Awareness change:", { changes, origin });

        if (origin === 'local') {
          try {
            // Get all client IDs that changed (process raw data here)
            const clients = [...changes.added, ...changes.updated, ...changes.removed];
            if (clients.length === 0) {
              console.log("No clients changed, skipping awareness update");
              return;
            }

            console.log("Processing awareness update for clients:", clients);

            // Import and encode awareness update with proper client data
            const encodedUpdate = encodeAwarenessUpdate(awareness, clients);

            console.log("Encoded awareness update size:", encodedUpdate.length, "bytes");

            // Pass the properly encoded update to the callback, not raw changes
            this.config.onAwarenessChange!(encodedUpdate, origin);

          } catch (error) {
            console.error("Error processing awareness update in YjsManager:", error);
          }
        }

        // this.syncCollaboratorsToDataState();
      });
    }

    this.documents = {
      mainDoc,
      imageDoc,
      type,
      commentsMap,
      imagesMap,
      metadata,
      awareness
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
    console.log("setting user info");
    if (!this.documents) return;
    console.log("not returning");
    console.log(this.documents.awareness.getLocalState(), "local state");
    this.documents.awareness.setLocalState({
      user: userInfo
    });
    console.log(this.documents.awareness.getLocalState(), "local state");
  }

  /**
   * Get or set metadata
   */
  getMetadata(key: string): any {
    return this.documents?.metadata.get(key);
  }

  setMetadata(key: string, value: any): void {
    this.documents?.metadata.set(key, value);
  }

  /**
   * Apply updates from remote
   */
  // Add to the applyUpdate method in YjsManager
  applyUpdate(update: Uint8Array | number[], docType: 'main' | 'images' = 'main', origin: any = 'sync'): void {
    const startTime = performance.now();

    if (!this.documents) return;

    const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);
    const targetDoc = docType === 'images' ? this.documents.imageDoc : this.documents.mainDoc;

    Y.applyUpdate(targetDoc, updateArray, origin);

    const endTime = performance.now();
    const updateTime = endTime - startTime;

    console.log(`[PERF] YJS ${docType} update applied in ${updateTime}ms, size: ${updateArray.length} bytes`);

  }
  /**
   * Get state as update
   */
  getStateAsUpdate(docType: 'main' | 'images' = 'main'): Uint8Array {
    if (!this.documents) return new Uint8Array();

    const targetDoc = docType === 'images' ? this.documents.imageDoc : this.documents.mainDoc;
    return Y.encodeStateAsUpdate(targetDoc);
  }

  /**
   * Get state vector for sync
   */
  getStateVector(docType: 'main' | 'images' = 'main'): Uint8Array {
    if (!this.documents) return new Uint8Array();

    const targetDoc = docType === 'images' ? this.documents.imageDoc : this.documents.mainDoc;
    return Y.encodeStateVector(targetDoc);
  }
  /**
   * Apply awareness update from remote clients
   */
  async applyAwarenessUpdate(update: Uint8Array | number[], sender: number): Promise<void> {
    if (!this.documents || sender === this.config.clientId) {
      console.log("Skipping awareness update: no documents or sender is self");
      return; // Don't apply our own updates
    }

    try {
      const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);

      if (updateArray.length === 0) {
        console.warn("⚠️ Received empty awareness update");
        return;
      }

      console.log(`📡 Applying awareness update from client ${sender}, size: ${updateArray.length}`);

      // Import the applyAwarenessUpdate function from y-protocols
      const { applyAwarenessUpdate } = await import('y-protocols/awareness');

      // Apply the awareness update with 'remote' origin to prevent loops
      applyAwarenessUpdate(this.documents.awareness, updateArray, 'remote');

      console.log("✅ Awareness update applied successfully");
      console.log("📊 Current awareness states after update:", Array.from(this.documents.awareness.getStates().entries()));

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
      // Remove afterAllTransactions listener if it exists
      if (this.afterTransactionsHandler) {
        this.documents.mainDoc.off("afterAllTransactions", this.afterTransactionsHandler);
        this.afterTransactionsHandler = null;
      }

      this.documents.mainDoc.destroy();
      this.documents.imageDoc.destroy();
      this.documents = null;
    }
  }
}
