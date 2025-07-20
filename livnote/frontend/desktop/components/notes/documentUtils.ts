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
 * Process document content to replace yjs-image URLs with base64 data
 */
function processDocumentImages(content: any[], assets: any[]): any[] {
  if (!content || !Array.isArray(content)) {
    return content;
  }

  // Create a map for quick asset lookup
  const assetMap = new Map();
  assets.forEach(asset => {
    assetMap.set(asset.id, asset.data);
  });

  return content.map(node => processNode(node, assetMap));
}

function processNode(node: any, assetMap: Map<string, string>): any {
  if (!node || typeof node !== 'object') {
    return node;
  }

  // Handle image nodes specifically
  if (node.type === 'image' && node.attrs?.src?.startsWith('yjs-image:')) {
    const imageId = node.attrs.src.replace('yjs-image:', '');
    const base64Data = assetMap.get(imageId);

    if (base64Data) {
      return {
        ...node,
        attrs: {
          ...node.attrs,
          src: base64Data
        }
      };
    } else {
      // If image not found, replace with placeholder text
      console.warn(`Image ${imageId} not found in assets for preview`);
      return {
        type: 'paragraph',
        content: [{
          type: 'text',
          text: '[Image not available]'
        }]
      };
    }
  }

  // Recursively process content if it exists
  if (node.content && Array.isArray(node.content)) {
    return {
      ...node,
      content: node.content.map((child: any) => processNode(child, assetMap))
    };
  }

  return node;
}

/**
 * Generate a lightweight preview from full note data
 * Takes only the first few nodes from the editor state to reduce memory usage
 */
/**
 * Debug version of generatePreview to understand the data structure
 */
export function generatePreview(fullNote: Note, maxNodes: number = 3): NotePreview {
  let previewHTML = "";

  try {

    if (fullNote.data.editor_state) {
      let editorState;

      // Parse editor state if it's a string
      if (typeof fullNote.data.editor_state === 'string') {
        editorState = JSON.parse(fullNote.data.editor_state);
      } else {
        editorState = fullNote.data.editor_state;
      }

      // Create truncated and processed content
      if (editorState && editorState.doc && editorState.doc.content) {
        const originalContent = editorState.doc.content;

        // Take only the first maxNodes nodes
        const truncatedContent = originalContent.slice(0, maxNodes);
        // Process images in the truncated content
        const processedContent = processDocumentImages(
          truncatedContent,
          fullNote.data.assets || []
        );

        // Convert to HTML
        previewHTML = contentToHTML(processedContent);
      }
    } else {
      console.log('No editor_state found in note data');
    }

    // Try fallback to content field
    if (!previewHTML.trim() && fullNote.data.content) {

      if (Array.isArray(fullNote.data.content)) {
        previewHTML = contentToHTML(fullNote.data.content.slice(0, maxNodes));
      } else if (typeof fullNote.data.content === 'object' && fullNote.data.content.content) {
        previewHTML = contentToHTML(fullNote.data.content.content.slice(0, maxNodes));
      }

    }


  } catch (error) {
    console.error('Error generating preview for note:', fullNote.id, error);
  }



  return {
    id: fullNote.id,
    title: fullNote.data.title,
    previewHTML,
    favourite: fullNote.favourite,
    folderId: fullNote.folderId,
    lastModified: fullNote.data.last_modified,
    lastAccessed: fullNote.data.last_accessed,
  };
}
/**
 * Convert ProseMirror content nodes to HTML
 */
function contentToHTML(content: any[]): string {
  if (!content || !Array.isArray(content)) {
    return "";
  }

  return content.map(node => nodeToHTML(node)).join('');
}

function nodeToHTML(node: any): string {
  if (!node || typeof node !== 'object') {
    return "";
  }

  switch (node.type) {
    case 'paragraph':
      const pContent = node.content ? contentToHTML(node.content) : '';
      return `<p>${pContent}</p>`;

    case 'heading':
      const level = node.attrs?.level || 1;
      const hContent = node.content ? contentToHTML(node.content) : '';
      return `<h${level}>${hContent}</h${level}>`;

    case 'text':
      let text = node.text || '';

      // Apply marks (bold, italic, etc.)
      if (node.marks && Array.isArray(node.marks)) {
        node.marks.forEach((mark: any) => {
          switch (mark.type) {
            case 'strong':
              text = `<strong>${text}</strong>`;
              break;
            case 'em':
              text = `<em>${text}</em>`;
              break;
            case 'code':
              text = `<code>${text}</code>`;
              break;
            case 'link':
              const href = mark.attrs?.href || '#';
              text = `<a href="${href}">${text}</a>`;
              break;
          }
        });
      }

      return text;

    case 'hard_break':
      return '<br>';

    case 'image':
      const src = node.attrs?.src || '';
      const alt = node.attrs?.alt || '';
      const width = node.attrs?.width ? ` width="${node.attrs.width}"` : '';
      const height = node.attrs?.height ? ` height="${node.attrs.height}"` : '';
      return `<img src="${src}" alt="${alt}"${width}${height} style="max-width: 100%; height: auto;">`;

    case 'bullet_list':
      const ulContent = node.content ? contentToHTML(node.content) : '';
      return `<ul>${ulContent}</ul>`;

    case 'ordered_list':
      const olContent = node.content ? contentToHTML(node.content) : '';
      return `<ol>${olContent}</ol>`;

    case 'list_item':
      const liContent = node.content ? contentToHTML(node.content) : '';
      return `<li>${liContent}</li>`;

    case 'blockquote':
      const bqContent = node.content ? contentToHTML(node.content) : '';
      return `<blockquote>${bqContent}</blockquote>`;

    case 'code_block':
      const codeContent = node.content ? contentToHTML(node.content) : '';
      return `<pre><code>${codeContent}</code></pre>`;

    default:
      // For unknown node types, try to render content if it exists
      if (node.content && Array.isArray(node.content)) {
        return contentToHTML(node.content);
      }
      return '';
  }
}


