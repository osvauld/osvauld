<script lang="ts">
/**
 * ImageBlock - Image display with staticAssets storage
 * BREAKING CHANGE: Only supports asset IDs (asset_image_*), not URLs
 */

import type { ImageBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL } from '../../lib/services/celEvaluator';
import { getAssetData, getAssetFilename, createAssetBlobUrl } from '../../lib/services/assetService';

interface Props {
  block: ImageBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context }: Props = $props();

// Interpolate src (image ID)
const imageId = $derived(interpolateCEL(block.src, context));

// Cache blob URLs by imageId to prevent recreation
let blobUrlCache = $state<Map<string, string>>(new Map());

// Get image data from staticAssets and create blob URL (cached)
const imageBlobUrl = $derived.by(() => {
  if (!imageId) {
    return null;
  }

  // Check cache first
  if (blobUrlCache.has(imageId)) {
    return blobUrlCache.get(imageId)!;
  }

  // Look up in staticAssets
  const imageData = getAssetData(imageId, 'image');
  if (!imageData) {
    console.warn('[ImageBlock] Image not found:', imageId);
    return null;
  }

  // Get filename for MIME type detection
  const filename = getAssetFilename(imageId);

  const blobUrl = createAssetBlobUrl(imageData, imageId, filename);

  // Cache it
  blobUrlCache.set(imageId, blobUrl);

  return blobUrl;
});

// Interpolate other properties
const alt = $derived(block.alt ? interpolateCEL(block.alt, context) : '');
const width = $derived(block.width ? interpolateCEL(String(block.width), context) : undefined);
const height = $derived(block.height ? interpolateCEL(String(block.height), context) : undefined);
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);
</script>

{#if imageBlobUrl}
  <img
    src={imageBlobUrl}
    {alt}
    {width}
    {height}
    style={styles}
    class={block.class}
  />
{:else}
  <div style="border: 2px dashed #fbbf24; padding: 1rem; background: #fef3c7; border-radius: 4px;">
    <strong>⚠️ Image not found</strong>
    <p style="margin: 0.5rem 0 0 0; color: #78350f;">
      {#if imageId}
        Asset ID: <code>{imageId}</code> not found in staticAssets.
      {:else}
        No image source specified.
      {/if}
    </p>
  </div>
{/if}
