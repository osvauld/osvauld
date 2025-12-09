//! Page handlers - Uses Butler facade and Scribe for live documents
//!
//! Handlers:
//! - handle_create_page: Create a new page in a space
//! - handle_open_page: Open page with Scribe subscription
//! - handle_close_page: Close page and unsubscribe from Scribe
//! - handle_apply_update: Apply CRDT update to page layer
//! - handle_list_pages: List all accessible pages

use crate::types::{
    ApplyUpdateInput, BaseCryptoResponse, CreatePageInput, OpenPageInput, ClosePageInput,
    PageMetadata, PageResponse,
};
use butler::{Butler, PageType};
use std::sync::Arc;
use tauri::State;
use tracing::{info, instrument};

/// Create a new page in a space
///
/// **Context**: User creates a new page (document)
/// **We call**: Butler.create_page with layer configuration
/// **We return**: Page metadata for UI display
#[tauri::command]
#[instrument(skip(input, butler), fields(space_id = %input.space_id, page_type = %input.page_type))]
pub async fn handle_create_page(
    input: CreatePageInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let metadata: serde_json::Value = serde_json::from_str(&input.metadata_json)
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;
    let title = metadata
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();

    let layer_names = Butler::extract_layer_names_from_template(&input.permit_template_json);

    let page_type = match input.page_type.as_str() {
        "content" => PageType::Content,
        "comments" => PageType::Comments,
        "submissions" => PageType::Submissions,
        _ => PageType::Content,
    };

    let page = butler
        .create_page(
            &input.space_id,
            &title,
            page_type,
            layer_names,
            &input.permit_template_json,
        )
        .await
        .map_err(|e| format!("Failed to create page: {}", e))?;

    info!(page_id = %page.id, "Page created");

    let response = PageMetadata {
        id: page.id.clone(),
        title,
        page_type: input.page_type,
        space_id: input.space_id,
        last_modified: page.created_at,
        favourite: false,
        preview: None,
    };

    Ok(BaseCryptoResponse::PageCreated(response))
}

/// Open a page and subscribe to Scribe updates
///
/// **Context**: User opens a page for editing
/// **We call**: Butler.open_page to get Scribe handle
/// **We subscribe**: Using user's DID and device ID
/// **We return**: Full page content and subscription confirmation
#[tauri::command]
#[instrument(skip(input, butler), fields(page_id = %input.page_id))]
pub async fn handle_open_page(
    input: OpenPageInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let (decrypted_page, _aes_key) = butler
        .get_decrypted_page(&input.page_id)
        .await
        .map_err(|e| format!("Failed to get page: {}", e))?;

    let data = decrypted_page.docs_to_json();

    // TODO: Subscribe to Scribe for live updates
    // This will be implemented in the subscription manager

    let response = PageResponse {
        id: decrypted_page.id,
        data,
        favourite: false,
        last_accessed: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
        space_id: decrypted_page.space_id,
    };

    Ok(BaseCryptoResponse::PageOpened(response))
}

/// Close a page and unsubscribe from Scribe
///
/// **Context**: User closes a page
/// **We call**: Scribe.Unsubscribe to stop receiving updates
#[tauri::command]
#[instrument(skip(input, butler), fields(page_id = %input.page_id))]
pub async fn handle_close_page(
    input: ClosePageInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    // TODO: Unsubscribe from Scribe
    // This will be implemented in the subscription manager
    let _ = butler; // Silence unused warning for now

    info!(page_id = %input.page_id, "Page closed");
    Ok(BaseCryptoResponse::Success)
}

/// Apply a CRDT update to a page layer
///
/// **Context**: User makes changes to page content
/// **We receive**: Layer name and binary update from frontend
/// **We call**: Scribe.ApplyUpdate with the update
/// **Scribe handles**: Merge, persistence, and broadcast to subscribers
#[tauri::command]
#[instrument(skip(input, butler), fields(page_id = %input.page_id, layer_name = %input.layer_name))]
pub async fn handle_apply_update(
    input: ApplyUpdateInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let scribe = butler
        .open_page(&input.page_id)
        .await
        .map_err(|e| format!("Failed to open page: {}", e))?;

    scribe
        .cast(butler::ScribeMessage::ApplyUpdate {
            layer_name: input.layer_name.clone(),
            update: input.update.clone(),
            from_peer: None, // Local update, not from peer
        })
        .map_err(|e| format!("Failed to apply update: {}", e))?;

    Ok(BaseCryptoResponse::Success)
}

/// List all pages with metadata
///
/// **Context**: User views their page list
/// **We call**: Butler.get_pages (no filtering - if stored, user has access)
/// **We return**: List of page metadata for UI display
#[tauri::command]
#[instrument(skip(butler))]
pub async fn handle_list_pages(
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let pages = butler
        .get_pages()
        .map_err(|e| format!("Failed to get pages: {}", e))?;

    let metadata_list: Vec<PageMetadata> = pages
        .into_iter()
        .map(|page| {
            let page_type = match page.page_type {
                PageType::Content => "content",
                PageType::Comments => "comments",
                PageType::Submissions => "submissions",
                PageType::PrivateChat => "private_chat",
            }
            .to_string();

            PageMetadata {
                id: page.id,
                title: page.name,
                page_type,
                space_id: page.space_id,
                last_modified: page.updated_at,
                favourite: false,
                preview: None,
            }
        })
        .collect();

    Ok(BaseCryptoResponse::PagesMetadata(metadata_list))
}
