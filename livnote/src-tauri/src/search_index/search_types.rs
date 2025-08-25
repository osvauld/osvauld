// search_index/types.rs

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub resource_id: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
    pub result_type: SearchResultType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SearchResultType {
    Document,
    Comment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSnapshot {
    pub documents: Vec<SerializedDocument>,
    pub version: u32,
    pub encrypted_key: String, // Store the encrypted AES key for the index
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedDocument {
    pub resource_id: String,
    pub title: String,
    pub content: String,
    pub folder_id: String,
    pub comments: Vec<String>,
}

#[derive(Error, Debug)]
pub enum IndexError {
    #[error("Index not initialized")]
    NotInitialized,

    #[error("Failed to create index: {0}")]
    CreationError(String),

    #[error("Failed to encrypt index: {0}")]
    EncryptionError(String),

    #[error("Failed to decrypt index: {0}")]
    DecryptionError(String),

    #[error("Failed to parse document: {0}")]
    ParsingError(String),

    #[error("Search failed: {0}")]
    SearchError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Tantivy error: {0}")]
    TantivyError(#[from] tantivy::TantivyError),
}

pub type IndexResult<T> = Result<T, IndexError>;
