import * as Y from 'yjs';

/**
 * Structure for BlockSuite content stored in backend
 * Note: state_vector is not needed for basic storage/retrieval
 * It's only used for real-time collaboration syncing
 */
export interface BlocksuiteContent {
  main_doc: number[];  // Simplified - just the Yjs updates
  client_id: string;
  last_modified: number;
  title: string;
}

export type ResourceType = 'website' | 'notice-board' | 'form';

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

  // Create content structure matching backend expectations
  const content: BlocksuiteContent = {
    main_doc: Array.from(encoded),  // Just the updates - state vector not needed!
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title
  };

  // Cleanup temporary document
  tempDoc.destroy();

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

  const content: BlocksuiteContent = {
    main_doc: Array.from(encoded),
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title
  };

  tempDoc.destroy();
  return content;
}

/**
 * Create a notice board document
 */
export function createNoticeBoardDoc(
  clientId: number,
  title: string = "Notice Board"
): BlocksuiteContent {
  const tempDoc = new Y.Doc();
  const blocks = tempDoc.getMap("blocks");
  const viewport = tempDoc.getMap("viewport");

  viewport.set("x", 0);
  viewport.set("y", 0);
  viewport.set("zoom", 1);

  // Add a notice board block
  blocks.set("notice-board-1", {
    id: "notice-board-1",
    type: "notice-board",
    x: 100,
    y: 100,
    width: 600,
    height: 500,
    zIndex: 1,
    content: JSON.stringify([]), // Empty messages array
    styles: {
      backgroundColor: "white",
      border: "2px solid #ddd"
    }
  });

  const encoded = Y.encodeStateAsUpdateV2(tempDoc);

  const content: BlocksuiteContent = {
    main_doc: Array.from(encoded),
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title
  };

  tempDoc.destroy();
  return content;
}

/**
 * Create a form document
 */
export function createFormDoc(
  clientId: number,
  title: string = "Form"
): BlocksuiteContent {
  const tempDoc = new Y.Doc();
  const blocks = tempDoc.getMap("blocks");
  const viewport = tempDoc.getMap("viewport");

  viewport.set("x", 0);
  viewport.set("y", 0);
  viewport.set("zoom", 1);

  // Add a form block with default fields
  blocks.set("form-1", {
    id: "form-1",
    type: "form",
    x: 100,
    y: 100,
    width: 500,
    height: 400,
    zIndex: 1,
    content: JSON.stringify({
      fields: [
        { id: "field-1", type: "text", label: "Name", placeholder: "Enter your name", required: true },
        { id: "field-2", type: "email", label: "Email", placeholder: "Enter your email", required: true },
        { id: "field-3", type: "textarea", label: "Message", placeholder: "Your message...", required: false }
      ],
      submitButtonText: "Submit"
    }),
    styles: {
      backgroundColor: "white",
      border: "2px solid #ddd"
    }
  });

  const encoded = Y.encodeStateAsUpdateV2(tempDoc);

  const content: BlocksuiteContent = {
    main_doc: Array.from(encoded),
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title
  };

  tempDoc.destroy();
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
    case 'notice-board':
      return createNoticeBoardDoc(clientId, title);
    case 'form':
      return createFormDoc(clientId, title);
    default:
      return createEmptyBlocksuiteDoc(clientId, title);
  }
}
