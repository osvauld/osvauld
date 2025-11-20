/**
 * Loro Coordinator - Simplified
 *
 * Manages Loro documents for template and state storage
 * NO TREES - Only Maps and Lists for simplicity
 */

import { LoroDoc, LoroList, type LoroMap, type VersionVector } from 'loro-crdt';
import { SubmissionsStore } from '../../lib/submissionsStore';

export interface Documents {
  templateDoc: LoroDoc;      // Stores HUML template
  contentDoc: LoroDoc;       // Stores dynamic state
  userContentDoc: LoroDoc;   // User-specific content
  collaborativeDoc: LoroDoc; // Collaborative features
  submissionsDoc: LoroDoc;   // Form submissions
  uiStateDoc: LoroDoc;       // Session-only UI state
}

export class LoroCoordinator {
  private documents: Documents;
  private submissionsStore?: SubmissionsStore;

  /**
   * Non-CRDT storage for static assets (videos, images, files)
   * These are immutable binaries that don't need CRDT overhead
   * Stored alongside CRDT snapshots in resource file
   */
  private staticAssets: Map<string, Uint8Array> = new Map();

  constructor() {
    this.documents = {
      templateDoc: new LoroDoc(),
      contentDoc: new LoroDoc(),
      userContentDoc: new LoroDoc(),
      collaborativeDoc: new LoroDoc(),
      submissionsDoc: new LoroDoc(),
      uiStateDoc: new LoroDoc()
    };

    console.log('🚀 [LoroCoordinator] Initialized with 6 documents');
  }

  /**
   * Get template metadata map
   * Stores: huml_source (string), name (string), version (string)
   */
  getTemplateMap(): LoroMap {
    return this.documents.templateDoc.getMap('template');
  }

  /**
   * Get content map for dynamic state
   * Stores: counter, user, etc.
   */
  getContentMap(): LoroMap {
    return this.documents.contentDoc.getMap('content');
  }

  /**
   * Get content list (for arrays like posts, items, etc.)
   */
  getContentList(key: string): LoroList {
    return this.documents.contentDoc.getList(key);
  }

  /**
   * Get videos map (for storing uploaded video files - LEGACY)
   * NOTE: New code should use staticAssets instead
   * Stores: video_id → Uint8Array (video binary data)
   */
  getVideosMap(): LoroMap {
    return this.documents.contentDoc.getMap('videos');
  }

  /**
   * Get static asset binary data (non-CRDT)
   * @param assetId Asset identifier (e.g. "asset_video_123")
   * @returns Binary data or null if not found
   */
  getStaticAsset(assetId: string): Uint8Array | null {
    return this.staticAssets.get(assetId) || null;
  }

  /**
   * Set static asset binary data (non-CRDT)
   * @param assetId Asset identifier
   * @param data Binary data
   */
  setStaticAsset(assetId: string, data: Uint8Array): void {
    this.staticAssets.set(assetId, data);
  }

  /**
   * List all static asset IDs
   */
  listStaticAssets(): string[] {
    return Array.from(this.staticAssets.keys());
  }

  /**
   * Delete static asset
   * @param assetId Asset identifier
   */
  deleteStaticAsset(assetId: string): void {
    this.staticAssets.delete(assetId);
  }

  /**
   * Get state map (for persisting publisher UI state)
   * Stores: state_key → any value
   */
  getStateMap(): LoroMap {
    return this.documents.contentDoc.getMap('publisherState');
  }

  /**
   * Get user content map
   */
  getUserContentMap(): LoroMap {
    return this.documents.userContentDoc.getMap('user_content');
  }

  /**
   * Get collaborative threads map
   */
  getCollaborativeThreads(): LoroMap {
    return this.documents.collaborativeDoc.getMap('threads');
  }

  /**
   * Get collaborative state map (shared between owner and viewers)
   * Stores: counter, comments, etc. - editable by both roles
   */
  getCollaborativeMap(): LoroMap {
    return this.documents.collaborativeDoc.getMap('collaborativeState');
  }

  /**
   * Get submissions list
   */
  getSubmissions(): LoroList {
    // Get the submissions list directly from the document root
    return this.documents.submissionsDoc.getList('submissions');
  }

  /**
   * Get UI state map (session-only, not persisted)
   */
  getUIStateMap(): LoroMap {
    return this.documents.uiStateDoc.getMap('uiState');
  }

  /**
   * Export document states as snapshots (for persistence)
   * Note: uiState is excluded as it's session-only
   * Includes staticAssets (non-CRDT binary storage)
   */
  exportSnapshots() {
    // Convert staticAssets Map to plain object for serialization
    const staticAssetsObj: Record<string, Uint8Array> = {};
    for (const [key, value] of this.staticAssets.entries()) {
      staticAssetsObj[key] = value;
    }

    return {
      template: this.documents.templateDoc.export({ mode: "snapshot" }),
      content: this.documents.contentDoc.export({ mode: "snapshot" }),
      userContent: this.documents.userContentDoc.export({ mode: "snapshot" }),
      collaborative: this.documents.collaborativeDoc.export({ mode: "snapshot" }),
      submissions: this.documents.submissionsDoc.export({ mode: "snapshot" }),
      staticAssets: staticAssetsObj
    };
  }

  /**
   * Export updates from a specific version (for incremental sync)
   */
  exportUpdates(from?: {
    template?: VersionVector;
    content?: VersionVector;
    userContent?: VersionVector;
    collaborative?: VersionVector;
    submissions?: VersionVector;
  }) {
    return {
      template: this.documents.templateDoc.export({ mode: "update", from: from?.template }),
      content: this.documents.contentDoc.export({ mode: "update", from: from?.content }),
      userContent: this.documents.userContentDoc.export({ mode: "update", from: from?.userContent }),
      collaborative: this.documents.collaborativeDoc.export({ mode: "update", from: from?.collaborative }),
      submissions: this.documents.submissionsDoc.export({ mode: "update", from: from?.submissions })
    };
  }

  /**
   * Create snapshots for a new empty document
   */
  createEmptyDocumentSnapshots(title: string = "Untitled") {
    const tempTemplateDoc = new LoroDoc();
    const tempContentDoc = new LoroDoc();
    const tempUserContentDoc = new LoroDoc();
    const tempCollaborativeDoc = new LoroDoc();
    const tempSubmissionsDoc = new LoroDoc();

    // Initialize template with empty HUML
    const templateMap = tempTemplateDoc.getMap('template');
    templateMap.set('huml_source', '');
    templateMap.set('name', title);
    templateMap.set('version', 'v1.0.0');

    // Initialize content with title
    const contentMap = tempContentDoc.getMap('content');
    contentMap.set('title', title);

    // Initialize submissions with empty list structure directly
    tempSubmissionsDoc.getList('submissions');

    // Commit changes
    tempTemplateDoc.commit();
    tempContentDoc.commit();
    tempUserContentDoc.commit();
    tempCollaborativeDoc.commit();
    tempSubmissionsDoc.commit();

    const snapshots = {
      template: tempTemplateDoc.export({ mode: "snapshot" }),
      content: tempContentDoc.export({ mode: "snapshot" }),
      userContent: tempUserContentDoc.export({ mode: "snapshot" }),
      collaborative: tempCollaborativeDoc.export({ mode: "snapshot" }),
      submissions: tempSubmissionsDoc.export({ mode: "snapshot" }),
      staticAssets: {} // Empty static assets for new document
    };

    console.log('📄 [LoroCoordinator] Created empty document snapshots');
    return snapshots;
  }

  /**
   * Load a document from exported snapshots
   */
  loadDocument(snapshots: {
    template: Uint8Array;
    content: Uint8Array;
    userContent: Uint8Array;
    collaborative: Uint8Array;
    submissions: Uint8Array;
    staticAssets?: Record<string, Uint8Array>;
  }) {
    // Create FRESH documents (don't merge, replace)
    this.documents.templateDoc = new LoroDoc();
    this.documents.contentDoc = new LoroDoc();
    this.documents.userContentDoc = new LoroDoc();
    this.documents.collaborativeDoc = new LoroDoc();
    this.documents.submissionsDoc = new LoroDoc();
    // Keep uiStateDoc as session state (don't replace)

    // Import snapshots
    this.documents.templateDoc.import(snapshots.template);
    this.documents.contentDoc.import(snapshots.content);
    this.documents.userContentDoc.import(snapshots.userContent);
    this.documents.collaborativeDoc.import(snapshots.collaborative);
    this.documents.submissionsDoc.import(snapshots.submissions);

    // Restore staticAssets
    this.staticAssets.clear();
    if (snapshots.staticAssets) {
      for (const [key, value] of Object.entries(snapshots.staticAssets)) {
        this.staticAssets.set(key, value);
      }
      console.log('📦 [LoroCoordinator] Restored', this.staticAssets.size, 'static assets');
    }

    // Reset submissions store to reference new document
    this.resetSubmissionsStore();

    console.log('📂 [LoroCoordinator] Loaded document');
  }

  /**
   * Merge incoming CRDT updates into existing documents
   * Used for real-time sync when updates arrive from network
   */
  mergeIncomingUpdates(updates: Record<string, Uint8Array>) {
    console.log('🔄 [LoroCoordinator] Merging incoming updates for', Object.keys(updates).length, 'documents');

    // Map document names to their LoroDoc instances
    const docMap: Record<string, LoroDoc> = {
      'template_doc': this.documents.templateDoc,
      'content_doc': this.documents.contentDoc,
      'user_content_doc': this.documents.userContentDoc,
      'collaborative_doc': this.documents.collaborativeDoc,
      'submissions_doc': this.documents.submissionsDoc,
    };

    // Merge updates into each document
    for (const [docName, updateBytes] of Object.entries(updates)) {
      const doc = docMap[docName];
      if (doc) {
        try {
          console.log(`📥 [LoroCoordinator] Merging ${updateBytes.length} bytes into ${docName}`);

          // Import updates - Loro automatically merges using CRDT logic
          doc.import(updateBytes);

          console.log(`✅ [LoroCoordinator] Successfully merged updates for ${docName}`);

          // Debug: Log document content after merge (especially for submissions)
          if (docName === 'submissions_doc') {
            try {
              const submissionsList = doc.getList('submissions');
              const submissionsData = submissionsList.toJSON();
              console.log(`📋 [LoroCoordinator] Submissions_doc content after merge:`, submissionsData);

              if (Array.isArray(submissionsData)) {
                console.log(`📊 [LoroCoordinator] Number of submissions in doc:`, submissionsData.length);
                console.log(`📊 [LoroCoordinator] Submission IDs:`, submissionsData.map((s: any) => s.id));
              } else {
                console.warn(`⚠️ [LoroCoordinator] Submissions data is not an array!`);
              }

              // Also log the raw document size
              const snapshot = doc.export({ mode: "snapshot" });
              console.log(`📏 [LoroCoordinator] Submissions_doc snapshot size: ${snapshot.length} bytes`);
            } catch (error) {
              console.error(`❌ [LoroCoordinator] Failed to extract submissions content:`, error);
            }
          }
        } catch (error) {
          console.error(`❌ [LoroCoordinator] Failed to merge updates for ${docName}:`, error);
        }
      } else {
        console.warn(`⚠️ [LoroCoordinator] Unknown document: ${docName}`);
      }
    }

    console.log('✅ [LoroCoordinator] All updates merged');
  }

  /**
   * Get all documents (for direct access if needed)
   */
  getDocuments() {
    return this.documents;
  }

  /**
   * Get or create the SubmissionsStore
   * Lazily instantiated and reused
   */
  getSubmissionsStore(): SubmissionsStore {
    if (!this.submissionsStore) {
      const submissionsList = this.getSubmissions();
      this.submissionsStore = new SubmissionsStore(submissionsList);
      console.log('📋 [LoroCoordinator] SubmissionsStore created with List-based storage');
    }
    return this.submissionsStore;
  }

  /**
   * Reset the submissions store (called when loading new document)
   * This ensures the store references the correct Loro map
   */
  resetSubmissionsStore(): void {
    this.submissionsStore = undefined;
    console.log('🔄 [LoroCoordinator] SubmissionsStore reset');
  }
}

// Singleton instance
export const loroCoordinator = new LoroCoordinator();
