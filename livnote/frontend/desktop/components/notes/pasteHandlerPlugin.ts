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
        (async () => {
          const clipboardData = event.clipboardData;

          try {
            const apiHandled = await tryNavigatorClipboardApi(view, event, imageStorage);

            if (apiHandled) {
              return;
            }

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

        return true;
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

    const html = event.clipboardData.getData('text/html');
    if (html) {
      await handleHtmlContent(view, html, imageStorage);
      event.preventDefault();
      return true;
    }

    const text = event.clipboardData.getData('text/plain');
    if (text) {
      handleTextContent(view, text);
      event.preventDefault();
      return true;
    }

    return false;
  } catch (error) {
    return false;
  }
}

/**
 * Insert image using the new asset storage system
 * This replaces the old insertImageWithStorage function
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
      src: `yjs-image:${imageId}`, // Use the same protocol for consistency
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
 * Updated to use the new asset storage system
 */
async function handleHtmlContent(view: EditorView, html: string, imageStorage: ImageStorageService): Promise<void> {
  try {
    const domElement = document.createElement('div');
    domElement.innerHTML = html;

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
              img.remove(); // Remove if blob fetch fails
            }
          }
        } else {
          img.remove(); // Remove if no src
        }
      }
    }

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
      return; // Nothing to insert
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
    const tr = view.state.tr.insertText(text);
    view.dispatch(tr);
  } catch (error) {
    console.error("Error handling text content:", error);
  }
}



/**
 * Check if a blob is likely an image based on magic numbers/signatures
 */
function isLikelyImage(blob: Blob): boolean {
  return blob.size > 10 && blob.size < 20 * 1024 * 1024;
}

/**
 * Check if byte array matches common image format headers
 */
function isProbablyImageHeader(bytes: Uint8Array): boolean {
  if (bytes.length < 4) return false;

  // JPEG - starts with FF D8 FF
  if (bytes[0] === 0xFF && bytes[1] === 0xD8 && bytes[2] === 0xFF) {
    return true;
  }

  // PNG - starts with 89 50 4E 47
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4E && bytes[3] === 0x47) {
    return true;
  }

  // GIF - starts with 47 49 46 38
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x38) {
    return true;
  }

  // BMP - starts with 42 4D
  if (bytes[0] === 0x42 && bytes[1] === 0x4D) {
    return true;
  }

  // WEBP - starts with 52 49 46 46 and has WEBP at offset 8
  if (bytes.length >= 12 &&
    bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46 &&
    bytes[8] === 0x57 && bytes[9] === 0x45 && bytes[10] === 0x42 && bytes[11] === 0x50) {
    return true;
  }

  // SVG - starts with either '<svg' or '<?xml'
  if (bytes.length >= 5) {
    // Check for '<svg'
    if (bytes[0] === 0x3C && bytes[1] === 0x73 && bytes[2] === 0x76 && bytes[3] === 0x67) {
      return true;
    }
    // Check for '<?xml'
    if (bytes[0] === 0x3C && bytes[1] === 0x3F && bytes[2] === 0x78 && bytes[3] === 0x6D && bytes[4] === 0x6C) {
      return true;
    }
  }

  return false;
}

/**
 * Convert blob to base64 string using a FileReader with proper async/await pattern
 */
async function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

/**
 * Read the first few bytes of a blob to analyze its header
 */
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
  if (bytes[0] === 0xFF && bytes[1] === 0xD8 && bytes[2] === 0xFF) {
    return 'image/jpeg';
  }
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4E && bytes[3] === 0x47) {
    return 'image/png';
  }
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x38) {
    return 'image/gif';
  }
  if (bytes[0] === 0x42 && bytes[1] === 0x4D) {
    return 'image/bmp';
  }
  if (bytes.length >= 12 &&
    bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46 &&
    bytes[8] === 0x57 && bytes[9] === 0x45 && bytes[10] === 0x42 && bytes[11] === 0x50) {
    return 'image/webp';
  }
  return 'image/png'; // Default
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
