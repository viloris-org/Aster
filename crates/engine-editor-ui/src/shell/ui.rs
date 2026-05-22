//! egui rendering for [`EditorShell`].
//!
//! Call [`draw_shell`] once per frame inside an egui context.

#![allow(deprecated)] // egui 0.34 keeps Panel::show(ctx) available.

use egui::{Frame, Margin};

use super::operations::command::{apply_visuals, handle_command_shortcuts};
use super::panels::{
    draw_bottom_dock, draw_center_dock, draw_close_project_dialog, draw_command_palette,
    draw_hierarchy, draw_inspector, draw_menu_bar, draw_status_bar, draw_toolbar,
};
use super::types::{InfernuxPalette, ShellUiState};
use super::widgets::layout::panel_frame;
use crate::EditorShell;
use engine_i18n::Translations;

/// Draw the full editor shell into `ctx`.
///
/// Returns `true` when the user requests the window to close.
pub fn draw_shell(
    ctx: &egui::Context,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
) -> bool {
    let pal = InfernuxPalette::dark();
    let tr = Translations::load(shell.preferences().locale);
    apply_visuals(ctx, &pal);
    handle_command_shortcuts(ctx, shell, ui_state, &tr);

    let close = false;

    // Top menu bar
    egui::TopBottomPanel::top("infernux_menu_bar")
        .exact_size(26.0)
        .frame(
            Frame::NONE
                .fill(pal.menu_bar)
                .inner_margin(Margin::symmetric(8, 0)),
        )
        .show(ctx, |ui| draw_menu_bar(ui, shell, ui_state, &pal, &tr));

    // Toolbar
    egui::TopBottomPanel::top("infernux_toolbar")
        .exact_size(36.0)
        .frame(
            Frame::NONE
                .fill(pal.panel_bg)
                .inner_margin(Margin::symmetric(6, 3)),
        )
        .show(ctx, |ui| draw_toolbar(ui, shell, ui_state, &pal, &tr));

    // Status bar
    egui::TopBottomPanel::bottom("infernux_status_bar")
        .exact_size(24.0)
        .frame(
            Frame::NONE
                .fill(pal.status_bar)
                .inner_margin(Margin::symmetric(8, 0)),
        )
        .show(ctx, |ui| draw_status_bar(ui, shell, ui_state, &pal, &tr));

    // Left hierarchy panel
    if ui_state.show_hierarchy {
        egui::SidePanel::left("infernux_hierarchy")
            .default_size(260.0)
            .min_width(180.0)
            .frame(panel_frame(&pal))
            .show(ctx, |ui| {
                draw_hierarchy(ui, shell, ui_state, &pal, &tr);
            });
    }

    // Right inspector panel
    if ui_state.show_inspector {
        egui::SidePanel::right("infernux_inspector")
            .default_size(330.0)
            .min_width(240.0)
            .frame(panel_frame(&pal))
            .show(ctx, |ui| {
                draw_inspector(ui, shell, ui_state, &pal, &tr);
            });
    }

    // Bottom dock (project + console)
    if ui_state.show_project || ui_state.show_console {
        egui::TopBottomPanel::bottom("infernux_bottom_dock")
            .default_height(230.0)
            .min_height(120.0)
            .frame(panel_frame(&pal))
            .show(ctx, |ui| draw_bottom_dock(ui, shell, ui_state, &pal, &tr));
    }

    // Center viewport
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(pal.window_bg))
        .show(ctx, |ui| draw_center_dock(ui, shell, ui_state, &pal, &tr));

    // Dialogs
    if ui_state.command_palette_open {
        draw_command_palette(ctx, shell, ui_state, &pal, &tr);
    }

    if ui_state.show_close_dialog {
        draw_close_project_dialog(ctx, shell, ui_state, &pal, &tr);
    }

    close
}

// Re-export build_editor_render_world (used by other modules)
pub use super::panels::viewport::build_editor_render_world;
