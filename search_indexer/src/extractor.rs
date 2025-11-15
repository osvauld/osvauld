use super::search_types::IndexResult;
use serde_json::Value;

pub struct ContentExtractor;

impl ContentExtractor {
    /// Create a new content extractor
    pub fn new() -> Self {
        Self
    }

    /// Extract text and comments from note content JSON
    /// STUBBED: YJS extraction removed, migrating to Loro
    pub fn extract_content_from_note(
        &self,
        _note_content: &Value,
    ) -> IndexResult<(String, String, Vec<String>)> {
        // TODO: Implement Loro-based content extraction
        Ok((String::new(), "Untitled Note".to_string(), Vec::new()))
    }
}

// Dead code removed - these methods are not used anymore:
// - extract_comments_from_comment_state
// - extract_text_from_prosemirror_xml

#[allow(dead_code)]
fn _legacy_xml_parser() {
    // Keeping imports for future Loro-based implementation
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use super::search_types::IndexError;

    /// Extract text from ProseMirror XML structure (legacy, not used)
    fn extract_text_from_prosemirror_xml(xml: &str) -> IndexResult<(String, String)> {
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
