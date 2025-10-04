import * as Y from "yjs";
import type { ImageAsset, ImageMetadata } from '../../types/notes.types'

export interface ImageLoadMetrics {
  totalTime: number;
  imageCount: number;
  totalSize: number;
  individualLoadTimes: Map<string, number>;
}

export class ImageStorageService {
  private imagesMap: Y.Map<ImageAsset> | null = null;
  private clientId: number;
  private imageCache: Map<string, string> = new Map();
  private mapObserver: ((event: Y.YMapEvent<ImageAsset>) => void) | null = null;
  constructor(clientId: number) {
    this.clientId = clientId;
  }
  setImageMap(map: Y.Map<ImageAsset>): void {
    // 1. Clean up the observer from the previous map, if it exists
    if (this.imagesMap && this.mapObserver) {
      this.imagesMap.unobserve(this.mapObserver);
    }

    // 2. Set the new map
    this.imagesMap = map;
    this.mapObserver = this.handleImageMapUpdate.bind(this);

    // 3. Observe the new map
    this.imagesMap.observe(this.mapObserver);

    // 4. Initialize the cache with data from the new map
    this.initializeCacheFromYjs();
  }



  /**
   * Initialize cache from YJS map (called after YJS state is applied)
   */
  initializeCacheFromYjs(): void {
    if (!this.imagesMap) return;
    
    this.imageCache.clear();
    this.imagesMap.forEach((asset, id) => {
      this.imageCache.set(id, asset.data);
    });
  }

  /**
   * Store an image in YJS map
   */
  async storeImage(base64Data: string, mimeType: string, filename?: string): Promise<string> {
    if (!this.imagesMap) {
      throw new Error("Image map not initialized");
    }
    
    const imageId = this.generateImageId();
    const dimensions = await this.extractImageDimensions(base64Data);

    const asset: ImageAsset = {
      id: imageId,
      data: base64Data,
      mimeType: mimeType || 'image/png',
      size: this.calculateBase64Size(base64Data),
      width: dimensions?.width,
      height: dimensions?.height,
      uploadedBy: this.clientId,
      timestamp: Date.now(),
      filename
    };
    this.imagesMap.set(imageId, asset);
    // Cache it immediately
    this.imageCache.set(imageId, base64Data);

    return imageId;
  }

  /**
   * Get image metadata by ID
   */
  getImageMetadata(imageId: string): ImageMetadata | null {
    if (!this.imagesMap) return null;
    
    const asset = this.imagesMap.get(imageId);
    if (!asset) return null;

    const { data, ...metadata } = asset;
    return metadata;
  }

  /**
   * Get image src (base64) by ID
   */
  getImageSrc(imageId: string): string | null {
    if (this.imageCache.has(imageId)) {
      return this.imageCache.get(imageId)!;
    }

    if (!this.imagesMap) return null;
    
    const asset = this.imagesMap.get(imageId);
    if (asset && asset.data) {
      this.imageCache.set(imageId, asset.data);
      return asset.data;
    }

    return null;
  }

  /**
   * Delete an image
   */
  deleteImage(imageId: string): void {
    if (!this.imagesMap) return;
    
    this.imagesMap.delete(imageId);
    this.imageCache.delete(imageId);
  }

  /**
   * Get all image IDs
   */
  getAllImageIds(): string[] {
    if (!this.imagesMap) return [];
    
    return Array.from(this.imagesMap.keys());
  }


  /**
   * Handle updates to the images map
   */
  private handleImageMapUpdate(event: Y.YMapEvent<ImageAsset>) {
    if (!this.imagesMap) return;
    
    event.changes.keys.forEach((change, key) => {
      if (change.action === 'add' || change.action === 'update') {
        const asset = this.imagesMap!.get(key);
        if (asset) {
          this.imageCache.set(key, asset.data);
          document.dispatchEvent(new CustomEvent('image-data-available', {
            detail: {
              imageId: key,
              asset: asset,
              src: asset.data
            }
          }));
        }
      } else if (change.action === 'delete') {
        this.imageCache.delete(key);
        document.dispatchEvent(new CustomEvent('image-data-removed', {
          detail: { imageId: key }
        }));
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
  }
  destroy(): void {
    if (this.imagesMap && this.mapObserver) {
      this.imagesMap.unobserve(this.mapObserver);
    }
    this.clearCache();
    this.imagesMap = null;
    this.mapObserver = null;
  }
}
