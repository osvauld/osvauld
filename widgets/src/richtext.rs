//! RichTextEdit widget state.
//!
//! Milestone 1 (current): no-op placeholder. The Slint surface in
//! `widgets/richtext.slint` carries a `string` property and the Lua app
//! binds it directly via `widgets.rich_text(id, { layer, field })` (binding
//! lives in `lua_runtime`).
//!
//! Milestone 2: this module owns a `LoroText` handle plus a parley
//! `PlainEditor`-like state machine. The Slint component becomes a
//! `ComponentContainer` host for a custom-rendered child driven from here.

use loro::LoroDoc;
use std::sync::Arc;

/// Per-widget state. Keyed by `widget-id` from the Slint side.
pub struct RichTextState {
    #[allow(dead_code)]
    doc: Arc<LoroDoc>,
    #[allow(dead_code)]
    field: String,
}

impl RichTextState {
    pub fn new(doc: Arc<LoroDoc>, field: impl Into<String>) -> Self {
        Self {
            doc,
            field: field.into(),
        }
    }
}
