use super::search_types::{IndexError, IndexResult};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::Value;
use yrs::{updates::decoder::Decode, Doc as YDoc, GetString, Map, Transact};

pub struct ContentExtractor {
    yjs_field_name: String,
}

impl ContentExtractor {
    /// Create a new content extractor with the specified YJS field name
    /// For livnote, use "main_doc". For chat, use "chat"
    pub fn new(yjs_field_name: String) -> Self {
        Self { yjs_field_name }
    }

    /// Extract text and comments from note content JSON
    pub fn extract_content_from_note(
        &self,
        note_content: &Value,
    ) -> IndexResult<(String, String, Vec<String>)> {
        // Extract main_doc bytes from JSON
        let main_doc_array = note_content
            .get(&self.yjs_field_name)
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                IndexError::ParsingError(format!(
                    "{} field not found or not an array",
                    self.yjs_field_name
                ))
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
        let comments = self.extract_comments_from_comment_state(note_content)?;
        // Use JSON title if available, otherwise use extracted title
        let final_title = if json_title != "Untitled Note" {
            json_title
        } else {
            extracted_title
        };

        Ok((content, final_title, comments))
    }

    /// Extract comments from the separate comment_state document
    fn extract_comments_from_comment_state(
        &self,
        note_content: &Value,
    ) -> IndexResult<Vec<String>> {
        let mut comments = Vec::new();

        // Get comment_state bytes
        let comment_state_array = match note_content.get("comment_state").and_then(|v| v.as_array())
        {
            Some(arr) => arr,
            None => return Ok(comments), // No comments
        };

        if comment_state_array.is_empty() {
            return Ok(comments);
        }

        let comment_bytes: Vec<u8> = comment_state_array
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as u8))
            .collect();

        // Create separate YDoc for comments
        let comment_doc = YDoc::new();
        comment_doc
            .transact_mut()
            .apply_update(
                yrs::Update::decode_v2(&comment_bytes)
                    .map_err(|e| IndexError::ParsingError(e.to_string()))?,
            )
            .map_err(|e| {
                IndexError::ParsingError(format!("Failed to apply comment update: {}", e))
            })?;

        // Get the replyContents map
        let reply_contents = comment_doc.get_or_insert_map("replyContents");
        let txn = comment_doc.transact();

        // Extract text from each reply's XML content
        for (_reply_id, content_value) in reply_contents.iter(&txn) {
            if let yrs::Out::YXmlFragment(fragment) = content_value {
                // Get XML string from fragment
                let xml_string = fragment.get_string(&txn);

                // Reuse existing XML parser - just extract content, ignore title
                let (comment_text, _) = self.extract_text_from_prosemirror_xml(&xml_string)?;

                if !comment_text.is_empty() {
                    comments.push(comment_text);
                }
            }
        }

        Ok(comments)
    }
    /// Extract text from ProseMirror XML structure
    fn extract_text_from_prosemirror_xml(&self, xml: &str) -> IndexResult<(String, String)> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().check_end_names = false;
        reader.config_mut().check_comments = false;
        reader.config_mut().allow_dangling_amp = true;
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
