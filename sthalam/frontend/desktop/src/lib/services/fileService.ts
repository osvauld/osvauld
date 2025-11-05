/**
 * File Service - Handle generic file uploads and downloads
 * Wrapper around assetService for file-specific operations
 */

import { uploadAsset, type AssetMetadata } from './assetService';
import { loroCoordinator } from '../../shared/loro/loroCoordinator';

export interface FileMetadata {
  id: string;
  filename: string;
  size: number;
  uploadedAt: number;
}

/**
 * Open file dialog and upload file to staticAssets
 * @returns File ID and metadata, or null if cancelled
 */
export async function uploadFile(): Promise<FileMetadata | null> {
  try {
    const result = await uploadAsset('file');

    if (!result) {
      return null; // User cancelled
    }

    // Convert AssetMetadata to FileMetadata
    const metadata: FileMetadata = {
      id: result.id,
      filename: result.filename,
      size: result.size,
      uploadedAt: result.uploadedAt
    };

    return metadata;
  } catch (error) {
    console.error('❌ [FileService] Upload failed:', error);
    throw error;
  }
}

/**
 * Get file data by ID
 * Priority: 1. Static assets (new), 2. contentDoc.files (legacy CRDT)
 * @param fileId File ID to retrieve
 * @returns File data as Uint8Array, or null if not found
 */
export function getFile(fileId: string): Uint8Array | null {
  try {
    // First check static assets (new non-CRDT storage)
    const staticAsset = loroCoordinator.getStaticAsset(fileId);
    if (staticAsset) {
      return Uint8Array.from(staticAsset);
    }

    // Fallback to contentDoc.files (legacy CRDT storage)
    const filesMap = loroCoordinator.getContentMap();
    const fileData = filesMap.get(fileId);

    if (fileData instanceof Uint8Array) {
      return Uint8Array.from(fileData);
    }

    return null;
  } catch (error) {
    console.error('❌ [FileService] Error getting file:', error);
    return null;
  }
}

/**
 * Get file filename from contentDoc
 * @param fileId File ID
 * @returns Filename or empty string
 */
export function getFileFilename(fileId: string): string {
  try {
    const filesMap = loroCoordinator.getContentMap();
    const filename = filesMap.get(`${fileId}_filename`);
    return typeof filename === 'string' ? filename : '';
  } catch (error) {
    console.error('❌ [FileService] Error getting file filename:', error);
    return '';
  }
}

/**
 * Delete file from contentDoc
 * @param fileId File ID to delete
 */
export function deleteFile(fileId: string): void {
  const filesMap = loroCoordinator.getContentMap();
  filesMap.delete(fileId);
  loroCoordinator.getDocuments().contentDoc.commit();
}

/**
 * List all files in contentDoc
 * @returns Array of file IDs
 */
export function listFiles(): string[] {
  const filesMap = loroCoordinator.getContentMap();
  const fileIds: string[] = [];

  // Get all keys from the map
  const mapData = filesMap.toJSON();
  for (const key in mapData) {
    if (key.startsWith('file_') || key.startsWith('asset_file_')) {
      fileIds.push(key);
    }
  }

  return fileIds;
}

/**
 * Trigger browser download of a file
 * Creates a blob URL and uses a temporary anchor element to trigger download
 * @param fileId File ID to download
 * @param filename Filename to save as
 */
export function downloadFile(fileId: string, filename: string): void {
  try {
    // Get file data
    const fileData = getFile(fileId);
    if (!fileData) {
      console.error('❌ [FileService] File not found:', fileId);
      return;
    }

    // Detect MIME type from filename extension
    let mimeType = 'application/octet-stream'; // default
    const ext = filename.toLowerCase().split('.').pop();
    switch (ext) {
      case 'pdf': mimeType = 'application/pdf'; break;
      case 'txt': mimeType = 'text/plain'; break;
      case 'doc': mimeType = 'application/msword'; break;
      case 'docx': mimeType = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'; break;
      case 'xls': mimeType = 'application/vnd.ms-excel'; break;
      case 'xlsx': mimeType = 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'; break;
      case 'csv': mimeType = 'text/csv'; break;
      case 'json': mimeType = 'application/json'; break;
      case 'xml': mimeType = 'application/xml'; break;
      case 'zip': mimeType = 'application/zip'; break;
    }

    // Create blob and blob URL
    const buffer = Uint8Array.from(fileData);
    const blob = new Blob([buffer], { type: mimeType });
    const url = URL.createObjectURL(blob);

    // Create temporary anchor element and trigger download
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);

    // Clean up blob URL after a short delay
    setTimeout(() => {
      URL.revokeObjectURL(url);
    }, 100);
  } catch (error) {
    console.error('❌ [FileService] Error downloading file:', error);
    throw error;
  }
}

/**
 * Get human-readable file size
 * @param bytes File size in bytes
 * @returns Formatted string (e.g., "1.5 MB")
 */
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + ' ' + sizes[i];
}

/**
 * Get file extension icon/emoji
 * @param filename Filename to detect extension
 * @returns Emoji representing file type
 */
export function getFileIcon(filename: string): string {
  const ext = filename.toLowerCase().split('.').pop();
  switch (ext) {
    case 'pdf': return '📄';
    case 'txt': return '📝';
    case 'doc': case 'docx': return '📃';
    case 'xls': case 'xlsx': case 'csv': return '📊';
    case 'json': case 'xml': return '📋';
    case 'zip': return '🗜️';
    default: return '📎';
  }
}
