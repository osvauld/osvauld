import * as Y from "yjs";
import { Awareness } from "y-protocols/awareness";
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
      awareness.on('change', this.config.onAwarenessChange);
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
   * Clean up and destroy documents
   */


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
