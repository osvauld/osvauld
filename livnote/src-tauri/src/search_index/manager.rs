// search_index/manager.rs

use super::extractor::ContentExtractor;
use super::operations::SearchIndexOperations;
use super::search_types::{IndexError, IndexResult, IndexSnapshot, SearchResult};
use super::storage::SearchIndexStorage;

use crypto_utils::CryptoUtils;
use log::{debug, error, info};
use persistance::database::RepositoryContext;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};
use tokio::sync::{Mutex, RwLock};

pub struct SearchIndexManager {
    index: Arc<RwLock<Option<Index>>>,
    writer: Arc<RwLock<Option<IndexWriter>>>,
    reader: Arc<RwLock<Option<IndexReader>>>,
    schema: Schema,
    storage: SearchIndexStorage,
    operations: SearchIndexOperations,
    extractor: ContentExtractor,
}

impl SearchIndexManager {
    /// Create a new search index manager
    pub fn new(app_data_dir: &Path) -> IndexResult<Self> {
        // Define schema
        let mut schema_builder = Schema::builder();

        let resource_id_field = schema_builder.add_text_field("resource_id", STRING | STORED);
        let title_field = schema_builder.add_text_field("title", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT | STORED);
        let folder_id_field = schema_builder.add_text_field("folder_id", STRING | STORED);
        let comments_field = schema_builder.add_text_field("comments", TEXT | STORED);

        let schema = schema_builder.build();

        // Create components
        let storage = SearchIndexStorage::new(app_data_dir);
        let operations = SearchIndexOperations::new(
            resource_id_field,
            title_field,
            content_field,
            folder_id_field,
            comments_field,
        );
        let extractor = ContentExtractor::new();

        Ok(Self {
            index: Arc::new(RwLock::new(None)),
            writer: Arc::new(RwLock::new(None)),
            reader: Arc::new(RwLock::new(None)),
            schema,
            storage,
            operations,
            extractor,
        })
    }

    /// Initialize the index (decrypt from disk if exists, otherwise create new)
    pub async fn initialize(
        &self,
        crypto_utils: &Arc<Mutex<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> IndexResult<()> {
        info!("Initializing search index...");

        // Check if encrypted index exists and load it
        if let Some(snapshot) = self.storage.load_encrypted(crypto_utils, repo_ctx).await? {
            info!("Restoring index from encrypted snapshot");
            self.restore_from_snapshot(snapshot).await?;
        } else {
            info!("No existing index found, creating new in-memory index");
            self.create_new_index().await?;
        }

        Ok(())
    }

    /// Create a new in-memory index
    async fn create_new_index(&self) -> IndexResult<()> {
        let index = Index::create_in_ram(self.schema.clone());

        // Create writer with 50MB heap
        let writer = index.writer(50_000_000)?;

        // Create reader - using Manual reload policy instead of OnCommit
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;

        // Store references
        *self.index.write().await = Some(index);
        *self.writer.write().await = Some(writer);
        *self.reader.write().await = Some(reader);

        Ok(())
    }

    /// Restore index from a snapshot
    async fn restore_from_snapshot(&self, snapshot: IndexSnapshot) -> IndexResult<()> {
        // Create new in-memory index
        self.create_new_index().await?;

        // Get writer
        let mut writer_guard = self.writer.write().await;
        let writer = writer_guard.as_mut().ok_or(IndexError::NotInitialized)?;

        // Rebuild index from snapshot
        for doc in snapshot.documents {
            self.operations.index_document(
                writer,
                &doc.resource_id,
                &doc.title,
                &doc.content,
                &doc.folder_id,
                &doc.comments,
            )?;
        }

        // Commit changes
        writer.commit()?;

        // Reload reader
        let reader_guard = self.reader.read().await;
        if let Some(reader) = reader_guard.as_ref() {
            reader.reload()?;
        }

        Ok(())
    }

    /// Save the current index to disk (encrypted)
    pub async fn save(
        &self,
        crypto_utils: &Arc<Mutex<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> IndexResult<()> {
        info!("Saving search index...");

        let reader_guard = self.reader.read().await;
        let reader = reader_guard.as_ref().ok_or(IndexError::NotInitialized)?;

        // Extract all documents
        let documents = self.operations.extract_all_documents(reader)?;

        // Create snapshot
        let snapshot = IndexSnapshot {
            documents,
            version: 1,
            encrypted_key: String::new(), // Will be set by storage
        };

        // Save encrypted
        self.storage
            .save_encrypted(snapshot, crypto_utils, repo_ctx)
            .await?;

        Ok(())
    }

    /// Index a resource from JSON value
    pub async fn index_resource(
        &self,
        resource_id: &str,
        note_content: &Value,
        folder_id: &str,
    ) -> IndexResult<()> {
        // Extract content from note
        let (content, title, comments) = self.extractor.extract_content_from_note(note_content)?;

        info!(
            "Indexing resource {} with title: {} and {} comments",
            resource_id,
            title,
            comments.len()
        );

        // Get writer
        let mut writer_guard = self.writer.write().await;
        let writer = writer_guard.as_mut().ok_or(IndexError::NotInitialized)?;

        // Index the document
        self.operations.index_document(
            writer,
            resource_id,
            &title,
            &content,
            folder_id,
            &comments,
        )?;

        // Commit changes
        writer.commit()?;

        // Reload reader
        let reader_guard = self.reader.read().await;
        if let Some(reader) = reader_guard.as_ref() {
            reader.reload()?;
        }

        Ok(())
    }

    /// Update a resource in the index
    pub async fn update_resource(
        &self,
        resource_id: &str,
        note_content: &Value,
        folder_id: &str,
    ) -> IndexResult<()> {
        // Same as index_resource since we delete and re-add
        self.index_resource(resource_id, note_content, folder_id)
            .await
    }

    /// Delete a resource from the index
    pub async fn delete_resource(&self, resource_id: &str) -> IndexResult<()> {
        let mut writer_guard = self.writer.write().await;
        let writer = writer_guard.as_mut().ok_or(IndexError::NotInitialized)?;

        self.operations.delete_document(writer, resource_id)?;
        writer.commit()?;

        // Reload reader
        let reader_guard = self.reader.read().await;
        if let Some(reader) = reader_guard.as_ref() {
            reader.reload()?;
        }

        Ok(())
    }

    /// Search for resources
    pub async fn search(&self, query_str: &str, limit: usize) -> IndexResult<Vec<SearchResult>> {
        let index_guard = self.index.read().await;
        let index = index_guard.as_ref().ok_or(IndexError::NotInitialized)?;

        let reader_guard = self.reader.read().await;
        let reader = reader_guard.as_ref().ok_or(IndexError::NotInitialized)?;

        self.operations.search(index, reader, query_str, limit)
    }

    /// Clear the index (useful for logout)
    pub async fn clear(&self) -> IndexResult<()> {
        *self.index.write().await = None;
        *self.writer.write().await = None;
        *self.reader.write().await = None;

        info!("Search index cleared from memory");
        Ok(())
    }

    /// Delete the encrypted index file
    pub async fn delete_encrypted(&self) -> IndexResult<()> {
        self.storage.delete().await
    }

    /// Check if index is initialized
    pub async fn is_initialized(&self) -> bool {
        self.index.read().await.is_some()
    }
}

/// Helper function to create a search index manager
pub fn create_search_index(app_data_dir: &Path) -> IndexResult<SearchIndexManager> {
    SearchIndexManager::new(app_data_dir)
}
