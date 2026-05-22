//! Status bar panel for the editor shell.

use egui::{Rect, RichText, Sense, Vec2};

use super::super::types::{InfernuxPalette, ShellUiState};
use crate::EditorShell;
use engine_i18n::Translations;
/// Renders the status bar at the bottom of the editor.

pub fn draw_status_bar(
    ui: &mut egui::Ui,
    shell: &EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    if ui_state.status_toast_frames > 0 {
        ui_state.status_toast_frames -= 1;
        if ui_state.status_toast_frames == 0 {
            ui_state.status_toast = None;
        }
    }
    ui.horizontal_centered(|ui| {
        if let Some(toast) = &ui_state.status_toast {
            ui.label(RichText::new(toast).size(11.0).color(pal.accent));
        } else {
            let status = if ui_state.playing {
                tr.tr("status_play_mode")
            } else if shell.project().is_some() {
                tr.tr("status_ready")
            } else {
                tr.tr("status_no_project")
            };
            ui.label(RichText::new(status).size(11.0).color(pal.text_dim));
        }
        ui.separator();
        ui.label(
            RichText::new(tr.tr_fmt(
                "status_console_count",
                &[&shell.console().entries().len().to_string()],
            ))
            .size(11.0)
            .color(pal.text_dim),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(project) = shell.project() {
                if !project.asset_imports.is_empty() {
                    let rect = ui
                        .allocate_exact_size(Vec2::new(180.0, 5.0), Sense::hover())
                        .0;
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::same(0), pal.frame_bg);
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            rect.min,
                            Vec2::new(rect.width() * 0.35, rect.height()),
                        ),
                        egui::CornerRadius::same(0),
                        pal.accent,
                    );
                    ui.label(
                        RichText::new(tr.tr("status_asset_indexing"))
                            .size(11.0)
                            .color(pal.text_dim),
                    );
                }
            }
        });
    });
}
