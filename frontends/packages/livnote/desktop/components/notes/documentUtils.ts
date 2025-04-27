import * as Y from 'yjs';
import { Schema } from 'prosemirror-model';
import { schema } from 'prosemirror-schema-basic';
import { addListNodes } from 'prosemirror-schema-list';
import { initProseMirrorDoc } from 'y-prosemirror';

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
