import * as Y from "yjs";
import type { ImageAsset, ImageMetadata } from '../../types/notes.types'
export class ImageStorageService {
  private imagesMap: Y.Map<ImageMetadata>; // Only metadata now
  private clientId: number;
  private imageCache: Map<string, string> = new Map(); // Cache for base64 data
  private assetsArray: ImageAsset[] = []; // Reference to note's assets array
  private pendingLoads: Map<string, Promise<string | null>> = new Map();

  constructor(imagesMap: Y.Map<ImageMetadata>, clientId: number) {
    this.imagesMap = imagesMap;
    this.clientId = clientId;

    // Listen for metadata updates
    this.imagesMap.observe(this.handleImageMapUpdate.bind(this));
  }

  /**
   * Set the assets array reference when loading a note
   */
  setAssetsArray(assets: ImageAsset[]): void {
    this.assetsArray = assets;
    // Pre-populate cache with loaded assets
    this.populateCacheFromAssets();
  }

  /**
   * Populate cache from assets array
   */
  private populateCacheFromAssets(): void {
    this.imageCache.clear();
    for (const asset of this.assetsArray) {
      this.imageCache.set(asset.id, asset.data);
    }
    console.log(`[ImageStorage] Populated cache with ${this.assetsArray.length} images`);
  }

  /**
   * Store an image in both metadata map and assets array
   */
  async storeImage(base64Data: string, mimeType: string, filename?: string): Promise<string> {
    const imageId = this.generateImageId();

    // Extract dimensions if possible
    const dimensions = await this.extractImageDimensions(base64Data);

    const metadata: ImageMetadata = {
      id: imageId,
      mimeType: mimeType || 'image/png',
      size: this.calculateBase64Size(base64Data),
      width: dimensions?.width,
      height: dimensions?.height,
      uploadedBy: this.clientId,
      timestamp: Date.now(),
      filename
    };

    const asset: ImageAsset = {
      ...metadata,
      data: base64Data // Include the actual data in asset
    };

    // Store metadata in Y.Map (this will sync to other clients)
    this.imagesMap.set(imageId, metadata);

    // Store asset in assets array
    this.assetsArray.push(asset);

    // Cache it immediately
    this.imageCache.set(imageId, base64Data);

    console.log(`[ImageStorage] Stored image ${imageId}, assets array now has ${this.assetsArray.length} items`);
    return imageId;
  }

  /**
   * Get image metadata by ID
   */
  getImageMetadata(imageId: string): ImageMetadata | null {
    return this.imagesMap.get(imageId) || null;
  }

  /**
   * Get image src (base64) by ID with caching
   */
  getImageSrc(imageId: string): string | null {
    // Check cache first
    if (this.imageCache.has(imageId)) {
      return this.imageCache.get(imageId)!;
    }

    // Try to find in assets array
    const asset = this.assetsArray.find(a => a.id === imageId);
    if (asset) {
      this.imageCache.set(imageId, asset.data);
      return asset.data;
    }

    console.warn(`[ImageStorage] Image ${imageId} not found in cache or assets`);
    return null;
  }

  /**
   * Get image src asynchronously (for future use with lazy loading)
   */
  async getImageSrcAsync(imageId: string): Promise<string | null> {
    // Check if we're already loading this image
    if (this.pendingLoads.has(imageId)) {
      return this.pendingLoads.get(imageId)!;
    }

    // Check cache first
    if (this.imageCache.has(imageId)) {
      return this.imageCache.get(imageId)!;
    }

    // Try to find in assets array
    const asset = this.assetsArray.find(a => a.id === imageId);
    if (asset) {
      this.imageCache.set(imageId, asset.data);
      return asset.data;
    }

    return null;
  }

  /**
   * Delete an image from both metadata and assets
   */
  deleteImage(imageId: string): void {
    // Remove from YJS metadata
    this.imagesMap.delete(imageId);

    // Remove from assets array
    const assetIndex = this.assetsArray.findIndex(a => a.id === imageId);
    if (assetIndex !== -1) {
      this.assetsArray.splice(assetIndex, 1);
    }

    // Remove from cache
    this.imageCache.delete(imageId);

    console.log(`[ImageStorage] Deleted image ${imageId}, assets array now has ${this.assetsArray.length} items`);
  }

  /**
   * Get all image IDs
   */
  getAllImageIds(): string[] {
    return Array.from(this.imagesMap.keys());
  }

  /**
   * Get all assets
   */
  getAllAssets(): ImageAsset[] {
    return [...this.assetsArray];
  }

  /**
   * Handle updates to the images metadata map
   */
  private handleImageMapUpdate(event: Y.YMapEvent<ImageMetadata>) {
    // When metadata changes, we might need to request the actual image data
    // For now, just log the changes
    event.changes.keys.forEach((change, key) => {
      if (change.action === 'add') {
        console.log(`[ImageStorage] New image metadata added: ${key}`);
        // TODO: In P2P scenario, request the actual image data from other clients
      } else if (change.action === 'delete') {
        console.log(`[ImageStorage] Image metadata deleted: ${key}`);
        this.imageCache.delete(key);
      }
    });
  }

  /**
   * Generate a unique image ID
   */
  private generateImageId(): string {
    return `img_${Date.now()}_${this.clientId}_${Math.random().toString(36).substr(2, 9)}`;
  }

  /**
   * Calculate approximate size of base64 string
   */
  private calculateBase64Size(base64: string): number {
    const base64String = base64.split(',')[1] || base64;
    return Math.round(base64String.length * 0.75);
  }

  /**
   * Extract image dimensions from base64 data
   */
  private async extractImageDimensions(base64Data: string): Promise<{ width: number; height: number } | null> {
    return new Promise((resolve) => {
      const img = new Image();
      img.onload = () => {
        resolve({ width: img.width, height: img.height });
      };
      img.onerror = () => {
        resolve(null);
      };
      img.src = base64Data;
    });
  }

  /**
   * Clear the image cache
   */
  clearCache(): void {
    this.imageCache.clear();
    console.log('[ImageStorage] Cache cleared');
  }

  /**
   * Get cache size
   */
  getCacheSize(): number {
    return this.imageCache.size;
  }

  /**
   * Get cache memory usage estimate (in bytes)
   */
  getCacheMemoryUsage(): number {
    let total = 0;
    for (const [key, value] of this.imageCache) {
      total += key.length + value.length;
    }
    return total;
  }
}
