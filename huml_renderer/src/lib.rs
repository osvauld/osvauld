//! HUML Renderer - Dependency-tracked template rendering
//!
//! **Architecture**:
//! ```text
//! Template (parsed JSON)
//!         │
//!         ▼
//! HumlRenderer
//!   ├── DependencyGraph (tracks what depends on what)
//!   ├── CelCache (cached expression results)
//!   ├── RenderTree (retained UI nodes)
//!   └── QueryBridge (data access)
//!         │
//!         ▼
//!   Layout (Taffy) → Render (Vello)
//! ```
//!
//! **Reactivity Flow**:
//! 1. Data changes → QueryBridge receives QueryDelta
//! 2. HumlRenderer.on_data_change(field) → DependencyGraph.invalidate
//! 3. Dirty computed values re-evaluated
//! 4. Dirty nodes updated in RenderTree
//! 5. Layout recomputed for changed nodes
//! 6. Only changed nodes re-rendered

pub mod dep_graph;
pub mod layout;
pub mod render_tree;
pub mod vello_backend;
pub mod window;

use std::collections::HashMap;
use std::sync::Arc;

use cel_runtime::CelEvaluator;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tracing::{debug, info, warn};

// Re-export QueryBridge types for convenience
pub use query_bridge::{QueryBridgeHandle, QueryBridgeMsg, RendererDelta};

pub use dep_graph::{DependencyGraph, NodeId};
pub use layout::LayoutEngine;
pub use render_tree::{RenderTree, RenderNode, Color, Direction, FontWeight, ButtonAction, SelectOption};
pub use vello_backend::{VelloBackend, LayoutRect};
pub use window::{HumlWindow, HumlApp, AppEvent, run_window};

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("Template parse error: {0}")]
    TemplateParse(String),

    #[error("CEL evaluation error: {0}")]
    CelEvaluation(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),
}

pub type Result<T> = std::result::Result<T, RenderError>;

/// Parsed HUML template structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTemplate {
    pub name: String,
    pub version: String,

    /// Document definitions (data sources)
    #[serde(default)]
    pub documents: HashMap<String, DocumentDef>,

    /// Query definitions
    #[serde(default)]
    pub queries: HashMap<String, QueryDef>,

    /// Computed value definitions
    #[serde(default)]
    pub computed: HashMap<String, ComputedDef>,

    /// UI sections
    pub ui: UiSection,
}

/// Document definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDef {
    #[serde(rename = "type")]
    pub data_type: String,
    pub initial: Option<Value>,
    /// Layer name (for sync) - accepts both "layer" and "document" in YAML
    #[serde(default, alias = "document")]
    pub layer: Option<String>,
}

/// Query definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryDef {
    /// Scribe layer name to query from
    pub layer: String,
    /// JSON path within the layer (e.g., "messages")
    #[serde(default)]
    pub path: String,
    /// CEL filter expression (optional)
    #[serde(default)]
    pub filter: Option<String>,
    /// Field to sort by (optional)
    #[serde(default)]
    pub sort_by: Option<String>,
    /// Sort order: "asc" or "desc"
    #[serde(default)]
    pub sort_order: Option<String>,
    /// Maximum items to return
    #[serde(default)]
    pub limit: Option<usize>,
    /// Dependencies (extracted from filter expression)
    #[serde(default)]
    pub deps: Vec<String>,
}

/// Computed value definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputedDef {
    pub expr: String,
    #[serde(default)]
    pub deps: Vec<String>,
}

/// UI section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSection {
    #[serde(default)]
    pub publisher: Vec<Screen>,
    #[serde(default)]
    pub viewer: Vec<Screen>,
}

/// Screen definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "isEntryPoint")]
    pub is_entry_point: Option<bool>,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

/// UI block (template AST node)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Block {
    Text {
        content: String,
        #[serde(default)]
        when: Option<String>,
    },
    Heading {
        level: u8,
        content: String,
        #[serde(default)]
        when: Option<String>,
    },
    Container {
        #[serde(default)]
        blocks: Vec<Block>,
        #[serde(default)]
        when: Option<String>,
    },
    Section {
        #[serde(default)]
        blocks: Vec<Block>,
        #[serde(default)]
        when: Option<String>,
    },
    Button {
        content: String,
        #[serde(default)]
        action: Option<String>,
        #[serde(default, rename = "stateUpdates")]
        state_updates: Option<HashMap<String, String>>,
        #[serde(default)]
        when: Option<String>,
    },
    Input {
        name: String,
        #[serde(default)]
        placeholder: Option<String>,
        #[serde(default)]
        when: Option<String>,
    },
    Textarea {
        name: String,
        #[serde(default)]
        placeholder: Option<String>,
        #[serde(default)]
        when: Option<String>,
    },
    Checkbox {
        name: String,
        label: String,
        #[serde(default)]
        when: Option<String>,
    },
    Select {
        name: String,
        options: Vec<SelectOption>,
        #[serde(default)]
        when: Option<String>,
    },
    Loop {
        each: String,
        r#as: String,
        #[serde(default)]
        blocks: Vec<Block>,
        #[serde(default)]
        when: Option<String>,
    },
    Screen {
        id: String,
        name: String,
        #[serde(default)]
        blocks: Vec<Block>,
    },
    Label {
        content: String,
        #[serde(default)]
        when: Option<String>,
    },
    Radio {
        name: String,
        options: Vec<SelectOption>,
        #[serde(default)]
        when: Option<String>,
    },
}

/// HUML Renderer with dependency-tracked reactivity
pub struct HumlRenderer {
    /// Parsed template
    template: ParsedTemplate,

    /// CEL evaluator
    cel: Arc<CelEvaluator>,

    /// Dependency graph
    dep_graph: DependencyGraph,

    /// Cached CEL results (computed values)
    cel_cache: HashMap<String, Value>,

    /// Source values (documents, query results)
    sources: HashMap<String, Value>,

    /// Render tree
    render_tree: RenderTree,

    /// Current screen ID
    current_screen: String,

    /// Pending state updates from user interactions
    pending_updates: Vec<(String, String)>,

    /// QueryBridge handle for Scribe data access (optional)
    ///
    /// **Context**: When set, commit() calls in CEL will persist to Scribe.
    /// When None, all state is local-only.
    query_bridge: Option<QueryBridgeHandle>,

    /// Mapping from field name to layer info for commits
    field_to_layer: HashMap<String, (String, String)>, // field -> (layer, path)
}

impl HumlRenderer {
    /// Create a new renderer for the given template (local-only, no Scribe)
    pub fn new(template: ParsedTemplate, cel: Arc<CelEvaluator>) -> Self {
        Self::with_query_bridge(template, cel, None)
    }

    /// Create a renderer with optional QueryBridge for Scribe integration
    ///
    /// **Context**: When query_bridge is Some, commit() calls persist to Scribe.
    pub fn with_query_bridge(
        template: ParsedTemplate,
        cel: Arc<CelEvaluator>,
        query_bridge: Option<QueryBridgeHandle>,
    ) -> Self {
        let mut renderer = Self {
            template,
            cel,
            dep_graph: DependencyGraph::new(),
            cel_cache: HashMap::new(),
            sources: HashMap::new(),
            render_tree: RenderTree::new(),
            current_screen: String::new(),
            pending_updates: Vec::new(),
            query_bridge,
            field_to_layer: HashMap::new(),
        };

        renderer.initialize();
        renderer
    }

    /// Initialize the renderer (build dep graph, initial render)
    fn initialize(&mut self) {
        // Register computed value dependencies
        for (name, def) in &self.template.computed {
            self.dep_graph.register_computed(name, &def.deps);
        }

        // Initialize document values from template defaults
        for (name, doc_def) in &self.template.documents {
            if let Some(initial) = &doc_def.initial {
                self.sources.insert(name.clone(), initial.clone());
            }
            // Track layer mapping for documents with layer/document field
            if let Some(layer) = &doc_def.layer {
                self.field_to_layer.insert(name.clone(), (layer.clone(), name.clone()));
            }
        }

        // Register queries from template and subscribe to Scribe
        for (name, query_def) in &self.template.queries {
            // Store layer mapping for commit() routing
            self.field_to_layer.insert(
                name.clone(),
                (query_def.layer.clone(), query_def.path.clone()),
            );

            // Register with QueryBridge (if available)
            if let Some(ref bridge) = self.query_bridge {
                bridge.register_query(
                    name.clone(),
                    query_def.layer.clone(),
                    query_def.path.clone(),
                );
                debug!(query_id = %name, layer = %query_def.layer, "Registered query");
            }

            // Initialize with empty array until data arrives
            self.sources.insert(name.clone(), Value::Array(Vec::new()));
        }

        // Find entry point screen
        self.current_screen = self.template.ui.publisher
            .iter()
            .find(|s| s.is_entry_point.unwrap_or(false))
            .or_else(|| self.template.ui.publisher.first())
            .map(|s| s.id.clone())
            .unwrap_or_default();

        // Evaluate all computed values before building render tree
        // This ensures `when` conditions can reference computed values
        self.evaluate_all_computed();

        // Build initial render tree
        self.build_render_tree();

        info!(
            template = %self.template.name,
            screen = %self.current_screen,
            nodes = self.render_tree.len(),
            "HUML renderer initialized"
        );
    }

    /// Build render tree from current screen's blocks
    pub fn build_render_tree(&mut self) {
        self.render_tree.clear();

        // Find current screen
        let screen = self.template.ui.publisher
            .iter()
            .find(|s| s.id == self.current_screen)
            .cloned();

        let Some(screen) = screen else {
            warn!(screen = %self.current_screen, "Screen not found");
            return;
        };

        // Create root container for screen
        let root_id = self.render_tree.add_root(RenderNode::Container {
            direction: Direction::Column,
            padding: 16.0,
            gap: 12.0,
            background: None,
            visible: true,
        });

        // Convert blocks to render nodes
        for block in &screen.blocks {
            self.convert_block(block, root_id);
        }

        debug!(
            screen = %self.current_screen,
            nodes = self.render_tree.len(),
            "Render tree built"
        );
    }

    /// Convert a Block (template AST) to RenderNode (render tree)
    fn convert_block(&mut self, block: &Block, parent_id: NodeId) {
        match block {
            Block::Text { content, when } => {
                let visible = self.eval_when(when.as_deref());
                let resolved = self.interpolate(content);
                self.render_tree.add_child(parent_id, RenderNode::Text {
                    content: resolved,
                    font_size: 16.0,
                    font_weight: FontWeight::Normal,
                    color: Color::BLACK,
                    visible,
                });
            }

            Block::Heading { level, content, when } => {
                let visible = self.eval_when(when.as_deref());
                let resolved = self.interpolate(content);
                self.render_tree.add_child(parent_id, RenderNode::Heading {
                    level: *level,
                    content: resolved,
                    visible,
                });
            }

            Block::Container { blocks, when } => {
                let visible = self.eval_when(when.as_deref());
                let container_id = self.render_tree.add_child(parent_id, RenderNode::Container {
                    direction: Direction::Column,  // Stack children vertically by default
                    padding: 0.0,
                    gap: 4.0,
                    background: None,
                    visible,
                });
                for child in blocks {
                    self.convert_block(child, container_id);
                }
            }

            Block::Section { blocks, when } => {
                let visible = self.eval_when(when.as_deref());
                let section_id = self.render_tree.add_child(parent_id, RenderNode::Container {
                    direction: Direction::Column,
                    padding: 12.0,
                    gap: 8.0,
                    background: Some(Color::rgba(0.95, 0.95, 0.95, 1.0)),
                    visible,
                });
                for child in blocks {
                    self.convert_block(child, section_id);
                }
            }

            Block::Button { content, action, state_updates, when } => {
                let visible = self.eval_when(when.as_deref());
                let label = self.interpolate(content);

                let button_action = if action.as_deref() == Some("setState") {
                    state_updates.as_ref().map(|updates| ButtonAction::SetState {
                        updates: updates.clone(),
                    })
                } else if let Some(action_name) = action {
                    Some(ButtonAction::Custom {
                        name: action_name.clone(),
                        data: Value::Null,
                    })
                } else {
                    None
                };

                self.render_tree.add_child(parent_id, RenderNode::Button {
                    label,
                    action: button_action,
                    enabled: true,
                    visible,
                });
            }

            Block::Input { name, placeholder, when } => {
                let visible = self.eval_when(when.as_deref());
                let value = self.sources.get(name)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                self.render_tree.add_child(parent_id, RenderNode::Input {
                    name: name.clone(),
                    placeholder: placeholder.clone().unwrap_or_default(),
                    value,
                    visible,
                });
            }

            Block::Textarea { name, placeholder, when } => {
                let visible = self.eval_when(when.as_deref());
                let value = self.sources.get(name)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                self.render_tree.add_child(parent_id, RenderNode::TextArea {
                    name: name.clone(),
                    placeholder: placeholder.clone().unwrap_or_default(),
                    value,
                    rows: 4,
                    visible,
                });
            }

            Block::Checkbox { name, label, when } => {
                let visible = self.eval_when(when.as_deref());
                let checked = self.sources.get(name)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                self.render_tree.add_child(parent_id, RenderNode::Checkbox {
                    name: name.clone(),
                    label: label.clone(),
                    checked,
                    visible,
                });
            }

            Block::Select { name, options, when } => {
                let visible = self.eval_when(when.as_deref());
                let selected = self.sources.get(name)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                self.render_tree.add_child(parent_id, RenderNode::Select {
                    name: name.clone(),
                    options: options.clone(),
                    selected,
                    visible,
                });
            }

            Block::Loop { each, r#as, blocks, when } => {
                let visible = self.eval_when(when.as_deref());

                // Evaluate the `each` expression to get the array
                // Can be a CEL expression like "${ messages }" or a direct key
                let items = if each.contains("${") {
                    self.evaluate_expr(each)
                        .and_then(|v| v.as_array().cloned())
                        .unwrap_or_default()
                } else {
                    self.sources.get(each)
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default()
                };

                let loop_id = self.render_tree.add_child(parent_id, RenderNode::Loop {
                    source_expr: each.clone(),
                    item_var: r#as.clone(),
                    item_count: items.len(),
                    visible,
                });

                // For each item, temporarily set the loop variable and render children
                for (_idx, item) in items.iter().enumerate() {
                    // Set loop variable in sources for this iteration
                    self.sources.insert(r#as.clone(), item.clone());

                    // Render all child blocks for this item
                    for child in blocks {
                        self.convert_block(child, loop_id);
                    }
                }

                // Remove loop variable after loop completes
                self.sources.remove(r#as);
            }

            Block::Screen { id: _, name: _, blocks } => {
                // Screens are handled at top level, but if nested, just render blocks
                for child in blocks {
                    self.convert_block(child, parent_id);
                }
            }

            Block::Label { content, when } => {
                let visible = self.eval_when(when.as_deref());
                let resolved = self.interpolate(content);
                self.render_tree.add_child(parent_id, RenderNode::Text {
                    content: resolved,
                    font_size: 14.0,
                    font_weight: FontWeight::Normal,
                    color: Color::rgba(0.4, 0.4, 0.4, 1.0),
                    visible,
                });
            }

            Block::Radio { name, options, when } => {
                let visible = self.eval_when(when.as_deref());
                let selected = self.sources.get(name)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Render as select for now (TODO: proper radio buttons)
                self.render_tree.add_child(parent_id, RenderNode::Select {
                    name: name.clone(),
                    options: options.clone(),
                    selected,
                    visible,
                });
            }
        }
    }

    /// Evaluate a `when` condition, defaulting to true if None
    fn eval_when(&self, when: Option<&str>) -> bool {
        match when {
            None => true,
            Some(expr) => {
                self.evaluate_expr(expr)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true)
            }
        }
    }

    /// Evaluate all computed values and cache results
    fn evaluate_all_computed(&mut self) {
        // Clone computed defs to avoid borrow issues
        let computed_defs: Vec<(String, ComputedDef)> = self.template.computed
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        for (name, def) in computed_defs {
            let ctx = self.build_context();
            match self.cel.evaluate(&def.expr, &ctx) {
                Ok(value) => {
                    debug!(computed = %name, value = ?value, "Evaluated computed value");
                    self.cel_cache.insert(name, value);
                }
                Err(e) => {
                    warn!(computed = %name, error = %e, "Failed to evaluate computed value");
                }
            }
        }
    }

    /// Update a source value (triggers dependency invalidation)
    pub fn update_source(&mut self, name: &str, value: Value) {
        self.sources.insert(name.to_string(), value);
        self.dep_graph.invalidate(name);
    }

    /// Get a source value
    pub fn get_source(&self, name: &str) -> Option<&Value> {
        self.sources.get(name)
    }

    /// Get a computed or source value
    pub fn get_value(&self, name: &str) -> Option<&Value> {
        self.cel_cache.get(name).or_else(|| self.sources.get(name))
    }

    /// Process pending updates and re-evaluate dirty values
    ///
    /// **Call this** at the start of each render frame
    pub fn process_updates(&mut self) {
        // Re-evaluate dirty computed values
        let dirty_computed = self.dep_graph.take_dirty_computed();
        for name in dirty_computed {
            if let Some(def) = self.template.computed.get(&name).cloned() {
                let ctx = self.build_context();
                match self.cel.evaluate(&def.expr, &ctx) {
                    Ok(value) => {
                        self.cel_cache.insert(name, value);
                    }
                    Err(e) => {
                        warn!(computed = %name, error = %e, "Failed to evaluate computed value");
                    }
                }
            }
        }

        // Update dirty render nodes
        let dirty_nodes = self.dep_graph.take_dirty_nodes();
        for node_id in dirty_nodes {
            self.update_node(node_id);
        }
    }

    /// Build evaluation context from sources and computed values
    pub fn build_context(&self) -> Value {
        let mut ctx = serde_json::Map::new();

        // Add sources
        for (name, value) in &self.sources {
            ctx.insert(name.clone(), value.clone());
        }

        // Add computed values
        for (name, value) in &self.cel_cache {
            ctx.insert(name.clone(), value.clone());
        }

        Value::Object(ctx)
    }

    /// Evaluate a CEL expression with current context
    pub fn evaluate_expr(&self, expr: &str) -> Option<Value> {
        let ctx = self.build_context();
        match self.cel.evaluate(expr, &ctx) {
            Ok(value) => Some(value),
            Err(e) => {
                warn!(expr = %expr, error = %e, "CEL evaluation failed");
                None
            }
        }
    }

    /// Interpolate `{{ expr }}` patterns in text
    pub fn interpolate(&self, template: &str) -> String {
        if !template.contains("{{") {
            return template.to_string();
        }

        let ctx = self.build_context();
        match self.cel.interpolate(template, &ctx) {
            Ok(result) => result,
            Err(e) => {
                warn!(template = %template, error = %e, "Interpolation failed");
                template.to_string()
            }
        }
    }

    /// Check if there are pending changes
    pub fn has_pending_changes(&self) -> bool {
        self.dep_graph.has_dirty()
    }

    /// Take pending state updates (for processing by data bridge)
    pub fn take_pending_updates(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.pending_updates)
    }

    /// Queue a state update (from button click, input change, etc.)
    pub fn queue_update(&mut self, field: String, expr: String) {
        self.pending_updates.push((field, expr));
    }

    /// Update a specific render node (re-resolve its values)
    fn update_node(&mut self, node_id: NodeId) {
        // Get deps for this node (for future use when re-evaluating expressions)
        let _deps = match self.dep_graph.get_node_deps(node_id) {
            Some(deps) => deps.clone(),
            None => return,
        };

        // Re-resolve node values based on its type
        if let Some(node) = self.render_tree.get_mut(node_id) {
            match node {
                RenderNode::Text { .. } => {
                    // TODO: Re-interpolate content
                    // Note: Original content expression would need to be stored
                    debug!(node_id, "Updating text node");
                }
                RenderNode::Button { .. } => {
                    debug!(node_id, "Updating button node");
                }
                _ => {
                    debug!(node_id, "Updating node");
                }
            }
        }
    }

    /// Get the render tree (for painting)
    pub fn render_tree(&self) -> &RenderTree {
        &self.render_tree
    }

    /// Get mutable render tree
    pub fn render_tree_mut(&mut self) -> &mut RenderTree {
        &mut self.render_tree
    }

    /// Get current screen ID
    pub fn current_screen(&self) -> &str {
        &self.current_screen
    }

    /// Navigate to a different screen
    pub fn navigate(&mut self, screen_id: &str) {
        if self.template.ui.publisher.iter().any(|s| s.id == screen_id) {
            info!(from = %self.current_screen, to = %screen_id, "Navigating");
            self.current_screen = screen_id.to_string();
            // Mark all nodes dirty for re-render
            self.dep_graph.mark_all_dirty();
        } else {
            warn!(screen = %screen_id, "Screen not found");
        }
    }

    /// Get the template
    pub fn template(&self) -> &ParsedTemplate {
        &self.template
    }

    /// Get the CEL evaluator
    pub fn cel(&self) -> &CelEvaluator {
        &self.cel
    }

    /// Handle a button click by node ID
    ///
    /// Executes the button's action (if any) and returns true if state changed
    pub fn on_button_click(&mut self, node_id: NodeId) -> bool {
        // Get the button's action from the render tree
        let action = {
            let Some(node) = self.render_tree.get(node_id) else {
                warn!(node_id, "Button node not found");
                return false;
            };

            match node {
                RenderNode::Button { action, enabled, .. } => {
                    if !enabled {
                        debug!(node_id, "Button is disabled");
                        return false;
                    }
                    action.clone()
                }
                _ => {
                    warn!(node_id, "Node is not a button");
                    return false;
                }
            }
        };

        // Execute the action
        let Some(action) = action else {
            debug!(node_id, "Button has no action");
            return false;
        };

        match action {
            ButtonAction::SetState { updates } => {
                debug!(node_id, updates = ?updates.keys().collect::<Vec<_>>(), "Executing SetState action");

                // IMPORTANT: Evaluate ALL expressions FIRST, before applying any updates.
                // This ensures each expression sees the context as it was at button click time.
                // Otherwise, if newMessageText is cleared before messages is evaluated,
                // the message would have empty text.
                let evaluated: Vec<(String, Value)> = updates
                    .iter()
                    .filter_map(|(field, expr)| {
                        self.evaluate_expr(expr).map(|value| (field.clone(), value))
                    })
                    .collect();

                // Now apply all updates
                let mut changed = false;
                for (field, value) in evaluated {
                    self.apply_state_update(&field, value);
                    changed = true;
                }

                if changed {
                    // Re-evaluate computed values
                    self.evaluate_all_computed();
                    // Rebuild render tree
                    self.build_render_tree();
                }

                changed
            }
            ButtonAction::Navigate { screen } => {
                info!(screen = %screen, "Navigate action");
                self.navigate(&screen);
                true
            }
            ButtonAction::Custom { name, .. } => {
                info!(action = %name, "Custom action (not implemented)");
                false
            }
        }
    }

    /// Apply a state update, detecting commit/local markers from CEL
    ///
    /// **Context**: CEL expressions can wrap values with:
    /// - `commit(value)` → `{ "__dataflow": "commit", "value": value }`
    /// - `local(value)` → `{ "__dataflow": "local", "value": value }`
    ///
    /// This method detects these markers and routes accordingly:
    /// - `__dataflow: "commit"`: Update local + send to Scribe via QueryBridge
    /// - `__dataflow: "local"`: Update local only
    /// - Plain value: Update local only
    fn apply_state_update(&mut self, field: &str, value: Value) {
        if let Some(obj) = value.as_object() {
            // Check for __dataflow marker from CEL commit()/local() functions
            let dataflow = obj.get("__dataflow").and_then(|v| v.as_str());

            match dataflow {
                Some("commit") => {
                    // Commit: update local + send to Scribe
                    let inner = obj.get("value").cloned().unwrap_or(Value::Null);
                    info!(field = %field, "Committing to Scribe");

                    // Send to Scribe via QueryBridge
                    if let Some(ref bridge) = self.query_bridge {
                        bridge.send_commit(field.to_string(), inner.clone());
                    }

                    // Also update local for immediate UI feedback (optimistic update)
                    self.sources.insert(field.to_string(), inner);
                    self.dep_graph.invalidate(field);
                }
                Some("local") => {
                    // Local only: just update sources
                    let inner = obj.get("value").cloned().unwrap_or(Value::Null);
                    debug!(field = %field, "Local-only update");
                    self.sources.insert(field.to_string(), inner);
                    self.dep_graph.invalidate(field);
                }
                _ => {
                    // Plain object value (no __dataflow marker)
                    self.update_source(field, value);
                }
            }
        } else {
            // Plain value (not an object with markers)
            self.update_source(field, value);
        }
    }

    /// Receive query result from QueryBridge (called when data arrives from Scribe)
    ///
    /// **Context**: Called by the window event loop when QueryDelta arrives.
    pub fn on_query_result(&mut self, query_id: &str, items: Vec<Value>) {
        info!(query_id = %query_id, count = items.len(), "Received query result");
        self.sources.insert(query_id.to_string(), Value::Array(items));
        self.dep_graph.invalidate(query_id);

        // Re-evaluate computed values that depend on this query
        self.evaluate_all_computed();
        // Rebuild render tree
        self.build_render_tree();
    }

    /// Check if this renderer has a QueryBridge
    pub fn has_query_bridge(&self) -> bool {
        self.query_bridge.is_some()
    }

    /// Load initial query data (fetched from Scribe before window creation)
    ///
    /// **Context**: Called after construction to populate queries with existing data.
    /// This ensures messages appear immediately when reopening a chat.
    pub fn load_initial_query_data(&mut self, data: HashMap<String, Vec<Value>>) {
        for (query_id, items) in data {
            info!(query_id = %query_id, count = items.len(), "Loading initial query data");
            self.sources.insert(query_id.clone(), Value::Array(items));
            self.dep_graph.invalidate(&query_id);
        }

        // Re-evaluate computed values and rebuild render tree
        self.evaluate_all_computed();
        self.build_render_tree();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mock_template() -> ParsedTemplate {
        ParsedTemplate {
            name: "Test".to_string(),
            version: "1.0".to_string(),
            documents: HashMap::new(),
            queries: HashMap::new(),
            computed: {
                let mut map = HashMap::new();
                map.insert("isPositive".to_string(), ComputedDef {
                    expr: "${ count > 0 }".to_string(),
                    deps: vec!["count".to_string()],
                });
                map
            },
            ui: UiSection {
                publisher: vec![Screen {
                    id: "main".to_string(),
                    name: "Main".to_string(),
                    is_entry_point: Some(true),
                    blocks: vec![],
                }],
                viewer: vec![],
            },
        }
    }

    #[test]
    fn test_dependency_registration() {
        use std::path::PathBuf;

        // Skip if CEL not available
        let cel_path = PathBuf::from("sthalam/template-transpiler/_build/default/eval-bin/cel_wasi.exe");
        if !cel_path.exists() {
            return;
        }

        let cel = Arc::new(CelEvaluator::new(cel_path).unwrap());
        let template = mock_template();
        let renderer = HumlRenderer::new(template, cel);

        // Computed "isPositive" should be registered
        assert!(renderer.template.computed.contains_key("isPositive"));
    }
}
