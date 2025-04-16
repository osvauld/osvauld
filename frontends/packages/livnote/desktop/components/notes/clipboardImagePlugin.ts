import { Plugin } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
/**
 * Converts an image file to base64 data URL
 */
function imageToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.readAsDataURL(file);
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = error => reject(error);
  });
}


/**
 * Creates a ProseMirror plugin that handles image paste events
 * and converts images to base64 before inserting them
 */
export function clipboardImagePlugin() {
  console.log("Initializing clipboard image plugin");
  return new Plugin({
    props: {
      handlePaste: async (view: EditorView, event: ClipboardEvent) => {
        const clipboardItems = await navigator.clipboard.read();
        console.log("Clipboard items:", clipboardItems.length);

        for (const item of clipboardItems) {
          console.log("Item types:", item.types);

          // Check for image types
          for (const type of item.types) {
            if (type.startsWith('image/')) {
              console.log("Found image type:", type);

              // Get the image as a blob
              const blob = await item.getType(type);
              console.log("Got image blob:", blob.size, "bytes");

              // Convert blob to base64
              const reader = new FileReader();
              reader.readAsDataURL(blob);

              reader.onload = () => {
                const base64Data = reader.result as string;

                // Create and insert the image node
                const { schema } = view.state;
                const imageNode = schema.nodes.image.create({
                  src: base64Data,
                  alt: 'Pasted image',
                  title: 'Pasted image'
                });

                const tr = view.state.tr.replaceSelectionWith(imageNode);
                view.dispatch(tr);

                console.log("Image inserted successfully");
              };

              // Prevent default handling
              event.preventDefault();
              return true;
            }
          }
        }

        console.log("No image found in clipboard items");
        return false;
      }
    }
  });
}
