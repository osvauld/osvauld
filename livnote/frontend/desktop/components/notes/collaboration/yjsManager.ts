import * as Y from "yjs";
import { Awareness } from "y-protocols/awareness";
import type {
  CommentThread,
  ImageMetadata,
  UserInfo
} from "../../../types/notes.types";

export interface YjsDocuments {
  mainDoc: Y.Doc;
  imageDoc: Y.Doc;
  type: Y.XmlFragment;
  commentsMap: Y.Map<CommentThread>;
  imagesMap: Y.Map<ImageMetadata>;
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
    const imagesMap = imageDoc.getMap<ImageMetadata>("images");
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
    if (!this.documents) {
      return;
    }

    const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);
    const targetDoc = docType === 'images' ? this.documents.imageDoc : this.documents.mainDoc;

    Y.applyUpdate(targetDoc, updateArray, origin);
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
  destroy(): void {
    if (this.documents) {
      this.documents.mainDoc.destroy();
      this.documents.imageDoc.destroy();
      this.documents = null;
    }
  }

  /**
   * Check if documents are initialized
   */
  isInitialized(): boolean {
    return this.documents !== null;
  }
}
