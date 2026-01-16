//! Log viewer panel

use eframe::egui;
use crate::LogEntry;

#[derive(Default)]
pub struct LogPanel {
    filter_level: LogLevel,
    filter_text: String,
    auto_scroll: bool,
}

#[derive(Default, Debug, PartialEq, Clone, Copy)]
enum LogLevel {
    #[default]
    All,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogPanel {
    pub fn ui(&mut self, ui: &mut egui::Ui, logs: &[LogEntry]) {
        // Filter controls
        ui.horizontal(|ui| {
            ui.label("Level:");
            egui::ComboBox::from_label("")
                .selected_text(format!("{:?}", self.filter_level))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filter_level, LogLevel::All, "All");
                    ui.selectable_value(&mut self.filter_level, LogLevel::Error, "Error");
                    ui.selectable_value(&mut self.filter_level, LogLevel::Warn, "Warn");
                    ui.selectable_value(&mut self.filter_level, LogLevel::Info, "Info");
                    ui.selectable_value(&mut self.filter_level, LogLevel::Debug, "Debug");
                    ui.selectable_value(&mut self.filter_level, LogLevel::Trace, "Trace");
                });

            ui.separator();
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.filter_text);

            ui.separator();
            ui.checkbox(&mut self.auto_scroll, "Auto-scroll");
        });

        ui.separator();

        // Log entries
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(self.auto_scroll)
            .show(ui, |ui| {
                for entry in logs.iter().filter(|e| self.matches_filter(e)) {
                    self.render_log_entry(ui, entry);
                }
            });
    }

    fn matches_filter(&self, entry: &LogEntry) -> bool {
        // Level filter
        let level_ok = match self.filter_level {
            LogLevel::All => true,
            LogLevel::Error => entry.level == "error",
            LogLevel::Warn => entry.level == "warn" || entry.level == "error",
            LogLevel::Info => ["info", "warn", "error"].contains(&entry.level.as_str()),
            LogLevel::Debug => ["debug", "info", "warn", "error"].contains(&entry.level.as_str()),
            LogLevel::Trace => true,
        };

        // Text filter
        let text_ok = self.filter_text.is_empty()
            || entry.msg.to_lowercase().contains(&self.filter_text.to_lowercase())
            || entry.target.to_lowercase().contains(&self.filter_text.to_lowercase());

        level_ok && text_ok
    }

    fn render_log_entry(&self, ui: &mut egui::Ui, entry: &LogEntry) {
        let level_color = match entry.level.as_str() {
            "error" => egui::Color32::RED,
            "warn" => egui::Color32::YELLOW,
            "info" => egui::Color32::GREEN,
            "debug" => egui::Color32::LIGHT_BLUE,
            "trace" => egui::Color32::GRAY,
            _ => egui::Color32::WHITE,
        };

        ui.horizontal(|ui| {
            // Timestamp (shortened)
            let ts = if entry.ts.len() > 19 {
                &entry.ts[11..19]
            } else {
                &entry.ts
            };
            ui.label(egui::RichText::new(ts).monospace().color(egui::Color32::GRAY));

            // Level badge
            ui.label(egui::RichText::new(format!("[{}]", entry.level.to_uppercase()))
                .monospace()
                .color(level_color));

            // Target
            ui.label(egui::RichText::new(&entry.target).monospace().color(egui::Color32::DARK_GRAY));

            // Message
            ui.label(egui::RichText::new(&entry.msg).monospace());
        });
    }
}
