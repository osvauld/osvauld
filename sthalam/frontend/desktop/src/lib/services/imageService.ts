/**
 * Image Service - Handle image file uploads
 * Wrapper around assetService for image-specific operations
 */

import { uploadAsset, type AssetMetadata } from './assetService';
import { loroCoordinator } from '../../shared/loro/loroCoordinator';

export interface ImageMetadata {
  id: string;
  filename: string;
  size: number;
  uploadedAt: number;
}

/**
 * Open file dialog and upload image to staticAssets
 * @returns Image ID and metadata, or null if cancelled
 */
export async function uploadImage(): Promise<ImageMetadata | null> {
  try {
    const result = await uploadAsset('image');

    if (!result) {
      return null; // User cancelled
    }

    // Convert AssetMetadata to ImageMetadata
    const metadata: ImageMetadata = {
      id: result.id,
      filename: result.filename,
      size: result.size,
      uploadedAt: result.uploadedAt
    };

    return metadata;
  } catch (error) {
    console.error('❌ [ImageService] Upload failed:', error);
    throw error;
  }
}

/**
 * Get image data by ID
 * Priority: 1. Static assets (new), 2. contentDoc.images (legacy CRDT)
 * @param imageId Image ID to retrieve
 * @returns Image data as Uint8Array, or null if not found
 */
export function getImage(imageId: string): Uint8Array | null {
  try {
    // First check static assets (new non-CRDT storage)
    const staticAsset = loroCoordinator.getStaticAsset(imageId);
    if (staticAsset) {
      return Uint8Array.from(staticAsset);
    }

    // Fallback to contentDoc.images (legacy CRDT storage)
    const imagesMap = loroCoordinator.getContentMap();
    const imageData = imagesMap.get(imageId);

    if (imageData instanceof Uint8Array) {
      return Uint8Array.from(imageData);
    }

    return null;
  } catch (error) {
    console.error('❌ [ImageService] Error getting image:', error);
    return null;
  }
}

/**
 * Get image filename from contentDoc
 * @param imageId Image ID
 * @returns Filename or empty string
 */
export function getImageFilename(imageId: string): string {
  try {
    const imagesMap = loroCoordinator.getContentMap();
    const filename = imagesMap.get(`${imageId}_filename`);
    return typeof filename === 'string' ? filename : '';
  } catch (error) {
    console.error('❌ [ImageService] Error getting image filename:', error);
    return '';
  }
}

/**
 * Delete image from contentDoc
 * @param imageId Image ID to delete
 */
export function deleteImage(imageId: string): void {
  const imagesMap = loroCoordinator.getContentMap();
  imagesMap.delete(imageId);
  loroCoordinator.getDocuments().contentDoc.commit();
}

/**
 * List all images in contentDoc
 * @returns Array of image IDs
 */
export function listImages(): string[] {
  const imagesMap = loroCoordinator.getContentMap();
  const imageIds: string[] = [];

  // Get all keys from the map
  const mapData = imagesMap.toJSON();
  for (const key in mapData) {
    if (key.startsWith('image_') || key.startsWith('asset_image_')) {
      imageIds.push(key);
    }
  }

  return imageIds;
}

/**
 * Convert image Uint8Array to blob URL for HTML img element
 * @param imageData Image binary data
 * @param filename Optional filename to detect MIME type
 * @returns Blob URL
 */
export function createImageBlobUrl(imageData: Uint8Array, filename?: string): string {
  try {
    // Detect MIME type from filename extension
    let mimeType = 'image/png'; // default
    if (filename) {
      const ext = filename.toLowerCase().split('.').pop();
      switch (ext) {
        case 'jpg': case 'jpeg': mimeType = 'image/jpeg'; break;
        case 'png': mimeType = 'image/png'; break;
        case 'gif': mimeType = 'image/gif'; break;
        case 'webp': mimeType = 'image/webp'; break;
        case 'svg': mimeType = 'image/svg+xml'; break;
      }
    }

    // Convert to standard Uint8Array and create blob
    const buffer = Uint8Array.from(imageData);
    const blob = new Blob([buffer], { type: mimeType });
    const url = URL.createObjectURL(blob);

    return url;
  } catch (error) {
    console.error('❌ [ImageService] Error creating blob URL:', error);
    throw error;
  }
}
