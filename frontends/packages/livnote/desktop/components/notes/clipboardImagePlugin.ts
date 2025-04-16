import { Plugin } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { DOMParser } from "prosemirror-model";

/**
 * Creates a ProseMirror plugin that handles clipboard content
 * using the navigator.clipboard API
 */
export function clipboardImagePlugin() {
  console.log("Initializing clipboard image plugin");

  return new Plugin({
    props: {
      handlePaste: async (view: EditorView, event: ClipboardEvent) => {
        // Prevent default paste behavior
        event.preventDefault();

        // Process clipboard content
        try {
          await processClipboardContent(view, event);
        } catch (error) {
          console.error("Error in paste handler:", error);
        }

        return true;
      }
    }
  });
}

/**
 * Process clipboard content using the Clipboard API
 */
async function processClipboardContent(view: EditorView, event: ClipboardEvent): Promise<void> {
  // Types to check in order of preference
  const typePreference = [
    'text/html',
    'text/plain',
    'image/png',
    'image/jpeg',
    'image/gif'
  ];

  try {
    // Use the modern clipboard API
    if (navigator.clipboard?.read) {
      const clipboardItems = await navigator.clipboard.read();
      console.log("Got clipboard items:", clipboardItems.length);

      // Process based on type preference
      let processed = false;

      for (const preferredType of typePreference) {
        for (const item of clipboardItems) {
          if (item.types.includes(preferredType)) {
            try {
              const blob = await item.getType(preferredType);

              if (preferredType === 'text/html') {
                const html = await blob.text();
                await handleHtmlContent(view, html);
                processed = true;
              } else if (preferredType === 'text/plain') {
                const text = await blob.text();
                handleTextContent(view, text);
                processed = true;
              } else if (preferredType.startsWith('image/')) {
                const base64Data = await blobToBase64(blob);
                insertImage(view, base64Data);
                processed = true;
              }

              if (processed) return;
            } catch (error) {
              console.error(`Error processing ${preferredType}:`, error);
            }
          }
        }
      }
    } else {
      console.error("navigator.clipboard.read() not supported");
    }
  } catch (error) {
    console.error("Error in clipboard processing:", error);
  }
}

// Using the native Blob.text() method instead of a custom blobToText function

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
      console.log(`Processing ${images.length} images in HTML content`);

      // Process each image to ensure they're accessible
      for (const img of Array.from(images)) {
        const src = img.getAttribute('src');

        if (src) {
          if (src.startsWith('data:')) {
            // Data URLs are already embedded, nothing to do
            console.log("Image already has data URL");
          } else if (src.startsWith('blob:')) {
            try {
              // For blob URLs, fetch the data and convert to base64
              const response = await fetch(src);
              const blob = await response.blob();
              const dataUrl = await blobToBase64(blob);
              img.setAttribute('src', dataUrl);
              console.log("Converted blob URL to data URL");
            } catch (error) {
              console.error("Failed to process blob URL:", error);
              // If we can't process the blob, remove the image
              img.remove();
            }
          } else {
            // For external URLs, we'll leave them as-is
            console.log("External image URL detected:", src);
          }
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

    console.log("HTML content processed successfully");
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
    console.log("Text content processed successfully");
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

    console.log("Image inserted successfully");
  } catch (error) {
    console.error("Error inserting image:", error);
  }
}
