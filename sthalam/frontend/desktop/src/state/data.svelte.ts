import { sendMessage } from '../utils/helper';
import { loroCoordinator } from '../shared/loro/loroCoordinator';
import type { Website, Resource, UserDetails } from '../types';
import { listen } from '@tauri-apps/api/event';
import { FOLDER_TEMPLATE, RESOURCE_TEMPLATE } from '../config/permissions';

/**
 * Clean Data State Management for Sthalam
 * Uses Loro CRDT for document management
 */
class DataState {
  // Websites (folders)
  websites = $state<Website[]>([{ id: "all", name: "All Websites" }]);
  currentWebsite = $state<Website>({ id: "all", name: "All Websites" });

  // Resources
  resources = $state<Resource[]>([]);
  currentResourceId = $state<string | null>(null);

  // User data
  userDetails = $state<UserDetails | null>(null);
  clientId: number = 0;

  // Sovereign node
  sovereignNodeId = $state<string | null>(null);

  // Loading states
  isDataLoading = $state<boolean>(false);

  // Event listeners cleanup
  private _unlisteners: Array<() => void> = [];

  /**
   * Generate client ID from device ID
   */
  private getClientId(): number {
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
      console.error('Failed to generate client ID:', error);
      return Math.floor(Math.random() * 1000000);
    }
  }

  /**
   * Initialize state - called by App.svelte on auth
   */
  async initializeState() {
    console.log('🔧 [DataState] initializeState() called');
    this.isDataLoading = true;

    // Clean up event listeners (but don't clear data)
    this.cleanupReactiveUpdates();

    try {
      console.log('🔄 [DataState] Getting user details and setting up...');

      // Get user details first
      this.userDetails = await sendMessage("getUserDetails", {});
      this.clientId = this.getClientId();

      console.log('✅ [DataState] User details loaded, clientId:', this.clientId);

      // Setup event listeners and load websites in parallel
      await Promise.all([
        this.setupEventListeners(),
        this.fetchWebsites()
      ]);

      console.log('✅ [DataState] Setup ready');

      // Fetch all resources (via events)
      await this.fetchAllResources();

    } catch (error) {
      console.error('❌ [DataState] Initialization failed:', error);
    } finally {
      this.isDataLoading = false;
      console.log('✅ [DataState] Initialization complete');
    }
  }

  /**
   * Setup backend event listeners
   */
  private async setupEventListeners() {
    try {
      // Listen for folder sync events
      const folderSyncUnlisten = await listen('folder-synced', (event: any) => {
        const { folderId, folderName } = event.payload;
        console.log('📁 [DataState] Folder synced:', folderId, folderName);

        // Refresh websites list to show new folder
        this.fetchWebsites();
      });
      this._unlisteners.push(folderSyncUnlisten);

      // Listen for resource sync events
      const resourceSyncUnlisten = await listen('resource-synced', (event: any) => {
        const metadata = event.payload;
        console.log('📄 [DataState] Resource synced:', metadata.id, metadata.title);

        // Add resource to local list if not already present
        const exists = this.resources.some((r: any) => r.id === metadata.id);
        if (!exists) {
          this.resources = [...this.resources, {
            id: metadata.id,
            title: metadata.title,
            websiteId: metadata.folderId,
            resourceType: metadata.resourceType,
            lastModified: metadata.lastModified,
          }];
        }
      });
      this._unlisteners.push(resourceSyncUnlisten);

      // Listen for resource update events (real-time CRDT merging)
      const resourceUpdateUnlisten = await listen('resource-updated', (event: any) => {
        const { resourceId, updates, metadata } = event.payload;
        console.log('🔄 [DataState] Resource updated:', resourceId, metadata);

        // Check if this is the currently active resource
        if (this.currentResourceId === resourceId) {
          console.log('✨ [DataState] Merging updates for currently active resource:', resourceId);

          // Merge updates into current Loro documents
          this.mergeIncomingUpdates(updates).catch((error) => {
            console.error('❌ [DataState] Failed to merge incoming updates:', error);
          });
        } else {
          console.log('ℹ️ [DataState] Updated resource is not currently active, skipping merge');
        }

        // Update lastModified in resources list if metadata includes timestamp
        if (metadata.timestamp) {
          this.updateResourceLastModified(resourceId, metadata.timestamp);
        }
      });
      this._unlisteners.push(resourceUpdateUnlisten);

      console.log('✅ [DataState] Event listeners setup complete');
    } catch (error) {
      console.error('❌ [DataState] Failed to setup listeners:', error);
    }
  }

  /**
   * Fetch websites from backend
   */
  async fetchWebsites() {
    try {
      const resp = await sendMessage("getFolder", {});
      const websites: Website[] = resp.map((item: any) => ({
        id: item.id || "",
        name: item.name || "",
        description: item.description,
        default: item.default
      }));
      this.websites = [{ id: "all", name: "All Websites" }, ...websites];
      console.log('📁 [DataState] Fetched websites:', websites.length);
    } catch (error) {
      console.error('❌ [DataState] Failed to fetch websites:', error);
      this.websites = [{ id: "all", name: "All Websites" }];
    }
  }

  /**
   * Fetch all resources metadata from backend
   */
  async fetchAllResources() {
    this.isDataLoading = true;
    try {
      console.log('📥 [DataState] Fetching all resources metadata...');
      const resourcesMetadata = await sendMessage("getAllResourcesMetadata", {});

      // Map to Resource type
      this.resources = resourcesMetadata.map((r: any) => ({
        id: r.id,
        title: r.title || 'Untitled',
        resourceType: r.resourceType || 'website',
        websiteId: r.folderId,
        lastModified: r.lastModified || Date.now(),
      }));

      console.log('✅ [DataState] Loaded', this.resources.length, 'resources');
    } catch (error) {
      console.error('❌ [DataState] Failed to fetch resources:', error);
    } finally {
      this.isDataLoading = false;
    }
  }

  /**
   * Switch to a different website
   */
  async switchWebsite(website: Website) {
    this.currentWebsite = website;
    this.currentResourceId = null;
    // Resources are already loaded via events, just filter by website
  }

  /**
   * Create a new website
   */
  async addWebsite(name: string) {
    try {
      const website = await sendMessage("addFolder", {
        name,
        description: "",
        folderTemplateJson: JSON.stringify(FOLDER_TEMPLATE)
      });
      this.websites = [...this.websites, website];
      console.log('✅ [DataState] Website created:', name);
      return website;
    } catch (error) {
      console.error('❌ [DataState] Failed to create website:', error);
      throw error;
    }
  }

  /**
   * Create a new resource with empty Loro document
   */
  async addResource(websiteId: string, title: string = "Untitled", resourceType: string = "website") {
    try {
      console.log('📝 [DataState] Creating resource:', { websiteId, title, resourceType });

      // Create empty document snapshots (doesn't mutate coordinator state)
      const snapshots = loroCoordinator.createEmptyDocumentSnapshots(title);

      // Create content structure for backend (ONLY Loro document arrays)
      const loroContent = {
        template_doc: Array.from(snapshots.template),
        content_doc: Array.from(snapshots.content),
        user_content_doc: Array.from(snapshots.userContent),
        collaborative_doc: Array.from(snapshots.collaborative),
        submissions_doc: Array.from(snapshots.submissions),
        static_assets: {} // Empty map for new resources
      };

      console.log('✅ [DataState] Loro content created');
      console.log('📦 [DataState] loroContent.static_assets (addResource):', loroContent.static_assets);

      // Create metadata (unencrypted) - includes title, timestamps, client info
      const metadata = {
        title: title,
        type: resourceType,
        client_id: this.clientId.toString(),
        last_modified: Date.now(),
        search: {
          docs: ["content_doc", "collaborative_doc"]
        }
      };

      // Send to backend - returns ResourceMetadata
      const resourceMetadata = await sendMessage("addCredential", {
        resourcePayload: JSON.stringify(loroContent),
        folderId: websiteId,
        resourceType: resourceType,
        permitTemplateJson: JSON.stringify(RESOURCE_TEMPLATE),
        metadataJson: JSON.stringify(metadata)
      });
      console.log("response we got back", resourceMetadata);

      console.log('✅ [DataState] Resource created:', resourceMetadata.id);

      // Add to resources list
      const newResource = {
        id: resourceMetadata.id,
        title: resourceMetadata.title,
        resourceType: resourceMetadata.resourceType,
        websiteId: resourceMetadata.folderId,
        lastModified: resourceMetadata.lastModified,
        favourite: resourceMetadata.favourite,
        preview: resourceMetadata.preview || ''
      };
      this.resources = [...this.resources, newResource];

      // Switch to the new resource (this loads the document into coordinator)
      await this.switchResource(resourceMetadata.id);

      return newResource;
    } catch (error) {
      console.error('❌ [DataState] Failed to create resource:', error);
      throw error;
    }
  }

  /**
   * Switch to a resource and load its Loro documents
   */
  async switchResource(resourceId: string) {
    try {
      console.log('📂 [DataState] Switching to resource:', resourceId);

      // Capture current resource's snapshots BEFORE loading new resource
      // This prevents race condition where coordinator data gets overwritten
      const currentResourceId = this.currentResourceId;
      if (currentResourceId && currentResourceId !== resourceId) {
        // Get snapshots synchronously from current coordinator state
        const snapshots = loroCoordinator.exportSnapshots();
        const contentMap = loroCoordinator.getContentMap();
        const title = contentMap.get('title') as string || 'Untitled';

        // Convert staticAssets to map format (asset_id -> base64_string)
        const staticAssetsMap: Record<string, string> = {};
        for (const [key, value] of Object.entries(snapshots.staticAssets || {})) {
          // Convert Uint8Array to base64 string in chunks to avoid stack overflow
          const uint8Array = value instanceof Uint8Array ? value : new Uint8Array(value);
          const chunkSize = 8192; // Process 8KB at a time
          let binaryString = '';

          for (let i = 0; i < uint8Array.length; i += chunkSize) {
            const chunk = uint8Array.subarray(i, Math.min(i + chunkSize, uint8Array.length));
            binaryString += String.fromCharCode(...Array.from(chunk));
          }

          const base64 = btoa(binaryString);
          staticAssetsMap[key] = base64;
        }

        console.log('🔍 [DataState] staticAssets from snapshots:', {
          keys: Object.keys(snapshots.staticAssets || {}),
          count: Object.keys(snapshots.staticAssets || {}).length,
          mapSize: Object.keys(staticAssetsMap).length
        });

        // Create Loro content structure
        const loroContent = {
          template_doc: Array.from(snapshots.template),
          content_doc: Array.from(snapshots.content),
          user_content_doc: Array.from(snapshots.userContent),
          collaborative_doc: Array.from(snapshots.collaborative),
          submissions_doc: Array.from(snapshots.submissions),
          static_assets: staticAssetsMap,
        };

        console.log('📦 [DataState] Captured snapshots for previous resource:', currentResourceId);
        console.log('📦 [DataState] loroContent.static_assets:', loroContent.static_assets);

        // Fire and forget - save in background
        sendMessage("updateCredential", {
          id: currentResourceId,
          data: JSON.stringify(loroContent)
        }).then(() => {
          console.log('✅ [DataState] Background save completed:', currentResourceId);
        }).catch(error => {
          console.error('❌ [DataState] Background save failed:', error);
        });
      }

      // Fetch full resource data from backend
      const resource = await sendMessage("getCredential", { resourceId });

      console.log('✅ [DataState] Resource fetched:', { id: resource.id, hasData: resource.data });

      // resource.data is already an object, no need to JSON.parse
      if (!resource.data) {
        console.error('❌ [DataState] No data in resource');
        return;
      }

      const loroData = resource.data;

      // Convert staticAssets map (base64 strings) back to Uint8Array object
      const staticAssetsObj: Record<string, Uint8Array> = {};
      if (loroData.static_assets && typeof loroData.static_assets === 'object') {
        for (const [assetId, base64Data] of Object.entries(loroData.static_assets)) {
          if (typeof base64Data === 'string') {
            // Convert base64 string back to Uint8Array
            const binaryString = atob(base64Data);
            const bytes = new Uint8Array(binaryString.length);
            for (let i = 0; i < binaryString.length; i++) {
              bytes[i] = binaryString.charCodeAt(i);
            }
            staticAssetsObj[assetId] = bytes;
          }
        }
        console.log('📦 [DataState] Loaded', Object.keys(loroData.static_assets).length, 'static assets');
      }

      // Load Loro documents from snapshots
      // Note: Documents may be missing for viewers (per Permit permission filtering)
      // Create empty snapshots for missing documents to avoid Loro import errors
      const snapshots = {
        template: loroData.template_doc
          ? new Uint8Array(loroData.template_doc)
          : loroCoordinator.createEmptyDocumentSnapshots().template,
        content: loroData.content_doc
          ? new Uint8Array(loroData.content_doc)
          : loroCoordinator.createEmptyDocumentSnapshots().content,
        userContent: loroData.user_content_doc
          ? new Uint8Array(loroData.user_content_doc)
          : loroCoordinator.createEmptyDocumentSnapshots().userContent,
        collaborative: loroData.collaborative_doc
          ? new Uint8Array(loroData.collaborative_doc)
          : loroCoordinator.createEmptyDocumentSnapshots().collaborative,
        submissions: loroData.submissions_doc
          ? new Uint8Array(loroData.submissions_doc)
          : loroCoordinator.createEmptyDocumentSnapshots().submissions,
        staticAssets: staticAssetsObj
      };

      loroCoordinator.loadDocument(snapshots);

      console.log('✅ [DataState] Documents loaded into coordinator');

      // Debug: Check what's in contentMap
      const contentMap = loroCoordinator.getContentMap();
      const humlSource = contentMap.get('huml_source');
      console.log('🔍 [DataState] ContentMap keys:', Array.from(contentMap.keys()));
      console.log('🔍 [DataState] HUML source length:', typeof humlSource === 'string' ? humlSource.length : 'not found');

      // IMPORTANT: Set currentResourceId AFTER loading documents
      // This ensures effects in BuilderApp see the fresh documents
      this.currentResourceId = resourceId;

      console.log('✅ [DataState] Resource switched successfully:', resourceId);
    } catch (error) {
      console.error('❌ [DataState] Failed to switch resource:', error);
      throw error;
    }
  }


  /**
   * Merge incoming CRDT updates into currently active Loro documents
   * Called when resource-updated event is received for the active resource
   */
  async mergeIncomingUpdates(updates: Record<string, number[]>) {
    try {
      console.log('🔄 [DataState] Merging incoming updates...');

      // Convert number arrays back to Uint8Array
      const updatesMap: Record<string, Uint8Array> = {};
      for (const [docName, updateArray] of Object.entries(updates)) {
        updatesMap[docName] = new Uint8Array(updateArray);
      }

      // Delegate to loroCoordinator to merge updates
      loroCoordinator.mergeIncomingUpdates(updatesMap);

      console.log('✅ [DataState] Updates merged successfully');
    } catch (error) {
      console.error('❌ [DataState] Failed to merge updates:', error);
      throw error;
    }
  }

  /**
   * Save the currently active resource
   * Gets Loro snapshots from coordinator and saves to backend
   */
  async saveCurrentResource(resourceId: string) {
    try {
      console.log('💾 [DataState] Saving resource:', resourceId);

      // Export current state from Loro coordinator
      const snapshots = loroCoordinator.exportSnapshots();

      // Get current title from contentDoc
      const contentMap = loroCoordinator.getContentMap();
      const title = contentMap.get('title') as string || 'Untitled';

      // Convert staticAssets to map format (asset_id -> base64_string)
      const staticAssetsMap: Record<string, string> = {};
      for (const [key, value] of Object.entries(snapshots.staticAssets || {})) {
        // Convert Uint8Array to base64 string in chunks to avoid stack overflow
        const uint8Array = value instanceof Uint8Array ? value : new Uint8Array(value);
        const chunkSize = 8192; // Process 8KB at a time
        let binaryString = '';

        for (let i = 0; i < uint8Array.length; i += chunkSize) {
          const chunk = uint8Array.subarray(i, Math.min(i + chunkSize, uint8Array.length));
          binaryString += String.fromCharCode(...Array.from(chunk));
        }

        const base64 = btoa(binaryString);
        staticAssetsMap[key] = base64;
      }

      console.log('🔍 [DataState] staticAssets from snapshots (save):', {
        keys: Object.keys(snapshots.staticAssets || {}),
        count: Object.keys(snapshots.staticAssets || {}).length,
        mapSize: Object.keys(staticAssetsMap).length
      });

      // Create Loro content structure for backend (documents only, no metadata)
      const loroContent = {
        template_doc: Array.from(snapshots.template),
        content_doc: Array.from(snapshots.content),
        user_content_doc: Array.from(snapshots.userContent),
        collaborative_doc: Array.from(snapshots.collaborative),
        submissions_doc: Array.from(snapshots.submissions),
        static_assets: staticAssetsMap
      };

      console.log('📦 [DataState] Loro content prepared:', {
        title,
        hasTemplateDoc: !!loroContent.template_doc,
        hasContentDoc: !!loroContent.content_doc,
        templateSize: loroContent.template_doc.length,
        contentSize: loroContent.content_doc.length,
        staticAssetsCount: Object.keys(loroContent.static_assets).length
      });
      console.log('📦 [DataState] loroContent.static_assets (save):', loroContent.static_assets);

      // Update lastModified timestamp locally
      const timestamp = Date.now();
      this.updateResourceLastModified(resourceId, timestamp);

      // Send to backend
      await sendMessage("updateCredential", {
        id: resourceId,
        data: JSON.stringify(loroContent)
      });

      console.log('✅ [DataState] Resource saved:', resourceId);
    } catch (error) {
      console.error('❌ [DataState] Failed to save resource:', error);
      throw error;
    }
  }

  /**
   * Update resource's lastModified timestamp in the resources array
   */
  private updateResourceLastModified(resourceId: string, timestamp: number) {
    const index = this.resources.findIndex(r => r.id === resourceId);
    if (index !== -1) {
      this.resources[index].lastModified = timestamp;
    }
  }

  /**
   * Cleanup reactive updates (event listeners)
   */
  cleanupReactiveUpdates() {
    console.log('🧹 [DataState] Cleaning up event listeners');
    this._unlisteners.forEach(fn => fn());
    this._unlisteners = [];
  }

  /**
   * Cleanup (alias for cleanupReactiveUpdates)
   */
  destroy() {
    this.cleanupReactiveUpdates();
  }

  /**
   * Get the Loro coordinator (alias for compatibility with SubmissionsViewer)
   */
  getBlocksuiteCoordinator() {
    return loroCoordinator;
  }
}

// Singleton instance
export const dataState = new DataState();
