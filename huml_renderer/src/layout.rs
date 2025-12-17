//! Taffy Layout Engine Integration
//!
//! Maps RenderTree nodes to Taffy layout nodes and computes flexbox layout.
//!
//! **Pipeline**:
//! ```text
//! RenderTree → build_taffy_tree() → TaffyTree
//!                                       │
//!                     compute_layout() ─┘
//!                           │
//!                           ▼
//!                    LayoutRect per node
//! ```

use std::collections::HashMap;

use taffy::prelude::*;
use tracing::debug;

use crate::dep_graph::NodeId;
use crate::render_tree::{Direction, RenderNode, RenderTree};
use crate::vello_backend::LayoutRect;

/// Helper to create a Rect with same value on all sides
fn length_rect(value: f32) -> Rect<LengthPercentage> {
    Rect {
        left: LengthPercentage::length(value),
        right: LengthPercentage::length(value),
        top: LengthPercentage::length(value),
        bottom: LengthPercentage::length(value),
    }
}

/// Helper to create a margin Rect with same value on all sides
fn margin_rect(value: f32) -> Rect<LengthPercentageAuto> {
    Rect {
        left: LengthPercentageAuto::length(value),
        right: LengthPercentageAuto::length(value),
        top: LengthPercentageAuto::length(value),
        bottom: LengthPercentageAuto::length(value),
    }
}

/// Layout engine wrapping Taffy
pub struct LayoutEngine {
    /// Taffy layout tree
    taffy: TaffyTree<NodeId>,

    /// Mapping from our NodeId to Taffy node
    node_to_taffy: HashMap<NodeId, taffy::NodeId>,

    /// Root taffy node
    root: Option<taffy::NodeId>,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            node_to_taffy: HashMap::new(),
            root: None,
        }
    }

    /// Build the Taffy tree from our RenderTree
    pub fn build_from_render_tree(&mut self, tree: &RenderTree) {
        // Clear existing layout
        self.clear();

        // Create a root container that holds all top-level nodes
        let root_style = Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            flex_direction: FlexDirection::Column,
            ..Default::default()
        };

        // Build children first
        let mut root_children = Vec::new();
        for &root_id in tree.roots() {
            if let Some(taffy_node) = self.build_node(tree, root_id) {
                root_children.push(taffy_node);
            }
        }

        // Create root node with children
        let root = self
            .taffy
            .new_with_children(root_style, &root_children)
            .expect("Failed to create root node");

        self.root = Some(root);
    }

    /// Recursively build a Taffy node from a RenderNode
    fn build_node(&mut self, tree: &RenderTree, node_id: NodeId) -> Option<taffy::NodeId> {
        let node = tree.get(node_id)?;

        // Skip invisible nodes
        if !node.is_visible() {
            return None;
        }

        let style = self.style_for_node(node);

        // Build children first
        let children: Vec<taffy::NodeId> = tree
            .children(node_id)
            .iter()
            .filter_map(|&child_id| self.build_node(tree, child_id))
            .collect();

        // Create Taffy node
        let taffy_node = if children.is_empty() {
            self.taffy
                .new_leaf_with_context(style, node_id)
                .expect("Failed to create leaf node")
        } else {
            let taffy_node = self
                .taffy
                .new_with_children(style, &children)
                .expect("Failed to create container node");
            // Store context manually since new_with_children doesn't support it
            self.taffy
                .set_node_context(taffy_node, Some(node_id))
                .expect("Failed to set node context");
            taffy_node
        };

        self.node_to_taffy.insert(node_id, taffy_node);
        Some(taffy_node)
    }

    /// Generate Taffy style for a RenderNode
    fn style_for_node(&self, node: &RenderNode) -> Style {
        match node {
            RenderNode::Container {
                direction,
                padding,
                gap,
                ..
            } => Style {
                flex_direction: match direction {
                    Direction::Column => FlexDirection::Column,
                    Direction::Row => FlexDirection::Row,
                },
                padding: length_rect(*padding),
                gap: Size {
                    width: LengthPercentage::length(*gap),
                    height: LengthPercentage::length(*gap),
                },
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::auto(),
                },
                ..Default::default()
            },

            RenderNode::Text { font_size, .. } => Style {
                // Text takes up content size
                size: Size {
                    width: Dimension::auto(),
                    height: Dimension::length(*font_size * 1.4), // line height
                },
                ..Default::default()
            },

            RenderNode::Heading { level, .. } => {
                let font_size = match level {
                    1 => 32.0,
                    2 => 24.0,
                    3 => 20.0,
                    4 => 18.0,
                    _ => 16.0,
                };
                Style {
                    size: Size {
                        width: Dimension::auto(),
                        height: Dimension::length(font_size * 1.4),
                    },
                    margin: Rect {
                        top: LengthPercentageAuto::length(font_size * 0.5),
                        bottom: LengthPercentageAuto::length(font_size * 0.25),
                        left: LengthPercentageAuto::length(0.0),
                        right: LengthPercentageAuto::length(0.0),
                    },
                    ..Default::default()
                }
            }

            RenderNode::Button { .. } => Style {
                size: Size {
                    width: Dimension::auto(),
                    height: Dimension::length(36.0),
                },
                min_size: Size {
                    width: Dimension::length(80.0),
                    height: Dimension::auto(),
                },
                padding: length_rect(8.0),
                margin: margin_rect(4.0),
                ..Default::default()
            },

            RenderNode::Input { .. } => Style {
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::length(36.0),
                },
                margin: Rect {
                    top: LengthPercentageAuto::length(4.0),
                    bottom: LengthPercentageAuto::length(4.0),
                    left: LengthPercentageAuto::length(0.0),
                    right: LengthPercentageAuto::length(0.0),
                },
                ..Default::default()
            },

            RenderNode::TextArea { rows, .. } => {
                let height = (*rows as f32) * 20.0 + 16.0; // 20px per row + padding
                Style {
                    size: Size {
                        width: Dimension::percent(1.0),
                        height: Dimension::length(height),
                    },
                    margin: Rect {
                        top: LengthPercentageAuto::length(4.0),
                        bottom: LengthPercentageAuto::length(4.0),
                        left: LengthPercentageAuto::length(0.0),
                        right: LengthPercentageAuto::length(0.0),
                    },
                    ..Default::default()
                }
            }

            RenderNode::Checkbox { .. } => Style {
                size: Size {
                    width: Dimension::auto(),
                    height: Dimension::length(24.0),
                },
                flex_direction: FlexDirection::Row,
                align_items: Some(AlignItems::Center),
                gap: Size {
                    width: LengthPercentage::length(8.0),
                    height: LengthPercentage::length(0.0),
                },
                margin: Rect {
                    top: LengthPercentageAuto::length(4.0),
                    bottom: LengthPercentageAuto::length(4.0),
                    left: LengthPercentageAuto::length(0.0),
                    right: LengthPercentageAuto::length(0.0),
                },
                ..Default::default()
            },

            RenderNode::Select { .. } => Style {
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::length(36.0),
                },
                margin: Rect {
                    top: LengthPercentageAuto::length(4.0),
                    bottom: LengthPercentageAuto::length(4.0),
                    left: LengthPercentageAuto::length(0.0),
                    right: LengthPercentageAuto::length(0.0),
                },
                ..Default::default()
            },

            RenderNode::Loop { .. } => Style {
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::auto(),
                },
                ..Default::default()
            },

            RenderNode::Image { width, height, .. } => Style {
                size: Size {
                    width: width
                        .map(Dimension::length)
                        .unwrap_or(Dimension::percent(1.0)),
                    height: height
                        .map(Dimension::length)
                        .unwrap_or(Dimension::length(200.0)),
                },
                ..Default::default()
            },

            RenderNode::Divider { orientation, .. } => {
                use crate::render_tree::Orientation;
                match orientation {
                    Orientation::Horizontal => Style {
                        size: Size {
                            width: Dimension::percent(1.0),
                            height: Dimension::length(1.0),
                        },
                        margin: Rect {
                            top: LengthPercentageAuto::length(8.0),
                            bottom: LengthPercentageAuto::length(8.0),
                            left: LengthPercentageAuto::length(0.0),
                            right: LengthPercentageAuto::length(0.0),
                        },
                        ..Default::default()
                    },
                    Orientation::Vertical => Style {
                        size: Size {
                            width: Dimension::length(1.0),
                            height: Dimension::percent(1.0),
                        },
                        margin: Rect {
                            top: LengthPercentageAuto::length(0.0),
                            bottom: LengthPercentageAuto::length(0.0),
                            left: LengthPercentageAuto::length(8.0),
                            right: LengthPercentageAuto::length(8.0),
                        },
                        ..Default::default()
                    },
                }
            }

            RenderNode::Spacer { min_size, flex } => Style {
                size: Size {
                    width: Dimension::auto(),
                    height: Dimension::auto(),
                },
                min_size: Size {
                    width: Dimension::length(*min_size),
                    height: Dimension::length(*min_size),
                },
                flex_grow: *flex,
                ..Default::default()
            },
        }
    }

    /// Compute layout for the given available space
    pub fn compute_layout(&mut self, width: f32, height: f32) {
        let Some(root) = self.root else {
            return;
        };

        let available_space = Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(height),
        };

        self.taffy
            .compute_layout(root, available_space)
            .expect("Layout computation failed");

        debug!(width, height, "Computed layout");
    }

    /// Get computed layout for a node
    pub fn get_layout(&self, node_id: NodeId) -> Option<LayoutRect> {
        let taffy_node = self.node_to_taffy.get(&node_id)?;
        let layout = self.taffy.layout(*taffy_node).ok()?;

        Some(LayoutRect {
            x: layout.location.x,
            y: layout.location.y,
            width: layout.size.width,
            height: layout.size.height,
        })
    }

    /// Get all computed layouts with absolute positions (for passing to VelloBackend)
    ///
    /// Taffy returns positions relative to parent, so we traverse the tree
    /// to compute absolute screen positions.
    pub fn get_all_layouts(&self) -> HashMap<NodeId, LayoutRect> {
        let mut result = HashMap::new();

        // Start from root and traverse
        if let Some(root) = self.root {
            self.collect_layouts_recursive(root, 0.0, 0.0, &mut result);
        }

        result
    }

    /// Recursively collect layouts with accumulated offsets
    fn collect_layouts_recursive(
        &self,
        taffy_node: taffy::NodeId,
        parent_x: f32,
        parent_y: f32,
        result: &mut HashMap<NodeId, LayoutRect>,
    ) {
        let Ok(layout) = self.taffy.layout(taffy_node) else {
            return;
        };

        // Compute absolute position
        let abs_x = parent_x + layout.location.x;
        let abs_y = parent_y + layout.location.y;

        // Store if this node has a NodeId (the root container doesn't)
        if let Some(&node_id) = self.taffy.get_node_context(taffy_node) {
            result.insert(
                node_id,
                LayoutRect {
                    x: abs_x,
                    y: abs_y,
                    width: layout.size.width,
                    height: layout.size.height,
                },
            );
        }

        // Traverse children
        let Ok(children) = self.taffy.children(taffy_node) else {
            return;
        };
        for child in children {
            self.collect_layouts_recursive(child, abs_x, abs_y, result);
        }
    }

    /// Clear all layout data
    pub fn clear(&mut self) {
        self.taffy.clear();
        self.node_to_taffy.clear();
        self.root = None;
    }

    /// Check if layout has been computed
    pub fn has_layout(&self) -> bool {
        self.root.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_tree::Color;

    #[test]
    fn test_basic_layout() {
        let mut tree = RenderTree::new();

        // Add a container with some text
        let container = tree.add_root(RenderNode::Container {
            direction: Direction::Column,
            padding: 10.0,
            gap: 5.0,
            background: None,
            visible: true,
        });

        tree.add_child(
            container,
            RenderNode::Text {
                content: "Hello".to_string(),
                font_size: 16.0,
                font_weight: crate::render_tree::FontWeight::Normal,
                color: Color::BLACK,
                visible: true,
            },
        );

        tree.add_child(
            container,
            RenderNode::Button {
                label: "Click me".to_string(),
                action: None,
                enabled: true,
                visible: true,
            },
        );

        let mut engine = LayoutEngine::new();
        engine.build_from_render_tree(&tree);
        engine.compute_layout(400.0, 300.0);

        // Check that layouts were computed
        let layouts = engine.get_all_layouts();
        assert!(!layouts.is_empty());

        // Container should have some size
        let container_layout = engine.get_layout(container).unwrap();
        assert!(container_layout.width > 0.0);
        assert!(container_layout.height > 0.0);
    }
}
