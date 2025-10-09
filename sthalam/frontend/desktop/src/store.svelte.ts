import { sendMessage, getUserDetails } from "./utils/helper";
import { listen, emit } from "@tauri-apps/api/event";
import { BlocksuiteCoordinator } from "./lib/blocksuiteCoordinator";
import type { UserDetails, Collaborator } from "./types/blocksuite.types";

/**
 * Data State for Blocksuite
 * Follows livnote's dataState pattern
 */
class DataState {
  userDetails = $state<UserDetails | null>(null);
  private blocksuiteCoordinator: BlocksuiteCoordinator | null = null;
  currentResourceId = $state<string | null>(null);
  collaborators = $state<Collaborator[]>([]);
  clientId: number = 0;
  private _unlisteners: Array<() => void> = [];
  isDataLoading = $state<boolean>(false);

  getBlocksuiteCoordinator(): BlocksuiteCoordinator | null {
    return this.blocksuiteCoordinator;
  }

  getCurrentResourceId(): string | null {
    return this.currentResourceId;
  }

  setCurrentResourceId(resourceId: string | null) {
    this.currentResourceId = resourceId;
  }

  updateCollaborators(newCollaborators: Collaborator[]) {
    this.collaborators = newCollaborators;
  }

  private generateUserColor(): string {
    const colors = [
      "#FF5630",
      "#FFAB00",
      "#36B37E",
      "#00B8D9",
      "#6554C0",
      "#FF7452",
    ];
    return colors[Math.floor(Math.random() * colors.length)];
  }

  async getUserDetails() {
    this.userDetails = await getUserDetails();
    this.clientId = this.getClientId();
  }

  getClientId(): number {
    if (!this.userDetails) {
      throw new Error("User details not available");
    }
    const decoded = atob(this.userDetails.deviceId);
    const bytes = new Uint8Array(decoded.length);
    for (let i = 0; i < decoded.length; i++) {
      bytes[i] = decoded.charCodeAt(i);
    }
    const hex = Array.from(bytes.slice(0, 4))
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');

    return parseInt(hex, 16);
  }

  private createCoordinator() {
    if (this.blocksuiteCoordinator) {
      this.blocksuiteCoordinator.destroy();
    }

    if (!this.userDetails) {
      throw new Error("User details not available for coordinator creation");
    }

    const userInfo = {
      name: this.userDetails.username,
      color: this.generateUserColor(),
      id: this.clientId,
      userId: this.userDetails.userId,
    };

    this.blocksuiteCoordinator = new BlocksuiteCoordinator({
      onCollaborationUpdate: async (update) => {
        if (!this.getCurrentResourceId()) return;
        await emit("sync-update", {
          update: Array.from(update),
          clientID: this.clientId,
          resource_id: this.getCurrentResourceId(),
        });
      },
      onAwarenessUpdate: async (changes) => {
        if (!this.getCurrentResourceId()) return;
        await emit("awareness-update", {
          update: Array.from(changes),
          clientID: this.clientId,
          resource_id: this.getCurrentResourceId(),
        });
      },
      userInfo,
    });

    this.blocksuiteCoordinator.setCachedDataState(this);
  }

  async initializeState() {
    this.isDataLoading = true;

    // Clear any existing state and event listeners first
    this.clearAllState();

    await Promise.all([
      this.getUserDetails(),
      this.setupReactiveUpdates()
    ]);

    this.createCoordinator();
    this.isDataLoading = false;
  }

  // Add a method to clear all state when logging out
  clearAllState() {
    this.currentResourceId = null;
    this.collaborators = [];

    // Clean up event listeners
    this.cleanupReactiveUpdates();

    // Clean up coordinator
    if (this.blocksuiteCoordinator) {
      this.blocksuiteCoordinator.destroy();
      this.blocksuiteCoordinator = null;
    }
  }

  async handleAwarenessUpdates(event: any) {
    try {
      const { resource_id, updates, client_id } = event.payload;
      const updatesArray = new Uint8Array(updates);
      const senderId = parseInt(client_id, 10);
      const coordinator = this.getBlocksuiteCoordinator();
      coordinator?.applyAwarenessUpdate(updatesArray, senderId);
    } catch (error) {
      console.error("Error handling awareness-updates:", error);
    }
  }

  async handleLiveUpdates(event: any) {
    try {
      const { resource_id, updates, client_id } = event.payload;
      if (!this.currentResourceId || this.currentResourceId !== resource_id) {
        return;
      }
      const updatesArray = new Uint8Array(updates);
      const senderId = parseInt(client_id, 10);
      const coordinator = this.getBlocksuiteCoordinator();
      coordinator?.applyRemoteUpdate(updatesArray, senderId);
    } catch (error) {
      console.error("Error handling live-updates:", error);
    }
  }

  async handleDocumentUpdates(event: any) {
    try {
      const { resource_id, updates } = event.payload;
      if (resource_id === this.currentResourceId) {
        const updatesJson = JSON.parse(updates);
        const documentUpdates = updatesJson.main_doc.updates;
        const documentUpdateArray = new Uint8Array(documentUpdates);
        const coordinator = this.getBlocksuiteCoordinator();
        coordinator?.applyRemoteUpdate(documentUpdateArray, updatesJson.client_id);
      }
    } catch (error) {
      console.error("Error handling document-updates:", error);
    }
  }

  async setupReactiveUpdates() {
    this._unlisteners = [];

    const documentUpdatesUnlisten = await listen("document-updates", this.handleDocumentUpdates.bind(this));
    const awarenessUpdatesUnlisten = await listen("awareness-updates", this.handleAwarenessUpdates.bind(this));
    const liveUpdatesUnlisten = await listen("live-updates", this.handleLiveUpdates.bind(this));

    this._unlisteners.push(
      documentUpdatesUnlisten,
      awarenessUpdatesUnlisten,
      liveUpdatesUnlisten,
    );
  }

  cleanupReactiveUpdates() {
    if (this._unlisteners) {
      for (const unlisten of this._unlisteners) {
        unlisten();
      }
      this._unlisteners = [];
    }
  }

  async saveBlocksuite(resourceId: string) {
    const coordinator = this.getBlocksuiteCoordinator();
    if (!coordinator) {
      throw new Error("Coordinator not available");
    }
    const blocksuiteContent = coordinator.saveBlocksuite();
    const stateVectors = coordinator.getStateVector();

    await sendMessage("updateCredential", {
      id: resourceId,
      data: JSON.stringify(blocksuiteContent),
    });

    emit("resource-update-complete", { id: resourceId, state_vectors: stateVectors });
  }

  async loadBlocksuite(resourceId: string) {
    this.setCurrentResourceId(resourceId);

    // Get blocksuite data from backend
    const resource = await sendMessage("getCredential", { resourceId });

    const coordinator = this.getBlocksuiteCoordinator();
    if (coordinator && resource) {
      coordinator.loadBlocksuite(JSON.parse(resource.data));
    }
  }
}

export const dataState = new DataState();
