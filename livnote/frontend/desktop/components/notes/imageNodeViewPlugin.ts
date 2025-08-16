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

  // Selection state
  private isSelected: boolean = false;
  private selectionRing: HTMLDivElement | null = null;

  // Resize handles
  private resizeHandles: HTMLDivElement[] = [];
  private resizeContainer: HTMLDivElement | null = null;

  // Only create these when needed
  private img: HTMLImageElement | null = null;
  private placeholder: HTMLDivElement | null = null;

  // Store node data for lazy loading
  private nodeAttrs: any;
  private imageStorage: ImageStorageService;
  private imageDataListener: ((event: CustomEvent) => void) | null = null;
  private isWaitingForData = false;

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

    // Create minimal DOM structure with container wrapper
    this.createImageContainer();

    // Set up intersection observer for lazy loading
    this.setupIntersectionObserver();
  }

  /**
   * Create image container wrapper for better control
   */
  private createImageContainer(): void {
    // Create wrapper container that will hold everything
    this.dom = document.createElement('div');
    this.dom.className = 'image-node-container';
    this.dom.style.display = 'inline-block';
    this.dom.style.position = 'relative'; // Important for absolute positioning of children
    this.dom.style.lineHeight = '0';
    this.dom.style.verticalAlign = 'bottom'; // Prevent extra space below

    // Create the actual image placeholder
    const imagePlaceholder = document.createElement('div');
    imagePlaceholder.className = 'image-node-lazy';
    imagePlaceholder.style.display = 'inline-block';
    imagePlaceholder.style.width = this.nodeAttrs.width ? `${this.nodeAttrs.width}px` : '200px';
    imagePlaceholder.style.height = this.nodeAttrs.height ? `${this.nodeAttrs.height}px` : '150px';
    imagePlaceholder.style.backgroundColor = '#1a1b23';
    imagePlaceholder.style.border = '1px solid #2a2b35';
    imagePlaceholder.style.borderRadius = '4px';
    imagePlaceholder.style.position = 'relative';

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

    imagePlaceholder.appendChild(indicator);
    this.dom.appendChild(imagePlaceholder);

    // Store reference to the placeholder
    this.placeholder = imagePlaceholder;

    // Add CSS animation if not exists
    this.ensureStyles();
  }

  /**
   * Ensure required CSS styles are added
   */
  private ensureStyles(): void {
    if (!document.querySelector('#lazy-image-styles')) {
      const style = document.createElement('style');
      style.id = 'lazy-image-styles';
      style.textContent = `
        @keyframes spin {
          0% { transform: rotate(0deg); }
          100% { transform: rotate(360deg); }
        }
        
        /* Completely suppress ProseMirror's default selection styles */
        .ProseMirror-selectednode {
          outline: none !important;
          background: transparent !important;
        }
        
        /* Override ProseMirror's gapcursor and selection styles around images */
        .ProseMirror .ProseMirror-selectednode::selection {
          background: transparent !important;
        }
        
        .ProseMirror .ProseMirror-selectednode::-moz-selection {
          background: transparent !important;
        }
        
        /* Prevent the blue overlay selection */
        .image-node-container.ProseMirror-selectednode::before {
          content: none !important;
        }
        
        .image-node-container {
          cursor: pointer;
          user-select: none;
          display: inline-block;
          vertical-align: top;
          line-height: 0;
          position: relative;
        }
        
        /* Our custom selection border */
        .image-node-container.image-selected::after {
          content: '';
          position: absolute;
          top: -2px;
          left: -2px;
          right: -2px;
          bottom: -2px;
          border: 2px solid #007AFF;
          border-radius: 6px;
          pointer-events: none;
          z-index: 20;
          box-sizing: border-box;
        }
        
        .image-node-container img {
          display: block;
          max-width: 100%;
          height: auto;
          vertical-align: top;
        }
        
        .image-node-lazy {
          display: block !important;
          vertical-align: top;
        }
        
        /* Ensure no selection artifacts */
        .image-node-container * {
          user-select: none;
          -webkit-user-select: none;
          -moz-user-select: none;
          -ms-user-select: none;
        }
      `;
      document.head.appendChild(style);
    }
  }

  /**
   * Create resize handles for the image
   */
  private createResizeHandles(): void {
    if (this.resizeContainer) return;

    // Create container for resize handles
    this.resizeContainer = document.createElement('div');
    this.resizeContainer.className = 'resize-handles-container';
    this.resizeContainer.style.position = 'absolute';
    this.resizeContainer.style.top = '0';
    this.resizeContainer.style.left = '0';
    this.resizeContainer.style.right = '0';
    this.resizeContainer.style.bottom = '0';
    this.resizeContainer.style.pointerEvents = 'none';
    this.resizeContainer.style.zIndex = '15';

    // Define handle positions
    const positions = [
      { name: 'nw', top: '-5px', left: '-5px', cursor: 'nw-resize' },
      { name: 'ne', top: '-5px', right: '-5px', cursor: 'ne-resize' },
      { name: 'sw', bottom: '-5px', left: '-5px', cursor: 'sw-resize' },
      { name: 'se', bottom: '-5px', right: '-5px', cursor: 'se-resize' }
    ];

    // Create each handle
    positions.forEach(pos => {
      const handle = document.createElement('div');
      handle.className = `resize-handle resize-handle-${pos.name}`;
      handle.dataset.position = pos.name;

      // Style the handle
      handle.style.position = 'absolute';
      handle.style.width = '10px';
      handle.style.height = '10px';
      handle.style.backgroundColor = '#007AFF';
      handle.style.border = '1px solid #0051D5';
      handle.style.borderRadius = '2px';
      handle.style.cursor = pos.cursor;
      handle.style.pointerEvents = 'auto';
      handle.style.zIndex = '20';

      // Position the handle
      if (pos.top) handle.style.top = pos.top;
      if (pos.bottom) handle.style.bottom = pos.bottom;
      if (pos.left) handle.style.left = pos.left;
      if (pos.right) handle.style.right = pos.right;

      // Add hover effect
      handle.addEventListener('mouseenter', () => {
        handle.style.backgroundColor = '#0051D5';
        handle.style.transform = 'scale(1.2)';
      });

      handle.addEventListener('mouseleave', () => {
        handle.style.backgroundColor = '#007AFF';
        handle.style.transform = 'scale(1)';
      });

      // Add mousedown handler for resize (Phase 3)
      handle.addEventListener('mousedown', (e) => {
        e.preventDefault();
        e.stopPropagation();
        // Phase 3: We'll implement actual resize logic here
      });

      this.resizeHandles.push(handle);
      this.resizeContainer.appendChild(handle);
    });

    this.dom.appendChild(this.resizeContainer);
  }

  /**
   * Show/hide resize handles
   */
  private toggleResizeHandles(show: boolean): void {
    if (this.resizeContainer) {
      this.resizeContainer.style.display = show ? 'block' : 'none';
    }
  }

  /**
   * Set up intersection observer for lazy loading
   */
  private setupIntersectionObserver(): void {
    const options = {
      root: null,
      rootMargin: '50px',
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
    this.createFullPlaceholder();
    this.createImageElement();
    this.attemptImageLoad();
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
    this.img.style.borderRadius = '4px';
    this.img.style.position = 'absolute'; // Position absolutely within container
    this.img.style.top = '0';
    this.img.style.left = '0';
    this.img.style.width = '100%';
    this.img.style.height = '100%';
    this.img.style.objectFit = 'contain'; // Maintain aspect ratio

    // Add image to container (not before placeholder)
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

    const imageSrc = this.imageStorage.getImageSrc(this.imageId);

    if (imageSrc) {
      this.displayImage(imageSrc);
    } else {
      this.showWaitingForDataState();
      this.listenForImageData();
    }
  }

  private listenForImageData(): void {
    this.removeImageDataListener();

    this.imageDataListener = (event: CustomEvent) => {
      if (event.detail.imageId === this.imageId && this.isWaitingForData) {
        this.displayImage(event.detail.src);
        this.removeImageDataListener();
        this.isWaitingForData = false;
      }
    };

    document.addEventListener('image-data-available', this.imageDataListener as EventListener);

    setTimeout(() => {
      if (this.isWaitingForData && !this.isDestroyed) {
        console.warn(`Timeout waiting for image data: ${this.imageId}`);
        this.onImageError('Image data not received');
        this.removeImageDataListener();
        this.isWaitingForData = false;
      }
    }, 15000);
  }

  private removeImageDataListener(): void {
    if (this.imageDataListener) {
      document.removeEventListener('image-data-available', this.imageDataListener as EventListener);
      this.imageDataListener = null;
    }
  }

  private showWaitingForDataState(): void {
    if (!this.placeholder) return;

    this.isWaitingForData = true;
    this.placeholder.innerHTML = `
      <div style="display: flex; flex-direction: column; align-items: center; gap: 8px;">
        <div class="spinner" style="width: 16px; height: 16px; border: 2px solid #3a3b44; border-top: 2px solid #85889C; border-radius: 50%; animation: spin 1s linear infinite;"></div>
        <div style="font-size: 12px; color: #85889C;">Waiting for image data...</div>
      </div>
    `;
  }

  /**
   * Handle external images (data URLs, http, etc.)
   */
  private handleExternalImage(): void {
    const src = this.nodeAttrs.src;

    if (src && (src.startsWith('data:') || src.startsWith('http') || src.startsWith('blob:'))) {
      if (this.img) {
        // Set up handlers BEFORE setting src
        this.img.onload = () => this.onImageLoad();
        this.img.onerror = () => this.onImageError('Failed to load external image');

        // Now set the src
        this.img.src = src;

        // Check if already loaded (for cached images)
        if (this.img.complete && this.img.naturalHeight !== 0) {
          this.onImageLoad();
        }
      }
    } else {
      this.onImageError('Unknown image format');
    }
  }
  /**
   * Display the loaded image
   */
  private displayImage(src: string): void {

    if (this.isDestroyed || !this.img) return;

    // Set up handlers BEFORE setting src
    this.img.onload = () => {
      this.onImageLoad();
    };
    this.img.onerror = () => {
      this.onImageError('Failed to display image');
    };

    // Now set the src
    this.img.src = src;

    // For base64 images that might load synchronously
    if (this.img.complete && this.img.naturalHeight !== 0) {
      this.onImageLoad();
    }
  }

  /**
   * Handle successful image load
   */

  private onImageLoad(): void {

    if (this.isDestroyed || !this.img) return;

    this.img.style.display = 'block';

    if (this.placeholder && this.placeholder.parentNode) {
      this.placeholder.parentNode.removeChild(this.placeholder);
      this.placeholder = null;
    }

    // Ensure container has proper dimensions
    const actualWidth = this.img.naturalWidth || this.nodeAttrs.width || 200;
    const actualHeight = this.img.naturalHeight || this.nodeAttrs.height || 150;

    // Update container to match actual image size
    this.dom.style.width = `${actualWidth}px`;
    this.dom.style.height = `${actualHeight}px`;

    // Reset image styles to fill container properly
    this.img.style.position = 'static';
    this.img.style.width = 'auto';
    this.img.style.height = 'auto';
    this.img.style.maxWidth = '100%';
    this.img.style.maxHeight = '100%';

  }


  private createFullPlaceholder(): void {
    if (!this.placeholder) return;

    // Clear minimal placeholder content
    this.placeholder.innerHTML = '';
    this.placeholder.style.backgroundColor = '#2a2b35';
    this.placeholder.style.display = 'flex';
    this.placeholder.style.alignItems = 'center';
    this.placeholder.style.justifyContent = 'center';
    this.placeholder.textContent = 'Loading image...';
    this.placeholder.style.color = '#85889C';
    this.placeholder.style.fontSize = '14px';
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
   * Handle node selection - UPDATED FOR PHASE 2
   */
  selectNode(): void {

    this.isSelected = true;
    this.dom.classList.add('image-selected');
    this.dom.classList.add('ProseMirror-selectednode');

    // Create and show selection border
    if (!this.selectionRing) {
      this.selectionRing = document.createElement('div');
      this.selectionRing.className = 'image-selection-ring';
      this.selectionRing.style.position = 'absolute';
      this.selectionRing.style.top = '0';
      this.selectionRing.style.left = '0';
      this.selectionRing.style.right = '0';
      this.selectionRing.style.bottom = '0';
      this.selectionRing.style.border = '2px solid #007AFF';
      this.selectionRing.style.borderRadius = '6px';
      this.selectionRing.style.pointerEvents = 'none';
      this.selectionRing.style.zIndex = '10';
      this.dom.appendChild(this.selectionRing);
    }

    this.selectionRing.style.display = 'block';

    // Create and show resize handles (Phase 2)
    this.createResizeHandles();
    this.toggleResizeHandles(true);
  }

  /**
   * Handle node deselection - UPDATED FOR PHASE 2
   */
  deselectNode(): void {

    this.isSelected = false;
    this.dom.classList.remove('image-selected');
    this.dom.classList.remove('ProseMirror-selectednode');

    // Hide the selection ring
    if (this.selectionRing) {
      this.selectionRing.style.display = 'none';
    }

    // Hide resize handles (Phase 2)
    this.toggleResizeHandles(false);
  }

  /**
   * Stop event propagation for certain events
   */
  stopEvent(event: Event): boolean {
    // Stop events from bubbling to prevent ProseMirror's default handling
    if (event.type === 'mousedown' || event.type === 'click') {
      // Allow the selection to work but prevent default paragraph selection
      return false;
    }
    return false;
  }

  /**
   * Ignore mutations to avoid unnecessary re-renders
   */
  ignoreMutation(mutation: MutationRecord): boolean {
    // Ignore all mutations to our custom node
    return true;
  }

  /**
   * Update node to handle attribute changes
   */
  update(node: PMNode): boolean {
    // Check if it's still an image node
    if (node.type.name !== 'image') return false;

    // Update stored attributes
    this.nodeAttrs = node.attrs;

    // Update dimensions if image is loaded
    if (this.img && this.isFullyLoaded) {
      if (node.attrs.width) this.img.width = node.attrs.width;
      if (node.attrs.height) this.img.height = node.attrs.height;
      this.dom.style.width = `${node.attrs.width}px`;
      this.dom.style.height = `${node.attrs.height}px`;
    }

    return true;
  }

  /**
   * Clean up when destroyed
   */
  destroy(): void {
    this.isDestroyed = true;

    if (this.intersectionObserver) {
      this.intersectionObserver.disconnect();
      this.intersectionObserver = null;
    }

    this.removeImageDataListener();

    if (this.img) {
      this.img.onload = null;
      this.img.onerror = null;
      this.img.src = '';
      this.img = null;
    }

    this.placeholder = null;
    this.selectionRing = null;
    this.isFullyLoaded = false;
    this.isSelected = false;
  }
}

/**
 * Plugin to register the lazy image node view
 */
export function imageNodeViewPlugin(imageStorage: ImageStorageService) {
  // Add global styles to suppress ProseMirror's selection
  if (!document.querySelector('#prosemirror-image-selection-override')) {
    const globalStyle = document.createElement('style');
    globalStyle.id = 'prosemirror-image-selection-override';
    globalStyle.textContent = `
      /* Global override for ProseMirror's node selection */
      .ProseMirror .ProseMirror-selectednode {
        outline: none !important;
      }
      
      /* Specifically target the selection overlay */
      .ProseMirror-selectednode::after {
        content: none !important;
      }
      
      /* Remove any selection background */
      .ProseMirror ::selection {
        background-color: rgba(0, 122, 255, 0.2);
      }
      
      .ProseMirror .ProseMirror-selectednode::selection,
      .ProseMirror .ProseMirror-selectednode *::selection {
        background: transparent !important;
      }
      
      .ProseMirror .ProseMirror-selectednode::-moz-selection,
      .ProseMirror .ProseMirror-selectednode *::-moz-selection {
        background: transparent !important;
      }
    `;
    document.head.appendChild(globalStyle);
  }

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
          // Cleanup if needed
        }
      };
    }
  });
}

/**
 * Utility function to notify all image node views that assets have been loaded
 */
export function notifyImageNodesAssetsLoaded(noteId?: string) {
  document.dispatchEvent(new CustomEvent('images-loaded', {
    detail: { noteId, type: 'assets' }
  }));

  document.dispatchEvent(new CustomEvent('assets-loaded', {
    detail: { noteId, type: 'assets' }
  }));
}
