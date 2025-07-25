use log::info;
use osvauld_core::models::document::{YjsDocExt, create_doc};
use regex::Regex;
use std::collections::HashMap;
use yrs::{Any, Doc, GetString, Map, Out, ReadTxn, Transact, types::ToJson};

pub struct PreviewGenerator {
    block_element_regex: Regex,
    image_url_regex: Regex,
}

impl PreviewGenerator {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Match any block-level element (simpler approach)
        let block_element_regex = Regex::new(
            r"<(?:paragraph|heading|blockquote|code_block|bullet_list|ordered_list|image|hard_break)[^>]*(?:/>|>.*?</(?:paragraph|heading|blockquote|code_block|bullet_list|ordered_list)>)",
        )?;

        let image_url_regex = Regex::new(r#"src="yjs-image:([^"]+)""#)?;

        Ok(Self {
            block_element_regex,
            image_url_regex,
        })
    }

    /// Generate preview HTML from YJS states
    pub async fn generate_preview_html(
        &self,
        yjs_state: &[u8],
        image_state: &[u8],
        max_nodes: usize,
    ) -> Result<(String, String), Box<dyn std::error::Error>> {
        // Extract content from main YJS state
        let (content_html, title) = self.extract_content_from_yjs_state(yjs_state).await?;

        if content_html.is_empty() {
            return Ok((String::new(), title));
        }

        // Extract limited number of nodes
        let limited_html = self.extract_first_nodes(&content_html, max_nodes);

        // Process images if we have image state and images in content
        let processed_html = if !image_state.is_empty() && limited_html.contains("yjs-image:") {
            self.process_html_images(&limited_html, image_state).await?
        } else {
            limited_html
        };

        Ok((processed_html, title))
    }

    /// Extract content HTML from YJS state
    async fn extract_content_from_yjs_state(
        &self,
        yjs_state: &[u8],
    ) -> Result<(String, String), Box<dyn std::error::Error>> {
        if yjs_state.is_empty() {
            return Ok((String::new(), "Untitled Note".to_string()));
        }

        // Create a temporary YJS document
        let mut doc = create_doc();

        // Apply the YJS state
        doc.apply_update_v2(yjs_state)
            .await
            .map_err(|e| format!("Failed to apply YJS state: {}", e))?;

        // Get the prosemirror XML fragment
        let txn = doc.transact();
        let xml_fragment = txn.get_xml_fragment("prosemirror");

        match xml_fragment {
            Some(xml_fragment) => {
                let content_string = xml_fragment.get_string(&txn);
                let title = if let Some(metadata_map) = txn.get_map("metadata") {
                    if let Some(title_out) = metadata_map.get(&txn, "title") {
                        match title_out {
                            Out::Any(Any::String(s)) => s.to_string(), // Convert Arc<str> to String
                            Out::Any(Any::Null) => "Untitled Note".to_string(),
                            _ => "Untitled Note".to_string(), // fallback for other types
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

    /// Extract first N nodes from HTML string using regex
    fn extract_first_nodes(&self, html: &str, max_nodes: usize) -> String {
        if html.is_empty() || max_nodes == 0 {
            return String::new();
        }

        let mut matches = Vec::new();
        let mut count = 0;

        for capture in self.block_element_regex.find_iter(html) {
            if count >= max_nodes {
                break;
            }
            matches.push(capture.as_str());
            count += 1;
        }

        matches.join("")
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

        // Create image map from YJS image state
        let image_map = self.extract_image_map(image_state).await?;

        if image_map.is_empty() {
            return Ok(html.to_string());
        }

        // Replace yjs-image URLs in HTML using regex
        let result = self
            .image_url_regex
            .replace_all(html, |caps: &regex::Captures| {
                let image_id = &caps[1];

                if let Some(base64_data) = image_map.get(image_id) {
                    format!(r#"src="{}""#, base64_data)
                } else {
                    // Keep original if image not found
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

        // Create temporary image document
        let mut image_doc = create_doc();

        // Apply image state
        image_doc
            .apply_update_v2(image_state)
            .await
            .map_err(|e| format!("Failed to apply image state: {}", e))?;

        // Get the images map
        let txn = image_doc.transact();
        // Extract image data - iterate through the map
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
    let yjs_state = data
        .get("yjs_state")
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
        .generate_preview_html(&yjs_state, &image_state, max_nodes)
        .await
}
