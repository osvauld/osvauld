use osvauld_core::models::document::{YjsDocExt, create_doc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use yrs::{Any, Map, Out, ReadTxn, Transact, types::ToJson, StateVector, Doc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    pub content: String,
    pub timestamp: i64,
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_by: Option<Vec<String>>, // User IDs who have read this message
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatPreviewData {
    pub last_message: String,
    pub last_message_time: i64,
    pub participants: Vec<String>, // All participant names
    pub participant_ids: Vec<String>, // All participant user IDs
    pub unread_count: i32,
}

pub struct ChatPreviewGenerator;

impl ChatPreviewGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Generate chat preview from YJS chat state
    pub async fn generate_chat_preview(
        &self,
        chat_state: &[u8],
        current_user_id: &str,
    ) -> Result<ChatPreviewData, Box<dyn std::error::Error>> {
        if chat_state.is_empty() {
            return Ok(ChatPreviewData {
                last_message: String::new(),
                last_message_time: 0,
                participants: Vec::new(),
                participant_ids: Vec::new(),
                unread_count: 0,
            });
        }

        // Load YJS document
        let mut chat_doc = create_doc();
        chat_doc
            .apply_update_v2(chat_state)
            .await
            .map_err(|e| format!("Failed to apply chat state: {}", e))?;

        let txn = chat_doc.transact();
        
        // Extract messages from Y.Map
        let messages = self.extract_messages(&txn)?;
        
        if messages.is_empty() {
            return Ok(ChatPreviewData {
                last_message: String::new(),
                last_message_time: 0,
                participants: Vec::new(),
                participant_ids: Vec::new(),
                unread_count: 0,
            });
        }

        // Get the last message
        let last_message = messages
            .iter()
            .max_by_key(|m| m.timestamp)
            .ok_or("No messages found")?;

        // Collect unique participants (excluding current user)
        let mut participant_names = HashSet::new();
        let mut participant_ids = HashSet::new();
        
        for message in &messages {
            if message.author_id != current_user_id {
                participant_names.insert(message.author_name.clone());
                participant_ids.insert(message.author_id.clone());
            }
        }

        // Calculate unread messages (messages not read by current user)
        let unread_count = messages
            .iter()
            .filter(|m| {
                // Message is unread if:
                // 1. It's not from the current user
                // 2. Current user is not in the read_by list
                m.author_id != current_user_id && 
                !m.read_by.as_ref()
                    .map(|readers| readers.contains(&current_user_id.to_string()))
                    .unwrap_or(false)
            })
            .count() as i32;

        Ok(ChatPreviewData {
            last_message: last_message.content.clone(),
            last_message_time: last_message.timestamp,
            participants: participant_names.into_iter().collect(),
            participant_ids: participant_ids.into_iter().collect(),
            unread_count,
        })
    }

    /// Extract messages from YJS Y.Map structure
    fn extract_messages(
        &self,
        txn: &yrs::Transaction,
    ) -> Result<Vec<ChatMessage>, Box<dyn std::error::Error>> {
        let mut messages = Vec::new();

        // Get the 'messages' Y.Map
        if let Some(messages_map) = txn.get_map("messages") {
            for (key, value) in messages_map.iter(txn) {
                let message_any = value.to_json(txn);
                
                if let Any::Map(message_map) = message_any {
                    let message = self.parse_message(&key, &message_map)?;
                    messages.push(message);
                }
            }
        }

        // Sort by timestamp
        messages.sort_by_key(|m| m.timestamp);

        Ok(messages)
    }

    /// Parse a message from the Any::Map format
    fn parse_message(
        &self,
        id: &str,
        message_map: &std::sync::Arc<HashMap<String, Any>>,
    ) -> Result<ChatMessage, Box<dyn std::error::Error>> {
        let author_id = match message_map.get("authorId") {
            Some(Any::String(s)) => s.to_string(),
            _ => return Err("Missing authorId".into()),
        };

        let author_name = match message_map.get("authorName") {
            Some(Any::String(s)) => s.to_string(),
            _ => return Err("Missing authorName".into()),
        };

        let content = match message_map.get("content") {
            Some(Any::String(s)) => s.to_string(),
            _ => return Err("Missing content".into()),
        };

        let timestamp = match message_map.get("timestamp") {
            Some(Any::Number(n)) => *n as i64,
            Some(Any::BigInt(n)) => *n,
            _ => return Err("Missing timestamp".into()),
        };

        let message_type = match message_map.get("type") {
            Some(Any::String(s)) => s.to_string(),
            _ => "text".to_string(),
        };

        let image_id = match message_map.get("imageId") {
            Some(Any::String(s)) => Some(s.to_string()),
            _ => None,
        };

        let read_by = match message_map.get("readBy") {
            Some(Any::Array(arr)) => {
                let readers: Vec<String> = arr
                    .iter()
                    .filter_map(|v| match v {
                        Any::String(s) => Some(s.to_string()),
                        _ => None,
                    })
                    .collect();
                Some(readers)
            }
            _ => None,
        };

        Ok(ChatMessage {
            id: id.to_string(),
            author_id,
            author_name,
            content,
            timestamp,
            message_type,
            image_id,
            read_by,
        })
    }

    /// Mark message as read by a user (updates the YJS document)
    pub async fn mark_message_read(
        &self,
        chat_state: &[u8],
        message_id: &str,
        user_id: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut chat_doc = create_doc();
        
        if !chat_state.is_empty() {
            chat_doc
                .apply_update_v2(chat_state)
                .await
                .map_err(|e| format!("Failed to apply chat state: {}", e))?;
        }

        let messages_map = chat_doc.get_or_insert_map("messages");
        let mut txn = chat_doc.transact_mut();

        if let Some(message_val) = messages_map.get(&txn, message_id) {
            if let Out::Any(Any::Map(message_map)) = message_val {
                // Get existing read_by list
                let mut read_by: Vec<String> = match message_map.get("readBy") {
                    Some(Any::Array(arr)) => arr
                        .iter()
                        .filter_map(|v| match v {
                            Any::String(s) => Some(s.to_string()),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                };

                // Add user if not already in list
                if !read_by.contains(&user_id.to_string()) {
                    read_by.push(user_id.to_string());
                    let mut updated_map = std::sync::Arc::try_unwrap(message_map)
                        .unwrap_or_else(|arc| (*arc).clone());
                    updated_map.insert(
                        "readBy".to_string(),
                        Any::Array(
                            read_by.into_iter().map(|s| Any::String(s.into())).collect()
                        )
                    );
                    
                    // Update the message in the map
                    messages_map.insert(&mut txn, message_id, Any::Map(std::sync::Arc::new(updated_map)));
                }
            }
        }

        drop(txn);

        // Return the updated state using V2 encoding
        let txn_read = chat_doc.transact();
        let state_vector = StateVector::default();
        let state_update = txn_read.encode_state_as_update_v2(&state_vector);
        Ok(state_update.to_vec())
    }
}

/// Convenience function for generating chat preview
pub async fn generate_chat_preview(
    data: &serde_json::Value,
    current_user_id: &str,
) -> Result<ChatPreviewData, Box<dyn std::error::Error>> {
    let generator = ChatPreviewGenerator::new();

    // Extract chat state from data
    let chat_state = match data.get("chat") {
        Some(serde_json::Value::Array(arr)) => {
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            bytes
        }
        _ => Vec::new(),
    };

    generator.generate_chat_preview(&chat_state, current_user_id).await
}
