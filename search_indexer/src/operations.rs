// search_index/operations.rs

use super::search_types::{
    IndexError, IndexResult, SearchResult, SearchResultType, SerializedDocument,
};
use log::{debug, info};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Value};
use tantivy::{Index, IndexReader, IndexWriter, TantivyDocument, Term, doc};

pub struct SearchIndexOperations {
    resource_id_field: Field,
    title_field: Field,
    content_field: Field,
    folder_id_field: Field,
    comments_field: Field,
}

impl SearchIndexOperations {
    pub fn new(
        resource_id_field: Field,
        title_field: Field,
        content_field: Field,
        folder_id_field: Field,
        comments_field: Field,
    ) -> Self {
        Self {
            resource_id_field,
            title_field,
            content_field,
            folder_id_field,
            comments_field,
        }
    }

    /// Index a document
    pub fn index_document(
        &self,
        writer: &IndexWriter,
        resource_id: &str,
        title: &str,
        content: &str,
        folder_id: &str,
        comments: &[String],
    ) -> IndexResult<()> {
        // Create Tantivy document using the doc! macro
        let doc = doc!(
            self.resource_id_field => resource_id,
            self.title_field => title,
            self.content_field => content,
            self.folder_id_field => folder_id,
            self.comments_field => comments.join("\n")
        );

        // Delete existing document if it exists (for updates)
        let resource_id_term = Term::from_field_text(self.resource_id_field, resource_id);
        writer.delete_term(resource_id_term.clone());

        // Add new document
        writer.add_document(doc)?;

        debug!("Indexed document: {} with title: {}", resource_id, title);
        Ok(())
    }

    /// Delete a document from the index
    pub fn delete_document(&self, writer: &IndexWriter, resource_id: &str) -> IndexResult<()> {
        let resource_id_term = Term::from_field_text(self.resource_id_field, resource_id);
        writer.delete_term(resource_id_term);

        info!("Deleted resource {} from search index", resource_id);
        Ok(())
    }

    /// Search for resources
    pub fn search(
        &self,
        index: &Index,
        reader: &IndexReader,
        query_str: &str,
        limit: usize,
    ) -> IndexResult<Vec<SearchResult>> {
        let searcher = reader.searcher();

        // Create query parser for content, title, and comments fields
        let query_parser = QueryParser::for_index(
            index,
            vec![self.title_field, self.content_field, self.comments_field],
        );

        // Parse query
        let query = query_parser
            .parse_query(query_str)
            .map_err(|e| IndexError::SearchError(e.to_string()))?;

        // Execute search
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| IndexError::SearchError(e.to_string()))?;

        // Collect results
        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;

            let resource_id = retrieved_doc
                .get_first(self.resource_id_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let title = retrieved_doc
                .get_first(self.title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();

            let content = retrieved_doc
                .get_first(self.content_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let comments = retrieved_doc
                .get_first(self.comments_field)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Determine result type based on where match was found
            let result_type = if !comments.is_empty()
                && query_str
                    .to_lowercase()
                    .split_whitespace()
                    .any(|word| comments.to_lowercase().contains(word))
            {
                SearchResultType::Comment
            } else {
                SearchResultType::Document
            };

            // Create snippet (first 200 chars of content or comment)
            let snippet = if result_type == SearchResultType::Comment && !comments.is_empty() {
                if comments.len() > 200 {
                    format!("{}...", &comments[..200])
                } else {
                    comments.to_string()
                }
            } else {
                if content.len() > 200 {
                    format!("{}...", &content[..200])
                } else {
                    content.clone()
                }
            };

            results.push(SearchResult {
                resource_id,
                title,
                snippet,
                score,
                result_type,
            });
        }

        Ok(results)
    }

    /// Extract all documents from the index (for saving)
    pub fn extract_all_documents(
        &self,
        reader: &IndexReader,
    ) -> IndexResult<Vec<SerializedDocument>> {
        let searcher = reader.searcher();
        let mut documents = Vec::new();

        // Use a match-all query to get all documents
        let all_query = tantivy::query::AllQuery;
        let top_docs = searcher
            .search(&all_query, &TopDocs::with_limit(10000))
            .map_err(|e| IndexError::SearchError(format!("Failed to search documents: {}", e)))?;

        for (_score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;

            let resource_id = doc
                .get_first(self.resource_id_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let title = doc
                .get_first(self.title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let content = doc
                .get_first(self.content_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let folder_id = doc
                .get_first(self.folder_id_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let comments_text = doc
                .get_first(self.comments_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let comments: Vec<String> = if !comments_text.is_empty() {
                comments_text.split('\n').map(|s| s.to_string()).collect()
            } else {
                vec![]
            };

            documents.push(SerializedDocument {
                resource_id,
                title,
                content,
                folder_id,
                comments,
            });
        }

        debug!("Extracted {} documents from index", documents.len());
        Ok(documents)
    }
}


