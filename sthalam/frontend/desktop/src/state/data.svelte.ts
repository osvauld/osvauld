import {
  listSpaces,
  listPages,
  createSpace,
  createPage,
  openPage,
  closePage,
  getUserDetails,
  getSovereignNodes,
  listenForSpaceSync,
  listenForPageSync,
  listenForPageUpdates,
  type SpaceResponse,
  type PageMetadata,
  type PageResponse,
  type SovereignNode,
  type UserDetails,
  type LayerUpdate,
} from "../utils/scribe";
import { loroCoordinator } from "../shared/loro/loroCoordinator";
import { syncManager } from "../shared/loro/syncManager";
import { SPACE_TEMPLATE, PAGE_TEMPLATE } from "../config/permissions";

/**
 * Data State Management for Sthalam
 *
 * Uses new terminology: Space (folder), Page (resource)
 * Auto-syncs Loro changes to backend via SyncManager - no manual save needed
 */

export interface Space {
  id: string;
  name: string;
  description?: string;
  isDefault?: boolean;
}

export interface Page {
  id: string;
  title: string;
  pageType: string;
  spaceId: string;
  lastModified: number;
  favourite?: boolean;
  preview?: string;
}

class DataState {
  // Spaces
  spaces = $state<Space[]>([{ id: "all", name: "All Spaces" }]);
  currentSpace = $state<Space>({ id: "all", name: "All Spaces" });

  // Pages
  pages = $state<Page[]>([]);
  currentPageId = $state<string | null>(null);

  // User data
  userDetails = $state<UserDetails | null>(null);
  clientId: number = 0;

  // Sovereign nodes (paired Kunki nodes)
  sovereignNodes = $state<SovereignNode[]>([]);
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
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
      return parseInt(hex, 16);
    } catch (error) {
      console.error("Failed to generate client ID:", error);
      return Math.floor(Math.random() * 1000000);
    }
  }

  /**
   * Initialize state - called by App.svelte on auth
   */
  async initializeState() {
    console.log("[DataState] initializeState() called");
    this.isDataLoading = true;

    // Clean up event listeners
    this.cleanupEventListeners();

    try {
      console.log("[DataState] Getting user details and setting up...");

      // Get user details first
      this.userDetails = await getUserDetails();
      this.clientId = this.getClientId();

      console.log("[DataState] User details loaded, clientId:", this.clientId);

      // Setup event listeners and load spaces in parallel
      await Promise.all([
        this.setupEventListeners(),
        this.fetchSpaces(),
        this.fetchSovereignNodes(),
      ]);

      console.log("[DataState] Setup ready");

      // Fetch all pages
      await this.fetchAllPages();
    } catch (error) {
      console.error("[DataState] Initialization failed:", error);
    } finally {
      this.isDataLoading = false;
      console.log("[DataState] Initialization complete");
    }
  }

  /**
   * Setup backend event listeners
   */
  private async setupEventListeners() {
    try {
      // Listen for space sync events
      const spaceSyncUnlisten = await listenForSpaceSync(({ spaceId, spaceName }) => {
        console.log("[DataState] Space synced:", spaceId, spaceName);
        this.fetchSpaces();
      });
      this._unlisteners.push(spaceSyncUnlisten);

      // Listen for page sync events
      const pageSyncUnlisten = await listenForPageSync((metadata) => {
        console.log("[DataState] Page synced:", metadata.id, metadata.title);

        const exists = this.pages.some((p) => p.id === metadata.id);
        if (!exists) {
          this.pages = [
            ...this.pages,
            {
              id: metadata.id,
              title: metadata.title,
              spaceId: metadata.spaceId,
              pageType: metadata.pageType,
              lastModified: metadata.lastModified,
            },
          ];
        }
      });
      this._unlisteners.push(pageSyncUnlisten);

      // Listen for page update events (real-time CRDT updates from P2P peers via Scribe)
      const pageUpdateUnlisten = await listenForPageUpdates((update) => {
        console.log("[DataState] Page update from P2P:", update.pageId, update.layerName);

        if (this.currentPageId === update.pageId) {
          console.log("[DataState] Applying remote update for active page");
          this.applyRemoteUpdate(update);
        }
      });
      this._unlisteners.push(pageUpdateUnlisten);

      console.log("[DataState] Event listeners setup complete");
    } catch (error) {
      console.error("[DataState] Failed to setup listeners:", error);
    }
  }

  /**
   * Apply remote update from P2P peer via SyncManager
   * SyncManager handles echo prevention - won't re-send to backend
   */
  private applyRemoteUpdate(update: LayerUpdate) {
    try {
      const docName = syncManager.layerNameToDocName(update.layerName);
      const docs = loroCoordinator.getDocuments();
      const doc = docs[docName as keyof typeof docs];

      if (doc) {
        syncManager.applyRemoteUpdate(update.layerName, update.update, doc);
        console.log("[DataState] Remote update applied:", update.layerName);
      } else {
        console.warn("[DataState] Unknown layer for remote update:", update.layerName);
      }
    } catch (error) {
      console.error("[DataState] Failed to apply remote update:", error);
    }
  }

  /**
   * Fetch spaces from backend
   */
  async fetchSpaces() {
    try {
      const resp = await listSpaces();
      const spaces: Space[] = resp.map((item) => ({
        id: item.id,
        name: item.name,
        description: item.description,
        isDefault: item.isDefault,
      }));
      this.spaces = [{ id: "all", name: "All Spaces" }, ...spaces];
      console.log("[DataState] Fetched spaces:", spaces.length);
    } catch (error) {
      console.error("[DataState] Failed to fetch spaces:", error);
      this.spaces = [{ id: "all", name: "All Spaces" }];
    }
  }

  /**
   * Fetch sovereign nodes (Kunki nodes we've paired with)
   */
  async fetchSovereignNodes() {
    try {
      const nodes = await getSovereignNodes();
      this.sovereignNodes = nodes || [];

      if (this.sovereignNodes.length > 0) {
        this.sovereignNodeId = this.sovereignNodes[0].nodeId;
      }

      console.log("[DataState] Fetched sovereign nodes:", this.sovereignNodes.length);
    } catch (error) {
      console.error("[DataState] Failed to fetch sovereign nodes:", error);
      this.sovereignNodes = [];
      this.sovereignNodeId = null;
    }
  }

  /**
   * Fetch all pages metadata from backend
   */
  async fetchAllPages() {
    this.isDataLoading = true;
    try {
      console.log("[DataState] Fetching all pages metadata...");
      const pagesMetadata = await listPages();

      this.pages = pagesMetadata.map((p) => ({
        id: p.id,
        title: p.title || "Untitled",
        pageType: p.pageType || "content",
        spaceId: p.spaceId,
        lastModified: p.lastModified || Date.now(),
      }));

      console.log("[DataState] Loaded", this.pages.length, "pages");
    } catch (error) {
      console.error("[DataState] Failed to fetch pages:", error);
    } finally {
      this.isDataLoading = false;
    }
  }

  /**
   * Switch to a different space
   */
  async switchSpace(space: Space) {
    this.currentSpace = space;
    this.currentPageId = null;
    syncManager.stopSync();
  }

  /**
   * Create a new space
   */
  async addSpace(name: string) {
    try {
      const space = await createSpace(name, "", JSON.stringify(SPACE_TEMPLATE));
      this.spaces = [...this.spaces, { id: space.id, name: space.name }];
      console.log("[DataState] Space created:", name);
      return space;
    } catch (error) {
      console.error("[DataState] Failed to create space:", error);
      throw error;
    }
  }

  /**
   * Create a new page with empty Loro document
   */
  async addPage(spaceId: string, title: string = "Untitled", pageType: string = "content") {
    try {
      console.log("[DataState] Creating page:", { spaceId, title, pageType });

      // Create metadata
      const metadata = {
        title: title,
        type: pageType,
        client_id: this.clientId.toString(),
        last_modified: Date.now(),
      };

      // Create page via backend
      // Send full PAGE_TEMPLATE - gurkha needs owner_template.delegation for permit chains
      const pageMetadata = await createPage(
        spaceId,
        pageType,
        JSON.stringify(PAGE_TEMPLATE),
        JSON.stringify(metadata)
      );

      console.log("[DataState] Page created:", pageMetadata.id);

      // Add to pages list
      const newPage: Page = {
        id: pageMetadata.id,
        title: pageMetadata.title,
        pageType: pageMetadata.pageType,
        spaceId: pageMetadata.spaceId,
        lastModified: pageMetadata.lastModified,
        favourite: pageMetadata.favourite,
        preview: pageMetadata.preview,
      };
      this.pages = [...this.pages, newPage];

      // Switch to the new page
      await this.switchPage(pageMetadata.id);

      return newPage;
    } catch (error) {
      console.error("[DataState] Failed to create page:", error);
      throw error;
    }
  }

  /**
   * Switch to a page and load its Loro documents
   * Auto-sync starts immediately after loading
   */
  async switchPage(pageId: string) {
    try {
      console.log("[DataState] Switching to page:", pageId);

      // Stop syncing previous page
      if (this.currentPageId && this.currentPageId !== pageId) {
        syncManager.stopSync();
        await closePage(this.currentPageId);
      }

      // Open page and get content
      const page = await openPage(pageId);

      console.log("[DataState] Page opened:", { id: page.id, hasData: !!page.data });

      if (!page.data) {
        console.error("[DataState] No data in page");
        return;
      }

      const loroData = page.data;

      // Convert staticAssets from base64 strings to Uint8Array
      const staticAssetsObj: Record<string, Uint8Array> = {};
      if (loroData.static_assets && typeof loroData.static_assets === "object") {
        for (const [assetId, base64Data] of Object.entries(loroData.static_assets)) {
          if (typeof base64Data === "string") {
            const binaryString = atob(base64Data);
            const bytes = new Uint8Array(binaryString.length);
            for (let i = 0; i < binaryString.length; i++) {
              bytes[i] = binaryString.charCodeAt(i);
            }
            staticAssetsObj[assetId] = bytes;
          }
        }
        console.log("[DataState] Loaded", Object.keys(loroData.static_assets).length, "static assets");
      }

      // Load Loro documents from snapshots
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
        staticAssets: staticAssetsObj,
      };

      loroCoordinator.loadDocument(snapshots);

      console.log("[DataState] Documents loaded into coordinator");

      // Set currentPageId AFTER loading documents
      this.currentPageId = pageId;

      // Start auto-sync for this page
      // Every Loro change will now be sent to backend automatically
      syncManager.startSync(pageId, loroCoordinator.getDocuments());

      console.log("[DataState] Page switched and auto-sync started:", pageId);
    } catch (error) {
      console.error("[DataState] Failed to switch page:", error);
      throw error;
    }
  }

  /**
   * Update page's lastModified timestamp
   */
  private updatePageLastModified(pageId: string, timestamp: number) {
    const index = this.pages.findIndex((p) => p.id === pageId);
    if (index !== -1) {
      this.pages[index].lastModified = timestamp;
    }
  }

  /**
   * Cleanup event listeners and stop sync
   */
  cleanupEventListeners() {
    console.log("[DataState] Cleaning up event listeners");
    syncManager.stopSync();
    this._unlisteners.forEach((fn) => fn());
    this._unlisteners = [];
  }

  /**
   * Alias for cleanupEventListeners (used by App.svelte)
   */
  cleanupReactiveUpdates() {
    this.cleanupEventListeners();
  }

  /**
   * Cleanup
   */
  destroy() {
    this.cleanupEventListeners();
  }

  /**
   * Get the Loro coordinator
   */
  getBlocksuiteCoordinator() {
    return loroCoordinator;
  }

  // ===== Compatibility aliases =====
  // These maintain backwards compatibility with old component code
  // TODO: Migrate components to new terminology and remove these

  get websites() {
    return this.spaces;
  }

  get currentWebsite() {
    return this.currentSpace;
  }

  get resources() {
    return this.pages.map((p) => ({
      id: p.id,
      title: p.title,
      resourceType: p.pageType,
      websiteId: p.spaceId,
      lastModified: p.lastModified,
      favourite: p.favourite,
      preview: p.preview,
    }));
  }

  get currentResourceId() {
    return this.currentPageId;
  }

  get isResourceLoading() {
    return this.isDataLoading;
  }

  async addResource(spaceId: string, title?: string, pageType?: string) {
    return this.addPage(spaceId, title || "Untitled", pageType || "content");
  }

  async switchResource(resourceId: string) {
    return this.switchPage(resourceId);
  }

  async addWebsite(name: string) {
    return this.addSpace(name);
  }

  switchWebsite(space: Space | string) {
    if (typeof space === "string") {
      const found = this.spaces.find((s) => s.id === space);
      if (found) {
        this.switchSpace(found);
      }
    } else {
      this.switchSpace(space);
    }
  }
}

// Singleton instance
export const dataState = new DataState();
