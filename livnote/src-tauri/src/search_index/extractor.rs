// search_index/extractor.rs

use super::search_types::{IndexError, IndexResult};
use log::debug;
use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::Value;
use yrs::{Doc as YDoc, GetString, Map, Transact, updates::decoder::Decode};

pub struct ContentExtractor;

impl ContentExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract text and comments from note content JSON
    pub fn extract_content_from_note(
        &self,
        note_content: &Value,
    ) -> IndexResult<(String, String, Vec<String>)> {
        // Extract main_doc bytes from JSON
        let main_doc_array = note_content
            .get("main_doc")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                IndexError::ParsingError("main_doc field not found or not an array".to_string())
            })?;

        // Convert JSON array to bytes
        let yjs_bytes: Vec<u8> = main_doc_array
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as u8))
            .collect();

        // Extract title from JSON if available
        let json_title = note_content
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled Note")
            .to_string();

        // Create temporary YDoc to apply updates
        let ydoc = YDoc::new();

        // Apply the update
        ydoc.transact_mut()
            .apply_update(
                yrs::Update::decode_v2(&yjs_bytes)
                    .map_err(|e| IndexError::ParsingError(e.to_string()))?,
            )
            .map_err(|e| IndexError::ParsingError(format!("Failed to apply update: {}", e)))?;

        // Get the XML fragment (ProseMirror content)
        let xml_fragment = ydoc.get_or_insert_xml_fragment("prosemirror");

        // Convert to XML string for parsing
        let xml_string = xml_fragment.get_string(&ydoc.transact());

        // Parse XML to extract text
        let (content, extracted_title) = self.extract_text_from_prosemirror_xml(&xml_string)?;

        // Extract comments from YJS document
        let comments = self.extract_comments_from_ydoc(&ydoc)?;

        // Use JSON title if available, otherwise use extracted title
        let final_title = if json_title != "Untitled Note" {
            json_title
        } else {
            extracted_title
        };

        Ok((content, final_title, comments))
    }

    /// Extract comments from YDoc
    fn extract_comments_from_ydoc(&self, ydoc: &YDoc) -> IndexResult<Vec<String>> {
        let mut comments = Vec::new();

        // Get comments map
        let comments_map = ydoc.get_or_insert_map("comments");
        let txn = ydoc.transact();

        // Iterate through comments
        for (_key, value) in comments_map.iter(&txn) {
            // Try to extract text from comment value
            if let yrs::Out::Any(any) = value {
                if let Ok(comment_str) = serde_json::to_string(&any) {
                    // Parse the comment JSON to extract text
                    if let Ok(comment_json) = serde_json::from_str::<Value>(&comment_str) {
                        // Extract text based on comment structure
                        if let Some(text) = comment_json.get("text").and_then(|v| v.as_str()) {
                            comments.push(text.to_string());
                        }
                        // Also check for replies if they exist
                        if let Some(replies) =
                            comment_json.get("replies").and_then(|v| v.as_array())
                        {
                            for reply in replies {
                                if let Some(reply_text) = reply.get("text").and_then(|v| v.as_str())
                                {
                                    comments.push(reply_text.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        debug!("Extracted {} comments from document", comments.len());
        Ok(comments)
    }

    /// Extract text from ProseMirror XML structure
    fn extract_text_from_prosemirror_xml(&self, xml: &str) -> IndexResult<(String, String)> {
        let mut reader = Reader::from_str(xml);
        // Note: trim_text might not be available in all versions
        // reader.trim_text(true);

        let mut content = Vec::new();
        let mut title = String::new();
        let mut is_first_paragraph = true;
        let mut current_text = String::new();
        let mut in_text_node = false;

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => match e.name().as_ref() {
                    b"paragraph" | b"heading" => {
                        in_text_node = true;
                        current_text.clear();
                    }
                    b"text" => {
                        in_text_node = true;
                    }
                    _ => {}
                },
                Ok(Event::Text(e)) => {
                    if in_text_node {
                        // Use escaped method instead of unescape if unescape is not available
                        let text = String::from_utf8_lossy(&e).to_string();
                        current_text.push_str(&text);
                    }
                }
                Ok(Event::End(ref e)) => {
                    match e.name().as_ref() {
                        b"paragraph" | b"heading" => {
                            if !current_text.is_empty() {
                                // Use first non-empty paragraph/heading as title
                                if is_first_paragraph && title.is_empty() {
                                    title = current_text.clone();
                                    is_first_paragraph = false;
                                }
                                content.push(current_text.clone());
                            }
                            in_text_node = false;
                            current_text.clear();
                        }
                        b"text" => {
                            in_text_node = false;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(IndexError::ParsingError(format!(
                        "Error parsing XML: {}",
                        e
                    )));
                }
                _ => {}
            }
            buf.clear();
        }

        // If no title was found, use default
        if title.is_empty() {
            title = "Untitled Note".to_string();
        }

        Ok((content.join(" "), title))
    }
}
