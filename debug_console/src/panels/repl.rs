//! Lua REPL panel

use eframe::egui;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::connection::DebugConnection;

#[derive(Default)]
pub struct ReplPanel {
    input: String,
    history: Vec<ReplEntry>,
    history_index: Option<usize>,
}

struct ReplEntry {
    input: String,
    output: String,
    is_error: bool,
}

impl ReplPanel {
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rt: &Arc<tokio::runtime::Runtime>,
        connection: &Arc<RwLock<Option<DebugConnection>>>,
    ) {
        // History display
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .max_height(ui.available_height() - 60.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for entry in &self.history {
                    // Input line
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(">").monospace().color(egui::Color32::GREEN));
                        ui.label(egui::RichText::new(&entry.input).monospace());
                    });

                    // Output line
                    let output_color = if entry.is_error {
                        egui::Color32::RED
                    } else {
                        egui::Color32::LIGHT_GRAY
                    };
                    ui.label(egui::RichText::new(&entry.output).monospace().color(output_color));
                    ui.add_space(8.0);
                }
            });

        ui.separator();

        // Input line
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(">").monospace().color(egui::Color32::GREEN));

            let response = ui.add(
                egui::TextEdit::singleline(&mut self.input)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(ui.available_width() - 100.0)
                    .hint_text("Enter Lua code...")
            );

            // Handle enter key
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.execute_command(rt, connection);
            }

            // Handle up/down for history navigation
            if response.has_focus() {
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    self.navigate_history(-1);
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    self.navigate_history(1);
                }
            }

            if ui.button("Run").clicked() {
                self.execute_command(rt, connection);
            }
        });

        // Helper buttons
        ui.horizontal(|ui| {
            if ui.small_button("loro:get_list('products')").clicked() {
                self.input = "loro:get_list('products'):to_json()".to_string();
            }
            if ui.small_button("permit:role()").clicked() {
                self.input = "permit:role()".to_string();
            }
            if ui.small_button("permit:page_id()").clicked() {
                self.input = "permit:page_id()".to_string();
            }
        });
    }

    fn execute_command(
        &mut self,
        rt: &Arc<tokio::runtime::Runtime>,
        connection: &Arc<RwLock<Option<DebugConnection>>>,
    ) {
        let input = self.input.trim().to_string();
        if input.is_empty() {
            return;
        }

        let code = input.clone();
        self.input.clear();
        self.history_index = None;

        // Execute command
        let connection = connection.clone();
        let cmd = serde_json::json!({
            "method": "eval",
            "params": {"code": code}
        }).to_string();

        let (output, is_error) = rt.block_on(async {
            let mut conn_guard = connection.write().await;
            if let Some(conn) = conn_guard.as_mut() {
                match conn.send_command(&cmd).await {
                    Ok(response) => {
                        // Parse response
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                            if let Some(error) = parsed.get("error") {
                                (error.to_string(), true)
                            } else if let Some(result) = parsed.get("result") {
                                (serde_json::to_string_pretty(result).unwrap_or_default(), false)
                            } else {
                                (response, false)
                            }
                        } else {
                            (response, false)
                        }
                    }
                    Err(e) => (format!("Connection error: {}", e), true),
                }
            } else {
                ("Not connected".to_string(), true)
            }
        });

        self.history.push(ReplEntry {
            input: code,
            output,
            is_error,
        });
    }

    fn navigate_history(&mut self, delta: i32) {
        if self.history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None if delta < 0 => Some(self.history.len() - 1),
            None => None,
            Some(i) => {
                let new_i = i as i32 + delta;
                if new_i < 0 {
                    Some(0)
                } else if new_i >= self.history.len() as i32 {
                    None
                } else {
                    Some(new_i as usize)
                }
            }
        };

        self.history_index = new_index;
        if let Some(i) = new_index {
            self.input = self.history[i].input.clone();
        } else {
            self.input.clear();
        }
    }
}
