import type { NodeView } from "prosemirror-view";
import { Node as PMNode } from "prosemirror-model";
import { EditorView } from "prosemirror-view";
import { ImageStorageService } from "./imageStorage";
import { Plugin, PluginKey } from "prosemirror-state";
import { createNodeSelection } from "./utils/prosemirror-helpers";
import { NodeSelection } from "prosemirror-state";
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

  // Add resize state tracking
  private isResizing: boolean = false;
  private resizeStartData: {
    startX: number;
    startY: number;
    startWidth: number;
    startHeight: number;
    handle: string;
    aspectRatio: number;
  } | null = null;
  private resizeOverlay: HTMLDivElement | null = null;

  // Add reference to editor view and getPos
  private view: EditorView;
  private getPos: () => number;

  constructor(
    node: PMNode,
    view: EditorView,
    getPos: () => number,
    imageStorage: ImageStorageService
  ) {
    // Store data for later use
    this.nodeAttrs = node.attrs;
    this.imageStorage = imageStorage;
    this.view = view;
    this.getPos = getPos;

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
    this.dom = document.createElement('div'); // Back to div for better control
    this.dom.className = 'image-node-container';
    this.dom.style.display = 'block'; // Block display
    this.dom.style.float = 'left'; // Float left to allow text wrapping
    this.dom.style.clear = 'none';
    this.dom.style.position = 'relative';
    this.dom.style.lineHeight = '0';
    this.dom.style.margin = '8px 12px 8px 0'; // Right and bottom margin for text spacing
    this.dom.style.cursor = 'default';

    // Make it non-editable to prevent text cursor
    this.dom.contentEditable = 'false';
    this.dom.setAttribute('draggable', 'false');

    // Create the actual image placeholder
    const imagePlaceholder = document.createElement('div');
    imagePlaceholder.className = 'image-node-lazy';
    imagePlaceholder.style.display = 'block';
    imagePlaceholder.style.width = this.nodeAttrs.width ? `${this.nodeAttrs.width}px` : '200px';
    imagePlaceholder.style.height = this.nodeAttrs.height ? `${this.nodeAttrs.height}px` : '150px';
    imagePlaceholder.style.backgroundColor = '#1a1b23';
    imagePlaceholder.style.border = '1px solid #2a2b35';
    imagePlaceholder.style.borderRadius = '4px';
    imagePlaceholder.style.position = 'relative';
    imagePlaceholder.style.cursor = 'pointer';

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

    // Add click handler to select the image
    this.dom.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      this.handleImageClick();
    });

    // Add CSS animation if not exists
    this.ensureStyles();
  }

  /**
   * Handle image click to select it properly
   */
  private handleImageClick(): void {
    if (this.isDestroyed || this.isResizing) return;

    const pos = this.getPos();
    const nodeSelection = createNodeSelection(this.view.state, pos);

    if (nodeSelection instanceof NodeSelection) {
      const tr = this.view.state.tr.setSelection(nodeSelection);
      this.view.dispatch(tr);
      this.view.focus();
    } else {
      console.warn("Failed to create NodeSelection for image at position", pos);
    }
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
        
        /* Image container with float for text wrapping */
        .image-node-container {
          cursor: default !important;
          user-select: none;
          display: block !important;
          float: left !important;
          clear: none !important;
          position: relative;
          line-height: 0;
          margin: 8px 12px 8px 0;
          max-width: 100%;
        }
        
        /* Alternative: centered block image */
        .image-node-container.image-centered {
          float: none !important;
          display: block !important;
          margin: 16px auto !important;
          clear: both !important;
        }
        
        /* Alternative: right-aligned image */
        .image-node-container.image-right {
          float: right !important;
          margin: 8px 0 8px 12px !important;
        }
        
        /* Alternative: full-width block image */
        .image-node-container.image-block {
          float: none !important;
          display: block !important;
          margin: 16px 0 !important;
          clear: both !important;
        }
        
        /* Prevent text cursor near images */
        .image-node-container:hover {
          cursor: pointer !important;
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
          position: absolute;
          top: 0;
          left: 0;
          width: 100%;
          height: 100%;
          object-fit: contain;
          vertical-align: top;
          cursor: pointer !important;
        }
        
        .image-node-lazy {
          display: block !important;
          vertical-align: top;
          cursor: pointer !important;
        }
        
        /* Ensure no selection artifacts */
        .image-node-container * {
          user-select: none;
          -webkit-user-select: none;
          -moz-user-select: none;
          -ms-user-select: none;
        }

        /* Resize overlay for visual feedback */
        .resize-overlay {
          position: fixed;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          z-index: 9999;
          cursor: inherit;
          background: transparent;
        }

        /* Size indicator tooltip */
        .size-indicator {
          position: absolute;
          background: #007AFF;
          color: white;
          padding: 4px 8px;
          border-radius: 4px;
          font-size: 12px;
          font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
          white-space: nowrap;
          pointer-events: none;
          z-index: 10000;
          box-shadow: 0 2px 4px rgba(0,0,0,0.2);
        }
        
        /* Clear floats after paragraphs with images */
        .ProseMirror p::after {
          content: "";
          display: table;
          clear: both;
        }
        
        /* Ensure proper paragraph spacing */
        .ProseMirror p {
          min-height: 1.5em;
          line-height: 1.5;
        }
        
        /* Prevent cursor height issues */
        .ProseMirror {
          line-height: 1.5;
          
        }
        
        /* Optional: Add a clear-fix utility class */
        .clear-both {
          clear: both !important;
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

      // Add mousedown handler for resize (Phase 3 - IMPLEMENTED)
      handle.addEventListener('mousedown', (e) => {
        e.preventDefault();
        e.stopPropagation();
        this.startResize(e, pos.name);
      });

      this.resizeHandles.push(handle);
      this.resizeContainer.appendChild(handle);
    });

    this.dom.appendChild(this.resizeContainer);
  }

  /**
   * Start the resize operation
   */
  private startResize(e: MouseEvent, handle: string): void {
    if (this.isDestroyed) return;

    this.isResizing = true;

    // Get current dimensions
    const rect = this.dom.getBoundingClientRect();
    const currentWidth = rect.width;
    const currentHeight = rect.height;

    // Store initial resize data
    this.resizeStartData = {
      startX: e.clientX,
      startY: e.clientY,
      startWidth: currentWidth,
      startHeight: currentHeight,
      handle: handle,
      aspectRatio: currentWidth / currentHeight
    };

    // Create resize overlay to capture mouse events
    this.createResizeOverlay(handle);

    // Add event listeners
    document.addEventListener('mousemove', this.handleResize);
    document.addEventListener('mouseup', this.endResize);

    // Prevent text selection during resize
    document.body.style.userSelect = 'none';
    document.body.style.cursor = this.getCursorForHandle(handle);
  }

  /**
   * Handle resize mouse movement
   */
  private handleResize = (e: MouseEvent): void => {
    if (!this.isResizing || !this.resizeStartData) return;

    e.preventDefault();

    const deltaX = e.clientX - this.resizeStartData.startX;
    const deltaY = e.clientY - this.resizeStartData.startY;

    let newWidth = this.resizeStartData.startWidth;
    let newHeight = this.resizeStartData.startHeight;

    // Calculate new dimensions based on handle position
    switch (this.resizeStartData.handle) {
      case 'se': // Southeast (bottom-right)
        newWidth = Math.max(50, this.resizeStartData.startWidth + deltaX);
        newHeight = Math.max(50, this.resizeStartData.startHeight + deltaY);
        break;
      case 'sw': // Southwest (bottom-left)
        newWidth = Math.max(50, this.resizeStartData.startWidth - deltaX);
        newHeight = Math.max(50, this.resizeStartData.startHeight + deltaY);
        break;
      case 'ne': // Northeast (top-right)
        newWidth = Math.max(50, this.resizeStartData.startWidth + deltaX);
        newHeight = Math.max(50, this.resizeStartData.startHeight - deltaY);
        break;
      case 'nw': // Northwest (top-left)
        newWidth = Math.max(50, this.resizeStartData.startWidth - deltaX);
        newHeight = Math.max(50, this.resizeStartData.startHeight - deltaY);
        break;
    }

    // Maintain aspect ratio if shift key is held
    if (e.shiftKey && this.resizeStartData.aspectRatio) {
      const scaleFactor = Math.max(
        newWidth / this.resizeStartData.startWidth,
        newHeight / this.resizeStartData.startHeight
      );
      newWidth = this.resizeStartData.startWidth * scaleFactor;
      newHeight = this.resizeStartData.startHeight * scaleFactor;
    }

    // Apply maximum dimensions
    newWidth = Math.min(newWidth, 1200);
    newHeight = Math.min(newHeight, 800);

    // Round to integers
    newWidth = Math.round(newWidth);
    newHeight = Math.round(newHeight);

    // Update visual dimensions
    this.updateVisualDimensions(newWidth, newHeight);

    // Show size indicator
    this.showSizeIndicator(newWidth, newHeight, e.clientX, e.clientY);
  };

  /**
   * End the resize operation
   */
  private endResize = (e: MouseEvent): void => {
    if (!this.isResizing || !this.resizeStartData) return;

    this.isResizing = false;

    // Get final dimensions
    const rect = this.dom.getBoundingClientRect();
    const finalWidth = Math.round(rect.width);
    const finalHeight = Math.round(rect.height);

    // Update the node in ProseMirror
    this.updateNodeDimensions(finalWidth, finalHeight);

    // Clean up
    document.removeEventListener('mousemove', this.handleResize);
    document.removeEventListener('mouseup', this.endResize);
    document.body.style.userSelect = '';
    document.body.style.cursor = '';

    // Remove overlay
    if (this.resizeOverlay) {
      this.resizeOverlay.remove();
      this.resizeOverlay = null;
    }

    // Remove size indicator
    this.removeSizeIndicator();

    this.resizeStartData = null;
  };

  /**
   * Update visual dimensions during resize
   */
  private updateVisualDimensions(width: number, height: number): void {
    // Update container dimensions
    this.dom.style.width = `${width}px`;
    this.dom.style.height = `${height}px`;

    // Update placeholder if still loading
    if (this.placeholder) {
      this.placeholder.style.width = `${width}px`;
      this.placeholder.style.height = `${height}px`;
    }

    // Update image if loaded - FIXED: Make image fill the container
    if (this.img) {
      // Make the image fill the entire container
      this.img.style.width = '100%';
      this.img.style.height = '100%';
      this.img.style.objectFit = 'contain'; // or 'cover' if you want to fill without maintaining aspect ratio
      this.img.style.position = 'absolute';
      this.img.style.top = '0';
      this.img.style.left = '0';
    }
  }

  /**
   * Update node dimensions in ProseMirror document
   */
  private updateNodeDimensions(width: number, height: number): void {
    const pos = this.getPos();
    const { tr } = this.view.state;

    // Get the current node
    const node = this.view.state.doc.nodeAt(pos);
    if (!node) return;

    // Create new attributes with updated dimensions
    const newAttrs = {
      ...node.attrs,
      width: width,
      height: height
    };

    // Update the node
    tr.setNodeMarkup(pos, undefined, newAttrs);
    this.view.dispatch(tr);

    // Update stored attributes
    this.nodeAttrs.width = width;
    this.nodeAttrs.height = height;
  }

  /**
   * Create resize overlay to capture mouse events
   */
  private createResizeOverlay(handle: string): void {
    this.resizeOverlay = document.createElement('div');
    this.resizeOverlay.className = 'resize-overlay';
    this.resizeOverlay.style.cursor = this.getCursorForHandle(handle);
    document.body.appendChild(this.resizeOverlay);
  }

  /**
   * Get cursor style for handle position
   */
  private getCursorForHandle(handle: string): string {
    const cursors: { [key: string]: string } = {
      'nw': 'nw-resize',
      'ne': 'ne-resize',
      'sw': 'sw-resize',
      'se': 'se-resize'
    };
    return cursors[handle] || 'default';
  }

  /**
   * Show size indicator tooltip
   */
  private showSizeIndicator(width: number, height: number, x: number, y: number): void {
    let indicator = document.querySelector('.size-indicator') as HTMLDivElement;

    if (!indicator) {
      indicator = document.createElement('div');
      indicator.className = 'size-indicator';
      document.body.appendChild(indicator);
    }

    indicator.textContent = `${width} × ${height}`;
    indicator.style.left = `${x + 10}px`;
    indicator.style.top = `${y - 30}px`;
  }

  /**
   * Remove size indicator
   */
  private removeSizeIndicator(): void {
    const indicator = document.querySelector('.size-indicator');
    if (indicator) {
      indicator.remove();
    }
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
    this.img.style.position = 'absolute';
    this.img.style.top = '0';
    this.img.style.left = '0';
    this.img.style.width = '100%';
    this.img.style.height = '100%';
    this.img.style.objectFit = 'contain';

    // Add image to container
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
        this.img.onload = () => this.onImageLoad();
        this.img.onerror = () => this.onImageError('Failed to load external image');
        this.img.src = src;

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

    this.img.onload = () => {
      this.onImageLoad();
    };
    this.img.onerror = () => {
      this.onImageError('Failed to display image');
    };

    this.img.src = src;

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

    // Use stored dimensions or natural dimensions
    const actualWidth = this.nodeAttrs.width || this.img.naturalWidth || 200;
    const actualHeight = this.nodeAttrs.height || this.img.naturalHeight || 150;

    // Update container to match dimensions
    this.dom.style.width = `${actualWidth}px`;
    this.dom.style.height = `${actualHeight}px`;

    // FIXED: Make image fill the container properly
    this.img.style.position = 'absolute';
    this.img.style.top = '0';
    this.img.style.left = '0';
    this.img.style.width = '100%';
    this.img.style.height = '100%';
    this.img.style.objectFit = 'contain'; // Maintains aspect ratio within container
  }

  private createFullPlaceholder(): void {
    if (!this.placeholder) return;

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
   * Handle node selection
   */
  selectNode(): void {
    this.isSelected = true;
    this.dom.classList.add('image-selected');
    this.dom.classList.add('ProseMirror-selectednode');

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

    // Create and show resize handles
    this.createResizeHandles();
    this.toggleResizeHandles(true);
  }

  /**
   * Handle node deselection
   */
  deselectNode(): void {
    this.isSelected = false;
    this.dom.classList.remove('image-selected');
    this.dom.classList.remove('ProseMirror-selectednode');

    if (this.selectionRing) {
      this.selectionRing.style.display = 'none';
    }

    this.toggleResizeHandles(false);
  }

  /**
   * Stop event propagation for certain events
   */
  stopEvent(event: Event): boolean {
    // Allow resize events to be handled
    if (this.isResizing) {
      return true;
    }

    // Prevent default text cursor behavior
    if (event.type === 'mousedown' || event.type === 'click' || event.type === 'mouseover') {
      // Let ProseMirror handle selection but prevent text cursor
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
    if (node.type.name !== 'image') return false;

    // Update stored attributes
    this.nodeAttrs = node.attrs;

    // Update dimensions if not currently resizing
    if (!this.isResizing) {
      const width = node.attrs.width;
      const height = node.attrs.height;

      if (width && height) {
        // Update container dimensions
        this.dom.style.width = `${width}px`;
        this.dom.style.height = `${height}px`;

        // Update placeholder if it exists
        if (this.placeholder) {
          this.placeholder.style.width = `${width}px`;
          this.placeholder.style.height = `${height}px`;
        }

        // FIXED: Ensure image fills container after update
        if (this.img && this.isFullyLoaded) {
          this.img.style.position = 'absolute';
          this.img.style.top = '0';
          this.img.style.left = '0';
          this.img.style.width = '100%';
          this.img.style.height = '100%';
          this.img.style.objectFit = 'contain';
        }
      }
    }

    return true;
  }

  /**
   * Clean up when destroyed
   */
  destroy(): void {
    this.isDestroyed = true;

    // Clean up resize listeners if active
    if (this.isResizing) {
      document.removeEventListener('mousemove', this.handleResize);
      document.removeEventListener('mouseup', this.endResize);
      document.body.style.userSelect = '';
      document.body.style.cursor = '';
    }

    // Remove overlays
    if (this.resizeOverlay) {
      this.resizeOverlay.remove();
      this.resizeOverlay = null;
    }

    this.removeSizeIndicator();

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
    this.resizeContainer = null;
    this.resizeHandles = [];
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
