import type { NodeView } from "prosemirror-view";
import { Node as PMNode } from "prosemirror-model";
import { EditorView } from "prosemirror-view";
import { ImageStorageService } from "./imageStorage";
import { Plugin, PluginKey } from "prosemirror-state";

class LazyImageNodeView implements NodeView {
  dom: HTMLElement;
  private imageId: string | null = null;
  private isDestroyed: boolean = false;
  private isFullyLoaded: boolean = false;
  private intersectionObserver: IntersectionObserver | null = null;

  // Only create these when needed
  private img: HTMLImageElement | null = null;
  private placeholder: HTMLDivElement | null = null;

  // Store node data for lazy loading
  private nodeAttrs: any;
  private imageStorage: ImageStorageService;

  constructor(
    node: PMNode,
    view: EditorView,
    getPos: () => number,
    imageStorage: ImageStorageService
  ) {
    // Store data for later use
    this.nodeAttrs = node.attrs;
    this.imageStorage = imageStorage;

    // Extract image ID early
    if (node.attrs.src?.startsWith('yjs-image:')) {
      this.imageId = node.attrs.src.replace('yjs-image:', '');
    }

    // Create minimal DOM structure
    this.createMinimalPlaceholder();

    // Set up intersection observer for lazy loading
    this.setupIntersectionObserver();

  }

  /**
   * Create minimal placeholder - very lightweight
   */
  private createMinimalPlaceholder(): void {
    this.dom = document.createElement('div');
    this.dom.className = 'image-node-lazy';
    this.dom.style.display = 'inline-block';
    this.dom.style.width = this.nodeAttrs.width ? `${this.nodeAttrs.width}px` : '200px';
    this.dom.style.height = this.nodeAttrs.height ? `${this.nodeAttrs.height}px` : '150px';
    this.dom.style.backgroundColor = '#1a1b23';
    this.dom.style.border = '1px solid #2a2b35';
    this.dom.style.borderRadius = '4px';
    this.dom.style.position = 'relative';

    // Minimal loading indicator
    const indicator = document.createElement('div');
    indicator.style.position = 'absolute';
    indicator.style.top = '50%';
    indicator.style.left = '50%';
    indicator.style.transform = 'translate(-50%, -50%)';
    indicator.style.width = '20px';
    indicator.style.height = '20px';
    indicator.style.border = '2px solid #3a3b44';
    indicator.style.borderTop = '2px solid #85889C';
    indicator.style.borderRadius = '50%';
    indicator.style.animation = 'spin 1s linear infinite';

    // Add CSS animation if not exists
    if (!document.querySelector('#lazy-image-styles')) {
      const style = document.createElement('style');
      style.id = 'lazy-image-styles';
      style.textContent = `
        @keyframes spin {
          0% { transform: translate(-50%, -50%) rotate(0deg); }
          100% { transform: translate(-50%, -50%) rotate(360deg); }
        }
      `;
      document.head.appendChild(style);
    }

    this.dom.appendChild(indicator);
  }

  /**
   * Set up intersection observer for lazy loading
   */
  private setupIntersectionObserver(): void {
    // Only create images when they're about to be visible
    const options = {
      root: null,
      rootMargin: '50px', // Start loading 50px before visible
      threshold: 0.1
    };

    this.intersectionObserver = new IntersectionObserver((entries) => {
      entries.forEach(entry => {
        if (entry.isIntersecting && !this.isFullyLoaded && !this.isDestroyed) {
          this.loadFullImage();
          this.intersectionObserver?.disconnect();
        }
      });
    }, options);

    this.intersectionObserver.observe(this.dom);
  }

  /**
   * Load the full image when it becomes visible
   */
  private loadFullImage(): void {
    if (this.isDestroyed || this.isFullyLoaded) return;

    this.isFullyLoaded = true;

    // Create full placeholder
    this.createFullPlaceholder();

    // Create image element
    this.createImageElement();

    // Try to load image
    this.attemptImageLoad();
  }

  /**
   * Create full placeholder (replacing minimal one)
   */
  private createFullPlaceholder(): void {
    // Clear minimal placeholder
    this.dom.innerHTML = '';

    this.placeholder = document.createElement('div');
    this.placeholder.className = 'image-placeholder';
    this.placeholder.style.width = '100%';
    this.placeholder.style.height = '100%';
    this.placeholder.style.backgroundColor = '#2a2b35';
    this.placeholder.style.display = 'flex';
    this.placeholder.style.alignItems = 'center';
    this.placeholder.style.justifyContent = 'center';
    this.placeholder.style.borderRadius = '4px';
    this.placeholder.textContent = 'Loading image...';
    this.placeholder.style.color = '#85889C';
    this.placeholder.style.fontSize = '14px';

    this.dom.appendChild(this.placeholder);
  }

  /**
   * Create image element
   */
  private createImageElement(): void {
    this.img = document.createElement('img');
    this.img.alt = this.nodeAttrs.alt || '';
    this.img.title = this.nodeAttrs.title || '';
    if (this.nodeAttrs.width) this.img.width = this.nodeAttrs.width;
    if (this.nodeAttrs.height) this.img.height = this.nodeAttrs.height;
    this.img.style.display = 'none';
    this.img.style.maxWidth = '100%';
    this.img.style.height = 'auto';
    this.img.style.borderRadius = '4px';

    this.dom.appendChild(this.img);
  }

  /**
   * Attempt to load the image
   */
  private attemptImageLoad(): void {
    if (!this.imageId || !this.img) {
      this.handleExternalImage();
      return;
    }

    // Try to get image from storage immediately
    const imageSrc = this.imageStorage.getImageSrc(this.imageId);

    if (imageSrc) {
      this.displayImage(imageSrc);
    } else {
      this.listenForImageEvents();
    }
  }

  /**
   * Handle external images (data URLs, http, etc.)
   */
  private handleExternalImage(): void {
    const src = this.nodeAttrs.src;

    if (src && (src.startsWith('data:') || src.startsWith('http') || src.startsWith('blob:'))) {
      if (this.img) {
        this.img.src = src;
        this.img.onload = () => this.onImageLoad();
        this.img.onerror = () => this.onImageError('Failed to load external image');
      }
    } else {
      this.onImageError('Unknown image format');
    }
  }

  /**
   * Listen for image loading events (instead of polling)
   */
  private listenForImageEvents(): void {
    const handleImageReady = () => {
      if (this.isDestroyed || !this.imageId) return;

      const imageSrc = this.imageStorage.getImageSrc(this.imageId);
      if (imageSrc) {
        this.displayImage(imageSrc);
        this.removeImageEventListeners();
      }
    };

    // Store bound functions for cleanup
    this.handleAssetsLoaded = handleImageReady;
    this.handleImagesLoaded = handleImageReady;

    document.addEventListener('assets-loaded', this.handleAssetsLoaded);
    document.addEventListener('images-loaded', this.handleImagesLoaded);

    // Fallback timeout
    setTimeout(() => {
      if (!this.isDestroyed && this.img && !this.img.src) {
        this.onImageError('Image not found');
        this.removeImageEventListeners();
      }
    }, 10000);
  }

  // Store bound functions for cleanup
  private handleAssetsLoaded: (() => void) | null = null;
  private handleImagesLoaded: (() => void) | null = null;

  /**
   * Remove image event listeners
   */
  private removeImageEventListeners(): void {
    if (this.handleAssetsLoaded) {
      document.removeEventListener('assets-loaded', this.handleAssetsLoaded);
      this.handleAssetsLoaded = null;
    }
    if (this.handleImagesLoaded) {
      document.removeEventListener('images-loaded', this.handleImagesLoaded);
      this.handleImagesLoaded = null;
    }
  }

  /**
   * Display the loaded image
   */
  private displayImage(src: string): void {
    if (this.isDestroyed || !this.img) return;

    this.img.src = src;
    this.img.onload = () => this.onImageLoad();
    this.img.onerror = () => this.onImageError('Failed to display image');
  }

  /**
   * Handle successful image load
   */
  private onImageLoad(): void {
    if (this.isDestroyed || !this.img || !this.placeholder) return;

    this.placeholder.style.display = 'none';
    this.img.style.display = 'block';
  }

  /**
   * Handle image load error
   */
  private onImageError(message: string): void {
    if (this.isDestroyed || !this.placeholder) return;

    console.error(`[LazyImageNodeView] ${message} for ${this.imageId}`);
    this.placeholder.textContent = 'Failed to load';
    this.placeholder.style.color = '#ff5630';
    this.placeholder.style.backgroundColor = '#2a1f1f';
  }

  /**
   * Handle node selection
   */
  selectNode(): void {
    this.dom.classList.add('ProseMirror-selectednode');
  }

  /**
   * Handle node deselection  
   */
  deselectNode(): void {
    this.dom.classList.remove('ProseMirror-selectednode');
  }

  /**
   * Stop event propagation for certain events
   */
  stopEvent(): boolean {
    return false;
  }

  /**
   * Ignore mutations to avoid unnecessary re-renders
   */
  ignoreMutation(): boolean {
    return true;
  }

  /**
   * Clean up when destroyed
   */
  destroy(): void {
    this.isDestroyed = true;

    // Disconnect intersection observer
    if (this.intersectionObserver) {
      this.intersectionObserver.disconnect();
      this.intersectionObserver = null;
    }

    // Remove event listeners
    this.removeImageEventListeners();

    // Clean up image
    if (this.img) {
      this.img.onload = null;
      this.img.onerror = null;
      this.img.src = '';
      this.img = null;
    }

    this.placeholder = null;
    this.isFullyLoaded = false;
  }
}

/**
 * Plugin to register the lazy image node view
 */
export function imageNodeViewPlugin(imageStorage: ImageStorageService) {
  return new Plugin({
    key: new PluginKey('imageNodeView'),
    props: {
      nodeViews: {
        image: (node: any, view: any, getPos: any) => {
          return new LazyImageNodeView(node, view, getPos, imageStorage);
        }
      }
    },
    view() {
      return {
        destroy: () => {
        }
      };
    }
  });
}

/**
 * Utility function to notify all image node views that assets have been loaded
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
