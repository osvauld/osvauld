/**
 * Asset Service - Generic handling for videos, images, and files
 * Supports static assets stored in non-CRDT storage
 */

import { open } from '@tauri-apps/plugin-dialog';
import { readFile } from '@tauri-apps/plugin-fs';
import { loroCoordinator } from '../../shared/loro/loroCoordinator';
import { generateAssetIdCEL } from './celEvaluator';

export type AssetType = 'video' | 'image' | 'file';

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
const DEFAULT_ASSET_FILTERS = {
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
 * @param assetId Asset ID (for filename/MIME type detection)
 * @returns Blob URL
 */
export function createAssetBlobUrl(data: Uint8Array, assetId: string): string {
  try {
    // Extract asset type from ID (format: asset_{type}_{timestamp}_{random})
    const parts = assetId.split('_');
    const assetType = (parts[1] || 'file') as AssetType;

    // Detect MIME type (would ideally also use filename, but we don't store it)
    const mimeType = getMimeType('', assetType);

    // Create blob with proper MIME type
    const buffer = Uint8Array.from(data);
    const blob = new Blob([buffer], { type: mimeType });
    const url = URL.createObjectURL(blob);

    console.log(`🔗 [AssetService] Blob URL created for ${assetType}:`, url);
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
