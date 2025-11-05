/**
 * Asset Service - Generic handling for videos, images, and files
 * Supports static assets stored in non-CRDT storage
 */

import { open } from '@tauri-apps/plugin-dialog';
import { readFile } from '@tauri-apps/plugin-fs';
import { loroCoordinator } from '../../shared/loro/loroCoordinator';
import { generateAssetIdCEL } from './celEvaluator';

export type AssetType = 'video' | 'image' | 'file' | 'audio';

export interface AssetMetadata {
  id: string;
  filename: string;
  size: number;
  type: AssetType;
  uploadedAt: number;
}

/**
 * Default file filters for different asset types
 */
const DEFAULT_ASSET_FILTERS: Record<AssetType, { name: string; extensions: string[] }> = {
  video: {
    name: 'Video (Browser Supported)',
    extensions: ['mp4', 'webm', 'ogg']
  },
  image: {
    name: 'Images',
    extensions: ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg']
  },
  file: {
    name: 'Documents',
    extensions: ['pdf', 'txt', 'doc', 'docx', 'xls', 'xlsx', 'csv']
  },
  audio: {
    name: 'Audio Files',
    extensions: ['mp3', 'ogg', 'aac', 'flac', 'm4a']
  }
};

/**
 * Get current file type filters (configurable for 'file' type)
 */
function getAssetFilters() {
  const filters = { ...DEFAULT_ASSET_FILTERS };

  // Check if custom file types are configured
  const stateMap = loroCoordinator.getStateMap();
  const customFileTypes = stateMap.get('allowedFileTypes');

  if (customFileTypes && Array.isArray(customFileTypes) && customFileTypes.length > 0) {
    filters.file = {
      name: 'Documents',
      extensions: customFileTypes
    };
  }

  return filters;
}

/**
 * MIME type detection from filename extension
 */
function getMimeType(filename: string, assetType: AssetType): string {
  const ext = filename.toLowerCase().split('.').pop() || '';

  if (assetType === 'video') {
    switch (ext) {
      case 'webm': return 'video/webm';
      case 'ogg': case 'ogv': return 'video/ogg';
      case 'mp4': case 'm4v': return 'video/mp4';
      default: return 'video/mp4';
    }
  }

  if (assetType === 'image') {
    switch (ext) {
      case 'jpg': case 'jpeg': return 'image/jpeg';
      case 'png': return 'image/png';
      case 'gif': return 'image/gif';
      case 'webp': return 'image/webp';
      case 'svg': return 'image/svg+xml';
      default: return 'image/jpeg';
    }
  }

  if (assetType === 'file') {
    switch (ext) {
      case 'pdf': return 'application/pdf';
      case 'txt': return 'text/plain';
      case 'doc': case 'docx': return 'application/msword';
      case 'xls': case 'xlsx': return 'application/vnd.ms-excel';
      case 'csv': return 'text/csv';
      default: return 'application/octet-stream';
    }
  }

  if (assetType === 'audio') {
    switch (ext) {
      case 'mp3': return 'audio/mpeg';
      case 'ogg': return 'audio/ogg';
      case 'aac': return 'audio/aac';
      case 'flac': return 'audio/flac';
      case 'm4a': return 'audio/mp4';
      default: return 'audio/mpeg';
    }
  }

  return 'application/octet-stream';
}

/**
 * Generate asset ID using OCaml WASM module
 * Format: asset_{type}_{timestamp}_{random}
 */
export function generateAssetId(assetType: AssetType): string {
  return generateAssetIdCEL(assetType);
}

/**
 * Open file dialog and upload asset to staticAssets
 * @param assetType Type of asset to upload
 * @returns Asset metadata, or null if cancelled
 */
export async function uploadAsset(assetType: AssetType): Promise<AssetMetadata | null> {
  try {
    // Get current filters (may be customized for 'file' type)
    const filters = getAssetFilters();

    // Open file dialog
    const selected = await open({
      multiple: false,
      filters: [filters[assetType]]
    });

    if (!selected || typeof selected !== 'string') {
      return null; // User cancelled
    }

    console.log(`📁 [AssetService] Selected ${assetType}:`, selected);

    // Read file as binary
    const fileData = await readFile(selected);

    // Extract filename from path
    const filename = selected.split(/[/\\\\]/).pop() || `${assetType}`;

    // Generate unique ID using OCaml
    const assetId = generateAssetId(assetType);

    // Store in staticAssets (non-CRDT)
    loroCoordinator.setStaticAsset(assetId, fileData);
    console.log(`📦 [AssetService] Stored ${assetType} in staticAssets:`, assetId);

    // Store filename in contentDoc for later retrieval (MIME type detection)
    const contentMap = loroCoordinator.getContentMap();
    contentMap.set(`${assetId}_filename`, filename);
    loroCoordinator.getDocuments().contentDoc.commit();

    const metadata: AssetMetadata = {
      id: assetId,
      filename,
      size: fileData.length,
      type: assetType,
      uploadedAt: Date.now()
    };

    console.log(`✅ [AssetService] ${assetType} uploaded:`, metadata);

    return metadata;
  } catch (error) {
    console.error(`❌ [AssetService] ${assetType} upload failed:`, error);
    throw error;
  }
}

/**
 * Get asset data from staticAssets by ID
 * @param assetId Asset ID to retrieve
 * @returns Asset data as Uint8Array, or null if not found
 */
export function getAsset(assetId: string): Uint8Array | null {
  try {
    const assetData = loroCoordinator.getStaticAsset(assetId);

    if (assetData) {
      // Convert to standard Uint8Array for compatibility
      return Uint8Array.from(assetData);
    }

    return null;
  } catch (error) {
    console.error('❌ [AssetService] Error getting asset:', error);
    return null;
  }
}

/**
 * Create blob URL from asset data
 * @param data Asset binary data
 * @param assetId Asset ID (for asset type detection)
 * @param filename Optional filename for better MIME type detection
 * @returns Blob URL
 */
export function createAssetBlobUrl(data: Uint8Array, assetId: string, filename?: string): string {
  try {
    // Extract asset type from ID (format: asset_{type}_{timestamp}_{random})
    const parts = assetId.split('_');
    const assetType = (parts[1] || 'file') as AssetType;

    // Detect MIME type using filename if provided, otherwise use asset type
    const mimeType = getMimeType(filename || '', assetType);

    // Create blob with proper MIME type
    // Use data directly if it's already a Uint8Array, otherwise convert
    const buffer = data instanceof Uint8Array ? data : new Uint8Array(data);
    const blob = new Blob([buffer], { type: mimeType });

    const url = URL.createObjectURL(blob);

    return url;
  } catch (error) {
    console.error('❌ [AssetService] Error creating blob URL:', error);
    throw error;
  }
}

/**
 * Delete asset from staticAssets
 * @param assetId Asset ID to delete
 */
export function deleteAsset(assetId: string): void {
  loroCoordinator.deleteStaticAsset(assetId);
  console.log('🗑️ [AssetService] Asset deleted:', assetId);
}

/**
 * List all assets
 * @returns Array of asset IDs
 */
export function listAssets(): string[] {
  return loroCoordinator.listStaticAssets();
}

/**
 * Set allowed file types for 'file' asset uploads
 * @param types Array of file extensions (e.g., ['pdf', 'txt', 'zip'])
 */
export function setAllowedFileTypes(types: string[]): void {
  const stateMap = loroCoordinator.getStateMap();
  stateMap.set('allowedFileTypes', types);
  loroCoordinator.getDocuments().contentDoc.commit();
  console.log('📝 [AssetService] Allowed file types updated:', types);
}

/**
 * Get currently allowed file types for 'file' asset uploads
 * @returns Array of file extensions
 */
export function getAllowedFileTypes(): string[] {
  const stateMap = loroCoordinator.getStateMap();
  const customFileTypes = stateMap.get('allowedFileTypes');

  if (customFileTypes && Array.isArray(customFileTypes)) {
    return customFileTypes;
  }

  // Return default if not configured
  return DEFAULT_ASSET_FILTERS.file.extensions;
}

/**
 * Get asset data with legacy CRDT fallback
 * Priority: 1. Static assets (new), 2. Legacy CRDT storage
 * @param assetId Asset ID to retrieve
 * @param assetType Optional asset type for legacy fallback
 * @returns Asset data as Uint8Array, or null if not found
 */
export function getAssetData(assetId: string, assetType?: AssetType): Uint8Array | null {
  try {
    // First check static assets (new non-CRDT storage)
    const staticAsset = loroCoordinator.getStaticAsset(assetId);
    if (staticAsset) {
      return Uint8Array.from(staticAsset);
    }

    // Fallback to legacy CRDT storage
    // Auto-detect type from ID if not provided
    const detectedType = assetType || (assetId.split('_')[1] as AssetType);

    if (detectedType === 'video') {
      // Videos were stored in a separate map
      const videosMap = loroCoordinator.getVideosMap();
      const videoData = videosMap.get(assetId);
      if (videoData instanceof Uint8Array) {
        return Uint8Array.from(videoData);
      }
    } else {
      // Images and files were stored in contentDoc
      const contentMap = loroCoordinator.getContentMap();
      const assetData = contentMap.get(assetId);
      if (assetData instanceof Uint8Array) {
        return Uint8Array.from(assetData);
      }
    }

    return null;
  } catch (error) {
    console.error('❌ [AssetService] Error getting asset data:', error);
    return null;
  }
}

/**
 * Get asset filename from contentDoc
 * @param assetId Asset ID
 * @returns Filename or empty string
 */
export function getAssetFilename(assetId: string): string {
  try {
    // Check contentDoc for filename (works for all asset types)
    const contentMap = loroCoordinator.getContentMap();
    const filename = contentMap.get(`${assetId}_filename`);
    if (typeof filename === 'string') {
      return filename;
    }

    // Fallback: check videos map for legacy video filenames
    const videosMap = loroCoordinator.getVideosMap();
    const videoFilename = videosMap.get(`${assetId}_filename`);
    if (typeof videoFilename === 'string') {
      return videoFilename;
    }

    return '';
  } catch (error) {
    console.error('❌ [AssetService] Error getting asset filename:', error);
    return '';
  }
}

/**
 * Download asset to user's computer
 * @param assetId Asset ID to download
 * @param filename Filename for the download
 */
export function downloadAsset(assetId: string, filename: string): void {
  try {
    // Get asset data
    const assetData = getAssetData(assetId);
    if (!assetData) {
      console.error('❌ [AssetService] Asset not found for download:', assetId);
      return;
    }

    // Extract asset type from ID
    const parts = assetId.split('_');
    const assetType = (parts[1] || 'file') as AssetType;

    // Detect MIME type
    const mimeType = getMimeType(filename, assetType);

    // Create blob and blob URL
    // Convert to standard Uint8Array for compatibility
    const buffer = Uint8Array.from(assetData);
    const blob = new Blob([buffer], { type: mimeType });
    const url = URL.createObjectURL(blob);

    // Create temporary anchor element and trigger download
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();

    // Cleanup
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    console.log('✅ [AssetService] Asset downloaded:', filename);
  } catch (error) {
    console.error('❌ [AssetService] Download failed:', error);
  }
}

/**
 * Format file size in human-readable format
 * @param bytes File size in bytes
 * @returns Formatted string (e.g., "1.5 MB")
 */
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B';

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const k = 1024;
  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${units[i]}`;
}

/**
 * Get emoji icon for asset type
 * @param filename Filename to detect type
 * @param assetType Optional asset type override
 * @returns Emoji icon
 */
export function getAssetIcon(filename: string, assetType?: AssetType): string {
  // If asset type is provided, use it
  if (assetType) {
    switch (assetType) {
      case 'image': return '🖼️';
      case 'video': return '🎬';
      case 'file': return '📄';
    }
  }

  // Otherwise detect from filename extension
  const ext = filename.toLowerCase().split('.').pop() || '';

  // Image extensions
  if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'].includes(ext)) {
    return '🖼️';
  }

  // Video extensions
  if (['mp4', 'webm', 'ogg', 'ogv', 'm4v'].includes(ext)) {
    return '🎬';
  }

  // Audio extensions
  if (['mp3', 'wav', 'ogg', 'aac', 'flac'].includes(ext)) {
    return '🎵';
  }

  // Document types
  if (['pdf'].includes(ext)) return '📕';
  if (['doc', 'docx'].includes(ext)) return '📘';
  if (['xls', 'xlsx', 'csv'].includes(ext)) return '📊';
  if (['txt', 'md'].includes(ext)) return '📝';
  if (['zip', 'rar', '7z', 'tar', 'gz'].includes(ext)) return '📦';

  // Default file icon
  return '📄';
}
