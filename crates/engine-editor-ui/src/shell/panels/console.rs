//! Console panel for the editor shell.

use egui::{Align2, CornerRadius, FontId, Sense, Vec2};

use super::super::types::{InfernuxPalette, ShellUiState};
use super::super::widgets::buttons::small_chip;
use super::super::widgets::layout::{search_field, toolbar_row};
use super::super::widgets::text::paint_text_in_rect;
use crate::EditorShell;
use engine_editor::ConsoleLevel;
use engine_i18n::Translations;

/// Renders the console panel with log entries and filtering.
pub fn draw_console(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    toolbar_row(ui, pal, |ui| {
        if small_chip(ui, tr.tr("console_clear"), 54.0, pal).clicked() {
            shell.console_mut().clear();
        }
        if small_chip(
            ui,
            if ui_state.console_collapse {
                tr.tr("console_expanded")
            } else {
                tr.tr("console_collapse")
            },
            74.0,
            pal,
        )
        .clicked()
        {
            ui_state.console_collapse = !ui_state.console_collapse;
        }
        ui.add_space(6.0);
        search_field(
            ui,
            tr.tr("console_filter"),
            &mut ui_state.console_filter,
            pal,
        );
    });

    let query = ui_state.console_filter.trim().to_lowercase();
    let mut last_message = String::new();
    egui::ScrollArea::vertical()
        .id_salt("infernux_console_scroll")
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for (idx, entry) in shell.console().entries().iter().enumerate() {
                let row_text = format!("[{:?}] {}", entry.level, entry.message);
                if !query.is_empty() && !row_text.to_lowercase().contains(&query) {
                    continue;
                }
                if ui_state.console_collapse && row_text == last_message {
                    continue;
                }
                last_message = row_text.clone();
                let rect = ui
                    .allocate_exact_size(Vec2::new(ui.available_width(), 23.0), Sense::click())
                    .0;
                if idx % 2 == 0 {
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(0), pal.row_alt);
                }
                let color = match entry.level {
                    ConsoleLevel::Trace | ConsoleLevel::Debug => pal.text_dim,
                    ConsoleLevel::Info => pal.text,
                    ConsoleLevel::Warn => pal.warning,
                    ConsoleLevel::Error => pal.error,
                };
                paint_text_in_rect(
                    ui,
                    rect.shrink2(Vec2::new(8.0, 0.0)),
                    &row_text,
                    FontId::proportional(12.0),
                    color,
                    Align2::LEFT_CENTER,
                );
            }
        });
}
