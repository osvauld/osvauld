import * as Y from 'yjs';
import { Schema, Node as PMNode } from 'prosemirror-model';
import { schema } from 'prosemirror-schema-basic';
import { addListNodes } from 'prosemirror-schema-list';
import { initProseMirrorDoc } from 'y-prosemirror';
import type { Note, NotePreview, NoteContent } from 'types/notes.types';

// Create a combined schema for our document
const editorSchema = new Schema({
  nodes: addListNodes(schema.spec.nodes, "paragraph block*", "block"),
  marks: schema.spec.marks,
});

export function createEmptyNoteContent(clientId: number, username?: string, title?: string): NoteContent {
  // Create temporary YJS documents
  const tempYDoc = new Y.Doc();

  // Create separate image doc
  const tempImageDoc = new Y.Doc();

  // Create separate comment doc
  const tempCommentDoc = new Y.Doc();

  // Create note content
  const noteContent: NoteContent = {
    main_doc: Array.from(Y.encodeStateAsUpdateV2(tempYDoc)),
    image_state: Array.from(Y.encodeStateAsUpdateV2(tempImageDoc)),
    comment_state: Array.from(Y.encodeStateAsUpdateV2(tempCommentDoc)),
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title: title || "Untitled",
  };

  // Clean up temporary docs
  tempYDoc.destroy();
  tempImageDoc.destroy();
  tempCommentDoc.destroy();

  return noteContent;
}

export function createNoteContentWithText(clientId: number, title: string, contentLines: string[]): NoteContent {
  // Create temporary YJS documents
  const tempYDoc = new Y.Doc();
  const xmlFragment = tempYDoc.getXmlFragment('prosemirror');

  // Create separate image doc
  const tempImageDoc = new Y.Doc();

  // Create separate comment doc
  const tempCommentDoc = new Y.Doc();

  // Create paragraphs for each line of content
  const paragraphNodes = contentLines.map(line => {
    if (line.trim() === '') {
      // Empty paragraph
      return editorSchema.nodes.paragraph.create();
    }
    
    // Check if line should be a heading (starts with # for h1, ## for h2, etc.)
    const headingMatch = line.match(/^(#{1,3})\s+(.+)$/);
    if (headingMatch) {
      const level = headingMatch[1].length as 1 | 2 | 3;
      const text = headingMatch[2];
      return editorSchema.nodes.heading.create(
        { level },
        editorSchema.text(text)
      );
    }
    
    // Check if line should be a bullet list item (starts with - or *)
    const bulletMatch = line.match(/^[-*]\s+(.+)$/);
    if (bulletMatch) {
      const text = bulletMatch[1];
      const listItem = editorSchema.nodes.list_item.create(
        null,
        editorSchema.nodes.paragraph.create(null, editorSchema.text(text))
      );
      return editorSchema.nodes.bullet_list.create(null, [listItem]);
    }
    
    // Check for bold text (**text**)
    let textContent: any[] = [];
    const boldRegex = /\*\*([^*]+)\*\*/g;
    let lastIndex = 0;
    let match;
    
    while ((match = boldRegex.exec(line)) !== null) {
      // Add text before the bold part
      if (match.index > lastIndex) {
        textContent.push(editorSchema.text(line.substring(lastIndex, match.index)));
      }
      // Add bold text
      textContent.push(editorSchema.text(match[1], [editorSchema.marks.strong.create()]));
      lastIndex = boldRegex.lastIndex;
    }
    
    // Add remaining text
    if (lastIndex < line.length) {
      textContent.push(editorSchema.text(line.substring(lastIndex)));
    }
    
    // If no formatting found, just add plain text
    if (textContent.length === 0) {
      textContent = [editorSchema.text(line)];
    }
    
    return editorSchema.nodes.paragraph.create(null, textContent);
  });

  // Create the desired document content
  const contentDoc = editorSchema.nodes.doc.create(null, paragraphNodes);
  
  // Convert ProseMirror nodes to Yjs XML elements
  const convertNodeToYXml = (node: PMNode): Y.XmlElement | Y.XmlText => {
    if (node.isText && node.text) {
      const textContent = node.text;
      const ytext = new Y.XmlText(textContent);
      // Apply marks if any
      if (node.marks && node.marks.length > 0) {
        node.marks.forEach(mark => {
          ytext.format(0, textContent.length, { [mark.type.name]: mark.attrs });
        });
      }
      return ytext;
    }
    
    const yelem = new Y.XmlElement(node.type.name);
    
    // Add attributes if any
    if (node.attrs) {
      Object.entries(node.attrs).forEach(([key, value]) => {
        if (value !== null && value !== undefined) {
          yelem.setAttribute(key, String(value));
        }
      });
    }
    
    // Add children recursively
    if (node.content && node.content.size > 0) {
      const children: (Y.XmlElement | Y.XmlText)[] = [];
      node.content.forEach(child => {
        children.push(convertNodeToYXml(child));
      });
      children.forEach((child, index) => {
        yelem.insert(index, [child]);
      });
    }
    
    return yelem;
  };
  
  // Add content directly to the Yjs fragment in a transaction
  tempYDoc.transact(() => {
    // Set the title in metadata
    const metadata = tempYDoc.getMap('metadata');
    metadata.set('title', title);
    
    // Add document content
    const children: (Y.XmlElement | Y.XmlText)[] = [];
    contentDoc.content.forEach(child => {
      children.push(convertNodeToYXml(child));
    });
    children.forEach((child, index) => {
      xmlFragment.insert(index, [child]);
    });
  });

  // Encode the state
  const noteContent: NoteContent = {
    main_doc: Array.from(Y.encodeStateAsUpdateV2(tempYDoc)),
    image_state: Array.from(Y.encodeStateAsUpdateV2(tempImageDoc)),
    comment_state: Array.from(Y.encodeStateAsUpdateV2(tempCommentDoc)),
    client_id: clientId.toString(),
    last_modified: Date.now(),
    title: title,
  };

  // Clean up temporary docs
  tempYDoc.destroy();
  tempImageDoc.destroy();
  tempCommentDoc.destroy();

  return noteContent;
}


