//! Integration tests for editor panel operations.
//!
//! Tests panel visibility, play mode state, and undo/redo
//! through the EditorShell and ShellUiState.

use engine_editor::EditorPreferences;
use engine_editor_ui::{EditorShell, ShellUiState};

#[test]
fn shell_opens_with_all_core_panels_registered() {
    let shell = EditorShell::with_core_services(EditorPreferences::default());
    for id in [
        "hierarchy",
        "inspector",
        "project",
        "console",
        "scene_view",
        "game_view",
    ] {
        assert!(shell.panels().get(id).is_some(), "missing core panel: {id}");
    }
}

#[test]
fn shell_opens_with_all_core_commands_registered() {
    let shell = EditorShell::with_core_services(EditorPreferences::default());
    for id in [
        "play.toggle",
        "play.pause",
        "play.stop",
        "assets.reload",
        "scene.save",
        "project.build",
    ] {
        assert!(
            shell.commands().get(id).is_some(),
            "missing core command: {id}"
        );
    }
}

#[test]
fn all_open_sets_all_panels_visible() {
    let ui = ShellUiState::all_open();
    assert!(ui.show_scene_view);
    assert!(ui.show_game_view);
    assert!(ui.show_hierarchy);
    assert!(ui.show_inspector);
    assert!(ui.show_project);
    assert!(ui.show_console);
}

#[test]
fn default_shell_ui_has_panels_closed() {
    let ui = ShellUiState::default();
    assert!(!ui.show_hierarchy);
    assert!(!ui.show_inspector);
    assert!(!ui.show_project);
    assert!(!ui.show_console);
    assert!(!ui.show_scene_view);
    assert!(!ui.show_game_view);
}

#[test]
fn toggle_panel_visibility_via_mutation() {
    let mut ui = ShellUiState::default();

    // Start hidden
    assert!(!ui.show_console);

    // Toggle on
    ui.show_console = true;
    assert!(ui.show_console);

    // Toggle off
    ui.show_console = false;
    assert!(!ui.show_console);
}

#[test]
fn multiple_panels_independent_visibility() {
    let mut ui = ShellUiState::all_open();

    ui.show_hierarchy = false;
    ui.show_inspector = false;
    ui.show_scene_view = false;

    assert!(!ui.show_hierarchy);
    assert!(!ui.show_inspector);
    assert!(!ui.show_scene_view);
    assert!(ui.show_game_view, "game_view unaffected");
    assert!(ui.show_project, "project unaffected");
    assert!(ui.show_console, "console unaffected");
}

#[test]
fn play_mode_state_updates() {
    let mut ui = ShellUiState::all_open();

    assert!(!ui.playing, "starts not playing");
    assert!(!ui.paused, "starts not paused");

    ui.playing = true;
    assert!(ui.playing);
    assert!(!ui.paused);

    ui.paused = true;
    assert!(ui.paused);

    ui.playing = false;
    ui.paused = false;
    assert!(!ui.playing);
    assert!(!ui.paused);
}

#[test]
fn undo_redo_capabilities() {
    let shell = EditorShell::with_core_services(EditorPreferences::default());

    assert!(!shell.undo_stack().can_undo(), "no undo initially");
    assert!(!shell.undo_stack().can_redo(), "no redo initially");
}

#[test]
fn hierarchy_filter_starts_empty() {
    let ui = ShellUiState::all_open();
    assert!(ui.hierarchy_filter.is_empty());
}

#[test]
fn project_filter_starts_empty() {
    let ui = ShellUiState::all_open();
    assert!(ui.project_filter.is_empty());
}

#[test]
fn console_filter_starts_empty() {
    let ui = ShellUiState::all_open();
    assert!(ui.console_filter.is_empty());
}

#[test]
fn default_camera_has_reasonable_values() {
    let ui = ShellUiState::default();
    assert_eq!(ui.editor_camera_pitch, 0.3);
    assert_eq!(ui.editor_camera_distance, 6.0);
    assert_eq!(ui.editor_camera_target, [0.0, 1.0, 0.0]);
}
