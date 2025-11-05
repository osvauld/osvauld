<script lang="ts">
/**
 * FileBlock - File display with metadata and download button
 * Supports staticAssets storage with asset IDs (asset_file_*)
 */

import type { FileBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL } from '../../lib/services/celEvaluator';
import { getFile, getFileFilename, downloadFile, formatFileSize, getFileIcon } from '../../lib/services/fileService';

interface Props {
  block: FileBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context }: Props = $props();

// Interpolate src (file ID)
const fileId = $derived(interpolateCEL(block.src, context));

// Check if file exists and get metadata
const fileExists = $derived.by(() => {
  if (!fileId) return false;
  const fileData = getFile(fileId);
  return fileData !== null;
});

const filename = $derived.by(() => {
  if (!fileId) return '';
  return getFileFilename(fileId) || fileId;
});

const fileSize = $derived.by(() => {
  if (!fileId) return '';
  const fileData = getFile(fileId);
  if (!fileData) return '';
  return formatFileSize(fileData.length);
});

const fileIcon = $derived.by(() => {
  return getFileIcon(filename);
});

// Interpolate other properties
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);

// Handle download button click
function handleDownload() {
  if (fileId && filename) {
    downloadFile(fileId, filename);
  }
}
</script>

{#if fileExists}
  <div
    class="file-block {block.class || ''}"
    style={styles || "border: 2px solid #e5e7eb; border-radius: 8px; padding: 1rem; background: #f9fafb; display: flex; align-items: center; gap: 1rem;"}
  >
    <!-- File Icon -->
    <div style="font-size: 2rem; flex-shrink: 0;">
      {fileIcon}
    </div>

    <!-- File Info -->
    <div style="flex: 1; min-width: 0;">
      <div style="font-weight: 600; color: #111827; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
        {filename}
      </div>
      <div style="font-size: 0.875rem; color: #6b7280; margin-top: 0.25rem;">
        {fileSize}
      </div>
    </div>

    <!-- Download Button -->
    <button
      onclick={handleDownload}
      style="padding: 0.5rem 1rem; background: #3b82f6; color: white; border: none; border-radius: 6px; cursor: pointer; font-weight: 500; flex-shrink: 0; transition: background 0.2s;"
      onmouseover={(e) => e.currentTarget.style.background = '#2563eb'}
      onmouseout={(e) => e.currentTarget.style.background = '#3b82f6'}
    >
      ⬇ Download
    </button>
  </div>
{:else}
  <div style="border: 2px dashed #fbbf24; padding: 1rem; background: #fef3c7; border-radius: 4px;">
    <strong>⚠️ File not found</strong>
    <p style="margin: 0.5rem 0 0 0; color: #78350f;">
      {#if fileId}
        Asset ID: <code>{fileId}</code> not found in staticAssets.
      {:else}
        No file source specified.
      {/if}
    </p>
  </div>
{/if}
