// search_index/manager.rs

use crate::search_types::SerializedDocument;

use super::extractor::ContentExtractor;
use super::operations::SearchIndexOperations;
use super::search_types::{IndexError, IndexResult, IndexSnapshot, SearchResult};
use super::storage::SearchIndexStorage;

use crypto_utils::CryptoUtils;
use log::{error, info};
use persistance::database::RepositoryContext;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tantivy::schema::*;
use tantivy::tokenizer::{LowerCaser, SimpleTokenizer, Stemmer, TextAnalyzer};
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
    user_pub_key: Option<String>,
}

impl SearchIndexManager {
    /// Create a new search index manager
    ///
    /// # Arguments
    /// * `app_data_dir` - Directory to store the encrypted search index
    /// * `yjs_field_name` - Name of the YJS field in the JSON (e.g., "main_doc" for livnote, "chat" for chat)
    pub fn new(app_data_dir: &Path, yjs_field_name: String) -> IndexResult<Self> {
        // Define schema with stemming
        let mut schema_builder = Schema::builder();

        let resource_id_field = schema_builder.add_text_field("resource_id", STRING | STORED);
        let folder_id_field = schema_builder.add_text_field("folder_id", STRING | STORED);

        // Configure text fields with stemming support
        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("en_stem")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        let title_field = schema_builder.add_text_field("title", text_options.clone());
        let content_field = schema_builder.add_text_field("content", text_options.clone());
        let comments_field = schema_builder.add_text_field("comments", text_options);

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
        let extractor = ContentExtractor::new(yjs_field_name);

        Ok(Self {
            index: Arc::new(RwLock::new(None)),
            writer: Arc::new(RwLock::new(None)),
            reader: Arc::new(RwLock::new(None)),
            schema,
            storage,
            operations,
            extractor,
            user_pub_key: None,
        })
    }

    /// Initialize the index (decrypt from disk if exists, otherwise create new)
    pub async fn initialize(
        &mut self,
        crypto_utils: &Arc<RwLock<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
        user_pub_key: String,
    ) -> IndexResult<()> {
        info!("Initializing search index...");
        self.user_pub_key = Some(user_pub_key.clone());

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

        // Register the English stemming tokenizer
        let tokenizer = TextAnalyzer::builder(SimpleTokenizer::default())
            .filter(LowerCaser)
            .filter(Stemmer::default())
            .build();

        index.tokenizers().register("en_stem", tokenizer);

        // Create writer with 50MB heap
        let writer = index.writer(50_000_000)?;

        // Create reader - using Manual reload policy
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
    pub async fn save(&self, repo_ctx: &Arc<RepositoryContext>) -> IndexResult<()> {
        info!("Saving search index...");

        // Extract what we need under a short-lived lock
        let (documents, user_pub_key) = {
            let reader_guard = self.reader.read().await;
            let reader = reader_guard.as_ref().ok_or(IndexError::NotInitialized)?;

            // Extract all documents while holding the lock
            let documents = self.operations.extract_all_documents(reader)?;
            let user_pub_key = self
                .user_pub_key
                .clone()
                .ok_or(IndexError::NotInitialized)?;

            (documents, user_pub_key)
            // Lock is dropped here
        };

        // Create snapshot (no locks held during this)
        let snapshot = IndexSnapshot {
            documents,
            version: 1,
            encrypted_key: String::new(), // Will be set by storage
        };

        // Perform I/O operations without holding any locks
        self.storage
            .save_encrypted(snapshot, repo_ctx, user_pub_key)
            .await?;

        Ok(())
    }

    /// Extract documents and user key for saving (minimal lock time)
    pub async fn extract_for_save(&self) -> IndexResult<(Vec<SerializedDocument>, String)> {
        let reader_guard = self.reader.read().await;
        let reader = reader_guard.as_ref().ok_or(IndexError::NotInitialized)?;
        let documents = self.operations.extract_all_documents(reader)?;
        let user_pub_key = self
            .user_pub_key
            .clone()
            .ok_or(IndexError::NotInitialized)?;
        Ok((documents, user_pub_key))
    }

    /// Save extracted data to disk (no locks held during I/O)
    pub async fn save_extracted_data(
        &self,
        documents: Vec<SerializedDocument>,
        user_pub_key: String,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> IndexResult<()> {
        info!("Saving search index...");

        let snapshot = IndexSnapshot {
            documents,
            version: 1,
            encrypted_key: String::new(), // Will be set by storage
        };

        self.storage
            .save_encrypted(snapshot, repo_ctx, user_pub_key)
            .await?;
        Ok(())
    }

    pub fn start_scheduled_save(
        search_manager: Arc<Mutex<SearchIndexManager>>,
        repo_ctx: Arc<RepositoryContext>,
        save_interval_minutes: u64,
    ) {
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(save_interval_minutes * 60));

            // Skip the first tick (immediate execution)
            interval.tick().await;

            info!(
                "Started scheduled search index save task (interval: {} minutes)",
                save_interval_minutes
            );

            loop {
                interval.tick().await;

                // Extract everything we need under lock (short duration)
                let extract_result = {
                    let manager = search_manager.lock().await;
                    if !manager.is_initialized().await {
                        return; // Early return if not initialized
                    }

                    // Extract data and clone storage reference
                    match manager.extract_for_save().await {
                        Ok((documents, user_pub_key)) => {
                            // Clone the storage reference while we have the lock
                            let storage_clone = manager.storage.clone();
                            Ok((documents, user_pub_key, storage_clone))
                        }
                        Err(e) => Err(e),
                    }
                    // Lock is dropped here
                };

                match extract_result {
                    Ok((documents, user_pub_key, storage)) => {
                        // Perform I/O without holding any locks
                        let snapshot = IndexSnapshot {
                            documents,
                            version: 1,
                            encrypted_key: String::new(),
                        };

                        let save_result = storage
                            .save_encrypted(snapshot, &repo_ctx, user_pub_key)
                            .await;

                        match save_result {
                            Ok(_) => {
                                info!("Scheduled search index save completed successfully");
                            }
                            Err(e) => {
                                error!("Scheduled search index save failed: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to extract data for scheduled save: {}", e);
                    }
                }
            }
        });
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
pub fn create_search_index(
    app_data_dir: &Path,
    yjs_field_name: String,
) -> IndexResult<SearchIndexManager> {
    SearchIndexManager::new(app_data_dir, yjs_field_name)
}
