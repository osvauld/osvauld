import { Plugin } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { DOMParser, Fragment, Slice, Node as PMNode } from "prosemirror-model";
import { ImageStorageService } from "./imageStorage";

/**
 * Creates a ProseMirror plugin that handles clipboard content
 * using the navigator.clipboard API with optimized asset storage
 */
export function pasteHandlerPlugin(imageStorage: ImageStorageService) {
  return new Plugin({
    props: {
      handlePaste: (view: EditorView, event: ClipboardEvent) => {
        // Process asynchronously but return true to indicate we're handling it
        (async () => {
          try {
            // First try the navigator clipboard API
            const apiHandled = await tryNavigatorClipboardApi(view, event, imageStorage);
            if (apiHandled) {
              return;
            }

            // Fall back to clipboardData
            const clipboardData = event.clipboardData;
            if (clipboardData) {
              const handled = await processClipboardEvent(view, event, imageStorage);
              if (handled) {
                return;
              }
            }
          } catch (error) {
            console.error("Error in paste handler:", error);
          }
        })();

        return true; // Always return true to prevent default
      }
    }
  });
}

/**
 * Tries to process clipboard content using the navigator.clipboard API
 * Returns true if successful, false otherwise
 */
async function tryNavigatorClipboardApi(
  view: EditorView,
  event: ClipboardEvent,
  imageStorage: ImageStorageService
): Promise<boolean> {
  if (!navigator.clipboard?.read) {
    return false;
  }

  try {
    const clipboardItems = await navigator.clipboard.read();

    const typePreference = [
      'text/html',
      'text/plain',
      'image/png',
      'image/jpeg',
      'image/gif',
      'image/webp',
      'image/bmp',
      'image/svg+xml',
      'application/octet-stream',
      'image/*'
    ];

    for (const preferredType of typePreference) {
      for (const item of clipboardItems) {
        if (preferredType === 'image/*') {
          const imageTypes = item.types.filter(type => type.startsWith('image/'));
          if (imageTypes.length > 0) {
            try {
              const imageType = imageTypes[0];
              const blob = await item.getType(imageType);
              const base64Data = await blobToBase64(blob);
              await insertImageWithAssetStorage(view, base64Data, imageType, imageStorage);
              event.preventDefault();
              return true;
            } catch (error) {
              continue;
            }
          }
          continue;
        }

        if (item.types.includes(preferredType)) {
          try {
            const blob = await item.getType(preferredType);

            if (preferredType === 'text/html') {
              const html = await blob.text();
              await handleHtmlContent(view, html, imageStorage);
              event.preventDefault();
              return true;
            } else if (preferredType === 'text/plain') {
              const text = await blob.text();
              handleTextContent(view, text);
              event.preventDefault();
              return true;
            } else if (preferredType.startsWith('image/')) {
              const base64Data = await blobToBase64(blob);
              await insertImageWithAssetStorage(view, base64Data, preferredType, imageStorage);
              event.preventDefault();
              return true;
            } else if (preferredType === 'application/octet-stream') {
              if (isLikelyImage(blob)) {
                const headerBytes = await readBlobHeader(blob, 12);
                if (isProbablyImageHeader(headerBytes)) {
                  const base64Data = await blobToBase64(blob);
                  const mimeType = detectMimeTypeFromHeader(headerBytes);
                  await insertImageWithAssetStorage(view, base64Data, mimeType, imageStorage);
                  event.preventDefault();
                  return true;
                }
              }
            }
          } catch (error) {
            continue;
          }
        }
      }
    }

    return false;
  } catch (error) {
    return false;
  }
}

/**
 * Process clipboard content from ClipboardEvent
 */
async function processClipboardEvent(
  view: EditorView,
  event: ClipboardEvent,
  imageStorage: ImageStorageService
): Promise<boolean> {
  if (!event.clipboardData) return false;

  try {
    // Handle files first
    if (event.clipboardData.files.length > 0) {
      for (let i = 0; i < event.clipboardData.files.length; i++) {
        const file = event.clipboardData.files[i];

        if (file.type.startsWith('image/')) {
          const base64Data = await blobToBase64(file);
          await insertImageWithAssetStorage(view, base64Data, file.type, imageStorage, file.name);
          event.preventDefault();
          return true;
        }

        if (file.type === 'application/octet-stream' || file.type === '') {
          if (file.name && /\.(jpg|jpeg|png|gif|bmp|webp|svg)$/i.test(file.name)) {
            const base64Data = await blobToBase64(file);
            const mimeType = getMimeTypeFromFilename(file.name);
            await insertImageWithAssetStorage(view, base64Data, mimeType, imageStorage, file.name);
            event.preventDefault();
            return true;
          }

          if (isLikelyImage(file)) {
            const headerBytes = await readBlobHeader(file, 12);
            if (isProbablyImageHeader(headerBytes)) {
              const base64Data = await blobToBase64(file);
              const mimeType = detectMimeTypeFromHeader(headerBytes);
              await insertImageWithAssetStorage(view, base64Data, mimeType, imageStorage, file.name);
              event.preventDefault();
              return true;
            }
          }
        }
      }
    }

    // Handle HTML content
    const html = event.clipboardData.getData('text/html');
    if (html) {
      await handleHtmlContent(view, html, imageStorage);
      event.preventDefault();
      return true;
    }

    // Handle plain text
    const text = event.clipboardData.getData('text/plain');
    if (text) {
      handleTextContent(view, text);
      event.preventDefault();
      return true;
    }

    return false;
  } catch (error) {
    console.error("Error processing clipboard:", error);
    return false;
  }
}

/**
 * Insert image using the new asset storage system
 */
async function insertImageWithAssetStorage(
  view: EditorView,
  base64Data: string,
  mimeType: string,
  imageStorage: ImageStorageService,
  filename?: string
): Promise<void> {
  try {
    const imageId = await imageStorage.storeImage(base64Data, mimeType, filename);
    const imageMetadata = imageStorage.getImageMetadata(imageId);

    const { schema } = view.state;
    const imageNode = schema.nodes.image.create({
      src: `yjs-image:${imageId}`,
      alt: filename || 'Pasted image',
      title: filename || 'Pasted image',
      width: imageMetadata?.width,
      height: imageMetadata?.height
    });

    const tr = view.state.tr.replaceSelectionWith(imageNode);
    view.dispatch(tr);
  } catch (error) {
    console.error("Error inserting image with asset storage:", error);
  }
}

/**
 * Handle HTML content from clipboard, including embedded images
 */
async function handleHtmlContent(view: EditorView, html: string, imageStorage: ImageStorageService): Promise<void> {
  try {
    const domElement = document.createElement('div');
    domElement.innerHTML = html;

    // Remove problematic black text colors for dark theme
    const styledElements = domElement.querySelectorAll('*[style]');
    styledElements.forEach(el => {
      if (el instanceof HTMLElement) {
        const style = el.style;
        if (style.color === 'rgb(0, 0, 0)' || style.color === '#000000' || style.color === 'black') {
          style.removeProperty('color');
        }
        if (style.caretColor === 'rgb(0, 0, 0)') {
          style.removeProperty('caret-color');
        }
        if (!style.cssText.trim()) {
          el.removeAttribute('style');
        }
      }
    });

    // Check if content should be parsed as markdown
    const textContent = domElement.textContent || '';
    if (textContent && detectMarkdown(textContent)) {
      handleTextContent(view, textContent);
      return;
    }

    // Process link formatting
    const spansToProcess = domElement.querySelectorAll('span[style*="text-decoration"]');
    spansToProcess.forEach(span => {
      if (span instanceof HTMLElement && span.style.textDecoration.includes('underline')) {
        let childLinkElement: HTMLAnchorElement | null = null;
        let hasOtherSignificantContent = false;

        const significantChildNodes = Array.from(span.childNodes).filter(node => {
          if (node.nodeType === Node.ELEMENT_NODE) return true;
          if (node.nodeType === Node.TEXT_NODE && node.textContent?.trim()) return true;
          return false;
        });

        if (significantChildNodes.length === 1 &&
          significantChildNodes[0].nodeName === 'A' &&
          (significantChildNodes[0] as HTMLAnchorElement).hasAttribute('href')) {
          childLinkElement = significantChildNodes[0] as HTMLAnchorElement;
        } else {
          hasOtherSignificantContent = true;
        }

        if (childLinkElement && !hasOtherSignificantContent) {
          const newLink = childLinkElement.cloneNode(true) as HTMLAnchorElement;
          let existingLinkStyle = newLink.getAttribute('style') || '';
          if (existingLinkStyle && !existingLinkStyle.trim().endsWith(';')) {
            existingLinkStyle += '; ';
          }
          if (!newLink.style.textDecoration.includes('underline')) {
            newLink.setAttribute('style', `${existingLinkStyle}text-decoration: underline;`);
          }
          if (span.parentNode) {
            span.parentNode.replaceChild(newLink, span);
          }
        }
      }
    });

    // Clean up anchor text decoration
    const allAnchors = domElement.querySelectorAll('a');
    allAnchors.forEach(anchor => {
      if (anchor.style.textDecoration.includes('underline')) {
        anchor.style.textDecoration = anchor.style.textDecoration.replace(/underline/g, '').trim();
        if (anchor.style.textDecoration === '') {
          anchor.removeAttribute('style');
        }
        const styleAttr = anchor.getAttribute('style');
        if (styleAttr && styleAttr.trim() === '') {
          anchor.removeAttribute('style');
        }
      }
    });

    // Process images
    const images = domElement.querySelectorAll('img');
    if (images.length > 0) {
      for (let i = 0; i < images.length; i++) {
        const img = images[i];
        const src = img.getAttribute('src');

        if (src) {
          if (src.startsWith('data:')) {
            try {
              const mimeMatch = src.match(/^data:([^;]+);/);
              const mimeType = mimeMatch ? mimeMatch[1] : 'image/png';
              const imageId = await imageStorage.storeImage(src, mimeType);
              img.setAttribute('src', `yjs-image:${imageId}`);

              const imageMetadata = imageStorage.getImageMetadata(imageId);
              if (imageMetadata?.width) img.setAttribute('width', imageMetadata.width.toString());
              if (imageMetadata?.height) img.setAttribute('height', imageMetadata.height.toString());
            } catch (error) {
              console.error('Error processing data: image:', error);
              img.remove();
            }
          } else if (src.startsWith('blob:')) {
            try {
              const response = await fetch(src);
              const blob = await response.blob();
              const dataUrl = await blobToBase64(blob);
              const imageId = await imageStorage.storeImage(dataUrl, blob.type);
              img.setAttribute('src', `yjs-image:${imageId}`);

              const imageMetadata = imageStorage.getImageMetadata(imageId);
              if (imageMetadata?.width) img.setAttribute('width', imageMetadata.width.toString());
              if (imageMetadata?.height) img.setAttribute('height', imageMetadata.height.toString());
            } catch (error) {
              console.error('Error processing blob: image:', error);
              img.remove();
            }
          }
        } else {
          img.remove();
        }
      }
    }

    // Parse and insert content
    const { schema } = view.state;
    const parser = DOMParser.fromSchema(schema);
    const slice = parser.parseSlice(domElement);

    const filteredNodes: PMNode[] = [];
    slice.content.forEach(node => {
      if (node.type === schema.nodes.paragraph) {
        if (node.content.size > 0 && node.textContent.trim() !== '') {
          filteredNodes.push(node);
        }
      } else {
        filteredNodes.push(node);
      }
    });

    if (filteredNodes.length === 0 && slice.content.size > 0) {
      return;
    }

    const newFragment = Fragment.fromArray(filteredNodes);
    const newSlice = new Slice(newFragment, slice.openStart, slice.openEnd);
    const tr = view.state.tr.replaceSelection(newSlice);
    view.dispatch(tr);
  } catch (error) {
    console.error("Error handling HTML content:", error);
  }
}

/**
 * Handle plain text content from clipboard
 */
function handleTextContent(view: EditorView, text: string): void {
  try {
    if (detectMarkdown(text)) {
      try {
        const nodes = parseMarkdown(text, view.state.schema);
        if (nodes.length > 0) {
          const fragment = Fragment.from(nodes);
          const slice = new Slice(fragment, 0, 0);
          const tr = view.state.tr.replaceSelection(slice);
          view.dispatch(tr);
          return;
        }
      } catch (error) {
        console.warn("Failed to parse as markdown, falling back to plain text:", error);
      }
    }

    const tr = view.state.tr.insertText(text);
    view.dispatch(tr);
  } catch (error) {
    console.error("Error handling text content:", error);
  }
}

// Include all the helper functions from your existing code
function parseMarkdown(text: string, schema: any): PMNode[] {
  const nodes: PMNode[] = [];
  const lines = text.split('\n');

  let i = 0;
  while (i < lines.length) {
    const line = lines[i];

    // Code blocks
    if (line.trim().startsWith('```')) {
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith('```')) {
        codeLines.push(lines[i]);
        i++;
      }
      const codeContent = codeLines.join('\n');
      nodes.push(schema.nodes.code_block.create({},
        codeContent ? schema.text(codeContent) : schema.text('')
      ));
      i++;
      continue;
    }

    // Headers
    const headerMatch = line.match(/^(#{1,6})\s+(.+)$/);
    if (headerMatch) {
      const level = headerMatch[1].length;
      const content = parseInlineMarkdown(headerMatch[2], schema);
      nodes.push(schema.nodes.heading.create({ level }, content));
      i++;
      continue;
    }

    // Other markdown parsing logic...
    // (Include the rest of your parseMarkdown implementation)

    // Regular paragraph
    if (line.trim() === '') {
      i++;
      continue;
    }

    const content = parseInlineMarkdown(line, schema);
    if (content.size > 0) {
      nodes.push(schema.nodes.paragraph.create({}, content));
    }
    i++;
  }

  return nodes;
}

function parseInlineMarkdown(text: string, schema: any): Fragment {
  // Your existing inline markdown parsing implementation
  if (!text) return Fragment.empty;
  return Fragment.from([schema.text(text)]);
}

function detectMarkdown(text: string): boolean {
  let markdownScore = 0;
  const threshold = 1.5;

  const strongIndicators = [
    { pattern: /^#{1,6}\s+\S/m, score: 2 },
    { pattern: /^```[^`]*```/ms, score: 3 },
    { pattern: /^\s*```\w*\s*$/m, score: 3 },
    { pattern: /^\s*[-*+]\s+\S.*(\n\s*[-*+]\s+|$)/m, score: 2 },
    { pattern: /^\s*\d+\.\s+\S.*(\n\s*\d+\.\s+|$)/m, score: 2 },
    { pattern: /^\s*>\s+\S/m, score: 2 },
    { pattern: /\[([^\]]+)\]\(([^)]+)\)/g, score: 2 },
    { pattern: /^[-*_]{3,}\s*$/m, score: 2 },
  ];

  for (const { pattern, score } of strongIndicators) {
    if (pattern.test(text)) {
      markdownScore += score;
    }
  }
  console.log(markdownScore, "md score")

  return markdownScore >= threshold;
}

// Utility functions
function isLikelyImage(blob: Blob): boolean {
  return blob.size > 10 && blob.size < 20 * 1024 * 1024;
}

function isProbablyImageHeader(bytes: Uint8Array): boolean {
  if (bytes.length < 4) return false;

  // JPEG
  if (bytes[0] === 0xFF && bytes[1] === 0xD8 && bytes[2] === 0xFF) return true;
  // PNG
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4E && bytes[3] === 0x47) return true;
  // GIF
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x38) return true;
  // BMP
  if (bytes[0] === 0x42 && bytes[1] === 0x4D) return true;

  return false;
}

async function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

async function readBlobHeader(blob: Blob, bytesToRead: number): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const arrayBuffer = reader.result as ArrayBuffer;
      resolve(new Uint8Array(arrayBuffer));
    };
    reader.onerror = () => reject(reader.error);

    const slicedBlob = blob.slice(0, bytesToRead);
    reader.readAsArrayBuffer(slicedBlob);
  });
}

function detectMimeTypeFromHeader(bytes: Uint8Array): string {
  if (bytes[0] === 0xFF && bytes[1] === 0xD8 && bytes[2] === 0xFF) return 'image/jpeg';
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4E && bytes[3] === 0x47) return 'image/png';
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x38) return 'image/gif';
  if (bytes[0] === 0x42 && bytes[1] === 0x4D) return 'image/bmp';
  return 'image/png';
}

function getMimeTypeFromFilename(filename: string): string {
  const ext = filename.toLowerCase().split('.').pop();
  const mimeMap: { [key: string]: string } = {
    'jpg': 'image/jpeg',
    'jpeg': 'image/jpeg',
    'png': 'image/png',
    'gif': 'image/gif',
    'bmp': 'image/bmp',
    'webp': 'image/webp',
    'svg': 'image/svg+xml'
  };
  return mimeMap[ext || ''] || 'image/png';
}
