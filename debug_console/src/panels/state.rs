//! State inspector panel

use eframe::egui;
use serde_json::Value as JsonValue;

#[derive(Default)]
pub struct StatePanel {
    expanded_paths: std::collections::HashSet<String>,
}

impl StatePanel {
    pub fn ui(&mut self, ui: &mut egui::Ui, state: &Option<JsonValue>) {
        match state {
            Some(value) => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        self.render_value(ui, value, "", 0);
                    });
            }
            None => {
                ui.label("No state loaded. Click 'Refresh' to fetch state.");
            }
        }
    }

    fn render_value(&mut self, ui: &mut egui::Ui, value: &JsonValue, path: &str, depth: usize) {
        let indent = "  ".repeat(depth);

        match value {
            JsonValue::Null => {
                ui.label(format!("{}null", indent));
            }
            JsonValue::Bool(b) => {
                ui.label(egui::RichText::new(format!("{}{}", indent, b))
                    .color(egui::Color32::LIGHT_BLUE));
            }
            JsonValue::Number(n) => {
                ui.label(egui::RichText::new(format!("{}{}", indent, n))
                    .color(egui::Color32::LIGHT_GREEN));
            }
            JsonValue::String(s) => {
                ui.label(egui::RichText::new(format!("{}\"{}\"", indent, s))
                    .color(egui::Color32::GOLD));
            }
            JsonValue::Array(arr) => {
                let expanded = self.expanded_paths.contains(path);
                let header = format!("{}Array [{} items]", indent, arr.len());

                if ui.selectable_label(expanded, egui::RichText::new(&header).monospace()).clicked() {
                    if expanded {
                        self.expanded_paths.remove(path);
                    } else {
                        self.expanded_paths.insert(path.to_string());
                    }
                }

                if expanded {
                    for (i, item) in arr.iter().enumerate() {
                        let item_path = format!("{}[{}]", path, i);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("{}[{}]:", indent, i))
                                .color(egui::Color32::GRAY));
                        });
                        self.render_value(ui, item, &item_path, depth + 1);
                    }
                }
            }
            JsonValue::Object(obj) => {
                let expanded = self.expanded_paths.contains(path) || depth == 0;
                let header = format!("{}Object {{{} keys}}", indent, obj.len());

                if depth > 0 {
                    if ui.selectable_label(expanded, egui::RichText::new(&header).monospace()).clicked() {
                        if expanded {
                            self.expanded_paths.remove(path);
                        } else {
                            self.expanded_paths.insert(path.to_string());
                        }
                    }
                }

                if expanded {
                    for (key, val) in obj.iter() {
                        let key_path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{}.{}", path, key)
                        };

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("{}{}:", indent, key))
                                .color(egui::Color32::LIGHT_BLUE)
                                .monospace());

                            // Show inline for simple values
                            match val {
                                JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) | JsonValue::String(_) => {
                                    self.render_inline_value(ui, val);
                                }
                                _ => {}
                            }
                        });

                        // Nested objects/arrays get their own line
                        match val {
                            JsonValue::Array(_) | JsonValue::Object(_) => {
                                self.render_value(ui, val, &key_path, depth + 1);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn render_inline_value(&self, ui: &mut egui::Ui, value: &JsonValue) {
        match value {
            JsonValue::Null => {
                ui.label(egui::RichText::new("null").color(egui::Color32::GRAY));
            }
            JsonValue::Bool(b) => {
                ui.label(egui::RichText::new(b.to_string()).color(egui::Color32::LIGHT_BLUE));
            }
            JsonValue::Number(n) => {
                ui.label(egui::RichText::new(n.to_string()).color(egui::Color32::LIGHT_GREEN));
            }
            JsonValue::String(s) => {
                let display = if s.len() > 50 {
                    format!("\"{}...\"", &s[..47])
                } else {
                    format!("\"{}\"", s)
                };
                ui.label(egui::RichText::new(display).color(egui::Color32::GOLD));
            }
            _ => {}
        }
    }
}
