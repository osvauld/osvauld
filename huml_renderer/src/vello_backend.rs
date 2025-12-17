//! Vello Rendering Backend
//!
//! Converts RenderTree nodes into Vello scene primitives for GPU rendering.
//!
//! **Pipeline**:
//! ```text
//! RenderTree → Layout (Taffy) → VelloBackend → Scene → GPU
//! ```

use parley::{FontContext, LayoutContext, Layout, PositionedLayoutItem};
use parley::style::{StyleProperty, GenericFamily};
use peniko::{Brush, Color as PenikoColor, Fill};
use vello::kurbo::{Affine, Point, Rect, RoundedRect};
use vello::Scene;

use crate::dep_graph::NodeId;
use crate::render_tree::{Color, RenderNode, RenderTree};

/// Simple brush type for parley text layout.
/// Wraps a peniko Color and implements Default (required by parley::Brush).
#[derive(Clone, Debug, PartialEq)]
pub struct TextBrush(pub PenikoColor);

impl Default for TextBrush {
    fn default() -> Self {
        TextBrush(PenikoColor::new([0.0, 0.0, 0.0, 1.0]))
    }
}

/// Layout information for a node (computed by Taffy)
#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LayoutRect {
    pub fn to_rect(&self) -> Rect {
        Rect::new(
            self.x as f64,
            self.y as f64,
            (self.x + self.width) as f64,
            (self.y + self.height) as f64,
        )
    }

    pub fn to_rounded_rect(&self, radius: f64) -> RoundedRect {
        RoundedRect::new(
            self.x as f64,
            self.y as f64,
            (self.x + self.width) as f64,
            (self.y + self.height) as f64,
            radius,
        )
    }
}

/// Vello rendering backend
pub struct VelloBackend {
    /// Computed layouts for each node
    layouts: std::collections::HashMap<NodeId, LayoutRect>,

    /// Font context for text rendering (font database)
    font_cx: FontContext,

    /// Layout context for text rendering (scratch space)
    layout_cx: LayoutContext<TextBrush>,

    /// Default font size
    pub default_font_size: f32,

    /// Default text color
    pub default_text_color: Color,

    /// Default background color
    pub default_background: Color,

    /// Button colors
    pub button_background: Color,
    pub button_text_color: Color,
    pub button_hover_background: Color,

    /// Input colors
    pub input_background: Color,
    pub input_border_color: Color,

    /// Corner radius for rounded elements
    pub corner_radius: f64,

    /// Padding
    pub default_padding: f32,

    /// Gap between elements
    pub default_gap: f32,
}

impl Default for VelloBackend {
    fn default() -> Self {
        Self {
            layouts: std::collections::HashMap::new(),
            font_cx: FontContext::new(),
            layout_cx: LayoutContext::new(),
            default_font_size: 16.0,
            default_text_color: Color::rgb(0.1, 0.1, 0.1),
            default_background: Color::rgb(0.98, 0.98, 0.98),
            button_background: Color::rgb(0.2, 0.5, 0.9),
            button_text_color: Color::WHITE,
            button_hover_background: Color::rgb(0.3, 0.6, 1.0),
            input_background: Color::WHITE,
            input_border_color: Color::rgb(0.8, 0.8, 0.8),
            corner_radius: 4.0,
            default_padding: 8.0,
            default_gap: 8.0,
        }
    }
}

impl VelloBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set layout for a node (called after Taffy layout computation)
    pub fn set_layout(&mut self, node_id: NodeId, layout: LayoutRect) {
        self.layouts.insert(node_id, layout);
    }

    /// Get layout for a node
    pub fn get_layout(&self, node_id: NodeId) -> Option<&LayoutRect> {
        self.layouts.get(&node_id)
    }

    /// Clear all layouts
    pub fn clear_layouts(&mut self) {
        self.layouts.clear();
    }

    /// Hit test: find the node at the given position
    /// Returns the node ID if a clickable element is found
    pub fn hit_test(&self, tree: &RenderTree, x: f32, y: f32) -> Option<NodeId> {
        // Search in reverse order (top elements rendered last)
        let mut found: Option<(NodeId, f32)> = None; // (id, z-order approximation)

        for (node_id, node) in tree.iter() {
            // Only test visible, clickable nodes (buttons, inputs, checkboxes)
            if !node.is_visible() {
                continue;
            }

            // Check if this is a clickable node type
            let is_clickable = matches!(
                node,
                RenderNode::Button { .. }
                    | RenderNode::Input { .. }
                    | RenderNode::Checkbox { .. }
                    | RenderNode::Select { .. }
            );

            if !is_clickable {
                continue;
            }

            // Get layout and check bounds
            if let Some(layout) = self.layouts.get(&node_id) {
                if x >= layout.x
                    && x <= layout.x + layout.width
                    && y >= layout.y
                    && y <= layout.y + layout.height
                {
                    // Use node_id as z-order (higher = later = on top)
                    let z = node_id as f32;
                    if found.is_none() || z > found.unwrap().1 {
                        found = Some((node_id, z));
                    }
                }
            }
        }

        found.map(|(id, _)| id)
    }

    /// Render the entire tree to a Vello scene
    pub fn render_tree(&mut self, scene: &mut Scene, tree: &RenderTree) {
        // Render root nodes
        for &root_id in tree.roots() {
            self.render_node(scene, tree, root_id);
        }
    }

    /// Render a single node and its children
    ///
    /// Layouts are stored with absolute positions, so no offset accumulation needed.
    fn render_node(
        &mut self,
        scene: &mut Scene,
        tree: &RenderTree,
        node_id: NodeId,
    ) {
        let Some(node) = tree.get(node_id) else {
            return;
        };

        // Skip invisible nodes
        if !node.is_visible() {
            return;
        }

        // Get layout with absolute position
        let layout = self.layouts.get(&node_id).copied().unwrap_or_default();
        let x = layout.x;
        let y = layout.y;

        match node {
            RenderNode::Container { background, .. } => {
                // Draw background if specified
                if let Some(bg) = background {
                    let rect = Rect::new(
                        x as f64,
                        y as f64,
                        (x + layout.width) as f64,
                        (y + layout.height) as f64,
                    );
                    scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        &Brush::Solid(color_to_peniko(*bg)),
                        None,
                        &rect,
                    );
                }

                // Render children
                for &child_id in tree.children(node_id) {
                    self.render_node(scene, tree, child_id);
                }
            }

            RenderNode::Text {
                content,
                font_size,
                color,
                ..
            } => {
                self.render_text(scene, content, x, y, *font_size, *color);
            }

            RenderNode::Heading {
                level,
                content,
                ..
            } => {
                let font_size = match level {
                    1 => 32.0,
                    2 => 24.0,
                    3 => 20.0,
                    4 => 18.0,
                    _ => 16.0,
                };
                self.render_text(scene, content, x, y, font_size, self.default_text_color);
            }

            RenderNode::Button { label, enabled, .. } => {
                self.render_button(scene, label, x, y, layout.width, layout.height, *enabled);
            }

            RenderNode::Input {
                value, placeholder, ..
            } => {
                let display_text = if value.is_empty() { placeholder } else { value };
                self.render_input(scene, display_text, x, y, layout.width, layout.height, value.is_empty());
            }

            RenderNode::TextArea {
                value, placeholder, ..
            } => {
                let display_text = if value.is_empty() { placeholder } else { value };
                self.render_input(scene, display_text, x, y, layout.width, layout.height, value.is_empty());
            }

            RenderNode::Checkbox {
                label,
                checked,
                ..
            } => {
                self.render_checkbox(scene, label, *checked, x, y);
            }

            RenderNode::Select {
                options,
                selected,
                ..
            } => {
                let label = options
                    .iter()
                    .find(|o| &o.value == selected)
                    .map(|o| o.label.as_str())
                    .unwrap_or("Select...");
                self.render_select(scene, label, x, y, layout.width, layout.height);
            }

            RenderNode::Loop { .. } => {
                // Loop container - render children (loop items are expanded as children)
                for &child_id in tree.children(node_id) {
                    self.render_node(scene, tree, child_id);
                }
            }

            RenderNode::Image { .. } => {
                // TODO: Image rendering requires loading textures
                // For now, draw a placeholder
                self.render_placeholder(scene, x, y, layout.width, layout.height, "Image");
            }

            RenderNode::Divider { orientation, .. } => {
                self.render_divider(scene, x, y, layout.width, layout.height, *orientation);
            }

            RenderNode::Spacer { .. } => {
                // Spacer is invisible
            }
        }
    }

    /// Render text at position using parley for layout and vello for rendering.
    fn render_text(
        &mut self,
        scene: &mut Scene,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: Color,
    ) {
        if text.is_empty() {
            return;
        }

        let peniko_color = color_to_peniko(color);
        let text_brush = TextBrush(peniko_color);
        let scale = 1.0; // Display scale

        // Build text layout using parley
        // Args: font_cx, text, scale, quantize (false = no pixel snapping)
        let mut builder = self.layout_cx.ranged_builder(&mut self.font_cx, text, scale, false);

        // Set default styles
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::Brush(text_brush));
        builder.push_default(GenericFamily::SansSerif);

        // Build the layout
        let mut layout: Layout<TextBrush> = builder.build(text);
        layout.break_all_lines(None); // No max width - single line
        layout.align(None, parley::Alignment::Start, parley::AlignmentOptions::default());

        // Render glyphs to scene
        for line in layout.lines() {
            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                    let run = glyph_run.run();
                    let font = run.font();
                    let run_font_size = run.font_size();
                    let brush = &glyph_run.style().brush;

                    // Get glyph run starting position
                    // offset() is the x position of the run start
                    // baseline() is the y baseline position
                    let mut gx = x + glyph_run.offset();
                    let gy = y + glyph_run.baseline();

                    // Collect glyphs with correct positions
                    let glyphs: Vec<vello::Glyph> = glyph_run
                        .glyphs()
                        .map(|glyph| {
                            let glyph_x = gx + glyph.x;
                            let glyph_y = gy - glyph.y;
                            gx += glyph.advance;
                            vello::Glyph {
                                id: glyph.id as u32,
                                x: glyph_x,
                                y: glyph_y,
                            }
                        })
                        .collect();

                    // Draw glyphs
                    scene
                        .draw_glyphs(font)
                        .brush(&Brush::Solid(brush.0))
                        .font_size(run_font_size)
                        .transform(Affine::IDENTITY)
                        .draw(Fill::NonZero, glyphs.into_iter());
                }
            }
        }
    }

    /// Render a button
    fn render_button(
        &mut self,
        scene: &mut Scene,
        label: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        enabled: bool,
    ) {
        let bg_color = if enabled {
            self.button_background
        } else {
            Color::rgb(0.7, 0.7, 0.7)
        };

        // Button background
        let rect = RoundedRect::new(
            x as f64,
            y as f64,
            (x + width) as f64,
            (y + height) as f64,
            self.corner_radius,
        );
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(color_to_peniko(bg_color)),
            None,
            &rect,
        );

        // Button text (centered)
        let text_x = x + (width - label.len() as f32 * 8.0) / 2.0;
        let text_y = y + (height - 16.0) / 2.0;
        self.render_text(scene, label, text_x, text_y, 14.0, self.button_text_color);
    }

    /// Render an input field
    fn render_input(
        &mut self,
        scene: &mut Scene,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        is_placeholder: bool,
    ) {
        // Input background
        let rect = RoundedRect::new(
            x as f64,
            y as f64,
            (x + width) as f64,
            (y + height) as f64,
            self.corner_radius,
        );
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(color_to_peniko(self.input_background)),
            None,
            &rect,
        );

        // Input border
        scene.stroke(
            &vello::kurbo::Stroke::new(1.0),
            Affine::IDENTITY,
            &Brush::Solid(color_to_peniko(self.input_border_color)),
            None,
            &rect,
        );

        // Input text
        let text_color = if is_placeholder {
            Color::rgb(0.6, 0.6, 0.6)
        } else {
            self.default_text_color
        };
        let text_x = x + self.default_padding;
        let text_y = y + (height - 14.0) / 2.0;
        self.render_text(scene, text, text_x, text_y, 14.0, text_color);
    }

    /// Render a checkbox
    fn render_checkbox(&mut self, scene: &mut Scene, label: &str, checked: bool, x: f32, y: f32) {
        let box_size = 18.0;

        // Checkbox box
        let rect = RoundedRect::new(
            x as f64,
            y as f64,
            (x + box_size) as f64,
            (y + box_size) as f64,
            2.0,
        );

        // Background
        let bg_color = if checked {
            self.button_background
        } else {
            self.input_background
        };
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(color_to_peniko(bg_color)),
            None,
            &rect,
        );

        // Border
        scene.stroke(
            &vello::kurbo::Stroke::new(1.0),
            Affine::IDENTITY,
            &Brush::Solid(color_to_peniko(self.input_border_color)),
            None,
            &rect,
        );

        // Checkmark (simple)
        if checked {
            // Draw a simple checkmark using lines
            let check_color = Brush::Solid(color_to_peniko(Color::WHITE));
            let mut path = vello::kurbo::BezPath::new();
            path.move_to(Point::new((x + 4.0) as f64, (y + 9.0) as f64));
            path.line_to(Point::new((x + 7.0) as f64, (y + 13.0) as f64));
            path.line_to(Point::new((x + 14.0) as f64, (y + 5.0) as f64));
            scene.stroke(
                &vello::kurbo::Stroke::new(2.0),
                Affine::IDENTITY,
                &check_color,
                None,
                &path,
            );
        }

        // Label
        self.render_text(
            scene,
            label,
            x + box_size + 8.0,
            y + 2.0,
            14.0,
            self.default_text_color,
        );
    }

    /// Render a select dropdown
    fn render_select(
        &mut self,
        scene: &mut Scene,
        label: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        // Similar to input but with dropdown indicator
        self.render_input(scene, label, x, y, width, height, false);

        // Dropdown arrow
        let arrow_x = x + width - 20.0;
        let arrow_y = y + height / 2.0;
        let mut path = vello::kurbo::BezPath::new();
        path.move_to(Point::new((arrow_x - 4.0) as f64, (arrow_y - 2.0) as f64));
        path.line_to(Point::new(arrow_x as f64, (arrow_y + 2.0) as f64));
        path.line_to(Point::new((arrow_x + 4.0) as f64, (arrow_y - 2.0) as f64));
        scene.stroke(
            &vello::kurbo::Stroke::new(1.5),
            Affine::IDENTITY,
            &Brush::Solid(color_to_peniko(self.default_text_color)),
            None,
            &path,
        );
    }

    /// Render a divider line
    fn render_divider(
        &mut self,
        scene: &mut Scene,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        orientation: crate::render_tree::Orientation,
    ) {
        let color = Brush::Solid(color_to_peniko(Color::rgb(0.8, 0.8, 0.8)));

        match orientation {
            crate::render_tree::Orientation::Horizontal => {
                let mut path = vello::kurbo::BezPath::new();
                path.move_to(Point::new(x as f64, (y + height / 2.0) as f64));
                path.line_to(Point::new((x + width) as f64, (y + height / 2.0) as f64));
                scene.stroke(
                    &vello::kurbo::Stroke::new(1.0),
                    Affine::IDENTITY,
                    &color,
                    None,
                    &path,
                );
            }
            crate::render_tree::Orientation::Vertical => {
                let mut path = vello::kurbo::BezPath::new();
                path.move_to(Point::new((x + width / 2.0) as f64, y as f64));
                path.line_to(Point::new((x + width / 2.0) as f64, (y + height) as f64));
                scene.stroke(
                    &vello::kurbo::Stroke::new(1.0),
                    Affine::IDENTITY,
                    &color,
                    None,
                    &path,
                );
            }
        }
    }

    /// Render a placeholder for unimplemented features
    fn render_placeholder(
        &mut self,
        scene: &mut Scene,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        label: &str,
    ) {
        // Gray background
        let rect = Rect::new(
            x as f64,
            y as f64,
            (x + width) as f64,
            (y + height) as f64,
        );
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(color_to_peniko(Color::rgb(0.9, 0.9, 0.9))),
            None,
            &rect,
        );

        // Border
        scene.stroke(
            &vello::kurbo::Stroke::new(1.0),
            Affine::IDENTITY,
            &Brush::Solid(color_to_peniko(Color::rgb(0.7, 0.7, 0.7))),
            None,
            &rect,
        );

        // Label
        let text_x = x + (width - label.len() as f32 * 8.0) / 2.0;
        let text_y = y + (height - 14.0) / 2.0;
        self.render_text(scene, label, text_x, text_y, 12.0, Color::rgb(0.5, 0.5, 0.5));
    }
}

/// Convert our Color to peniko Color
fn color_to_peniko(color: Color) -> PenikoColor {
    PenikoColor::new([color.r, color.g, color.b, color.a])
}
