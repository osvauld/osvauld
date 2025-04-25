import * as Y from 'yjs';
import { EditorState } from 'prosemirror-state';
import { Schema } from 'prosemirror-model';
import { schema } from 'prosemirror-schema-basic';
import { addListNodes } from 'prosemirror-schema-list';
import { initProseMirrorDoc } from 'y-prosemirror';

// Create a combined schema for our document (similar to what's in notes.ts)
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
  content: any,  // Using any to match the type in notes.ts
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

  try {
    // Try to access the YJS XML fragment
    let type;

    // First, check if the method exists on the prototype
    if (typeof yDoc.getXmlFragment === 'function') {
      // Use the method directly
      type = yDoc.getXmlFragment('prosemirror');
    } else {
      // If getXmlFragment isn't available, fall back to using Y.XmlFragment directly
      // Create a default XML fragment
      type = new Y.XmlFragment();

      // Add some default content
      type.insert(0, [new Y.XmlElement('paragraph')]);
    }

    // Extract content from the XML fragment
    const content = type.toJSON();

    try {
      // Try to create a ProseMirror document from the XML fragment
      const result = initProseMirrorDoc(type, editorSchema);
      const prosemirrorDoc = result.doc;

      // Create a selection
      const selection = { type: "text", anchor: 1, head: 1 };

      // Create an editor state structure
      const editorStateJSON = {
        doc: prosemirrorDoc.toJSON(),
        selection: selection
      };

      return {
        yjs_state: newState,
        content,
        editor_state: editorStateJSON
      };
    } catch (docError) {
      console.error("Error creating ProseMirror document:", docError);

      // Create a fallback editor state
      const basicEditorState = {
        doc: {
          type: "doc",
          content: [{ type: "paragraph", content: [] }]
        },
        selection: { type: "text", anchor: 1, head: 1 }
      };

      return {
        yjs_state: newState,
        content,
        editor_state: basicEditorState
      };
    }
  } catch (error) {
    console.error("Error processing YJS document:", error);

    // Create fallback content and state
    const fallbackContent = [{ type: "paragraph", content: [] }];

    const fallbackState = {
      doc: {
        type: "doc",
        content: fallbackContent
      },
      selection: { type: "text", anchor: 1, head: 1 }
    };

    return {
      yjs_state: newState,
      content: fallbackContent,
      editor_state: fallbackState
    };
  }
}
