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

export type ResourceType = 'website' | 'noticeboard' | 'form';

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
 * Create a thread document with split docs for post and comments
 * NEW: Returns both thread_doc (main post) and thread_comments_doc (comments)
 */
export function createNoticeBoardDoc(
  clientId: number,
  title: string = "Thread"
): BlocksuiteContent {
  // Create thread_doc (for main post)
  const threadDoc = new Y.Doc();
  const threadBlocks = threadDoc.getMap("blocks");
  const viewport = threadDoc.getMap("viewport");

  viewport.set("x", 0);
  viewport.set("y", 0);
  viewport.set("zoom", 1);

  // Add initial thread post block
  const postId = `thread-post-${Date.now()}`;
  threadBlocks.set(postId, {
    id: postId,
    type: "thread-post",
    content: "",
    mode: "markdown",
    css: "",
    author: "Owner",
    timestamp: new Date().toISOString(),
    order: 0
  });

  const threadEncoded = Y.encodeStateAsUpdateV2(threadDoc);

  // Create thread_comments_doc (for comments)
  const commentsDoc = new Y.Doc();
  const commentsBlocks = commentsDoc.getMap("blocks");
  const commentsEncoded = Y.encodeStateAsUpdateV2(commentsDoc);

  const content: BlocksuiteContent = {
    thread_doc: Array.from(threadEncoded),          // Main post
    thread_comments_doc: Array.from(commentsEncoded), // Comments (empty initially)
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title
  };

  threadDoc.destroy();
  commentsDoc.destroy();
  return content;
}

/**
 * Create a form document with split docs for definition and submissions
 * NEW: Returns both form_doc (form definition) and form_submissions_doc (submissions)
 */
export function createFormDoc(
  clientId: number,
  title: string = "Form"
): BlocksuiteContent {
  // Create form_doc (for form definition/fields)
  const formDoc = new Y.Doc();
  const formBlocks = formDoc.getMap("blocks");

  // Store form configuration in a special block
  formBlocks.set("form-config", {
    id: "form-config",
    type: "form-config",
    content: JSON.stringify({
      fields: [],  // Empty form - user will add fields
      submitButtonText: "Submit"
    }),
    x: 0,
    y: 0,
    width: 0,
    height: 0,
    zIndex: 0,
    styles: {}
  });

  const formEncoded = Y.encodeStateAsUpdateV2(formDoc);

  // Create form_submissions_doc (for submissions)
  const submissionsDoc = new Y.Doc();
  const submissionsBlocks = submissionsDoc.getMap("blocks");
  // Empty initially - viewers/users will append submissions
  const submissionsEncoded = Y.encodeStateAsUpdateV2(submissionsDoc);

  const content: BlocksuiteContent = {
    form_doc: Array.from(formEncoded),                  // Form definition
    form_submissions_doc: Array.from(submissionsEncoded), // Submissions (empty initially)
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title
  };

  formDoc.destroy();
  submissionsDoc.destroy();
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
  switch (resourceType) {
    case 'website':
      return createEmptyBlocksuiteDoc(clientId, title);
    case 'noticeboard':
      return createNoticeBoardDoc(clientId, title);
    case 'form':
      return createFormDoc(clientId, title);
    default:
      return createEmptyBlocksuiteDoc(clientId, title);
  }
}
