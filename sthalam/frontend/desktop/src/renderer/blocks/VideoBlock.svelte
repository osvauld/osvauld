<script lang="ts">
/**
 * VideoBlock - HTML5 video player with contentDoc video storage
 */

import type { VideoBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL } from '../../lib/services/celEvaluator';
import { getVideo, getVideoFilename, createVideoBlobUrl } from '../../lib/services/videoService';
import { onDestroy } from 'svelte';

interface Props {
  block: VideoBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context }: Props = $props();

// Interpolate src (video ID)
const videoId = $derived(interpolateCEL(block.src, context));

// Cache blob URLs by videoId to prevent recreation
let blobUrlCache = $state<Map<string, string>>(new Map());

// Get video data from contentDoc and create blob URL (cached)
const videoBlobUrl = $derived.by(() => {
  if (!videoId) {
    return null;
  }

  // Check if it's a regular URL (http/https)
  if (videoId.startsWith('http://') || videoId.startsWith('https://')) {
    return videoId;
  }

  // Check cache first
  if (blobUrlCache.has(videoId)) {
    return blobUrlCache.get(videoId)!;
  }

  // Otherwise, look up in contentDoc
  const videoData = getVideo(videoId);
  if (!videoData) {
    console.warn('[VideoBlock] Video not found:', videoId);
    return null;
  }

  // Get filename for MIME type detection
  const filename = getVideoFilename(videoId);

  const blobUrl = createVideoBlobUrl(videoData, filename);

  // Cache it
  blobUrlCache.set(videoId, blobUrl);

  return blobUrl;
});

// Interpolate other properties
const width = $derived(block.width ? interpolateCEL(String(block.width), context) : undefined);
const height = $derived(block.height ? interpolateCEL(String(block.height), context) : undefined);
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);

// Track previous blob URL to clean up
let previousBlobUrl: string | null = null;

// Cleanup blob URL when component is destroyed or URL changes
// TEMPORARILY DISABLED - blob URLs being revoked too early
/*
$effect(() => {
  const currentUrl = videoBlobUrl;

  // Revoke previous blob URL if it changed
  if (previousBlobUrl && previousBlobUrl !== currentUrl && previousBlobUrl.startsWith('blob:')) {
    URL.revokeObjectURL(previousBlobUrl);
  }

  previousBlobUrl = currentUrl;
});

onDestroy(() => {
  if (previousBlobUrl && previousBlobUrl.startsWith('blob:')) {
    URL.revokeObjectURL(previousBlobUrl);
  }
});
*/

// Video element reference for cleanup
let videoElement: HTMLVideoElement | null = $state(null);

// Explicitly load video when src changes
$effect(() => {
  if (videoElement && videoBlobUrl) {
    // Wait for DOM to update before calling load()
    // This ensures the src attribute is actually set on the element
    requestAnimationFrame(() => {
      if (!videoElement) return;
      videoElement.load();
    });
  }
});
</script>

{#if videoBlobUrl}
  <!-- svelte-ignore a11y_media_has_caption -->
  <video
      bind:this={videoElement}
      src={videoBlobUrl}
      controls={block.controls !== false}
      autoplay={block.autoplay === true}
      loop={block.loop === true}
      muted={block.muted ?? block.autoplay === true}
      preload="auto"
      {width}
      {height}
      style={styles}
      class={block.class}
      onerror={(e) => {
        console.error('[VideoBlock] Video error:', videoElement?.error);
      }}
      onstalled={() => {
        if (videoElement) {
          videoElement.load();
        }
      }}
    >
      Your browser does not support the video tag.
    </video>
{:else}
  <div style="border: 2px dashed #fbbf24; padding: 1rem; background: #fef3c7; border-radius: 4px;">
    <strong>⚠️ Video not found</strong>
    <p style="margin: 0.5rem 0 0 0; color: #78350f;">
      {#if videoId}
        Video ID: <code>{videoId}</code> not found in contentDoc.
      {:else}
        No video source specified.
      {/if}
    </p>
  </div>
{/if}
