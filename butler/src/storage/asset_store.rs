use crate::error::{ButlerError, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Filesystem storage for encrypted static assets (images, PDFs, etc.)
///
/// - Assets stored as: {base_path}/{asset_id}.enc
/// - Encrypted at rest
/// - Load on-demand only (no in-memory caching)
/// - Stream decrypt when serving
pub struct AssetStore {
    base_path: PathBuf,
}

impl AssetStore {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();

        // Create directory if it doesn't exist
        fs::create_dir_all(&base_path)?;

        Ok(Self { base_path })
    }

    /// Get the path for an asset
    pub fn asset_path(&self, asset_id: &str) -> PathBuf {
        self.base_path.join(format!("{}.enc", asset_id))
    }

    /// Check if an asset exists
    pub fn exists(&self, asset_id: &str) -> bool {
        self.asset_path(asset_id).exists()
    }

    /// Store encrypted asset bytes
    ///
    /// The caller is responsible for encrypting the data before calling this.
    pub fn put(&self, asset_id: &str, encrypted_bytes: &[u8]) -> Result<()> {
        let path = self.asset_path(asset_id);
        fs::write(&path, encrypted_bytes)?;
        Ok(())
    }

    /// Get encrypted asset bytes
    ///
    /// Returns None if asset doesn't exist.
    /// The caller is responsible for decrypting the data.
    pub fn get(&self, asset_id: &str) -> Result<Option<Vec<u8>>> {
        let path = self.asset_path(asset_id);
        if path.exists() {
            let bytes = fs::read(&path)?;
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
    }

    /// Delete an asset
    pub fn delete(&self, asset_id: &str) -> Result<bool> {
        let path = self.asset_path(asset_id);
        if path.exists() {
            fs::remove_file(&path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List all asset IDs
    pub fn list(&self) -> Result<Vec<String>> {
        let mut asset_ids = Vec::new();

        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(name) = path.file_stem() {
                    if let Some(ext) = path.extension() {
                        if ext == "enc" {
                            if let Some(id) = name.to_str() {
                                asset_ids.push(id.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(asset_ids)
    }

    /// Get the size of an asset in bytes
    pub fn size(&self, asset_id: &str) -> Result<Option<u64>> {
        let path = self.asset_path(asset_id);
        if path.exists() {
            let metadata = fs::metadata(&path)?;
            Ok(Some(metadata.len()))
        } else {
            Ok(None)
        }
    }

    /// Get total size of all assets
    pub fn total_size(&self) -> Result<u64> {
        let mut total = 0u64;

        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "enc" {
                        let metadata = fs::metadata(&path)?;
                        total += metadata.len();
                    }
                }
            }
        }

        Ok(total)
    }

    /// Copy an asset to a new location (for export)
    pub fn copy_to<P: AsRef<Path>>(&self, asset_id: &str, dest: P) -> Result<bool> {
        let src = self.asset_path(asset_id);
        if src.exists() {
            fs::copy(&src, dest)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
