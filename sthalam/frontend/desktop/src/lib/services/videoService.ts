/**
 * Video Service - Handle video file uploads
 * Wrapper around assetService for video-specific operations
 */

import { uploadAsset, type AssetMetadata } from './assetService';
import { loroCoordinator } from '../../shared/loro/loroCoordinator';

export interface VideoMetadata {
  id: string;
  filename: string;
  size: number;
  uploadedAt: number;
}

/**
 * Open file dialog and upload video to staticAssets
 * @returns Video ID and metadata, or null if cancelled
 */
export async function uploadVideo(): Promise<VideoMetadata | null> {
  try {
    // Use generic asset service
    const result = await uploadAsset('video');

    if (!result) {
      return null; // User cancelled
    }

    // Convert AssetMetadata to VideoMetadata
    const metadata: VideoMetadata = {
      id: result.id,
      filename: result.filename,
      size: result.size,
      uploadedAt: result.uploadedAt
    };

    return metadata;
  } catch (error) {
    console.error('❌ [VideoService] Upload failed:', error);
    throw error;
  }
}

/**
 * Get video data by ID
 * Priority: 1. Static assets (new), 2. contentDoc.videos (legacy CRDT)
 * @param videoId Video ID to retrieve
 * @returns Video data as Uint8Array, or null if not found
 */
export function getVideo(videoId: string): Uint8Array | null {
  try {
    // First check static assets (new non-CRDT storage)
    const staticAsset = loroCoordinator.getStaticAsset(videoId);
    if (staticAsset) {
      return Uint8Array.from(staticAsset);
    }

    // Fallback to contentDoc.videos (legacy CRDT storage)
    const videosMap = loroCoordinator.getVideosMap();
    const videoData = videosMap.get(videoId);

    if (videoData instanceof Uint8Array) {
      return Uint8Array.from(videoData);
    }

    return null;
  } catch (error) {
    console.error('❌ [VideoService] Error getting video:', error);
    return null;
  }
}

/**
 * Get video filename from contentDoc
 * @param videoId Video ID
 * @returns Filename or empty string
 */
export function getVideoFilename(videoId: string): string {
  try {
    const videosMap = loroCoordinator.getVideosMap();
    const filename = videosMap.get(`${videoId}_filename`);
    return typeof filename === 'string' ? filename : '';
  } catch (error) {
    console.error('❌ [VideoService] Error getting video filename:', error);
    return '';
  }
}

/**
 * Delete video from contentDoc
 * @param videoId Video ID to delete
 */
export function deleteVideo(videoId: string): void {
  const videosMap = loroCoordinator.getVideosMap();
  videosMap.delete(videoId);
  loroCoordinator.getDocuments().contentDoc.commit();
}

/**
 * List all videos in contentDoc
 * @returns Array of video IDs
 */
export function listVideos(): string[] {
  const videosMap = loroCoordinator.getVideosMap();
  const videoIds: string[] = [];

  // Get all keys from the map
  const mapData = videosMap.toJSON();
  for (const key in mapData) {
    if (key.startsWith('video_')) {
      videoIds.push(key);
    }
  }

  return videoIds;
}

/**
 * Convert video Uint8Array to blob URL for HTML5 video element
 * @param videoData Video binary data
 * @param filename Optional filename to detect MIME type
 * @returns Blob URL
 */
export function createVideoBlobUrl(videoData: Uint8Array, filename?: string): string {
  try {
    // Detect MIME type from filename extension
    let mimeType = 'video/mp4'; // default
    if (filename) {
      const ext = filename.toLowerCase().split('.').pop();
      switch (ext) {
        case 'webm': mimeType = 'video/webm'; break;
        case 'ogg': case 'ogv': mimeType = 'video/ogg'; break;
        case 'mp4': case 'm4v': mimeType = 'video/mp4'; break;
      }
    }

    // Convert to standard Uint8Array and create blob
    const buffer = Uint8Array.from(videoData);
    const blob = new Blob([buffer], { type: mimeType });
    const url = URL.createObjectURL(blob);

    return url;
  } catch (error) {
    console.error('❌ [VideoService] Error creating blob URL:', error);
    throw error;
  }
}
