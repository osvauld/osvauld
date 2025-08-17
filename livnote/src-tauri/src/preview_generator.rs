use log::info;
use osvauld_core::models::document::{YjsDocExt, create_doc};
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use regex::Regex;
use std::collections::HashMap;
use std::io::Cursor;
use yrs::{Any, Doc, GetString, Map, Out, ReadTxn, Transact, types::ToJson};

pub struct PreviewGenerator {
    image_url_regex: Regex,
}

impl PreviewGenerator {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let image_url_regex = Regex::new(r#"yjs-image:([^"'\s]+)"#)?;
        Ok(Self { image_url_regex })
    }

    /// Generate preview HTML from YJS states
    pub async fn generate_preview_html(
        &self,
        main_doc_state: &[u8],
        image_state: &[u8],
        max_nodes: usize,
    ) -> Result<(String, String), Box<dyn std::error::Error>> {
        // Extract content from main YJS state
        let (content_xml, title) = self.extract_content_from_yjs_state(main_doc_state).await?;

        if content_xml.is_empty() {
            return Ok((String::new(), title));
        }

        // Convert ProseMirror XML to HTML
        let html = self.convert_prosemirror_to_html(&content_xml, max_nodes)?;

        info!("generated html {}", html);
        // Process images if we have image state and images in content
        let processed_html = if !image_state.is_empty() && html.contains("yjs-image:") {
            self.process_html_images(&html, image_state).await?
        } else {
            html
        };

        Ok((processed_html, title))
    }

    /// Convert ProseMirror XML to standard HTML
    fn convert_prosemirror_to_html(
        &self,
        xml_content: &str,
        max_nodes: usize,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut reader = Reader::from_str(xml_content);
        // Remove the trim_text call - handle whitespace differently if needed

        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let mut buf = Vec::new();
        let mut block_count = 0;
        let mut depth = 0;
        let mut skip_depth = None;
        let mut in_code_block = false;
        let mut element_stack: Vec<String> = Vec::new(); // Track open elements for proper closing

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    // If we're skipping, increase depth counter
                    if skip_depth.is_some() {
                        depth += 1;
                        buf.clear();
                        continue;
                    }

                    // Check if this is a block-level element at root level
                    if depth == 0 && self.is_block_element(e.name().as_ref()) {
                        if block_count >= max_nodes {
                            skip_depth = Some(depth);
                            buf.clear();
                            continue;
                        }
                        block_count += 1;
                    }

                    // Convert ProseMirror element to HTML
                    let html_tag = match e.name().as_ref() {
                        b"paragraph" => {
                            writer.write_event(Event::Start(BytesStart::new("p")))?;
                            "p".to_string()
                        }
                        b"heading" => {
                            let level = self.get_attribute(e, "level").unwrap_or("1".to_string());
                            let tag_name = format!("h{}", level);
                            writer.write_event(Event::Start(BytesStart::new(tag_name.clone())))?;
                            tag_name
                        }
                        b"blockquote" => {
                            writer.write_event(Event::Start(BytesStart::new("blockquote")))?;
                            "blockquote".to_string()
                        }
                        b"code_block" => {
                            in_code_block = true;
                            writer.write_event(Event::Start(BytesStart::new("pre")))?;
                            writer.write_event(Event::Start(BytesStart::new("code")))?;
                            "code_block".to_string()
                        }
                        b"code" => {
                            if !in_code_block {
                                writer.write_event(Event::Start(BytesStart::new("code")))?;
                            }
                            "code".to_string()
                        }
                        b"bullet_list" => {
                            writer.write_event(Event::Start(BytesStart::new("ul")))?;
                            "ul".to_string()
                        }
                        b"ordered_list" => {
                            writer.write_event(Event::Start(BytesStart::new("ol")))?;
                            "ol".to_string()
                        }
                        b"list_item" => {
                            writer.write_event(Event::Start(BytesStart::new("li")))?;
                            "li".to_string()
                        }
                        b"table" => {
                            writer.write_event(Event::Start(BytesStart::new("table")))?;
                            "table".to_string()
                        }
                        b"table_row" => {
                            writer.write_event(Event::Start(BytesStart::new("tr")))?;
                            "tr".to_string()
                        }
                        b"table_header" => {
                            writer.write_event(Event::Start(BytesStart::new("th")))?;
                            "th".to_string()
                        }
                        b"table_cell" => {
                            writer.write_event(Event::Start(BytesStart::new("td")))?;
                            "td".to_string()
                        }
                        b"image" => {
                            let mut img_tag = BytesStart::new("img");

                            // Copy relevant attributes
                            for attr in e.attributes() {
                                if let Ok(attr) = attr {
                                    match attr.key.as_ref() {
                                        b"src" => {
                                            img_tag.push_attribute((
                                                "src",
                                                std::str::from_utf8(&attr.value)?,
                                            ));
                                        }
                                        b"alt" | b"title" => {
                                            img_tag.push_attribute((
                                                std::str::from_utf8(attr.key.as_ref())?,
                                                std::str::from_utf8(&attr.value)?,
                                            ));
                                        }
                                        b"width" | b"height" => {
                                            img_tag.push_attribute((
                                                std::str::from_utf8(attr.key.as_ref())?,
                                                std::str::from_utf8(&attr.value)?,
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            writer.write_event(Event::Empty(img_tag))?;
                            "".to_string() // Empty elements don't need closing
                        }
                        b"hard_break" => {
                            writer.write_event(Event::Empty(BytesStart::new("br")))?;
                            "".to_string() // Empty elements don't need closing
                        }
                        b"strong" | b"bold" => {
                            writer.write_event(Event::Start(BytesStart::new("strong")))?;
                            "strong".to_string()
                        }
                        b"em" | b"italic" => {
                            writer.write_event(Event::Start(BytesStart::new("em")))?;
                            "em".to_string()
                        }
                        _ => {
                            // Unknown elements - skip silently
                            "".to_string()
                        }
                    };

                    if !html_tag.is_empty() {
                        element_stack.push(html_tag);
                    }

                    depth += 1;
                }
                Ok(Event::End(ref e)) => {
                    depth -= 1;

                    // If we were skipping and we're back to the skip depth, stop skipping
                    if let Some(skip_d) = skip_depth {
                        if depth == skip_d {
                            skip_depth = None;
                        }
                        buf.clear();
                        continue;
                    }

                    // Pop the corresponding opening tag from stack
                    let closing_tag = if !element_stack.is_empty()
                        && matches!(
                            e.name().as_ref(),
                            b"paragraph"
                                | b"heading"
                                | b"blockquote"
                                | b"code_block"
                                | b"code"
                                | b"bullet_list"
                                | b"ordered_list"
                                | b"list_item"
                                | b"table"
                                | b"table_row"
                                | b"table_header"
                                | b"table_cell"
                                | b"strong"
                                | b"bold"
                                | b"em"
                                | b"italic"
                        ) {
                        element_stack.pop()
                    } else {
                        None
                    };

                    // Convert ProseMirror closing tags to HTML
                    match e.name().as_ref() {
                        b"paragraph" => {
                            writer.write_event(Event::End(BytesStart::new("p").to_end()))?;
                        }
                        b"heading" => {
                            if let Some(tag) = closing_tag {
                                writer.write_event(Event::End(BytesStart::new(tag).to_end()))?;
                            }
                        }
                        b"blockquote" => {
                            writer
                                .write_event(Event::End(BytesStart::new("blockquote").to_end()))?;
                        }
                        b"code_block" => {
                            writer.write_event(Event::End(BytesStart::new("code").to_end()))?;
                            writer.write_event(Event::End(BytesStart::new("pre").to_end()))?;
                            in_code_block = false;
                        }
                        b"code" => {
                            if !in_code_block {
                                writer.write_event(Event::End(BytesStart::new("code").to_end()))?;
                            }
                        }
                        b"bullet_list" => {
                            writer.write_event(Event::End(BytesStart::new("ul").to_end()))?;
                        }
                        b"ordered_list" => {
                            writer.write_event(Event::End(BytesStart::new("ol").to_end()))?;
                        }
                        b"list_item" => {
                            writer.write_event(Event::End(BytesStart::new("li").to_end()))?;
                        }
                        b"table" => {
                            writer.write_event(Event::End(BytesStart::new("table").to_end()))?;
                        }
                        b"table_row" => {
                            writer.write_event(Event::End(BytesStart::new("tr").to_end()))?;
                        }
                        b"table_header" => {
                            writer.write_event(Event::End(BytesStart::new("th").to_end()))?;
                        }
                        b"table_cell" => {
                            writer.write_event(Event::End(BytesStart::new("td").to_end()))?;
                        }
                        b"strong" | b"bold" => {
                            writer.write_event(Event::End(BytesStart::new("strong").to_end()))?;
                        }
                        b"em" | b"italic" => {
                            writer.write_event(Event::End(BytesStart::new("em").to_end()))?;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if skip_depth.is_none() {
                        writer.write_event(Event::Text(e.clone()))?;
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    if skip_depth.is_none() {
                        // Handle self-closing elements
                        match e.name().as_ref() {
                            b"hard_break" => {
                                writer.write_event(Event::Empty(BytesStart::new("br")))?;
                            }
                            b"image" => {
                                let mut img_tag = BytesStart::new("img");

                                for attr in e.attributes() {
                                    if let Ok(attr) = attr {
                                        match attr.key.as_ref() {
                                            b"src" | b"alt" | b"title" | b"width" | b"height" => {
                                                img_tag.push_attribute((
                                                    std::str::from_utf8(attr.key.as_ref())?,
                                                    std::str::from_utf8(&attr.value)?,
                                                ));
                                            }
                                            _ => {}
                                        }
                                    }
                                }

                                writer.write_event(Event::Empty(img_tag))?;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(Box::new(e)),
                _ => {}
            }
            buf.clear();
        }

        let result = writer.into_inner().into_inner();
        Ok(String::from_utf8(result)?)
    }

    /// Check if an element is a block-level element
    fn is_block_element(&self, name: &[u8]) -> bool {
        matches!(
            name,
            b"paragraph"
                | b"heading"
                | b"blockquote"
                | b"code_block"
                | b"bullet_list"
                | b"ordered_list"
                | b"table"
                | b"image"
                | b"hard_break"
        )
    }

    /// Get attribute value from XML element
    fn get_attribute(&self, element: &BytesStart, name: &str) -> Option<String> {
        for attr in element.attributes() {
            if let Ok(attr) = attr {
                if attr.key.as_ref() == name.as_bytes() {
                    return std::str::from_utf8(&attr.value).ok().map(|s| s.to_string());
                }
            }
        }
        None
    }

    /// Extract content HTML from YJS state
    async fn extract_content_from_yjs_state(
        &self,
        yjs_state: &[u8],
    ) -> Result<(String, String), Box<dyn std::error::Error>> {
        if yjs_state.is_empty() {
            return Ok((String::new(), "Untitled Note".to_string()));
        }

        let mut doc = create_doc();
        doc.apply_update_v2(yjs_state)
            .await
            .map_err(|e| format!("Failed to apply YJS state: {}", e))?;

        let txn = doc.transact();
        let xml_fragment = txn.get_xml_fragment("prosemirror");

        match xml_fragment {
            Some(xml_fragment) => {
                let content_string = xml_fragment.get_string(&txn);
                let title = if let Some(metadata_map) = txn.get_map("metadata") {
                    if let Some(title_out) = metadata_map.get(&txn, "title") {
                        match title_out {
                            Out::Any(Any::String(s)) => s.to_string(),
                            Out::Any(Any::Null) => "Untitled Note".to_string(),
                            _ => "Untitled Note".to_string(),
                        }
                    } else {
                        "Untitled Note".to_string()
                    }
                } else {
                    "Untitled Note".to_string()
                };

                Ok((content_string, title))
            }
            None => Ok((String::new(), "Untitled Note".to_string())),
        }
    }

    /// Process HTML to replace yjs-image URLs with actual image data
    async fn process_html_images(
        &self,
        html: &str,
        image_state: &[u8],
    ) -> Result<String, Box<dyn std::error::Error>> {
        if html.is_empty() || image_state.is_empty() {
            return Ok(html.to_string());
        }

        let image_map = self.extract_image_map(image_state).await?;

        if image_map.is_empty() {
            return Ok(html.to_string());
        }

        // Replace yjs-image URLs in HTML
        let result = self
            .image_url_regex
            .replace_all(html, |caps: &regex::Captures| {
                let image_id = &caps[1];

                if let Some(base64_data) = image_map.get(image_id) {
                    base64_data.clone()
                } else {
                    caps.get(0).unwrap().as_str().to_string()
                }
            });

        Ok(result.to_string())
    }

    /// Extract image map from YJS image state
    async fn extract_image_map(
        &self,
        image_state: &[u8],
    ) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        let mut image_map = HashMap::new();

        if image_state.is_empty() {
            return Ok(image_map);
        }

        let mut image_doc = create_doc();
        image_doc
            .apply_update_v2(image_state)
            .await
            .map_err(|e| format!("Failed to apply image state: {}", e))?;

        let txn = image_doc.transact();
        if let Some(images_ymap) = txn.get_map("images") {
            for (key, value) in images_ymap.iter(&txn) {
                let asset_any = value.to_json(&txn);
                if let Any::Map(asset_map) = asset_any {
                    if let Some(Any::String(data)) = asset_map.get("data") {
                        image_map.insert(key.to_string(), data.to_string());
                    }
                }
            }
        }

        Ok(image_map)
    }
}

/// Convenience function for generating preview HTML
pub async fn generate_preview_html(
    data: &serde_json::Value,
    max_nodes: usize,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let main_doc = data
        .get("main_doc")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| v.as_u64().unwrap_or(0) as u8)
                .collect::<Vec<u8>>()
        })
        .unwrap_or_default();

    let image_state = data
        .get("image_state")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| v.as_u64().unwrap_or(0) as u8)
                .collect::<Vec<u8>>()
        })
        .unwrap_or_default();

    let generator = PreviewGenerator::new()?;
    generator
        .generate_preview_html(&main_doc, &image_state, max_nodes)
        .await
}
