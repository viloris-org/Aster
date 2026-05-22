//! Scene object operations for the editor shell.

use crate::{EditorShell, ProjectContext};
use engine_assets::AssetGuid;
use engine_core::EntityId;
use engine_ecs::{
    AudioSourceComponentData, CameraComponentData, ColliderComponentData, ComponentData,
    LightComponentData, MeshRendererComponentData, RigidbodyComponentData, ScriptComponentProxy,
};
use engine_editor::UndoCommand;

use super::command::push_error;

/// Capture a JSON snapshot of the current scene state for undo/redo.
pub fn scene_snapshot(shell: &EditorShell) -> Option<String> {
    shell
        .project()
        .and_then(|project| project.scene.to_json("Editor").ok())
}

/// Push a scene undo command if the scene state changed.
pub fn push_scene_undo(
    shell: &mut EditorShell,
    label: &str,
    target: String,
    before: Option<String>,
) {
    let Some(before) = before else {
        return;
    };
    if let Some(after) = scene_snapshot(shell) {
        if before != after {
            shell.push_undo(UndoCommand::new(label, target, before, after));
        }
    }
}

/// Reparent an object to a new parent (or root if parent_id is None).
pub fn reparent_object(shell: &mut EditorShell, child_id: EntityId, parent_id: Option<EntityId>) {
    let before = scene_snapshot(shell);
    let result = shell.project_mut().and_then(|project| {
        let child = project.scene.find_by_id(child_id)?;
        let parent = parent_id.and_then(|id| project.scene.find_by_id(id));
        match project.scene.set_parent(child, parent) {
            Ok(()) => {
                project.scene_dirty = true;
                Some(Ok(()))
            }
            Err(error) => Some(Err(error)),
        }
    });
    match result {
        Some(Ok(())) => push_scene_undo(
            shell,
            "Reparent Object",
            format!("{:032x}", child_id.as_u128()),
            before,
        ),
        Some(Err(error)) => push_error(shell, error.to_string()),
        None => {}
    }
}

/// Duplicate an object and all its components.
pub fn duplicate_object(shell: &mut EditorShell, id: EntityId) {
    let before = scene_snapshot(shell);
    let result = shell.project_mut().and_then(|project| {
        let entity = project.scene.find_by_id(id)?;
        match project.scene.clone_object(entity) {
            Ok(clone) => {
                project.scene_dirty = true;
                project.scene.object(clone).map(|object| Ok(object.id))
            }
            Err(error) => Some(Err(error)),
        }
    });
    match result {
        Some(Ok(cloned_id)) => {
            shell.select_entity_id(cloned_id);
            push_scene_undo(
                shell,
                "Duplicate Object",
                format!("{:032x}", id.as_u128()),
                before,
            );
        }
        Some(Err(error)) => push_error(shell, error.to_string()),
        None => {}
    }
}

/// Delete an object from the scene.
pub fn delete_object(shell: &mut EditorShell, id: EntityId) {
    let before = scene_snapshot(shell);
    let result = shell.project_mut().and_then(|project| {
        let entity = project.scene.find_by_id(id)?;
        match project
            .scene
            .destroy_deferred(entity)
            .and_then(|()| project.scene.process_deferred_destroy())
        {
            Ok(()) => {
                project.scene_dirty = true;
                Some(Ok(()))
            }
            Err(error) => Some(Err(error)),
        }
    });
    match result {
        Some(Ok(())) => {
            shell.selection_mut().clear();
            push_scene_undo(
                shell,
                "Delete Object",
                format!("{:032x}", id.as_u128()),
                before,
            );
        }
        Some(Err(error)) => push_error(shell, error.to_string()),
        None => {}
    }
}

/// Rename an object.
pub fn rename_object(
    shell: &mut EditorShell,
    entity: engine_ecs::Entity,
    id: EntityId,
    new_name: String,
) {
    let before = scene_snapshot(shell);
    let result = shell.project_mut().and_then(|project| {
        let object = project.scene.object_mut(entity)?;
        object.name = new_name;
        project.scene_dirty = true;
        Some(())
    });
    if result.is_some() {
        push_scene_undo(
            shell,
            "Rename Object",
            format!("{:032x}", id.as_u128()),
            before,
        );
    }
}

/// Create an empty root object.
pub fn create_empty_object(shell: &mut EditorShell) {
    let before = scene_snapshot(shell);
    let result = shell.project_mut().and_then(|project| {
        let name = format!("GameObject {}", project.scene.objects().len() + 1);
        let entity = project.scene.create_object(name).ok()?;
        project.scene_dirty = true;
        project.scene.object(entity).map(|object| object.id)
    });
    if let Some(id) = result {
        shell.select_entity_id(id);
        push_scene_undo(
            shell,
            "Create Empty Object",
            format!("{:032x}", id.as_u128()),
            before,
        );
    }
}

/// Create an empty child object under a parent.
pub fn create_empty_child(
    shell: &mut EditorShell,
    parent_entity: engine_ecs::Entity,
    parent_id: EntityId,
) {
    let before = scene_snapshot(shell);
    let result = shell.project_mut().and_then(|project| {
        let count = project.scene.objects().len();
        let name = format!("GameObject {count}");
        let child = project.scene.create_object(name).ok()?;
        project.scene.set_parent(child, Some(parent_entity)).ok()?;
        project.scene_dirty = true;
        project.scene.object(child).map(|o| o.id)
    });
    match result {
        Some(child_id) => {
            shell.select_entity_id(child_id);
            push_scene_undo(
                shell,
                "Create Empty Child",
                format!("{:032x}", parent_id.as_u128()),
                before,
            );
        }
        None => {}
    }
}

/// Create a root object with a specific component.
pub fn create_root_object_with_component(
    shell: &mut EditorShell,
    label: &str,
    component: ComponentData,
) {
    let before = scene_snapshot(shell);
    let result = shell.project_mut().and_then(|project| {
        let entity = project.scene.create_object(label).ok()?;
        project.scene.upsert_component(entity, component).ok()?;
        project.scene_dirty = true;
        project.scene.object(entity).map(|object| object.id)
    });
    if let Some(id) = result {
        shell.select_entity_id(id);
        push_scene_undo(
            shell,
            &format!("Create {label}"),
            format!("{:032x}", id.as_u128()),
            before,
        );
    }
}

/// Create an object with a specific component.
pub fn create_object_with_component(
    shell: &mut EditorShell,
    parent_entity: engine_ecs::Entity,
    label: &str,
    component: ComponentData,
) {
    let before = scene_snapshot(shell);
    let result = shell.project_mut().and_then(|project| {
        let name = format!("{label}");
        let child = project.scene.create_object(name).ok()?;
        project.scene.set_parent(child, Some(parent_entity)).ok()?;
        project.scene.upsert_component(child, component).ok()?;
        project.scene_dirty = true;
        project.scene.object(child).map(|o| o.id)
    });
    match result {
        Some(child_id) => {
            shell.select_entity_id(child_id);
            push_scene_undo(
                shell,
                &format!("Create {label}"),
                format!("{:032x}", child_id.as_u128()),
                before,
            );
        }
        None => {}
    }
}

/// Add or replace a component on the selected object.
pub fn add_component_to_selected(shell: &mut EditorShell, label: &str, component: ComponentData) {
    let Some(id) = shell.selected_entity_id() else {
        push_error(
            shell,
            "Select a GameObject before adding a component".to_owned(),
        );
        return;
    };
    let before = scene_snapshot(shell);
    let result = shell.project_mut().and_then(|project| {
        let entity = project.scene.find_by_id(id)?;
        match project.scene.upsert_component(entity, component) {
            Ok(()) => {
                project.scene_dirty = true;
                Some(Ok(()))
            }
            Err(error) => Some(Err(error)),
        }
    });
    match result {
        Some(Ok(())) => push_scene_undo(
            shell,
            &format!("Add {label}"),
            format!("{:032x}", id.as_u128()),
            before,
        ),
        Some(Err(error)) => push_error(shell, error.to_string()),
        None => push_error(shell, "Selected GameObject no longer exists".to_owned()),
    }
}

/// Create a GameObject from a dragged asset (drag-to-scene).
pub fn create_object_from_asset(
    shell: &mut EditorShell,
    guid: AssetGuid,
    kind: engine_assets::ResourceKind,
) -> Option<EntityId> {
    let before = scene_snapshot(shell);
    let asset_id = guid.as_asset_id();

    let name = shell
        .project()
        .and_then(|p| p.assets.iter().find(|a| a.guid == guid))
        .and_then(|a| a.source_path.file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("Asset")
        .to_owned();

    let result = {
        let project = shell.project_mut()?;
        match kind {
            engine_assets::ResourceKind::Model | engine_assets::ResourceKind::SkinnedModel => {
                let entity = project.scene.create_object(&name).ok()?;
                let mut renderer = MeshRendererComponentData::default();
                renderer.mesh = Some(asset_id);
                renderer.builtin_mesh = None;
                let _ = project
                    .scene
                    .upsert_component(entity, ComponentData::MeshRenderer(renderer));
                let id = project.scene.object(entity)?.id;
                project.scene_dirty = true;
                Some(id)
            }
            engine_assets::ResourceKind::Texture => {
                let entity = project.scene.create_object(&name).ok()?;
                let mut renderer = MeshRendererComponentData::default();
                renderer.material.asset = Some(asset_id);
                renderer.material.builtin = None;
                let _ = project
                    .scene
                    .upsert_component(entity, ComponentData::MeshRenderer(renderer));
                let id = project.scene.object(entity)?.id;
                project.scene_dirty = true;
                Some(id)
            }
            engine_assets::ResourceKind::Audio => {
                let entity = project.scene.create_object(&name).ok()?;
                let mut source = AudioSourceComponentData::default();
                source.clip = Some(asset_id);
                let _ = project
                    .scene
                    .upsert_component(entity, ComponentData::AudioSource(source));
                let id = project.scene.object(entity)?.id;
                project.scene_dirty = true;
                Some(id)
            }
            _ => None,
        }
    };

    if let Some(id) = result {
        let label = match kind {
            engine_assets::ResourceKind::Model | engine_assets::ResourceKind::SkinnedModel => {
                "Add Model from Asset"
            }
            engine_assets::ResourceKind::Texture => "Add Texture from Asset",
            engine_assets::ResourceKind::Audio => "Add Audio from Asset",
            _ => "Add Asset",
        };
        push_scene_undo(shell, label, format!("{:032x}", id.as_u128()), before);
    }

    result
}

/// Select the first scene object (Player if exists, otherwise first object).
pub fn select_first_scene_object(shell: &mut EditorShell) {
    if let Some(id) = shell.project().and_then(|project| {
        project
            .scene
            .find_by_name("Player")
            .and_then(|entity| project.scene.object(entity).map(|object| object.id))
            .or_else(|| {
                project
                    .scene
                    .objects()
                    .into_iter()
                    .next()
                    .map(|(_, object)| object.id)
            })
    }) {
        shell.select_entity_id(id);
    }
}

/// Assign an asset to an object (drag asset onto object).
pub fn assign_asset_to_object(
    project: &mut ProjectContext,
    entity: engine_ecs::Entity,
    guid: AssetGuid,
) -> bool {
    let Some(asset) = project
        .assets
        .iter()
        .find(|asset| asset.guid == guid)
        .cloned()
    else {
        return false;
    };
    match asset.kind {
        engine_assets::ResourceKind::Model | engine_assets::ResourceKind::SkinnedModel => {
            let mut renderer = project
                .scene
                .components(entity)
                .unwrap_or(&[])
                .iter()
                .find_map(|component| match component {
                    ComponentData::MeshRenderer(renderer) => Some(renderer.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            renderer.mesh = Some(guid.as_asset_id());
            renderer.builtin_mesh = None;
            project
                .scene
                .upsert_component(entity, ComponentData::MeshRenderer(renderer))
                .is_ok()
        }
        engine_assets::ResourceKind::Material => {
            let mut renderer = project
                .scene
                .components(entity)
                .unwrap_or(&[])
                .iter()
                .find_map(|component| match component {
                    ComponentData::MeshRenderer(renderer) => Some(renderer.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            renderer.material.asset = Some(guid.as_asset_id());
            renderer.material.builtin = None;
            project
                .scene
                .upsert_component(entity, ComponentData::MeshRenderer(renderer))
                .is_ok()
        }
        engine_assets::ResourceKind::Audio => {
            let mut source = project
                .scene
                .components(entity)
                .unwrap_or(&[])
                .iter()
                .find_map(|component| match component {
                    ComponentData::AudioSource(source) => Some(source.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            source.clip = Some(guid.as_asset_id());
            project
                .scene
                .upsert_component(entity, ComponentData::AudioSource(source))
                .is_ok()
        }
        _ => false,
    }
}

/// Get default components for the "Add Component" menu.
pub fn default_components() -> Vec<(&'static str, ComponentData)> {
    vec![
        (
            "Camera",
            ComponentData::Camera(CameraComponentData::default()),
        ),
        (
            "Mesh Renderer",
            ComponentData::MeshRenderer(MeshRendererComponentData::default()),
        ),
        ("Light", ComponentData::Light(LightComponentData::default())),
        (
            "Rigidbody",
            ComponentData::Rigidbody(RigidbodyComponentData::default()),
        ),
        (
            "Collider",
            ComponentData::Collider(ColliderComponentData::default()),
        ),
        (
            "Audio Source",
            ComponentData::AudioSource(AudioSourceComponentData::default()),
        ),
        (
            "Script",
            ComponentData::Script(ScriptComponentProxy {
                backend: "rhai".into(),
                script: String::new(),
                state_json: None,
                pending_recovery: false,
            }),
        ),
    ]
}
