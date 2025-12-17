//! Render Tree - Retained UI node structure
//!
//! Unlike immediate-mode GUI where everything is rebuilt each frame,
//! the render tree persists across frames. Only dirty nodes are updated.
//!
//! **Node types** map to HUML block types with resolved/cached values.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::dep_graph::NodeId;

/// Render tree containing all UI nodes
#[derive(Debug, Default)]
pub struct RenderTree {
    /// All nodes by ID
    nodes: HashMap<NodeId, RenderNode>,

    /// Root node IDs (top-level blocks)
    roots: Vec<NodeId>,

    /// Next node ID to assign
    next_id: NodeId,

    /// Parent relationships (child → parent)
    parents: HashMap<NodeId, NodeId>,

    /// Children relationships (parent → children)
    children: HashMap<NodeId, Vec<NodeId>>,
}

impl RenderTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a new node ID
    pub fn alloc_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Add a root node
    pub fn add_root(&mut self, node: RenderNode) -> NodeId {
        let id = self.alloc_id();
        self.nodes.insert(id, node);
        self.roots.push(id);
        id
    }

    /// Add a child node to a parent
    pub fn add_child(&mut self, parent_id: NodeId, node: RenderNode) -> NodeId {
        let id = self.alloc_id();
        self.nodes.insert(id, node);
        self.parents.insert(id, parent_id);
        self.children.entry(parent_id).or_default().push(id);
        id
    }

    /// Get a node by ID
    pub fn get(&self, id: NodeId) -> Option<&RenderNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable node by ID
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut RenderNode> {
        self.nodes.get_mut(&id)
    }

    /// Get root node IDs
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// Get children of a node
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.children.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Iterate all nodes
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &RenderNode)> {
        self.nodes.iter().map(|(id, node)| (*id, node))
    }

    /// Clear all nodes (for template reload)
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.roots.clear();
        self.parents.clear();
        self.children.clear();
        self.next_id = 0;
    }

    /// Get node count
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// A rendered UI node with resolved values
///
/// Each variant holds the final, resolved values ready for painting.
/// When a node is dirty, only its resolved values are recomputed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RenderNode {
    /// Container node (groups children)
    Container {
        /// Layout direction
        direction: Direction,
        /// Padding in pixels
        padding: f32,
        /// Gap between children
        gap: f32,
        /// Background color (optional)
        background: Option<Color>,
        /// Visibility (resolved from `when` condition)
        visible: bool,
    },

    /// Text node
    Text {
        /// Resolved text content (after interpolation)
        content: String,
        /// Font size
        font_size: f32,
        /// Font weight
        font_weight: FontWeight,
        /// Text color
        color: Color,
        /// Visibility
        visible: bool,
    },

    /// Heading node
    Heading {
        /// Heading level (1-6)
        level: u8,
        /// Resolved text content
        content: String,
        /// Visibility
        visible: bool,
    },

    /// Button node
    Button {
        /// Button label (resolved)
        label: String,
        /// Action to perform on click
        action: Option<ButtonAction>,
        /// Enabled state
        enabled: bool,
        /// Visibility
        visible: bool,
    },

    /// Text input node
    Input {
        /// Field name (for data binding)
        name: String,
        /// Placeholder text
        placeholder: String,
        /// Current value
        value: String,
        /// Visibility
        visible: bool,
    },

    /// Multi-line text area
    TextArea {
        /// Field name
        name: String,
        /// Placeholder
        placeholder: String,
        /// Current value
        value: String,
        /// Number of visible rows
        rows: u32,
        /// Visibility
        visible: bool,
    },

    /// Checkbox
    Checkbox {
        /// Field name
        name: String,
        /// Label text
        label: String,
        /// Checked state
        checked: bool,
        /// Visibility
        visible: bool,
    },

    /// Select dropdown
    Select {
        /// Field name
        name: String,
        /// Available options
        options: Vec<SelectOption>,
        /// Currently selected value
        selected: String,
        /// Visibility
        visible: bool,
    },

    /// Loop container (renders children for each item)
    Loop {
        /// Source array expression
        source_expr: String,
        /// Loop variable name
        item_var: String,
        /// Number of items (resolved)
        item_count: usize,
        /// Visibility
        visible: bool,
    },

    /// Image node
    Image {
        /// Image source URL
        src: String,
        /// Alt text
        alt: String,
        /// Width (optional)
        width: Option<f32>,
        /// Height (optional)
        height: Option<f32>,
        /// Visibility
        visible: bool,
    },

    /// Divider/separator
    Divider {
        /// Orientation
        orientation: Orientation,
        /// Visibility
        visible: bool,
    },

    /// Spacer (flexible space)
    Spacer {
        /// Minimum size
        min_size: f32,
        /// Flex grow factor
        flex: f32,
    },
}

impl RenderNode {
    /// Check if this node is visible
    pub fn is_visible(&self) -> bool {
        match self {
            RenderNode::Container { visible, .. } => *visible,
            RenderNode::Text { visible, .. } => *visible,
            RenderNode::Heading { visible, .. } => *visible,
            RenderNode::Button { visible, .. } => *visible,
            RenderNode::Input { visible, .. } => *visible,
            RenderNode::TextArea { visible, .. } => *visible,
            RenderNode::Checkbox { visible, .. } => *visible,
            RenderNode::Select { visible, .. } => *visible,
            RenderNode::Loop { visible, .. } => *visible,
            RenderNode::Image { visible, .. } => *visible,
            RenderNode::Divider { visible, .. } => *visible,
            RenderNode::Spacer { .. } => true,
        }
    }

    /// Set visibility
    pub fn set_visible(&mut self, vis: bool) {
        match self {
            RenderNode::Container { visible, .. } => *visible = vis,
            RenderNode::Text { visible, .. } => *visible = vis,
            RenderNode::Heading { visible, .. } => *visible = vis,
            RenderNode::Button { visible, .. } => *visible = vis,
            RenderNode::Input { visible, .. } => *visible = vis,
            RenderNode::TextArea { visible, .. } => *visible = vis,
            RenderNode::Checkbox { visible, .. } => *visible = vis,
            RenderNode::Select { visible, .. } => *visible = vis,
            RenderNode::Loop { visible, .. } => *visible = vis,
            RenderNode::Image { visible, .. } => *visible = vis,
            RenderNode::Divider { visible, .. } => *visible = vis,
            RenderNode::Spacer { .. } => {}
        }
    }
}

/// Layout direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Column,
    Row,
}

/// Orientation for dividers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

/// Font weight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontWeight {
    #[default]
    Normal,
    Bold,
    Light,
}

/// RGBA color
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const TRANSPARENT: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

/// Button action
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ButtonAction {
    /// Update state with CEL expression results
    SetState {
        /// Field → expression mappings
        updates: HashMap<String, String>,
    },
    /// Navigate to a screen
    Navigate {
        /// Target screen ID
        screen: String,
    },
    /// Custom action (handled by app)
    Custom {
        /// Action name
        name: String,
        /// Action data
        data: Value,
    },
}

/// Select option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}
