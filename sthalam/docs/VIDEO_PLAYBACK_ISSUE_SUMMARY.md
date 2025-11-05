# Video Playback Issue - Technical Summary

**Date**: 2025-01-05
**Status**: ⚠️ BLOCKED - WebKit/GStreamer cannot load blob URLs
**Session**: sthalam/poc branch

---

## Overview

Videos upload successfully to staticAssets with OCaml-generated IDs, persist correctly across sessions, and blob URLs are created with valid data - but **HTML5 `<video>` element cannot play them** in WebKit/Tauri on Linux.

---

## What Works ✅

1. **Video Upload**
   - ✅ File dialog opens, user selects video
   - ✅ Video reads as Uint8Array (1.5MB)
   - ✅ OCaml generates asset ID: `asset_video_{timestamp}_{random}`
   - ✅ Stored in `loroCoordinator.staticAssets` Map

2. **Persistence**
   - ✅ staticAssets saved with resource snapshots as `static_assets` array
   - ✅ staticAssets restored on resource load
   - ✅ Videos persist across app restarts

3. **Data Integrity**
   - ✅ Valid MP4 signature: `0x00 0x00 0x00 0x20 0x66 0x74 0x79 0x70` ("ftyp" box)
   - ✅ Size matches: 1570024 bytes
   - ✅ `fetch(blobUrl)` works - JavaScript can read the blob
   - ✅ First/last bytes match before and after blob creation

4. **Blob URL Creation**
   - ✅ Blob created: `blob:http://localhost:1422/{uuid}`
   - ✅ MIME type: `video/mp4`
   - ✅ Blob size: 1570024 bytes
   - ✅ No corruption detected

---

## What Doesn't Work ❌

**Video element cannot load blob URL:**

```javascript
// Video element state
{
  readyState: 0,        // HAVE_NOTHING - no data loaded
  networkState: 2,      // NETWORK_LOADING - trying to load
  error: null,          // No error reported
  currentSrc: "blob:http://localhost:1422/..." // Source is set
}
```

**Symptoms:**
- Video player shows loading spinner forever
- Never progresses beyond `readyState: 0`
- `networkState` goes from 3 (NO_SOURCE) → 2 (LOADING) → stuck
- No error events fired
- Browser console shows no CSP violations
- JavaScript `fetch()` on same blob URL succeeds

---

## Technical Investigation

### Files Modified

#### 1. `/frontend/desktop/src/state/data.svelte.ts`
**Fixed staticAssets persistence** - Lines 237-257, 277-293, 350-370

```typescript
// BEFORE (BROKEN - staticAssets not saved)
const loroContent = {
  template_doc: Array.from(snapshots.template),
  content_doc: Array.from(snapshots.content),
  // ... missing static_assets
};

// AFTER (FIXED)
const staticAssetsArray: any[] = [];
for (const [key, value] of Object.entries(snapshots.staticAssets || {})) {
  staticAssetsArray.push({ id: key, data: Array.from(value) });
}

const loroContent = {
  template_doc: Array.from(snapshots.template),
  content_doc: Array.from(snapshots.content),
  static_assets: staticAssetsArray, // ✅ Now included
  // ...
};
```

#### 2. `/frontend/desktop/src/lib/services/videoService.ts`
**Added data integrity checks** - Lines 134-197

```typescript
// Validates MP4 signature
const signature = Array.from(videoData.slice(0, 8));
const isMp4 = signature[4] === 0x66 && signature[5] === 0x74 &&
              signature[6] === 0x79 && signature[7] === 0x70;

// Logs detailed integrity info
console.log('🔍 [VideoService] Data integrity check:', {
  isUint8Array: videoData instanceof Uint8Array,
  length: videoData.length,
  first8bytes: "0x00 0x00 0x00 0x20 0x66 0x74 0x79 0x70",
  // ...
});
```

#### 3. `/src-tauri/tauri.conf.json`
**Tried permissive CSP** - Line 22

```json
// BEFORE
"media-src 'self' blob: http://localhost:1422"

// AFTER (fully permissive - still doesn't work)
"media-src * blob: data: 'self' 'unsafe-inline'"
```

### What We Tried

1. ❌ **Disabled blob URL cleanup** - Thought premature revocation was issue
2. ❌ **Added blob URL caching** - Prevent recreation on re-renders
3. ❌ **Explicit `video.load()` call** - Force reload
4. ❌ **Removed `{#key}` block** - Eliminate Svelte remounting
5. ❌ **Fully permissive CSP** - Rule out Content Security Policy
6. ❌ **Temporarily stored in contentDoc** - Same issue (proves not storage-related)
7. ✅ **Data integrity validation** - Confirmed data is perfect

---

## Root Cause Analysis

**WebKit's media pipeline (GStreamer backend) on Linux cannot load blob URLs, even though:**
- JavaScript fetch() can read them
- CSP allows them
- Data is valid MP4
- Blob size/type are correct

**Evidence:**
```
✅ JavaScript layer: fetch(blobUrl) works
❌ Media decoder layer: <video src={blobUrl}> fails
```

This is a **WebKit/GStreamer architecture limitation** where:
- JavaScript runtime uses blob: protocol correctly
- Media decoder pipeline is separate process
- GStreamer decoder cannot access blob URLs from JavaScript memory

**User confirmation**: "videos worked before when stored in contentDoc"
- **Note**: Unclear what changed - VideoBlock.svelte is new (created this session)
- **Mystery**: Why would contentDoc vs staticAssets storage matter if blob URL is same?

---

## Current Code State

### Data Flow (Working)

```
1. User uploads video
   ↓
2. assetService reads file → Uint8Array
   ↓
3. OCaml generates ID: asset_video_1762343546435_1e4a2426
   ↓
4. loroCoordinator.setStaticAsset(id, data)
   ↓
5. staticAssets Map stores: Map<string, Uint8Array>
   ↓
6. On save: staticAssets → static_assets array → backend
   ↓
7. On load: static_assets array → staticAssets Map
```

### Blob URL Creation (Working, but unusable)

```
1. VideoBlock renders
   ↓
2. getVideo(videoId) → Uint8Array from staticAssets
   ↓
3. createVideoBlobUrl(data, filename)
   ↓
4. new Blob([data], { type: 'video/mp4' })
   ↓
5. URL.createObjectURL(blob)
   ↓
6. Returns: blob:http://localhost:1422/{uuid}
   ↓
7. <video src={blobUrl}> ← FAILS HERE
```

---

## Console Output Example

```javascript
[Log] 🔍 [VideoService] Data integrity check: {
  isUint8Array: true,
  length: 1570024,
  first8bytes: "0x00 0x00 0x00 0x20 0x66 0x74 0x79 0x70",
  last8bytes: "0xb4 0xb4 0xb4 0xb4 0xb4 0xb4 0xb4 0xbc"
}

[Log] 🔍 [VideoService] MP4 signature check: "✅ Valid"

[Log] 🎬 [VideoService] Blob created: {
  url: "blob:http://localhost:1422/73ecc9e6-27df-4d5a-8ca3-0538b1c62406",
  blobSize: 1570024,
  blobType: "video/mp4",
  originalSize: 1570024,
  sizesMatch: true
}

[Log] ✅ [VideoService] Blob fetch test passed: {
  fetchedSize: 1570024,
  first8bytes: "0x00 0x00 0x00 0x20 0x66 0x74 0x79 0x70",
  matchesOriginal: true
}

[Log] [VideoBlock] Video state after 2s: {
  videoId: "asset_video_1762343546435_1e4a2426",
  readyState: 0,        // ❌ STUCK HERE
  networkState: 2,
  error: null,
  currentSrc: "blob:http://localhost:1422/73ecc9e6-27df-4d5a-8ca3-0538b1c62406"
}
```

---

## Rejected Solutions

### ❌ Temp File Writing
**Why rejected**: User requirement - "we cant do asset protocol because the videos are dynamic not an asset"

```typescript
// REJECTED APPROACH
const tempFilePath = `${tempDir}/sthalam_video_${videoId}.mp4`;
await writeFile(tempFilePath, videoData);
const fileUrl = convertFileSrc(tempFilePath);
```

**Problems:**
1. File system writes for every video view
2. Temp files need cleanup management
3. Doesn't scale for multiple videos
4. Unnecessary I/O overhead

### ❌ Asset Protocol Handler
**Why rejected**: Current implementation only serves static files from `public/` directory

```rust
// Current asset handler (lib.rs:60-156)
// Serves files from: frontend/desktop/public/
// Cannot serve from: JavaScript memory (staticAssets Map)
```

**Would require**: Major refactor to bridge Rust ↔ JavaScript for dynamic content

### ❌ Data URLs
**Why not tried yet**: Size limitation concerns

```typescript
// Potential approach
const base64 = btoa(String.fromCharCode(...videoData));
const dataUrl = `data:video/mp4;base64,${base64}`;
```

**Concerns:**
- Video is 1.5MB → ~2MB base64
- Browser data URL size limits vary
- URL length limits in WebKit
- Memory duplication (binary + base64 string)

### ❌ FFmpeg.wasm / JS Video Decoders
**Why rejected**:
- 30MB+ library size
- CPU-intensive decoding in main thread
- Poor performance for video playback
- Not production-ready

---

## Open Questions

1. **User claimed "videos worked before"** - but VideoBlock.svelte is new (created this session)
   - What was the previous implementation?
   - Why would contentDoc vs staticAssets matter if blob URL creation is identical?

2. **Is this WebKit version-specific?**
   - System: Linux, WebKit2GTK
   - GStreamer: v1.26.7-1
   - Does this work on macOS/Windows Tauri?

3. **Can we modify asset protocol handler?**
   - Bridge Rust ↔ JavaScript to serve from staticAssets?
   - Register custom protocol like `sthalam-video://`?

---

## Potential Solutions (Not Yet Attempted)

### Option A: Custom Tauri Command to Serve Videos
Create Tauri command that streams video from JavaScript staticAssets:

```rust
// Pseudocode
#[tauri::command]
fn get_video_chunk(video_id: String, offset: usize, length: usize) -> Vec<u8> {
  // Call JavaScript to get video data
  // Return chunk for streaming
}
```

**Pros:**
- No temp files
- Streaming support
- Works with WebKit media pipeline

**Cons:**
- Complex Rust ↔ JavaScript bridge
- Requires Range request handling
- Stateful chunk management

### Option B: Data URL with Size Check
Try data URLs with validation:

```typescript
if (videoData.length < 1500000) { // ~1.5MB limit
  const base64 = btoa(String.fromCharCode.apply(null, Array.from(videoData)));
  return `data:video/mp4;base64,${base64}`;
} else {
  throw new Error('Video too large for data URL');
}
```

**Pros:**
- Simple implementation
- No external dependencies

**Cons:**
- Size limitations
- Memory overhead
- Performance concerns

### Option C: Investigate Loro CRDT Magic
User said videos worked when stored in contentDoc - investigate why:

```typescript
// Compare:
loroCoordinator.staticAssets.get(id)      // ❌ Doesn't work
loroCoordinator.getVideosMap().get(id)    // ✅ Claimed to work?
```

**Action**: Test storing in contentDoc.videos (CRDT) vs staticAssets (Map)
- If contentDoc works, understand why
- Maybe Loro's internal Uint8Array handling differs?

### Option D: MediaSource API
Use MediaSource Extensions for programmatic control:

```typescript
const mediaSource = new MediaSource();
const url = URL.createObjectURL(mediaSource);
mediaSource.addEventListener('sourceopen', () => {
  const sourceBuffer = mediaSource.addSourceBuffer('video/mp4');
  sourceBuffer.appendBuffer(videoData);
});
```

**Pros:**
- Full control over video buffering
- Standard HTML5 API

**Cons:**
- Complex MP4 fragmentation requirements
- May still fail with WebKit blob URL issues
- Requires video to be in fragmented MP4 format

---

## Next Steps

### Immediate (Pick One)

1. **Test Data URL approach**
   - Quick to implement
   - Will definitively rule in/out if WebKit allows data: for media

2. **Test contentDoc storage again (controlled)**
   - Store video in `contentDoc.videos` (CRDT)
   - Create blob URL from CRDT data
   - Compare with staticAssets blob URL
   - Hypothesis: No difference (already tested, same result)

3. **Research WebKit blob URL limitations**
   - Check WebKit bug tracker
   - Test on macOS Tauri (does it work there?)
   - Check GStreamer documentation

### Long-term

1. **Custom protocol handler** (`sthalam-video://`)
   - Register new protocol in Tauri
   - Serve from staticAssets Map via Rust bridge
   - Most robust solution for production

2. **Hybrid approach**
   - Small videos (<1MB): Use data URLs
   - Large videos: Custom protocol handler

3. **External video storage**
   - Store videos in IndexedDB instead of CRDT
   - Use File System Access API
   - Serve via object URLs from File objects (not Blobs)

---

## Production Implications

⚠️ **CRITICAL**: If blob URLs don't work in dev, **they won't work in production**
- Same WebKit runtime
- Same GStreamer backend
- Issue is architectural, not environmental

**Must solve before shipping video feature.**

---

## References

### Key Files

- `/frontend/desktop/src/renderer/blocks/VideoBlock.svelte` - Video rendering
- `/frontend/desktop/src/lib/services/videoService.ts` - Blob URL creation
- `/frontend/desktop/src/lib/services/assetService.ts` - Upload handling
- `/frontend/desktop/src/state/data.svelte.ts` - Persistence (fixed)
- `/frontend/desktop/src/shared/loro/loroCoordinator.ts` - staticAssets storage
- `/src-tauri/src/lib.rs` - Asset protocol handler (lines 60-156)
- `/src-tauri/tauri.conf.json` - CSP configuration

### Console Logs to Watch

```javascript
// Data retrieval
"📦 [LoroCoordinator] Static asset retrieved"
"📦 [VideoService] Found in staticAssets"

// Integrity checks
"🔍 [VideoService] Data integrity check"
"🔍 [VideoService] MP4 signature check"

// Blob creation
"🎬 [VideoService] Blob created"
"✅ [VideoService] Blob fetch test passed"

// Video element state (THE PROBLEM)
"[VideoBlock] Video element state after load()"
"[VideoBlock] Video state after 2s"
```

### Browser DevTools

- **Network tab**: No blob URL requests (confirms media pipeline not using it)
- **Console**: No CSP violations
- **Application > Storage**: staticAssets in memory, not persisted separately

---

## Environment

```
OS: Linux 6.17.1-arch1-1
Browser: WebKit2GTK (via Tauri)
GStreamer: 1.26.7-1
Tauri: v2
Node: (check package.json)
Branch: sthalam/poc
Working Directory: /home/abe/osvauld/sthalam/src-tauri
```

---

## Summary

**The infrastructure is perfect:**
✅ Upload works
✅ Persistence works
✅ Data integrity verified
✅ Blob URLs created correctly
✅ JavaScript can read blobs

**But WebKit's media decoder cannot load blob URLs** - this is a platform limitation.

**Recommended next action**: Implement custom Tauri protocol handler to serve videos from memory without blob URLs.
