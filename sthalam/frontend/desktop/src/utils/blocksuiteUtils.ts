import * as Y from 'yjs';

/**
 * Structure for BlockSuite content stored in backend
 * Note: state_vector is not needed for basic storage/retrieval
 * It's only used for real-time collaboration syncing
 */
export interface BlocksuiteContent {
  [key: string]: any;  // Dynamic key based on resource type
  client_id: string;
  last_modified: number;
  title: string;
}

export type ResourceType = 'website';

/**
 * Create an empty BlockSuite document with initial state
 * Similar to livnote's createEmptyNoteContent but for BlockSuite
 */
export function createEmptyBlocksuiteDoc(
  clientId: number,
  title: string = "Untitled"
): BlocksuiteContent {
  // Create temporary Yjs document
  const tempDoc = new Y.Doc();

  // Get the maps (same structure as yjsManager.ts)
  const blocks = tempDoc.getMap("blocks");
  const viewport = tempDoc.getMap("viewport");
  const metadata = tempDoc.getMap("metadata");

  // Set metadata with title (for preview generator)
  metadata.set("title", title);

  // Initialize viewport with default values
  viewport.set("x", 0);
  viewport.set("y", 0);
  viewport.set("zoom", 1);

  // Add a welcome heading block as template
  blocks.set("welcome-1", {
    id: "welcome-1",
    type: "heading",
    x: 100,
    y: 100,
    width: 400,
    height: 60,
    zIndex: 1,
    content: title,
    styles: {
      fontSize: "32px",
      fontWeight: "700",
      color: "#333"
    }
  });

  // Add a text block with instructions
  blocks.set("welcome-2", {
    id: "welcome-2",
    type: "text",
    x: 100,
    y: 200,
    width: 400,
    height: 100,
    zIndex: 2,
    content: "Click and drag blocks to move them. Select a block to edit its properties.",
    styles: {
      fontSize: "16px",
      color: "#666"
    }
  });

  // Encode to byte array (V2 format for better compression)
  const encoded = Y.encodeStateAsUpdateV2(tempDoc);

  // Create empty documents for thread_comments and form_submissions
  const commentsDoc = new Y.Doc();
  const formsDoc = new Y.Doc();
  const encodedComments = Y.encodeStateAsUpdateV2(commentsDoc);
  const encodedForms = Y.encodeStateAsUpdateV2(formsDoc);

  // Create content structure matching backend expectations with all 3 docs
  const content: BlocksuiteContent = {
    blocksuite_doc: Array.from(encoded),  // Main website content
    thread_comments_doc: Array.from(encodedComments),  // Comments document (empty initially)
    form_submissions_doc: Array.from(encodedForms),  // Form submissions (empty initially)
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title
  };

  // Cleanup temporary documents
  tempDoc.destroy();
  commentsDoc.destroy();
  formsDoc.destroy();

  return content;
}

/**
 * Create a completely empty BlockSuite document (no template blocks)
 */
export function createBlankBlocksuiteDoc(
  clientId: number,
  title: string = "Untitled"
): BlocksuiteContent {
  const tempDoc = new Y.Doc();
  const viewport = tempDoc.getMap("viewport");
  const metadata = tempDoc.getMap("metadata");

  // Set metadata with title (for preview generator)
  metadata.set("title", title);

  // Only initialize viewport, no blocks
  viewport.set("x", 0);
  viewport.set("y", 0);
  viewport.set("zoom", 1);

  const encoded = Y.encodeStateAsUpdateV2(tempDoc);

  // Create empty documents for thread_comments and form_submissions
  const commentsDoc = new Y.Doc();
  const formsDoc = new Y.Doc();
  const encodedComments = Y.encodeStateAsUpdateV2(commentsDoc);
  const encodedForms = Y.encodeStateAsUpdateV2(formsDoc);

  const content: BlocksuiteContent = {
    blocksuite_doc: Array.from(encoded),
    thread_comments_doc: Array.from(encodedComments),
    form_submissions_doc: Array.from(encodedForms),
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title
  };

  tempDoc.destroy();
  commentsDoc.destroy();
  formsDoc.destroy();
  return content;
}

/**
 * Create a resource document based on type
 */
export function createResourceDoc(
  resourceType: ResourceType,
  clientId: number,
  title: string
): BlocksuiteContent {
  // Only 'website' type exists now - forms and threads are blocks within websites
  return createEmptyBlocksuiteDoc(clientId, title);
}
