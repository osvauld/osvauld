import { sendMessage } from '../utils/helper';
import { createEmptyBlocksuiteDoc } from '../utils/blocksuiteUtils';
import type { Website, Resource, UserDetails } from '../types';
import { listen } from '@tauri-apps/api/event';
import { dataState as oldDataState } from '../store.svelte';

/**
 * Data State Management for Sthalam
 * Inspired by livnote's architecture but adapted for website builder
 */
class DataState {
  // Websites (folders)
  websites = $state<Website[]>([{ id: "all", name: "All Websites" }]);
  currentWebsite = $state<Website>({ id: "all", name: "All Websites" });

  // Resources
  resources = $state<Resource[]>([]);
  currentResourceId = $state<string | null>(null);
  currentResourceData = $state<Resource | null>(null);

  // User data
  userDetails = $state<UserDetails | null>(null);
  clientId: number = 0;

  // Sovereign node
  sovereignNodeId = $state<string | null>(null);

  // UI state
  isDataLoading = $state<boolean>(false);

  // Event listeners
  private _unlisteners: Array<() => void> = [];

  // Derived: Filter resources by current website
  filteredResources = $derived.by(() => {
    if (this.currentWebsite.id === "all") {
      return this.resources;
    }
    return this.resources.filter(r => r.websiteId === this.currentWebsite.id);
  });

  /**
   * Initialize state - fetch websites and user details
   */
  async initializeState() {
    this.isDataLoading = true;

    // Clean up event listeners (but don't clear data)
    this.cleanupReactiveUpdates();

    try {
      await Promise.all([
        this.getUserDetails(),
        this.fetchWebsites(),
        this.setupReactiveUpdates()
      ]);

      // Fetch all resources
      await this.fetchAllResources();
    } catch (error) {
      console.error("Error initializing state:", error);
    } finally {
      this.isDataLoading = false;
    }
  }

  /**
   * Fetch all websites/folders
   */
  async fetchWebsites() {
    try {
      const resp = await sendMessage("getFolder");
      const websites: Website[] = resp.map((item: any) => ({
        id: item.id || "",
        name: item.name || "",
        description: item.description,
        default: item.default
      }));
      this.websites = [{ id: "all", name: "All Websites" }, ...websites];
    } catch (error) {
      console.error("Error fetching websites:", error);
      this.websites = [{ id: "all", name: "All Websites" }];
    }
  }

  /**
   * Fetch all resources
   * Uses emit_all_resources which sends resource-added events that our listeners will handle
   */
  async fetchAllResources() {
    this.isDataLoading = true;
    try {
      console.log("📥 Triggering resource emission...");

      // emit_all_resources will emit resource-added events for each resource
      // Our event listener (handleResourceAdded) will add them to this.resources
      // When complete, the backend emits resources-loading-complete
      await sendMessage("emitAllResources");

      console.log("✅ Resource emission triggered (resources will arrive via events)");
    } catch (error) {
      console.error("❌ Error triggering resource emission:", error);
      this.isDataLoading = false;
    }
    // Note: isDataLoading will be set to false by handleResourcesLoadingComplete
  }

  /**
   * Get user details
   */
  async getUserDetails() {
    try {
      this.userDetails = await sendMessage('getUserDetails');
      this.clientId = this.getClientId();
    } catch (error) {
      console.error("Error getting user details:", error);
    }
  }

  /**
   * Generate client ID from device ID
   */
  getClientId(): number {
    if (!this.userDetails) {
      return Math.floor(Math.random() * 1000000);
    }
    try {
      const decoded = atob(this.userDetails.deviceId);
      const bytes = new Uint8Array(decoded.length);
      for (let i = 0; i < decoded.length; i++) {
        bytes[i] = decoded.charCodeAt(i);
      }
      const hex = Array.from(bytes.slice(0, 4))
        .map(b => b.toString(16).padStart(2, '0'))
        .join('');

      return parseInt(hex, 16);
    } catch (error) {
      console.error("Error generating client ID:", error);
      return Math.floor(Math.random() * 1000000);
    }
  }

  /**
   * Switch to a different website/folder
   */
  async switchWebsite(website: Website) {
    this.currentWebsite = website;

    // Clear current resource selection when switching websites
    if (this.currentResourceId) {
      this.clearCurrentResource();
    }
  }

  /**
   * Add a new website/folder
   */
  async addWebsite(name: string, description?: string) {
    try {
      const website = await sendMessage("addFolder", {
        name,
        description: description || "" // Backend requires description field
      });

      this.websites = [...this.websites, website];
      return website;
    } catch (error) {
      console.error("Error creating website:", error);
      throw error;
    }
  }

  /**
   * Add a new resource to a website
   */
  async addResource(websiteId: string, title: string = "Untitled") {
    try {
      console.log("📝 Creating resource:", { websiteId, title, clientId: this.clientId });

      // Create empty BlockSuite document with initial template blocks
      const blocksuiteContent = createEmptyBlocksuiteDoc(
        this.clientId,
        title
      );

      console.log("✅ BlockSuite content created:", {
        title: blocksuiteContent.title,
        updatesLength: blocksuiteContent.main_doc.length,
        clientId: blocksuiteContent.client_id
      });

      // Send to backend
      const resource = await sendMessage("addCredential", {
        resourcePayload: JSON.stringify(blocksuiteContent),
        folderId: websiteId,
        resourceType: "website"
      });

      console.log("✅ Resource created by backend:", resource);

      // Don't manually add to array - backend will emit resource-added event
      // which will add it via handleResourceAdded

      // Switch to the new resource (fetches full data, doesn't need preview)
      await this.switchResource(resource.id);

      return resource;
    } catch (error) {
      console.error("❌ Error creating resource:", error);
      throw error;
    }
  }

  /**
   * Switch to a different resource
   * Following livnote's pattern: save current, fetch new data, load coordinator
   */
  async switchResource(resourceId: string | null) {
    // Save the current resource before switching away from it (non-blocking)
    const currentResourceId = this.currentResourceId;
    if (currentResourceId && currentResourceId !== resourceId) {
      // Fire and forget - save in background without blocking the switch
      this.saveCurrentResource(currentResourceId).catch(error => {
        console.error("❌ Error saving resource before switching:", error);
      });
    }

    if (resourceId) {
      try {
        console.log("🔄 Switching to resource:", resourceId);

        // Fetch full resource data from backend
        const resource = await sendMessage("getCredential", {
          resourceId: resourceId
        });

        console.log("✅ Resource data fetched:", {
          id: resource.id,
          hasData: !!resource.data,
          dataType: typeof resource.data,
          resourceKeys: Object.keys(resource),
          fullResource: resource
        });

        // Store the full resource data (includes BlockSuite content)
        this.currentResourceData = resource;

        // Set the resource ID in oldDataState (for coordinator events)
        oldDataState.setCurrentResourceId(resourceId);

        // Load the resource data into the coordinator FIRST (livnote pattern!)
        // This reinitializes Yjs documents with fresh data
        const coordinator = oldDataState.getBlocksuiteCoordinator();
        if (!coordinator) {
          console.error("❌ No coordinator available!");
          return;
        }

        // Parse the resource data - backend returns data as a JSON string
        // Similar to livnote which also parses the note data
        let blocksuiteData;
        if (resource.data) {
          if (typeof resource.data === 'string') {
            blocksuiteData = JSON.parse(resource.data);
          } else {
            blocksuiteData = resource.data;
          }
        } else {
          // Fallback: use the whole resource if no data field
          blocksuiteData = resource;
        }

        console.log("🔧 Loading resource data into coordinator...", {
          hasMainDoc: !!blocksuiteData.main_doc,
          hasUpdates: !!blocksuiteData.main_doc?.updates
        });
        coordinator.loadBlocksuite(blocksuiteData);
        console.log("✅ Coordinator loaded with resource data");

        // IMPORTANT: Set currentResourceId AFTER loadBlocksuite completes
        // This ensures $effect in WebsiteBuilder gets the FRESH documents
        this.currentResourceId = resourceId;

        // Update current website to match the resource's website
        const resourcePreview = this.resources.find(r => r.id === resourceId);
        if (resourcePreview?.websiteId) {
          const website = this.websites.find(w => w.id === resourcePreview.websiteId);
          if (website) {
            this.currentWebsite = website;
          }
        }

        console.log("✅ Resource switched successfully");
      } catch (error) {
        console.error("❌ Error switching resource:", error);
      }
    } else {
      this.clearCurrentResource();
    }
  }

  /**
   * Save the currently active resource
   * Gets content from coordinator and saves to backend
   */
  async saveCurrentResource(resourceId: string) {
    try {
      const coordinator = oldDataState.getBlocksuiteCoordinator();
      if (!coordinator) {
        console.warn("⚠️ No coordinator available for saving");
        return;
      }

      console.log("💾 Saving current resource:", resourceId);

      // Get the current blocksuite content from coordinator
      const blocksuiteContent = coordinator.saveBlocksuite();

      console.log("📦 BlockSuite content to save:", {
        hasMainDoc: !!blocksuiteContent?.main_doc,
        hasUpdates: !!blocksuiteContent?.main_doc?.updates,
        updatesLength: blocksuiteContent?.main_doc?.updates?.length,
        lastModified: blocksuiteContent?.last_modified
      });

      // Update lastModified timestamp locally first
      const timestamp = Date.now();
      this.updateResourceLastModified(resourceId, timestamp);

      await sendMessage("updateCredential", {
        id: resourceId,
        data: JSON.stringify(blocksuiteContent)
      });

      console.log("✅ Resource saved:", resourceId);
    } catch (error) {
      console.error("❌ Error saving current resource:", error);
      throw error;
    }
  }

  /**
   * Save current resource (legacy method - use saveCurrentResource instead)
   */
  async saveResource(resourceId: string, content: any) {
    try {
      // Update lastModified timestamp locally first
      const timestamp = Date.now();
      this.updateResourceLastModified(resourceId, timestamp);

      await sendMessage("updateCredential", {
        id: resourceId,
        data: JSON.stringify(content)
      });
    } catch (error) {
      console.error("Error saving resource:", error);
      throw error;
    }
  }

  /**
   * Update resource's lastModified timestamp in the resources array
   */
  updateResourceLastModified(resourceId: string, timestamp: number) {
    const index = this.resources.findIndex(r => r.id === resourceId);
    if (index !== -1) {
      this.resources[index].lastModified = timestamp;
    }
  }

  /**
   * Get a resource by ID
   */
  getResourceById(id: string): Resource | undefined {
    return this.resources.find(r => r.id === id);
  }

  /**
   * Clear current resource
   */
  clearCurrentResource() {
    this.currentResourceId = null;
    this.currentResourceData = null;

    // Also clear in oldDataState
    oldDataState.setCurrentResourceId(null);
  }

  /**
   * Set sovereign node ID
   */
  setSovereignNodeId(userId: string) {
    this.sovereignNodeId = userId;
    console.log("✅ Sovereign node ID set:", userId);
  }

  /**
   * Get sovereign node ID
   */
  getSovereignNodeId(): string | null {
    return this.sovereignNodeId;
  }

  /**
   * Check if a resource is published to sovereign node
   */
  async isResourcePublished(resourceId: string): Promise<boolean> {
    if (!this.sovereignNodeId) return false;

    // TODO: Query backend to check if resource is shared with sovereign node
    // For now, we'll implement this later when we have the backend API
    return false;
  }

  /**
   * Publish resource to sovereign node
   */
  async publishToSovereignNode(resourceId: string) {
    if (!this.sovereignNodeId) {
      throw new Error("No sovereign node connected. Please add a sovereign node first.");
    }

    try {
      console.log("📤 Publishing resource to sovereign node:", resourceId);

      await sendMessage("shareResource", {
        userId: this.sovereignNodeId,
        resourceId: resourceId,
        permissions: { read: true, write: true }
      });

      console.log("✅ Resource published successfully to sovereign node");
    } catch (error) {
      console.error("❌ Failed to publish resource:", error);
      throw error;
    }
  }

  /**
   * Update website on sovereign node (re-sync)
   */
  async updateWebsiteOnSovereignNode(resourceId: string) {
    // For now, this just re-triggers the share which will sync
    // Later we can add a dedicated update endpoint
    await this.publishToSovereignNode(resourceId);
  }

  /**
   * Clear all state (for logout)
   */
  clearAllState() {
    // Clean up event listeners first
    this.cleanupReactiveUpdates();

    this.websites = [{ id: "all", name: "All Websites" }];
    this.currentWebsite = { id: "all", name: "All Websites" };
    this.resources = [];
    this.currentResourceId = null;
    this.currentResourceData = null;
    this.userDetails = null;
    this.clientId = 0;
    this.sovereignNodeId = null;
    this.isDataLoading = false;
  }

  /**
   * Handle resource-added event from backend
   * Receives ResourcePreview from emit_all_resources
   */
  async handleResourceAdded(event: any) {
    try {
      const resourceData = event.payload;

      // Map ResourcePreview to Resource type
      const newResource: Resource = {
        id: resourceData.id || "",
        title: resourceData.title || "Untitled",
        resourceType: 'website' as const,
        websiteId: resourceData.folder_id || resourceData.folderId || "",
        lastModified: resourceData.last_modified || resourceData.lastModified || Date.now(),
        favourite: resourceData.favourite || false,
        preview: resourceData.preview || "",
      };

      // Check if resource already exists (avoid duplicates)
      const exists = this.resources.some(r => r.id === newResource.id);
      if (!exists) {
        this.resources = [...this.resources, newResource];
        console.log("✅ Resource added:", newResource.title || newResource.id);
      }
    } catch (error) {
      console.error("❌ Error handling resource-added event:", error);
    }
  }

  /**
   * Handle resource-update event from backend
   * Receives ResourcePreview when a resource is updated
   * Only updates the preview in the resources array, does NOT refetch full data
   */
  async handleResourceUpdate(event: any) {
    try {
      const resourceData = event.payload;

      // Find and update the resource preview in the array
      const index = this.resources.findIndex(r => r.id === resourceData.id);
      if (index !== -1) {
        const existingResource = this.resources[index];

        // Preserve local lastModified if it's newer than backend version
        // This handles the case where we just saved locally but backend hasn't updated yet
        let lastModified = resourceData.last_modified || resourceData.lastModified || Date.now();
        if (existingResource.lastModified && resourceData.lastModified) {
          lastModified = Math.max(existingResource.lastModified, resourceData.lastModified);
        } else if (existingResource.lastModified && !resourceData.lastModified) {
          lastModified = existingResource.lastModified;
        }

        this.resources[index] = {
          ...existingResource,
          title: resourceData.title || existingResource.title,
          lastModified,
          favourite: resourceData.favourite ?? existingResource.favourite,
          preview: resourceData.preview || existingResource.preview,
          websiteId: resourceData.folder_id || existingResource.websiteId,
        };
        console.log("✅ Resource preview updated:", this.resources[index].title);
      } else {
        // Resource doesn't exist in our array - add it
        const newResource: Resource = {
          id: resourceData.id || "",
          title: resourceData.title || "Untitled",
          resourceType: 'website' as const,
          websiteId: resourceData.folder_id || "",
          lastModified: resourceData.last_modified || resourceData.lastModified || Date.now(),
          favourite: resourceData.favourite || false,
          preview: resourceData.preview || "",
        };
        this.resources = [...this.resources, newResource];
        console.log("✅ Resource added via update event:", newResource.title);
      }
    } catch (error) {
      console.error("❌ Error handling resource-update event:", error);
    }
  }

  /**
   * Handle folders-added-notification event from backend
   */
  async handleFoldersAdded(event: any) {
    try {
      console.log("📥 Folders added event:", event.payload);
      const { folders } = event.payload;

      if (folders && Array.isArray(folders)) {
        for (const folder of folders) {
          // Check if folder already exists
          const exists = this.websites.some(w => w.id === folder.id);
          if (!exists) {
            this.websites = [...this.websites, {
              id: folder.id,
              name: folder.name,
              description: folder.description,
              default: folder.default
            }];
            console.log("✅ Folder added:", folder.name);
          }
        }
      }
    } catch (error) {
      console.error("❌ Error handling folders-added event:", error);
    }
  }

  /**
   * Handle resources-loading-complete event from backend
   */
  handleResourcesLoadingComplete() {
    console.log("✅ Resources loading complete!");
    this.isDataLoading = false;
  }

  /**
   * Set up real-time event listeners for resource/folder updates
   */
  async setupReactiveUpdates() {
    console.log("🔌 Setting up reactive updates...");

    // Clean up any existing listeners first
    this.cleanupReactiveUpdates();

    try {
      const resourceAddedUnlisten = await listen("resource-added", this.handleResourceAdded.bind(this));
      const resourceUpdateUnlisten = await listen("resource-update", this.handleResourceUpdate.bind(this));
      const foldersAddedUnlisten = await listen("folders-added-notification", this.handleFoldersAdded.bind(this));
      const resourcesCompleteUnlisten = await listen("resources-loading-complete", this.handleResourcesLoadingComplete.bind(this));

      this._unlisteners.push(
        resourceAddedUnlisten,
        resourceUpdateUnlisten,
        foldersAddedUnlisten,
        resourcesCompleteUnlisten
      );

      console.log("✅ Reactive updates set up successfully");
    } catch (error) {
      console.error("❌ Error setting up reactive updates:", error);
    }
  }

  /**
   * Clean up event listeners
   */
  cleanupReactiveUpdates() {
    if (this._unlisteners && this._unlisteners.length > 0) {
      console.log("🧹 Cleaning up event listeners...");
      for (const unlisten of this._unlisteners) {
        unlisten();
      }
      this._unlisteners = [];
    }
  }
}

export const dataState = new DataState();
