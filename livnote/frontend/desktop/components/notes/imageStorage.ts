import * as Y from "yjs";
import type { ImageAsset, ImageMetadata } from '../../types/notes.types'

export interface ImageLoadMetrics {
  totalTime: number;
  imageCount: number;
  totalSize: number;
  individualLoadTimes: Map<string, number>;
}

export class ImageStorageService {
  private imagesMap: Y.Map<ImageAsset>; // Full asset data in YJS
  private clientId: number;
  private imageCache: Map<string, string> = new Map();

  constructor(imagesMap: Y.Map<ImageAsset>, clientId: number) {
    this.imagesMap = imagesMap;
    this.clientId = clientId;
    this.imagesMap.observe(this.handleImageMapUpdate.bind(this));
  }



  /**
   * Initialize cache from YJS map (called after YJS state is applied)
   */
  initializeCacheFromYjs(): void {
    this.imageCache.clear();
    let totalSize = 0;
    let count = 0;
    this.imagesMap.forEach((asset, id) => {
      const loadStart = performance.now();
      this.imageCache.set(id, asset.data);
      totalSize += asset.size || 0;
      count++;
    });


  }

  /**
   * Store an image in YJS map
   */
  async storeImage(base64Data: string, mimeType: string, filename?: string): Promise<string> {
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
    this.imagesMap.delete(imageId);
    this.imageCache.delete(imageId);
  }

  /**
   * Get all image IDs
   */
  getAllImageIds(): string[] {
    return Array.from(this.imagesMap.keys());
  }


  /**
   * Handle updates to the images map
   */
  private handleImageMapUpdate(event: Y.YMapEvent<ImageAsset>) {
    event.changes.keys.forEach((change, key) => {
      if (change.action === 'add' || change.action === 'update') {
        const asset = this.imagesMap.get(key);
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
}
