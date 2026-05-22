//! Toolbar panel for the editor shell.

use egui::{Color32, RichText, Vec2};

use super::super::operations::command::{command_enabled, execute_shell_command};
use super::super::types::{InfernuxPalette, ShellUiState};
use super::super::widgets::buttons::{dropdown_pill, panel_toggle, small_text_button_widget};
use super::super::widgets::icons::transport;
use crate::EditorShell;
use engine_i18n::Translations;
/// Renders the toolbar with transport controls.

pub fn draw_toolbar(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    ui.horizontal_centered(|ui| {
        dropdown_pill(ui, tr.tr("tool_global"), 76.0, pal);
        dropdown_pill(ui, tr.tr("tool_pivot"), 68.0, pal);
        ui.add_space(12.0);
        if transport_command_button(
            ui,
            shell,
            ui_state,
            "play.toggle",
            transport::PLAY,
            pal.play,
            pal,
        )
        .clicked()
        {
            execute_shell_command(shell, ui_state, "play.toggle", tr);
        }
        if transport_command_button(
            ui,
            shell,
            ui_state,
            "play.pause",
            transport::PAUSE,
            pal.pause,
            pal,
        )
        .clicked()
        {
            execute_shell_command(shell, ui_state, "play.pause", tr);
        }
        if transport_command_button(
            ui,
            shell,
            ui_state,
            "play.stop",
            transport::STOP,
            pal.accent,
            pal,
        )
        .clicked()
        {
            execute_shell_command(shell, ui_state, "play.stop", tr);
        }
        ui.separator();

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            command_text_button(
                ui,
                shell,
                ui_state,
                "scene.save",
                tr.tr("tool_save"),
                pal,
                tr,
            );
            command_text_button(
                ui,
                shell,
                ui_state,
                "edit.redo",
                tr.tr("command_redo"),
                pal,
                tr,
            );
            command_text_button(
                ui,
                shell,
                ui_state,
                "edit.undo",
                tr.tr("command_undo"),
                pal,
                tr,
            );
            panel_toggle(
                ui,
                tr.tr("panel_game_view"),
                &mut ui_state.show_game_view,
                pal,
            );
            panel_toggle(
                ui,
                tr.tr("panel_scene_view"),
                &mut ui_state.show_scene_view,
                pal,
            );
            panel_toggle(ui, tr.tr("panel_console"), &mut ui_state.show_console, pal);
            panel_toggle(ui, tr.tr("panel_project"), &mut ui_state.show_project, pal);
            panel_toggle(
                ui,
                tr.tr("panel_inspector"),
                &mut ui_state.show_inspector,
                pal,
            );
            panel_toggle(
                ui,
                tr.tr("panel_hierarchy"),
                &mut ui_state.show_hierarchy,
                pal,
            );
        });
    });
}
/// Renders a transport control button (play/pause/stop).

pub fn transport_command_button(
    ui: &mut egui::Ui,
    shell: &EditorShell,
    ui_state: &ShellUiState,
    command_id: &str,
    icon: &str,
    active_color: Color32,
    pal: &InfernuxPalette,
) -> egui::Response {
    let active = match command_id {
        "play.toggle" => ui_state.playing,
        "play.pause" => ui_state.paused,
        _ => false,
    };
    let enabled = shell
        .commands()
        .get(command_id)
        .map(|command| command_enabled(shell, ui_state, command))
        .unwrap_or(false);

    let icon_text = RichText::new(icon).size(16.0).color(pal.text);

    ui.add_enabled(
        enabled,
        egui::Button::new(icon_text)
            .fill(if active { active_color } else { pal.frame_bg })
            .min_size(Vec2::new(32.0, 28.0)),
    )
}
/// Renders a text-based command button.

pub fn command_text_button(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    command_id: &str,
    fallback_label: &str,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    let command = shell.commands().get(command_id).cloned();
    let enabled = command
        .as_ref()
        .map(|command| command_enabled(shell, ui_state, command))
        .unwrap_or(false);
    let label = command
        .as_ref()
        .map(|command| command.label.as_str())
        .unwrap_or(fallback_label);
    if ui
        .add_enabled(enabled, small_text_button_widget(label, pal))
        .clicked()
    {
        execute_shell_command(shell, ui_state, command_id, tr);
    }
}
