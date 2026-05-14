//! Canvas Bindings - Immediate-mode drawing commands
//!
//! Defines draw command types and color parsing for Raylib rendering.

use raylib::prelude::*;

/// Draw command types collected from Lua and executed by the renderer
#[derive(Debug, Clone)]
pub enum DrawCommand {
    Clear {
        color: Color,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Color,
    },
    Circle {
        x: f32,
        y: f32,
        r: f32,
        color: Color,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        color: Color,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Color,
    },
    // --- 3D commands ---
    /// Enter 3D mode with a camera. All Sphere3D/Line3D commands until EndMode3D
    /// are rendered in 3D space.
    BeginMode3D {
        // camera position
        cam_x: f32,
        cam_y: f32,
        cam_z: f32,
        // look-at target
        target_x: f32,
        target_y: f32,
        target_z: f32,
        // field-of-view in degrees
        fovy: f32,
    },
    EndMode3D,
    Sphere3D {
        x: f32,
        y: f32,
        z: f32,
        r: f32,
        color: Color,
    },
    Line3D {
        x1: f32,
        y1: f32,
        z1: f32,
        x2: f32,
        y2: f32,
        z2: f32,
        color: Color,
    },
    Cylinder3D {
        x1: f32,
        y1: f32,
        z1: f32,
        x2: f32,
        y2: f32,
        z2: f32,
        radius: f32,
        color: Color,
    },
}

/// Parse hex color string to Raylib Color
pub fn parse_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        let a = if hex.len() >= 8 {
            u8::from_str_radix(&hex[6..8], 16).unwrap_or(255)
        } else {
            255
        };
        Color::new(r, g, b, a)
    } else {
        Color::WHITE
    }
}
