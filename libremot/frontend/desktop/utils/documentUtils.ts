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



export function createEmptyNoteContent(clientId: number, username?: string): NoteContent {
  // Create temporary YJS documents
  const tempYDoc = new Y.Doc();
  const tempType = tempYDoc.getXmlFragment('prosemirror');

  // Create separate image doc
  const tempImageDoc = new Y.Doc();

  // Initialize empty ProseMirror document
  const prosemirrorDoc = initProseMirrorDoc(tempType, editorSchema);

  // Create note content
  const noteContent: NoteContent = {
    main_doc: Array.from(Y.encodeStateAsUpdateV2(tempYDoc)),
    image_state: Array.from(Y.encodeStateAsUpdateV2(tempImageDoc)), // Separate image doc
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title: "Untitled Note",
  };

  // Clean up temporary docs
  tempYDoc.destroy();
  tempImageDoc.destroy();

  return noteContent;
}

