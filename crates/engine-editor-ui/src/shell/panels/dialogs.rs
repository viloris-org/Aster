//! Dialog panels for the editor shell.

use egui::{Color32, RichText, Vec2};
use std::fs;

use super::super::operations::command::{execute_shell_command, push_error};
use super::super::types::{EditorAction, InfernuxPalette, ShellUiState};
use crate::EditorShell;
use engine_i18n::Translations;

/// Renders the close project confirmation dialog.
pub fn draw_close_project_dialog(
    ctx: &egui::Context,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    egui::Window::new(tr.tr("dialog_unsaved_changes_title"))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .auto_sized()
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(
                RichText::new(tr.tr("dialog_unsaved_changes_message"))
                    .size(13.0)
                    .color(pal.text),
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(tr.tr("dialog_save")).color(Color32::WHITE),
                        )
                        .fill(pal.accent)
                        .min_size(Vec2::new(80.0, 28.0)),
                    )
                    .clicked()
                {
                    let exit_app = ui_state.close_dialog_exit_app;
                    let after_close = ui_state.pending_action_after_close.take();
                    match shell.save_scene() {
                        Ok(_) => {
                            shell.close_project();
                            ui_state.show_close_dialog = false;
                            ui_state.pending_action = Some(after_close.unwrap_or_else(|| {
                                if exit_app {
                                    EditorAction::CloseWindow
                                } else {
                                    EditorAction::ReturnToHub
                                }
                            }));
                        }
                        Err(error) => push_error(shell, error.to_string()),
                    }
                }
                if ui
                    .add(
                        egui::Button::new(RichText::new(tr.tr("dialog_discard")).color(pal.text))
                            .fill(pal.frame_bg)
                            .min_size(Vec2::new(80.0, 28.0)),
                    )
                    .clicked()
                {
                    let exit_app = ui_state.close_dialog_exit_app;
                    let after_close = ui_state.pending_action_after_close.take();
                    shell.close_project();
                    ui_state.show_close_dialog = false;
                    ui_state.pending_action = Some(after_close.unwrap_or_else(|| {
                        if exit_app {
                            EditorAction::CloseWindow
                        } else {
                            EditorAction::ReturnToHub
                        }
                    }));
                }
                if ui
                    .add(
                        egui::Button::new(RichText::new(tr.tr("dialog_cancel")).color(pal.text))
                            .fill(pal.frame_bg)
                            .min_size(Vec2::new(80.0, 28.0)),
                    )
                    .clicked()
                {
                    ui_state.pending_action_after_close = None;
                    ui_state.show_close_dialog = false;
                }
            });
        });
}

/// Renders the command palette dialog for quick command access.
pub fn draw_command_palette(
    ctx: &egui::Context,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    use super::super::operations::command::command_enabled;

    let mut open = ui_state.command_palette_open;
    egui::Window::new(tr.tr("command_palette_title"))
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.add_sized(
                Vec2::new(ui.available_width(), 24.0),
                egui::TextEdit::singleline(&mut ui_state.command_filter)
                    .hint_text(tr.tr("command_palette_search"))
                    .font(egui::FontId::proportional(13.0))
                    .text_color(pal.text),
            );
            ui.add_space(6.0);
            let query = ui_state.command_filter.trim().to_lowercase();
            let commands = shell
                .commands()
                .commands()
                .filter(|command| {
                    query.is_empty()
                        || command.label.to_lowercase().contains(&query)
                        || command.id.to_lowercase().contains(&query)
                        || command.category.to_lowercase().contains(&query)
                })
                .cloned()
                .collect::<Vec<_>>();
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for command in &commands {
                        let enabled = command_enabled(shell, ui_state, command);
                        let shortcut = command.shortcut.as_deref().unwrap_or("");
                        let text = if shortcut.is_empty() {
                            format!("{}  /  {}", command.label, command.category)
                        } else {
                            format!(
                                "{}  /  {}  /  {}",
                                command.label, command.category, shortcut
                            )
                        };
                        if ui
                            .add_enabled(
                                enabled,
                                egui::Button::new(RichText::new(text).size(12.0))
                                    .fill(pal.frame_bg)
                                    .min_size(Vec2::new(ui.available_width(), 24.0)),
                            )
                            .clicked()
                        {
                            execute_shell_command(shell, ui_state, &command.id, tr);
                            ui_state.command_palette_open = false;
                        }
                    }
                });
            if let Some(status) = &ui_state.command_status {
                ui.label(RichText::new(status).size(11.0).color(pal.text_dim));
            }
        });
    ui_state.command_palette_open = open && ui_state.command_palette_open;
}

/// Renders the in-editor script source editor.
pub fn draw_script_editor(
    ctx: &egui::Context,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    let Some(editor) = ui_state.script_editor.as_mut() else {
        return;
    };

    let mut open = true;
    let mut save_requested = false;
    let mut reload_requested = false;
    egui::Window::new(tr.tr("script_editor_title"))
        .default_width(720.0)
        .default_height(520.0)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(editor.relative_path.display().to_string())
                        .size(12.0)
                        .color(pal.text_dim),
                );
                ui.add_space(8.0);
                if editor.dirty {
                    ui.label(RichText::new(tr.tr("script_editor_dirty")).color(pal.warning));
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new(tr.tr("script_editor_save")))
                            .fill(pal.accent)
                            .min_size(Vec2::new(72.0, 24.0)),
                    )
                    .clicked()
                {
                    save_requested = true;
                }
                if ui
                    .add(
                        egui::Button::new(RichText::new(tr.tr("script_editor_reload")))
                            .fill(pal.frame_bg)
                            .min_size(Vec2::new(72.0, 24.0)),
                    )
                    .clicked()
                {
                    reload_requested = true;
                }
                if let Some(status) = &editor.status {
                    ui.label(RichText::new(status).size(11.0).color(pal.text_dim));
                }
            });
            ui.add_space(6.0);
            let response = ui.add_sized(
                ui.available_size(),
                egui::TextEdit::multiline(&mut editor.source)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(24)
                    .lock_focus(true)
                    .code_editor()
                    .text_color(pal.text),
            );
            if response.changed() {
                editor.dirty = true;
                editor.status = None;
            }
        });

    if save_requested {
        save_script_editor(shell, ui_state, tr);
    }
    if reload_requested {
        reload_script_editor(shell, ui_state, tr);
    }
    if !open {
        ui_state.script_editor = None;
    }
}

fn save_script_editor(shell: &mut EditorShell, ui_state: &mut ShellUiState, tr: &Translations) {
    let Some(editor) = ui_state.script_editor.as_mut() else {
        return;
    };
    let Some(project) = shell.project_mut() else {
        return;
    };
    let path = project
        .root
        .join(&project.manifest.asset_root)
        .join(&editor.relative_path);
    match fs::write(&path, &editor.source) {
        Ok(()) => match project.rescan_assets() {
            Ok(()) => {
                editor.dirty = false;
                editor.status = Some(tr.tr("script_editor_saved").to_owned());
            }
            Err(error) => push_error(shell, error.to_string()),
        },
        Err(source) => push_error(
            shell,
            format!("Failed to save script {}: {source}", path.display()),
        ),
    }
}

fn reload_script_editor(shell: &mut EditorShell, ui_state: &mut ShellUiState, tr: &Translations) {
    let Some(editor) = ui_state.script_editor.as_mut() else {
        return;
    };
    let Some(project) = shell.project() else {
        return;
    };
    let path = project
        .root
        .join(&project.manifest.asset_root)
        .join(&editor.relative_path);
    match fs::read_to_string(&path) {
        Ok(source) => {
            editor.source = source;
            editor.dirty = false;
            editor.status = Some(tr.tr("script_editor_reloaded").to_owned());
        }
        Err(source) => push_error(
            shell,
            format!("Failed to reload script {}: {source}", path.display()),
        ),
    }
}
