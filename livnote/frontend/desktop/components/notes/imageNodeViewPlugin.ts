import type { NodeView } from "prosemirror-view";
import { Node as PMNode } from "prosemirror-model";
import { EditorView } from "prosemirror-view";
import { ImageStorageService } from "./imageStorage";
import { Plugin, PluginKey } from "prosemirror-state";

class ImageNodeView implements NodeView {
  dom: HTMLElement;
  img: HTMLImageElement;
  placeholder: HTMLDivElement;
  private imageId: string | null = null;
  private checkInterval: number | null = null;
  private loadingTimeout: number | null = null;
  private isDestroyed: boolean = false;

  constructor(
    node: PMNode,
    view: EditorView,
    getPos: () => number,
    imageStorage: ImageStorageService
  ) {
    // Create container
    this.dom = document.createElement('div');
    this.dom.className = 'image-node-container';
    this.dom.style.display = 'inline-block';
    this.dom.style.position = 'relative';

    // Create placeholder while loading
    this.placeholder = document.createElement('div');
    this.placeholder.className = 'image-placeholder';
    this.placeholder.style.width = node.attrs.width ? `${node.attrs.width}px` : '200px';
    this.placeholder.style.height = node.attrs.height ? `${node.attrs.height}px` : '150px';
    this.placeholder.style.backgroundColor = '#2a2b35';
    this.placeholder.style.display = 'flex';
    this.placeholder.style.alignItems = 'center';
    this.placeholder.style.justifyContent = 'center';
    this.placeholder.style.borderRadius = '4px';
    this.placeholder.textContent = 'Loading image...';
    this.placeholder.style.color = '#85889C';

    // Create image element
    this.img = document.createElement('img');
    this.img.alt = node.attrs.alt || '';
    this.img.title = node.attrs.title || '';
    if (node.attrs.width) this.img.width = node.attrs.width;
    if (node.attrs.height) this.img.height = node.attrs.height;
    this.img.style.display = 'none';
    this.img.style.maxWidth = '100%';
    this.img.style.height = 'auto';

    // Add to container
    this.dom.appendChild(this.placeholder);
    this.dom.appendChild(this.img);

    // Bind event handlers
    this.handleImagesLoaded = this.handleImagesLoaded.bind(this);
    this.handleAssetsLoaded = this.handleAssetsLoaded.bind(this);

    // Listen for both legacy and new image loading events
    document.addEventListener('images-loaded', this.handleImagesLoaded);
    document.addEventListener('assets-loaded', this.handleAssetsLoaded);

    // Load image
    this.loadImage(node.attrs.src, imageStorage);
  }

  /**
   * Handle legacy images-loaded event
   */
  private handleImagesLoaded(event: Event) {
    if (this.isDestroyed) return;

    // When images are loaded, try to display our image
    if (this.imageId && !this.img.src) {
      const imageStorage = (window as any).notesInstance?.getImageStorage();
      if (imageStorage) {
        this.tryDisplayImage(imageStorage);
      }
    }
  }

  /**
   * Handle new assets-loaded event
   */
  private handleAssetsLoaded(event: Event) {
    if (this.isDestroyed) return;

    console.log(`[ImageNodeView] Assets loaded event received for image ${this.imageId}`);

    if (this.imageId && !this.img.src) {
      const imageStorage = (window as any).notesInstance?.getImageStorage();
      if (imageStorage) {
        this.tryDisplayImage(imageStorage);
      }
    }
  }

  /**
   * Try to display the image from storage
   */
  private tryDisplayImage(imageStorage: ImageStorageService) {
    if (this.isDestroyed || !this.imageId) return;

    try {
      const imageSrc = imageStorage.getImageSrc(this.imageId);
      if (imageSrc) {
        console.log(`[ImageNodeView] Found image data for ${this.imageId}, displaying`);
        this.displayImage(imageSrc);
        this.clearIntervals();
      } else {
        console.log(`[ImageNodeView] Image data not yet available for ${this.imageId}`);
      }
    } catch (error) {
      console.error(`[ImageNodeView] Error getting image src for ${this.imageId}:`, error);
    }
  }

  /**
   * Display the image and hide placeholder
   */
  private displayImage(src: string) {
    if (this.isDestroyed) return;

    this.img.src = src;
    this.img.onload = () => {
      if (this.isDestroyed) return;
      this.placeholder.style.display = 'none';
      this.img.style.display = 'block';
      console.log(`[ImageNodeView] Successfully displayed image ${this.imageId}`);
    };
    this.img.onerror = () => {
      if (this.isDestroyed) return;
      console.error(`[ImageNodeView] Failed to load image ${this.imageId}`);
      this.placeholder.textContent = 'Failed to load image';
      this.placeholder.style.color = '#ff5630';
    };
  }

  /**
   * Clear all intervals and timeouts
   */
  private clearIntervals() {
    if (this.checkInterval) {
      clearInterval(this.checkInterval);
      this.checkInterval = null;
    }
    if (this.loadingTimeout) {
      clearTimeout(this.loadingTimeout);
      this.loadingTimeout = null;
    }
  }

  /**
   * Load image with the new asset storage system
   */
  private async loadImage(src: string, imageStorage: ImageStorageService) {
    if (this.isDestroyed) return;

    if (src.startsWith('yjs-image:')) {
      // Extract image ID
      this.imageId = src.replace('yjs-image:', '');
      console.log(`[ImageNodeView] Loading image with ID: ${this.imageId}`);

      // Try to get image immediately from storage
      const imageSrc = imageStorage.getImageSrc(this.imageId);

      if (imageSrc) {
        console.log(`[ImageNodeView] Image ${this.imageId} found immediately in storage`);
        this.displayImage(imageSrc);
      } else {
        console.log(`[ImageNodeView] Image ${this.imageId} not immediately available, setting up polling`);
        this.placeholder.textContent = 'Loading image...';

        // Set up polling to check for the image
        this.checkInterval = window.setInterval(() => {
          if (this.isDestroyed) {
            this.clearIntervals();
            return;
          }

          const imageSrc = imageStorage.getImageSrc(this.imageId!);
          if (imageSrc) {
            console.log(`[ImageNodeView] Image ${this.imageId} found via polling`);
            this.displayImage(imageSrc);
            this.clearIntervals();
          }
        }, 100); // Check every 100ms

        // Set a timeout to stop trying after 30 seconds
        this.loadingTimeout = window.setTimeout(() => {
          if (this.isDestroyed) return;

          console.warn(`[ImageNodeView] Timeout loading image ${this.imageId}`);
          this.placeholder.textContent = 'Image not found';
          this.placeholder.style.color = '#ff5630';
          this.clearIntervals();
        }, 30000);
      }
    } else if (src.startsWith('data:') || src.startsWith('http') || src.startsWith('blob:')) {
      // Handle regular URLs, data URLs, or blob URLs
      console.log(`[ImageNodeView] Loading external image: ${src.substring(0, 50)}...`);
      this.img.src = src;
      this.img.onload = () => {
        if (this.isDestroyed) return;
        this.placeholder.style.display = 'none';
        this.img.style.display = 'block';
        console.log(`[ImageNodeView] Successfully loaded external image`);
      };
      this.img.onerror = () => {
        if (this.isDestroyed) return;
        console.error(`[ImageNodeView] Failed to load external image`);
        this.placeholder.textContent = 'Failed to load image';
        this.placeholder.style.color = '#ff5630';
      };
    } else {
      // Unknown src format
      console.warn(`[ImageNodeView] Unknown image src format: ${src}`);
      this.placeholder.textContent = 'Unknown image format';
      this.placeholder.style.color = '#ff5630';
    }
  }

  /**
   * Handle node selection
   */
  selectNode() {
    this.dom.classList.add('ProseMirror-selectednode');
  }

  /**
   * Handle node deselection
   */
  deselectNode() {
    this.dom.classList.remove('ProseMirror-selectednode');
  }

  /**
   * Stop event propagation for certain events
   */
  stopEvent(event: Event) {
    return false;
  }

  /**
   * Ignore mutations to avoid unnecessary re-renders
   */
  ignoreMutation() {
    return true;
  }

  /**
   * Clean up when the node view is destroyed
   */
  destroy() {
    console.log(`[ImageNodeView] Destroying image node view for ${this.imageId}`);
    this.isDestroyed = true;

    // Clear all intervals and timeouts
    this.clearIntervals();

    // Remove event listeners
    document.removeEventListener('images-loaded', this.handleImagesLoaded);
    document.removeEventListener('assets-loaded', this.handleAssetsLoaded);

    // Clear image src to stop any ongoing loading
    if (this.img) {
      this.img.src = '';
      this.img.onload = null;
      this.img.onerror = null;
    }
  }
}

/**
 * Plugin to register the custom node view with enhanced asset storage support
 */
export function imageNodeViewPlugin(imageStorage: ImageStorageService) {
  return new Plugin({
    key: new PluginKey('imageNodeView'),
    props: {
      nodeViews: {
        image: (node: any, view: any, getPos: any) => {
          console.log(`[ImageNodeViewPlugin] Creating image node view for: ${node.attrs.src}`);
          return new ImageNodeView(node, view, getPos, imageStorage);
        }
      }
    },
    view(editorView) {
      return {
        update: (view, prevState) => {
          // Optional: Handle view updates if needed
        },
        destroy: () => {
          // Optional: Plugin cleanup
          console.log('[ImageNodeViewPlugin] Plugin destroyed');
        }
      };
    }
  });
}

/**
 * Utility function to notify all image node views that assets have been loaded
 * This should be called from the Notes class when assets are loaded
 */
export function notifyImageNodesAssetsLoaded(noteId?: string) {
  console.log(`[ImageNodeViewPlugin] Notifying image nodes that assets are loaded for note: ${noteId || 'current'}`);

  // Dispatch both events for backward compatibility
  document.dispatchEvent(new CustomEvent('images-loaded', {
    detail: { noteId, type: 'assets' }
  }));

  document.dispatchEvent(new CustomEvent('assets-loaded', {
    detail: { noteId, type: 'assets' }
  }));
}
