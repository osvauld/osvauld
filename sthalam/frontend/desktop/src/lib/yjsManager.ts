import * as Y from "yjs";
import { applyAwarenessUpdate, Awareness, encodeAwarenessUpdate } from "y-protocols/awareness";
import type { UserInfo, Collaborator } from "../types/blocksuite.types";

export interface YjsDocuments {
  mainDoc: Y.Doc;
  blocks: Y.Map<any>;
  viewport: Y.Map<any>;
  awareness: Awareness;
}

export interface YjsManagerConfig {
  clientId: number;
  onUpdate?: (update: Uint8Array, origin: any) => void;
  onAwarenessChange?: (changes: any, origin: string) => void;
}

/**
 * Manages YJS document lifecycle and synchronization for blocksuite
 * Separated from UI concerns - follows livnote pattern
 */
export class YjsManager {
  private documents: YjsDocuments | null = null;
  private config: YjsManagerConfig;
  private cachedDataState: any = null;

  constructor(config: YjsManagerConfig) {
    this.config = config;
  }

  /**
   * Initialize YJS documents and structures
   * IMPORTANT: Always destroys old documents first to ensure clean slate (livnote pattern)
   */
  initialize(): YjsDocuments {
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

    // Set up update listener
    if (this.config.onUpdate) {
      mainDoc.on("updateV2", (update: Uint8Array, origin: any) => {
        if (origin !== "sync" && origin !== "loading") {
          this.config.onUpdate!(update, origin);
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

        // Sync collaborators whenever awareness changes
        this.syncCollaboratorsToDataState();
      });
    }

    this.documents = {
      mainDoc,
      blocks,
      viewport,
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
   * Get state vector for sync
   */
  getStateVector(): Uint8Array {
    if (!this.documents) return new Uint8Array();
    return Y.encodeStateVector(this.documents.mainDoc);
  }

  /**
   * Apply update from remote
   */
  applyUpdate(update: Uint8Array | number[], origin: any = 'sync'): void {
    if (!this.documents) return;
    const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);
    Y.applyUpdateV2(this.documents.mainDoc, updateArray, origin);
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
   * Sync collaborators from awareness state to dataState
   */
  public syncCollaboratorsToDataState(): void {
    if (!this.documents) return;

    const awareness = this.documents.awareness;
    const states = awareness.getStates();
    const collaborators: Collaborator[] = [];

    states.forEach((state: any, clientId: number) => {
      // Skip our own client
      if (clientId === this.config.clientId) return;

      if (state && state.user && state.user.name) {
        collaborators.push({
          id: clientId.toString(),
          name: state.user.name,
          color: state.user.color,
          clientId: clientId
        });
      }
    });

    // Update dataState if cached
    if (this.cachedDataState) {
      this.cachedDataState.updateCollaborators(collaborators);
    }
  }

  /**
   * Cache dataState for collaborator updates
   */
  setCachedDataState(dataState: any): void {
    this.cachedDataState = dataState;
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
      this.documents.mainDoc.destroy();
      this.documents = null;
    }
  }
}
