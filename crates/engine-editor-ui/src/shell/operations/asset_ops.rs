//! Asset file operations for the editor shell.

use crate::EditorShell;
use egui::Color32;
use engine_assets::{AssetGuid, ResourceKind, ResourceState};
use engine_core::EngineResult;
use engine_i18n::Translations;
use std::fs;
use std::path::Path;

use super::super::types::{rgb, ShellUiState};
use super::command::push_error;

/// Open an asset in the appropriate application.
pub fn open_asset(
    shell: &mut EditorShell,
    _ui_state: &mut ShellUiState,
    _guid: AssetGuid,
    kind: ResourceKind,
    relative_path: &Path,
    _tr: &Translations,
) {
    let Some(project) = shell.project() else {
        return;
    };
    let abs_path = project
        .root
        .join(&project.manifest.asset_root)
        .join(relative_path);
    match kind {
        ResourceKind::Scene => {
            if let Err(error) = shell.load_scene(&abs_path) {
                push_error(shell, error.to_string());
            }
        }
        ResourceKind::Script => {
            let _ = std::process::Command::new("xdg-open")
                .arg(&abs_path)
                .spawn();
        }
        _ => {
            let _ = std::process::Command::new("xdg-open")
                .arg(&abs_path)
                .spawn();
        }
    }
}

/// Delete an asset file from disk and rescan.
pub fn delete_asset(
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    relative_path: &Path,
    tr: &Translations,
) {
    let Some(project) = shell.project_mut() else {
        return;
    };
    let abs_path = project
        .root
        .join(&project.manifest.asset_root)
        .join(relative_path);
    if std::fs::remove_file(&abs_path).is_ok() {
        if project.rescan_assets().is_ok() {
            shell.selection_mut().clear();
            ui_state.status_toast = Some(tr.tr("project_asset_deleted").to_owned());
            ui_state.status_toast_frames = 180;
        }
    } else {
        push_error(
            shell,
            format!("Failed to delete asset: {}", abs_path.display()),
        );
    }
}

/// Reimport an asset by marking it stale and rescanning.
pub fn reimport_asset(
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    relative_path: &Path,
    tr: &Translations,
) {
    let Some(project) = shell.project_mut() else {
        return;
    };
    if let Some(entry) = project.database.entries_mut().get_mut(relative_path) {
        entry.import_state = ResourceState::Stale;
    }
    if project.rescan_assets().is_ok() {
        ui_state.status_toast = Some(tr.tr("project_asset_reimported").to_owned());
        ui_state.status_toast_frames = 180;
    }
}

/// Show asset in the OS file manager.
pub fn show_in_file_manager(shell: &EditorShell, relative_path: &Path) {
    let Some(project) = shell.project() else {
        return;
    };
    let abs_path = project
        .root
        .join(&project.manifest.asset_root)
        .join(relative_path);
    let _ = std::process::Command::new("xdg-open")
        .arg(abs_path.parent().unwrap_or(Path::new(".")))
        .spawn();
}

/// Create a default material file in the project.
pub fn create_default_material(shell: &mut EditorShell) {
    let Some(project) = shell.project_mut() else {
        return;
    };
    let asset_root = project.root.join(&project.manifest.asset_root);
    let material_dir = asset_root.join("materials");
    let material_path = material_dir.join("new_material.material.json");
    let result: EngineResult<()> = (|| {
        fs::create_dir_all(&material_dir).map_err(|source| {
            engine_core::EngineError::Filesystem {
                path: material_dir.clone(),
                source,
            }
        })?;
        if !material_path.exists() {
            fs::write(
                &material_path,
                "{\n  \"version\": 1,\n  \"shader\": \"00000000000000000000000000000000\",\n  \"textures\": {},\n  \"parameters\": {}\n}\n",
            )
            .map_err(|source| engine_core::EngineError::Filesystem {
                path: material_path.clone(),
                source,
            })?;
        }
        project.rescan_assets()
    })();
    match result {
        Ok(()) => project
            .asset_imports
            .push(format!("created {}", material_path.display())),
        Err(error) => push_error(shell, error.to_string()),
    }
}

/// Get the thumbnail color for a resource kind.
pub fn thumbnail_color(kind: ResourceKind) -> Color32 {
    match kind {
        ResourceKind::Texture => rgb(91, 157, 245),
        ResourceKind::Material => rgb(235, 87, 87),
        ResourceKind::Shader => rgb(160, 130, 220),
        ResourceKind::Audio => rgb(113, 183, 139),
        ResourceKind::Model | ResourceKind::SkinnedModel => rgb(220, 167, 80),
        ResourceKind::Animation => rgb(120, 180, 190),
        ResourceKind::Script => rgb(180, 130, 200),
        ResourceKind::Scene => rgb(90, 170, 100),
    }
}
