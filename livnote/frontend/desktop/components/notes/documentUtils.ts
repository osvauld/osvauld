import * as Y from 'yjs';
import { Schema } from 'prosemirror-model';
import { schema } from 'prosemirror-schema-basic';
import { addListNodes } from 'prosemirror-schema-list';
import { initProseMirrorDoc } from 'y-prosemirror';
import type { Note, NotePreview, NoteContent } from 'types/notes.types';

// Create a combined schema for our document
const editorSchema = new Schema({
  nodes: addListNodes(schema.spec.nodes, "paragraph block*", "block"),
  marks: schema.spec.marks,
});

/**
 * Apply YJS updates to an existing document state
 * @param currentState - The current YJS state as Uint8Array or array
 * @param updates - The updates to apply
 * @returns An object containing the new state and content
 */
export function applyYjsUpdates(currentState: Uint8Array | number[] | null, updates: Uint8Array): {
  yjs_state: Uint8Array,
  content: any,
  editor_state: any
} {
  // Create a new YDoc
  const yDoc = new Y.Doc();

  // Apply the existing state if it exists
  if (currentState) {
    const stateArray = currentState instanceof Uint8Array
      ? currentState
      : new Uint8Array(currentState);
    Y.applyUpdate(yDoc, stateArray);
  }

  // Apply the new updates
  Y.applyUpdate(yDoc, updates);

  // Get the updated YJS state
  const newState = Y.encodeStateAsUpdate(yDoc);

  // Get the XML fragment
  const xmlFragment = yDoc.getXmlFragment('prosemirror');

  // Extract content from the XML fragment
  const content = xmlFragment.toJSON();

  // Create a ProseMirror document from the XML fragment
  const result = initProseMirrorDoc(xmlFragment, editorSchema);

  const pos = Math.min(1, result.doc.content.size);
  const selection = { type: "text", anchor: pos, head: pos };
  // Create an editor state structure
  const editorStateJSON = {
    doc: result.doc.toJSON(),
    selection
  };

  return {
    yjs_state: newState,
    content,
    editor_state: editorStateJSON
  };
}

/**
 * Create an empty note content without requiring a coordinator instance
 * @param clientId - The client ID for the note
 * @param username - The username for the note creator
 * @returns A new empty NoteContent object
 */
export function createEmptyNoteContent(clientId: number, username?: string): NoteContent {
  // Create temporary YJS documents just for content creation
  const tempYDoc = new Y.Doc();
  const tempType = tempYDoc.getXmlFragment('prosemirror');

  // Initialize empty ProseMirror document
  const prosemirrorDoc = initProseMirrorDoc(tempType, editorSchema);
  // Create note content
  const noteContent: NoteContent = {
    content: tempType.toJSON(),
    yjs_state: Array.from(Y.encodeStateAsUpdate(tempYDoc)),
    image_state: Array.from(Y.encodeStateAsUpdate(tempYDoc)), // Same doc for simplicity
    assets: [],
    editor_state: {
      doc: prosemirrorDoc.doc.toJSON(),
      selection: { type: "text", anchor: 1, head: 1 }
    },
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title: "Untitled Note",
  };

  // Clean up temporary doc
  tempYDoc.destroy();

  return noteContent;
}
/**
 * Generate a lightweight preview from full note data
 * Takes only the first few nodes from the editor state to reduce memory usage
 */
export function generatePreview(fullNote: Note, maxNodes: number = 3): NotePreview {
  let previewEditorState = null;

  try {
    if (fullNote.data.editor_state) {
      let editorState;

      // Parse editor state if it's a string
      if (typeof fullNote.data.editor_state === 'string') {
        editorState = JSON.parse(fullNote.data.editor_state);
      } else {
        editorState = fullNote.data.editor_state;
      }

      // Create truncated editor state with only first few nodes
      if (editorState && editorState.doc && editorState.doc.content) {
        const originalContent = editorState.doc.content;

        // Take only the first maxNodes nodes
        const truncatedContent = originalContent.slice(0, maxNodes);

        // Create new editor state with truncated content
        previewEditorState = {
          ...editorState,
          doc: {
            ...editorState.doc,
            content: truncatedContent
          }
        };
      }
    }
  } catch (error) {
    console.error('Error generating preview for note:', fullNote.id, error);
    // If there's an error, we'll just have null previewEditorState
  }

  return {
    id: fullNote.id,
    title: fullNote.data.title,
    previewEditorState,
    favourite: fullNote.favourite,
    folderId: fullNote.folderId,
    lastModified: fullNote.data.last_modified,
    lastAccessed: fullNote.data.last_accessed,
  };
}
