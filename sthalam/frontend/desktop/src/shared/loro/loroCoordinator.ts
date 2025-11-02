/**
 * Loro Coordinator - Manages all Loro documents
 * Tree-based architecture for fine-grained reactivity
 */

import { LoroDoc, type LoroTree, type LoroMap, type VersionVector } from 'loro-crdt';
import type { Documents } from '../types/document.types';

export class LoroCoordinator {
  private documents: Documents;

  constructor() {
    // Initialize the 6 Loro documents
    this.documents = {
      templateDoc: new LoroDoc(),
      uiStateDoc: new LoroDoc(),
      contentDoc: new LoroDoc(),
      userContentDoc: new LoroDoc(),
      collaborativeDoc: new LoroDoc(),
      submissionsDoc: new LoroDoc()
    };

    console.log('🚀 [LoroCoordinator] Initialized with 6 documents');
  }

  // Get template tree (for block structure)
  getTemplateTree(): LoroTree {
    return this.documents.templateDoc.getTree('blocks');
  }

  // Get UI state tree (for component state with fine-grained reactivity)
  getUIStateTree(): LoroTree {
    return this.documents.uiStateDoc.getTree('uiState');
  }

  // Get content map (for publisher data)
  getContentMap(): LoroMap {
    return this.documents.contentDoc.getMap('content');
  }

  // Get user content map
  getUserContentMap(): LoroMap {
    return this.documents.userContentDoc.getMap('user_content');
  }

  // Get collaborative threads (Map of Trees for thread structure)
  getCollaborativeThreads(): LoroMap {
    return this.documents.collaborativeDoc.getMap('threads');
  }

  // Get submissions (Map of Lists for form data)
  getSubmissions(): LoroMap {
    return this.documents.submissionsDoc.getMap('submissions');
  }

  // Export document states as snapshots (for persistence)
  // Note: uiState is excluded as it's session-only
  exportSnapshots() {
    return {
      template: this.documents.templateDoc.export({ mode: "snapshot" }),
      content: this.documents.contentDoc.export({ mode: "snapshot" }),
      userContent: this.documents.userContentDoc.export({ mode: "snapshot" }),
      collaborative: this.documents.collaborativeDoc.export({ mode: "snapshot" }),
      submissions: this.documents.submissionsDoc.export({ mode: "snapshot" })
    };
  }

  // Export updates from a specific version (for incremental sync)
  // Note: uiState is excluded as it's session-only
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

  // Create snapshots for a new empty document (without mutating coordinator state)
  createEmptyDocumentSnapshots(title: string = "Untitled") {
    // Create temporary documents
    const tempTemplateDoc = new LoroDoc();
    const tempContentDoc = new LoroDoc();
    const tempUserContentDoc = new LoroDoc();
    const tempCollaborativeDoc = new LoroDoc();
    const tempSubmissionsDoc = new LoroDoc();

    // Initialize content map with title and empty HUML source
    const contentMap = tempContentDoc.getMap('content');
    contentMap.set('title', title);
    contentMap.set('huml_source', '');

    // Commit changes
    tempTemplateDoc.commit();
    tempContentDoc.commit();
    tempUserContentDoc.commit();
    tempCollaborativeDoc.commit();
    tempSubmissionsDoc.commit();

    // Export snapshots
    const snapshots = {
      template: tempTemplateDoc.export({ mode: "snapshot" }),
      content: tempContentDoc.export({ mode: "snapshot" }),
      userContent: tempUserContentDoc.export({ mode: "snapshot" }),
      collaborative: tempCollaborativeDoc.export({ mode: "snapshot" }),
      submissions: tempSubmissionsDoc.export({ mode: "snapshot" })
    };

    console.log('📄 [LoroCoordinator] Created empty document snapshots');
    return snapshots;
  }

  // Load a document from exported snapshots
  loadDocument(snapshots: {
    template: Uint8Array;
    content: Uint8Array;
    userContent: Uint8Array;
    collaborative: Uint8Array;
    submissions: Uint8Array;
  }) {
    // Create FRESH documents to replace existing ones (not merge)
    // This ensures we start with clean state for the new resource
    this.documents.templateDoc = new LoroDoc();
    this.documents.contentDoc = new LoroDoc();
    this.documents.userContentDoc = new LoroDoc();
    this.documents.collaborativeDoc = new LoroDoc();
    this.documents.submissionsDoc = new LoroDoc();
    // Keep uiStateDoc as session state (don't replace)

    // Import snapshots into the fresh documents
    this.documents.templateDoc.import(snapshots.template);
    this.documents.contentDoc.import(snapshots.content);
    this.documents.userContentDoc.import(snapshots.userContent);
    this.documents.collaborativeDoc.import(snapshots.collaborative);
    this.documents.submissionsDoc.import(snapshots.submissions);

    console.log('📂 [LoroCoordinator] Loaded document with fresh state');
  }

  // Get all documents (for direct access if needed)
  getDocuments() {
    return this.documents;
  }
}

// Singleton instance
export const loroCoordinator = new LoroCoordinator();
