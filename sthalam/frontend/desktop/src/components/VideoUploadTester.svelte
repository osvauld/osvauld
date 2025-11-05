<script lang="ts">
/**
 * VideoUploadTester - Utility component for testing video uploads
 * Add this to your UI temporarily to upload videos to contentDoc
 */

import { uploadVideo, listVideos, deleteVideo, type VideoMetadata } from '../lib/services/videoService';
import { loroCoordinator } from '../shared/loro/loroCoordinator';

let uploading = $state(false);
let lastUpload: VideoMetadata | null = $state(null);
let error: string | null = $state(null);
let videoList = $state<string[]>([]);

// Refresh video list
function refreshVideoList() {
  videoList = listVideos();
}

// Initial load
refreshVideoList();

// Handle upload
async function handleUpload() {
  try {
    uploading = true;
    error = null;

    const result = await uploadVideo();

    if (result) {
      lastUpload = result;
      refreshVideoList();
      console.log('✅ Video uploaded successfully:', result);
    }
  } catch (err) {
    error = err instanceof Error ? err.message : String(err);
    console.error('❌ Upload failed:', err);
  } finally {
    uploading = false;
  }
}

// Handle delete
function handleDelete(videoId: string) {
  if (confirm(`Delete video ${videoId}?`)) {
    deleteVideo(videoId);
    refreshVideoList();
  }
}

// Copy video ID to clipboard
function copyToClipboard(text: string) {
  navigator.clipboard.writeText(text);
  alert(`Copied: ${text}`);
}
</script>

<div style="
  border: 2px solid #2563eb;
  padding: 1.5rem;
  margin: 1rem;
  background: #eff6ff;
  border-radius: 8px;
  font-family: system-ui;
">
  <h3 style="margin: 0 0 1rem 0; color: #1e40af;">📹 Video Upload Tester</h3>

  <button
    onclick={handleUpload}
    disabled={uploading}
    style="
      padding: 0.75rem 1.5rem;
      background: {uploading ? '#9ca3af' : '#2563eb'};
      color: white;
      border: none;
      border-radius: 6px;
      cursor: {uploading ? 'not-allowed' : 'pointer'};
      font-weight: 600;
      font-size: 1rem;
    "
  >
    {uploading ? '⏳ Uploading...' : '📤 Upload Video'}
  </button>

  {#if lastUpload}
    <div style="
      margin-top: 1rem;
      padding: 1rem;
      background: #d1fae5;
      border-radius: 4px;
      border: 1px solid #10b981;
    ">
      <strong style="color: #065f46;">✅ Last Upload:</strong>
      <div style="margin-top: 0.5rem; font-family: monospace; font-size: 0.875rem;">
        <div><strong>ID:</strong> {lastUpload.id}</div>
        <div><strong>Filename:</strong> {lastUpload.filename}</div>
        <div><strong>Size:</strong> {(lastUpload.size / 1024 / 1024).toFixed(2)} MB</div>
        <button
          onclick={() => copyToClipboard(lastUpload.id)}
          style="
            margin-top: 0.5rem;
            padding: 0.5rem 1rem;
            background: #059669;
            color: white;
            border: none;
            border-radius: 4px;
            cursor: pointer;
            font-size: 0.875rem;
          "
        >
          📋 Copy ID
        </button>
      </div>
    </div>
  {/if}

  {#if error}
    <div style="
      margin-top: 1rem;
      padding: 1rem;
      background: #fee2e2;
      border-radius: 4px;
      border: 1px solid #ef4444;
      color: #991b1b;
    ">
      <strong>❌ Error:</strong> {error}
    </div>
  {/if}

  <div style="margin-top: 1.5rem;">
    <h4 style="margin: 0 0 0.5rem 0; color: #1e40af;">
      📚 Videos in ContentDoc ({videoList.length})
      <button
        onclick={refreshVideoList}
        style="
          margin-left: 0.5rem;
          padding: 0.25rem 0.5rem;
          background: #60a5fa;
          color: white;
          border: none;
          border-radius: 4px;
          cursor: pointer;
          font-size: 0.75rem;
        "
      >
        🔄 Refresh
      </button>
    </h4>
    {#if videoList.length === 0}
      <p style="color: #6b7280; font-style: italic; margin: 0.5rem 0;">
        No videos uploaded yet.
      </p>
    {:else}
      <div style="display: flex; flex-direction: column; gap: 0.5rem;">
        {#each videoList as videoId}
          <div style="
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 0.5rem;
            background: white;
            border: 1px solid #d1d5db;
            border-radius: 4px;
          ">
            <code style="font-size: 0.875rem; color: #374151;">{videoId}</code>
            <div style="display: flex; gap: 0.5rem;">
              <button
                onclick={() => copyToClipboard(videoId)}
                style="
                  padding: 0.25rem 0.75rem;
                  background: #10b981;
                  color: white;
                  border: none;
                  border-radius: 4px;
                  cursor: pointer;
                  font-size: 0.75rem;
                "
              >
                📋 Copy
              </button>
              <button
                onclick={() => handleDelete(videoId)}
                style="
                  padding: 0.25rem 0.75rem;
                  background: #ef4444;
                  color: white;
                  border: none;
                  border-radius: 4px;
                  cursor: pointer;
                  font-size: 0.75rem;
                "
              >
                🗑️ Delete
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div style="
    margin-top: 1.5rem;
    padding: 1rem;
    background: #fef3c7;
    border-radius: 4px;
    font-size: 0.875rem;
    color: #78350f;
  ">
    <strong>💡 How to use:</strong>
    <ol style="margin: 0.5rem 0 0 0; padding-left: 1.5rem;">
      <li>Click "Upload Video" and select a video file</li>
      <li>Copy the video ID</li>
      <li>Use in HUML: <code>src: "video_xxx"</code></li>
      <li>Videos are stored in contentDoc and sync to viewers!</li>
    </ol>
  </div>
</div>
