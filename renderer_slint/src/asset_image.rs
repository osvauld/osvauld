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

/// Response with decoded image (sent back to Slint main thread)
///
/// **Context**: Background task successfully decoded the image
pub struct ImageLoadResponse {
    /// Blake3 hash of the asset
    pub hash: String,
    /// Decoded Slint image ready for rendering
    pub image: Image,
    /// Model name to update
    pub model_name: String,
    /// Row index to update
    pub row_index: usize,
}

/// In-memory cache of decoded asset images
///
/// **Lifecycle**: Lives as long as the SlintRuntime (per-app window)
/// **Key**: Blake3 hash of the asset
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

    /// Check if a hash is already cached
    pub fn is_cached(&self, hash: &str) -> bool {
        self.images.contains_key(hash)
    }
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
