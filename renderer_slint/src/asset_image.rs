//! Asset Image Loading — decrypt, decode, and cache asset images for Slint rendering
//!
//! **Flow**:
//!   1. Lua sets `attachment_hash` on a message in a VecModel
//!   2. SlintRuntime detects hashes needing images via `scan_model_for_asset_hashes`
//!   3. Sends `ImageLoadRequest` to tokio task via channel
//!   4. Tokio task: butler.assets().get_bytes() → image crate decode → slint::Image
//!   5. Response sent back via `ImageLoadResponse` channel
//!   6. Timer loop applies the decoded image to the VecModel row
//!
//! **Caching**: Decoded images are cached by hash to avoid repeated decryption/decode

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;

/// Request to load an asset image (sent to tokio background task)
///
/// **Context**: SlintRuntime detected a message with an attachment_hash that isn't cached
#[derive(Debug, Clone)]
pub struct ImageLoadRequest {
    /// Blake3 hash of the asset
    pub hash: String,
    /// Page ID for decryption key lookup
    pub page_id: String,
    /// Model name containing the row (e.g., "messages")
    pub model_name: String,
    /// Index of the row in the VecModel to update after loading
    pub row_index: usize,
}

/// Response with decoded image data (sent back to Slint main thread)
///
/// **Context**: Background task successfully decoded the image.
///
/// **Why raw bytes instead of `slint::Image`**: `slint::Image` is `!Send` (it contains
/// `VRc<OpaqueImageVTable>` which wraps a raw pointer).  We carry the decoded RGBA8 pixels
/// across the thread boundary as a plain `Vec<u8>` and reconstruct the `slint::Image` on
/// the Slint main thread inside `apply_loaded_images`.
pub struct ImageLoadResponse {
    /// Blake3 hash of the asset
    pub hash: String,
    /// Raw RGBA8 pixel data (row-major, 4 bytes per pixel)
    pub rgba_bytes: Vec<u8>,
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Model name to update
    pub model_name: String,
    /// Row index to update
    pub row_index: usize,
}

/// In-memory cache of decoded asset images
///
/// **Lifecycle**: Lives as long as the SlintRuntime (per-app window)
/// **Key**: Blake3 hash of the asset
/// **Threading**: Only accessed on the Slint main thread — `slint::Image` is `!Send`.
pub struct ImageCache {
    /// Cached decoded images
    images: HashMap<String, Image>,
    /// Hashes currently being loaded (to avoid duplicate requests)
    loading: HashSet<String>,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            loading: HashSet::new(),
        }
    }

    /// Get a cached image by hash
    pub fn get(&self, hash: &str) -> Option<&Image> {
        self.images.get(hash)
    }

    /// Insert a decoded image into the cache
    pub fn insert(&mut self, hash: String, image: Image) {
        self.loading.remove(&hash);
        self.images.insert(hash, image);
    }

    /// Check if a hash is currently being loaded
    pub fn is_loading(&self, hash: &str) -> bool {
        self.loading.contains(hash)
    }

    /// Mark a hash as being loaded
    pub fn mark_loading(&mut self, hash: String) {
        self.loading.insert(hash);
    }

    /// Clear a hash from the loading set (used on terminal failure)
    pub fn clear_loading(&mut self, hash: &str) {
        self.loading.remove(hash);
    }

    /// Check if a hash is already cached
    pub fn is_cached(&self, hash: &str) -> bool {
        self.images.contains_key(hash)
    }
}

/// Reconstruct a `slint::Image` from raw RGBA8 pixel bytes on the main thread.
///
/// **Context**: Called in `apply_loaded_images` after receiving `ImageLoadResponse` from the
/// background task.  The background task cannot produce `slint::Image` directly because
/// `slint::Image` is `!Send`.
///
/// # Arguments
/// * `rgba_bytes` - Raw RGBA8 pixel data (row-major, 4 bytes per pixel)
/// * `width`      - Image width in pixels
/// * `height`     - Image height in pixels
pub fn image_from_rgba_bytes(rgba_bytes: &[u8], width: u32, height: u32) -> Image {
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba_bytes, width, height);
    Image::from_rgba8(buffer)
}

/// Decode raw image bytes (PNG, JPEG, GIF, WebP, BMP) into a Slint Image
///
/// **Context**: Called on tokio background thread after decryption
/// **Returns**: Slint Image ready for rendering, or error
pub fn decode_image_bytes(bytes: &[u8]) -> Result<Image, String> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess image format: {}", e))?;

    let dynamic_image = reader
        .decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    // Convert to RGBA8
    let rgba = dynamic_image.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba.as_raw(), width, height);

    Ok(Image::from_rgba8(buffer))
}

/// Decode raw image bytes with a max dimension constraint for thumbnails
///
/// **Context**: For inline chat previews, we don't need full-resolution images
/// **Constraint**: Resizes to fit within max_dimension while preserving aspect ratio
pub fn decode_image_thumbnail(bytes: &[u8], max_dimension: u32) -> Result<Image, String> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess image format: {}", e))?;

    let dynamic_image = reader
        .decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    // Resize if larger than max_dimension
    let resized = if dynamic_image.width() > max_dimension || dynamic_image.height() > max_dimension
    {
        dynamic_image.thumbnail(max_dimension, max_dimension)
    } else {
        dynamic_image
    };

    let rgba = resized.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba.as_raw(), width, height);

    Ok(Image::from_rgba8(buffer))
}

/// Decode raw image bytes into RGBA8 pixel data suitable for cross-thread transfer.
///
/// **Context**: Called on a background thread (tokio task).  Returns raw bytes + dimensions
/// rather than `slint::Image` because `slint::Image` is `!Send`.  The caller reconstructs
/// the `slint::Image` on the Slint main thread via `image_from_rgba_bytes`.
///
/// **Constraint**: Resizes to fit within `max_dimension` while preserving aspect ratio.
///
/// # Returns
/// `(rgba_bytes, width, height)` on success, or an error string.
pub fn decode_image_thumbnail_bytes(
    bytes: &[u8],
    max_dimension: u32,
) -> Result<(Vec<u8>, u32, u32), String> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess image format: {}", e))?;

    let dynamic_image = reader
        .decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    let resized = if dynamic_image.width() > max_dimension || dynamic_image.height() > max_dimension
    {
        dynamic_image.thumbnail(max_dimension, max_dimension)
    } else {
        dynamic_image
    };

    let rgba = resized.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let raw = rgba.into_raw();

    Ok((raw, width, height))
}

/// Check if a MIME type is a renderable image
pub fn is_image_mime(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "image/png" | "image/jpeg" | "image/jpg" | "image/gif" | "image/webp" | "image/bmp"
    )
}

/// Check if a filename has an image extension
pub fn is_image_filename(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".bmp")
}
