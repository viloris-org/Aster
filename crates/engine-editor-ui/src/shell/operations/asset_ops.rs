//! Asset file operations for the editor shell.

use crate::EditorShell;
use egui::Color32;
use engine_assets::{AssetGuid, ResourceKind, ResourceState};
use engine_core::EngineResult;
use engine_i18n::Translations;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::super::types::{rgb, ScriptEditorState, ScriptTemplateBackend, ShellUiState};
use super::command::push_error;

/// Open an asset in the appropriate application.
pub fn open_asset(
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    _guid: AssetGuid,
    kind: ResourceKind,
    relative_path: &Path,
    tr: &Translations,
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
            open_script_editor(shell, ui_state, tr, relative_path, &abs_path);
        }
        _ => {
            open_path(&abs_path);
        }
    }
}

fn open_script_editor(
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    tr: &Translations,
    relative_path: &Path,
    abs_path: &Path,
) {
    match fs::read_to_string(abs_path) {
        Ok(source) => {
            ui_state.script_editor = Some(ScriptEditorState {
                relative_path: relative_path.to_path_buf(),
                source,
                dirty: false,
                status: None,
            });
        }
        Err(source) => push_error(
            shell,
            tr.tr_fmt(
                "error_failed_open_script",
                &[&abs_path.display().to_string(), &source.to_string()],
            ),
        ),
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
            tr.tr_fmt(
                "error_failed_delete_asset",
                &[&abs_path.display().to_string()],
            ),
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
    open_path(abs_path.parent().unwrap_or(Path::new(".")));
}

fn open_path(path: &Path) {
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd")
        .args(["/C", "start", "", &path.display().to_string()])
        .spawn();

    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(path).spawn();

    #[cfg(all(unix, not(target_os = "macos")))]
    let result = Command::new("xdg-open").arg(path).spawn();

    let _ = result;
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

/// Create a script asset from the Project panel template controls.
pub fn create_script_asset(
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    tr: &Translations,
) {
    let Some(project) = shell.project_mut() else {
        return;
    };
    let backend = ui_state.project_new_script_backend;
    let file_name = script_file_name(&ui_state.project_new_script_name, backend);
    let relative_path = PathBuf::from("scripts").join(file_name);
    let asset_root = project.root.join(&project.manifest.asset_root);
    let script_dir = asset_root.join("scripts");
    let script_path = asset_root.join(&relative_path);

    let result: EngineResult<()> = (|| {
        fs::create_dir_all(&script_dir).map_err(|source| engine_core::EngineError::Filesystem {
            path: script_dir.clone(),
            source,
        })?;
        if script_path.exists() {
            return Err(engine_core::EngineError::other(format!(
                "Script already exists: {}",
                relative_path.display()
            )));
        }
        fs::write(&script_path, script_template(backend)).map_err(|source| {
            engine_core::EngineError::Filesystem {
                path: script_path.clone(),
                source,
            }
        })?;
        project.rescan_assets()
    })();

    match result {
        Ok(()) => {
            ui_state.project_import_status = Some(tr.tr_fmt(
                "project_script_created",
                &[&relative_path.display().to_string()],
            ));
            ui_state.script_editor = Some(ScriptEditorState {
                relative_path,
                source: script_template(backend).to_owned(),
                dirty: false,
                status: Some(tr.tr("script_editor_created").to_owned()),
            });
        }
        Err(error) => push_error(shell, error.to_string()),
    }
}

fn script_file_name(input: &str, backend: ScriptTemplateBackend) -> String {
    let mut stem = input.trim().replace('\\', "/");
    if let Some(last) = stem.rsplit('/').next() {
        stem = last.to_owned();
    }
    let stem = stem
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let stem = stem.trim_matches('.').trim_matches('_');
    let stem = if stem.is_empty() { "new_script" } else { stem };
    let extension = backend.extension();
    if stem
        .rsplit_once('.')
        .map_or(false, |(_, ext)| ext.eq_ignore_ascii_case(extension))
    {
        stem.to_owned()
    } else {
        format!("{stem}.{extension}")
    }
}

fn script_template(backend: ScriptTemplateBackend) -> &'static str {
    match backend {
        ScriptTemplateBackend::Python => {
            "def start(ctx):\n    ctx.state[\"started\"] = True\n\n\ndef update(ctx):\n    speed = 2.0\n    ctx.transform.translation.x += ctx.input.action_value(\"MoveX\") * speed * ctx.dt\n    ctx.transform.translation.z += ctx.input.action_value(\"MoveY\") * speed * ctx.dt\n\n\ndef fixed_update(ctx):\n    pass\n"
        }
        ScriptTemplateBackend::Rhai => {
            "fn on_start() {\n    print(\"script started\");\n}\n\nfn on_update(dt) {\n    let speed = 2.0;\n    translate(axis(\"MoveX\") * speed * dt, 0.0, axis(\"MoveY\") * speed * dt);\n}\n\nfn on_fixed_update(fixed_dt) {\n}\n"
        }
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
