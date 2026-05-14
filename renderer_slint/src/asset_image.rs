//! Asset image loading: decrypt, decode, and cache asset images for Slint rendering.
//!
//! Decoded images are cached by hash to avoid repeated decryption/decode.

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;

/// Request to load an asset image (sent to tokio background task).
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

/// Response with decoded image data (sent back to Slint main thread).
///
/// Carries raw RGBA8 bytes (not `slint::Image`) because `slint::Image` is `!Send`;
/// reconstruction happens on the main thread in `apply_loaded_images`.
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

/// In-memory cache of decoded asset images, keyed by Blake3 hash.
///
/// Lives as long as the SlintRuntime; main-thread only since `slint::Image` is `!Send`.
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

/// Reconstruct a `slint::Image` from raw RGBA8 pixel bytes on the main thread
/// (the background task can't, since `slint::Image` is `!Send`).
pub fn image_from_rgba_bytes(rgba_bytes: &[u8], width: u32, height: u32) -> Image {
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba_bytes, width, height);
    Image::from_rgba8(buffer)
}

/// Decode raw image bytes (PNG, JPEG, GIF, WebP, BMP) into a Slint Image.
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

/// Decode image bytes, resizing to fit within `max_dimension` while preserving aspect ratio.
pub fn decode_image_thumbnail(bytes: &[u8], max_dimension: u32) -> Result<Image, String> {
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

    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba.as_raw(), width, height);

    Ok(Image::from_rgba8(buffer))
}

/// Decode image bytes into RGBA8 pixels for cross-thread transfer.
///
/// Returns raw bytes (not `slint::Image`, which is `!Send`); caller reconstructs
/// via `image_from_rgba_bytes` on the Slint main thread. Resizes to fit
/// `max_dimension` preserving aspect ratio.
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
