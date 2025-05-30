import { Plugin } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { DOMParser } from "prosemirror-model";

/**
 * Creates a ProseMirror plugin that handles clipboard content
 * using the navigator.clipboard API
 */
export function pasteHandlerPlugin() {
  return new Plugin({
    props: {
      handlePaste: (view: EditorView, event: ClipboardEvent) => {
        // Wrap async logic in an IIAFE
        (async () => {
          const clipboardData = event.clipboardData;

          try {
            // First try to handle with modern clipboard API
            const apiHandled = await tryNavigatorClipboardApi(view, event); // Pass event

            if (apiHandled) {
              // preventDefault is called inside tryNavigatorClipboardApi if handled
              return; // Exit IIAFE
            }

            // If the clipboard API fails or isn't supported, fall back to event.clipboardData
            if (clipboardData) {
              const handled = await processClipboardEvent(view, event); // Pass event
              if (handled) {
                 // preventDefault is called inside processClipboardEvent if handled
                return; // Exit IIAFE
              }
            }
          } catch (error) {
            console.error("Error in paste handler:", error);
          }
        })(); // Immediately invoke the async function

        // Let ProseMirror continue for now; async task will prevent default if handled
        return true; // Indicate that the plugin will handle the paste
      }
    }
  });
}

/**
 * Tries to process clipboard content using the navigator.clipboard API
 * Returns true if successful, false otherwise
 */
async function tryNavigatorClipboardApi(view: EditorView, event: ClipboardEvent): Promise<boolean> {
  if (!navigator.clipboard?.read) {
    return false;
  }

  try {
    const clipboardItems = await navigator.clipboard.read();
    
    // Types to check in order of preference
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

    // Process based on type preference
    for (const preferredType of typePreference) {
      for (const item of clipboardItems) {
        // Handle wildcard matching for image/*
        if (preferredType === 'image/*') {
          const imageTypes = item.types.filter(type => type.startsWith('image/'));
          if (imageTypes.length > 0) {
            try {
              const imageType = imageTypes[0];
              const blob = await item.getType(imageType);
              const base64Data = await blobToBase64(blob);
              insertImage(view, base64Data);
              event.preventDefault(); // Prevent default on success
              return true;
            } catch (error) {
              continue;
            }
          }
          continue;
        }
        
        // Regular type matching
        if (item.types.includes(preferredType)) {
          try {
            const blob = await item.getType(preferredType);

            if (preferredType === 'text/html') {
              const html = await blob.text();
              await handleHtmlContent(view, html);
              event.preventDefault(); // Prevent default on success
              return true;
            } else if (preferredType === 'text/plain') {
              const text = await blob.text();
              handleTextContent(view, text);
              event.preventDefault(); // Prevent default on success
              return true;
            } else if (preferredType.startsWith('image/')) {
              const base64Data = await blobToBase64(blob);
              insertImage(view, base64Data);
              event.preventDefault(); // Prevent default on success
              return true;
            } else if (preferredType === 'application/octet-stream') {
              if (isLikelyImage(blob)) {
                const headerBytes = await readBlobHeader(blob, 12);
                if (isProbablyImageHeader(headerBytes)) {
                  const base64Data = await blobToBase64(blob);
                  insertImage(view, base64Data);
                  event.preventDefault(); // Prevent default on success
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
async function processClipboardEvent(view: EditorView, event: ClipboardEvent): Promise<boolean> {
  if (!event.clipboardData) return false;
  
  try {
    // Check for files first (direct image paste)
    if (event.clipboardData.files.length > 0) {
      for (let i = 0; i < event.clipboardData.files.length; i++) {
        const file = event.clipboardData.files[i];
        
        // Check if it's an image file
        if (file.type.startsWith('image/')) {
          const base64Data = await blobToBase64(file);
          insertImage(view, base64Data);
          event.preventDefault(); // Prevent default on success
          return true;
        } 
        
        // Handle files marked as 'application/octet-stream' but likely images
        if (file.type === 'application/octet-stream' || file.type === '') {
          // Check filename extension if available
          if (file.name && /\.(jpg|jpeg|png|gif|bmp|webp|svg)$/i.test(file.name)) {
            const base64Data = await blobToBase64(file);
            insertImage(view, base64Data);
            event.preventDefault(); // Prevent default on success
            return true;
          }
          
          // Size-based heuristic for images
          if (isLikelyImage(file)) {
            const headerBytes = await readBlobHeader(file, 12);
            if (isProbablyImageHeader(headerBytes)) {
              const base64Data = await blobToBase64(file);
              insertImage(view, base64Data);
              event.preventDefault(); // Prevent default on success
              return true;
            }
          }
        }
      }
    }
    
    // Check for HTML content
    const html = event.clipboardData.getData('text/html');
    if (html) {
      await handleHtmlContent(view, html);
      event.preventDefault(); // Prevent default on success
      return true;
    }
    
    // Fall back to text
    const text = event.clipboardData.getData('text/plain');
    if (text) {
      handleTextContent(view, text);
      event.preventDefault(); // Prevent default on success
      return true;
    }
    
    return false;
  } catch (error) {
    return false;
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
 * Handle HTML content from clipboard, including embedded images
 */
async function handleHtmlContent(view: EditorView, html: string): Promise<void> {
  try {
    // Create a temporary element to hold the HTML
    const domElement = document.createElement('div');
    domElement.innerHTML = html;

    // Process embedded images to ensure they're properly handled
    const images = domElement.querySelectorAll('img');

    if (images.length > 0) {
      // Process each image to ensure they're accessible
      for (let i = 0; i < images.length; i++) {
        const img = images[i];
        const src = img.getAttribute('src');

        if (src) {
          if (src.startsWith('data:')) {
            // Data URLs are already embedded, nothing to do
          } else if (src.startsWith('blob:')) {
            try {
              const response = await fetch(src);
              const blob = await response.blob();
              const dataUrl = await blobToBase64(blob);
              img.setAttribute('src', dataUrl);
            } catch (error) {
              img.remove();
            }
          }
        } else {
          img.remove();
        }
      }
    }

    // Parse into ProseMirror format
    const { schema } = view.state;
    const parser = DOMParser.fromSchema(schema);
    const slice = parser.parseSlice(domElement);

    // Insert the content with processed images
    const tr = view.state.tr.replaceSelection(slice);
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
 * Insert an image node at the current selection
 */
function insertImage(view: EditorView, src: string): void {
  try {
    const { schema } = view.state;
    const imageNode = schema.nodes.image.create({
      src: src,
      alt: 'Pasted image',
      title: 'Pasted image'
    });

    const tr = view.state.tr.replaceSelectionWith(imageNode);
    view.dispatch(tr);
  } catch (error) {
    console.error("Error inserting image:", error);
  }
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