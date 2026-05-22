//! Menu bar panel for the editor shell.

use egui::RichText;

use super::super::operations::command::{command_enabled, execute_shell_command};
use super::super::types::{InfernuxPalette, ShellUiState};
use super::super::widgets::buttons::ghost_button;
use crate::EditorShell;
use engine_editor::EditorCommand;
use engine_i18n::Translations;
/// Renders the main menu bar.

pub fn draw_menu_bar(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    ui.horizontal_centered(|ui| {
        command_menu(
            ui,
            shell,
            ui_state,
            "File",
            tr.tr("menu_file"),
            54.0,
            pal,
            tr,
        );
        command_menu(
            ui,
            shell,
            ui_state,
            "Edit",
            tr.tr("menu_edit"),
            54.0,
            pal,
            tr,
        );
        command_menu(
            ui,
            shell,
            ui_state,
            "Assets",
            tr.tr("menu_assets"),
            54.0,
            pal,
            tr,
        );
        ghost_button(ui, tr.tr("menu_gameobject"), 86.0, pal);
        ghost_button(ui, tr.tr("menu_component"), 86.0, pal);
        command_menu(
            ui,
            shell,
            ui_state,
            "Window",
            tr.tr("menu_window"),
            64.0,
            pal,
            tr,
        );
        ghost_button(ui, tr.tr("menu_help"), 54.0, pal);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let title = shell
                .project()
                .map(|project| project.name().to_owned())
                .unwrap_or_else(|| tr.tr("editor_untitled").to_owned());
            ui.label(RichText::new(title).size(12.0).color(pal.text));
            if ui_state.playing {
                ui.label(
                    RichText::new(tr.tr("editor_play_indicator"))
                        .size(11.0)
                        .strong()
                        .color(pal.play),
                );
            }
        });
    });
}

/// Renders a command menu with items.
pub fn command_menu(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    category: &str,
    label: &str,
    width: f32,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    let commands = shell
        .commands()
        .commands()
        .filter(|command| command.category == category)
        .cloned()
        .collect::<Vec<_>>();
    if commands.is_empty() {
        ghost_button(ui, label, width, pal);
        return;
    }

    ui.menu_button(RichText::new(label).size(12.0).color(pal.text), |ui| {
        for command in &commands {
            command_menu_item(ui, shell, ui_state, command, tr);
        }
    });
}

/// Renders a single command menu item.
pub fn command_menu_item(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    command: &EditorCommand,
    tr: &Translations,
) {
    let enabled = command_enabled(shell, ui_state, command);
    let text = match command.shortcut.as_deref() {
        Some(shortcut) => format!("{}\t{}", command.label, shortcut),
        None => command.label.clone(),
    };
    if ui
        .add_enabled(enabled, egui::Button::new(text).frame(false))
        .clicked()
    {
        execute_shell_command(shell, ui_state, &command.id, tr);
        ui.close();
    }
}
