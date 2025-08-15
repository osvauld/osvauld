import * as Y from "yjs";
import { applyAwarenessUpdate, Awareness, encodeAwarenessUpdate } from "y-protocols/awareness";
import type {
  CommentThread,
  ImageAsset,
  UserInfo
} from "../../types/notes.types";

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
    this.destroy();
    const mainDoc = new Y.Doc({
      gc: true,
      gcFilter: () => false,
    });
    const imageDoc = new Y.Doc({
      gc: true,
      gcFilter: () => false
    });

    mainDoc.clientID = this.config.clientId;
    imageDoc.clientID = this.config.clientId;

    const type = mainDoc.getXmlFragment("prosemirror");
    const commentsMap = mainDoc.getMap<CommentThread>("comments");
    const metadata = mainDoc.getMap("metadata");
    const imagesMap = imageDoc.getMap<ImageAsset>("images");
    const awareness = new Awareness(mainDoc);

    if (this.config.onUpdate) {
      mainDoc.on("updateV2", (update: Uint8Array, origin: any) => {
        if (origin !== "sync" && origin !== "loading") {
          this.config.onUpdate!(update, origin, "main");
        }
      });
      imageDoc.on("updateV2", (update: Uint8Array, origin: any) => {
        if (origin !== "sync" && origin !== "loading") {
          this.config.onUpdate!(update, origin, "images");
        }
      });
    }

    if (this.config.onAwarenessChange) {
      awareness.on('change', async (changes: { added: number[], updated: number[], removed: number[] }, origin: string) => {

        if (origin === 'local') {
          try {
            // Get all client IDs that changed (process raw data here)
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

        // Sync collaborators whenever awareness changes
        this.syncCollaboratorsToDataState();
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
    if (!this.documents) return;
    this.documents.awareness.setLocalState({
      user: userInfo
    });
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
  applyUpdate(update: Uint8Array | number[], docType: 'main' | 'images' = 'main', origin: any = 'sync'): void {
    if (!this.documents) return;
    const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);
    const targetDoc = docType === 'images' ? this.documents.imageDoc : this.documents.mainDoc;
    Y.applyUpdateV2(targetDoc, updateArray, origin);
  }
  /**
   * Get state as update
   */
  getStateAsUpdate(docType: 'main' | 'images' = 'main'): Uint8Array {
    if (!this.documents) return new Uint8Array();

    const targetDoc = docType === 'images' ? this.documents.imageDoc : this.documents.mainDoc;
    return Y.encodeStateAsUpdateV2(targetDoc);
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
   * Sync collaborators from awareness state to dataState
   */
  public syncCollaboratorsToDataState(): void {
    if (!this.documents) return;

    const awareness = this.documents.awareness;
    const states = awareness.getStates();
    const collaborators: any[] = [];

    // console.log("🔍 Awareness states:", states);
    // console.log("🔍 Current client ID:", this.config.clientId);

    states.forEach((state: any, clientId: number) => {
      // Skip our own client
      if (clientId === this.config.clientId) return;
      
      // console.log(`🔍 Client ${clientId} state:`, state);
      
      // Check if state has user info and is not null/undefined
      if (state && state.user && state.user.name) {
        collaborators.push({
          id: clientId.toString(),
          name: state.user.name,
          color: state.user.color,
          clientId: clientId
        });
      }
    });

    // console.log("🔍 Final collaborators:", collaborators);

    // Import dataState and update collaborators
    import("../../state").then(({ dataState }) => {
      dataState.updateCollaborators(collaborators);
    }).catch(error => {
      console.error("Error updating collaborators:", error);
    });
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
