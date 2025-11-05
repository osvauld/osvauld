<script lang="ts">
/**
 * AudioBlock - HTML5 audio player with staticAssets storage
 */

import type { AudioBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL } from '../../lib/services/celEvaluator';
import { getAssetData, getAssetFilename, createAssetBlobUrl } from '../../lib/services/assetService';

interface Props {
  block: AudioBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context }: Props = $props();

// Interpolate src (audio ID)
const audioId = $derived(interpolateCEL(block.src, context));

// Cache blob URLs by audioId to prevent recreation
let blobUrlCache = $state<Map<string, string>>(new Map());

// Get audio data from staticAssets and create blob URL (cached)
const audioBlobUrl = $derived.by(() => {
  if (!audioId) {
    return null;
  }

  // Check if it's a regular URL (http/https)
  if (audioId.startsWith('http://') || audioId.startsWith('https://')) {
    return audioId;
  }

  // Check cache first
  if (blobUrlCache.has(audioId)) {
    return blobUrlCache.get(audioId)!;
  }

  // Otherwise, look up in staticAssets
  const audioData = getAssetData(audioId, 'audio');
  if (!audioData) {
    return null;
  }

  // Get filename for MIME type detection
  const filename = getAssetFilename(audioId);

  const blobUrl = createAssetBlobUrl(audioData, audioId, filename);

  // Cache it
  blobUrlCache.set(audioId, blobUrl);

  return blobUrl;
});

// Get filename for display
const filename = $derived.by(() => {
  if (!audioId) return '';
  return getAssetFilename(audioId) || 'Audio File';
});

// Interpolate other properties
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);

// Audio element reference (for error handling only)
let audioElement: HTMLAudioElement | null = $state(null);
</script>

{#if audioBlobUrl}
  <div
    class="audio-block {block.class || ''}"
    style={styles || "display: flex; align-items: center; gap: 1rem; padding: 1rem; background: #f9fafb; border: 2px solid #e5e7eb; border-radius: 8px;"}
  >
    <!-- Audio Icon -->
    <div style="font-size: 2rem; flex-shrink: 0;">
      🎵
    </div>

    <!-- Audio Info and Player -->
    <div style="flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0.5rem;">
      <div style="font-weight: 600; color: #111827; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
        {filename}
      </div>

      <!-- HTML5 Audio Player -->
      <audio
        bind:this={audioElement}
        src={audioBlobUrl}
        controls={block.controls !== false}
        autoplay={block.autoplay === true}
        loop={block.loop === true}
        muted={block.muted ?? block.autoplay === true}
        preload="metadata"
        style="width: 100%;"
        onerror={(e) => {
          console.error('[AudioBlock] Audio error:', audioElement?.error);
        }}
      >
        Your browser does not support the audio tag.
      </audio>
    </div>
  </div>
{:else}
  <div style="border: 2px dashed #fbbf24; padding: 1rem; background: #fef3c7; border-radius: 4px;">
    <strong>⚠️ Audio not found</strong>
    <p style="margin: 0.5rem 0 0 0; color: #78350f;">
      {#if audioId}
        Audio ID: <code>{audioId}</code> not found in staticAssets.
      {:else}
        No audio source specified.
      {/if}
    </p>
  </div>
{/if}
