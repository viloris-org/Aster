//! Scene command types for structured AI-driven scene editing.
//!
//! These types describe atomic scene mutations that an AI agent can request.
//! Each `SceneCommand` is validated and then converted into one or more
//! `ScenePatch` entries that are applied transactionally.
//!
//! The flow is:
//!
//! ```text
//! SceneCommand -> validation -> ScenePatch -> apply / undo
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use engine_ecs::patch::{SceneCommand, ScenePatch};
//!
//! // An agent requests a command
//! let cmd = SceneCommand::SetTransform {
//!     entity: some_entity,
//!     position: Vec3::new(0.0, 5.0, 0.0),
//!     rotation: None,
//!     scale: None,
//! };
//!
//! // Validate and convert to patches
//! let patches = cmd.validate(&scene)?;
//!
//! // Apply patches transactionally
//! let results = ScenePatch::apply_batch(&mut scene, &patches)?;
//! ```

use engine_core::{
    AssetId, EngineError, EngineResult, EntityId,
    math::{Quat, Transform, Vec3},
};

use crate::{ComponentData, scene::Scene, world::Entity};

/// A single AI-requested scene operation.
///
/// Commands are high-level — they describe *what* to do, not *how*.
/// `validate()` turns them into low-level [`ScenePatch`] entries with concrete
/// entity handles.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "op", content = "args")]
pub enum SceneCommand {
    /// Create a new entity with an optional parent.
    CreateEntity {
        /// Suggested name for the new entity.
        name: String,
        /// Optional parent entity ID. If absent, entity is created at root.
        #[serde(default)]
        parent: Option<EntityId>,
        /// Initial local position.
        #[serde(default)]
        position: Vec3,
        /// Initial local rotation (Euler angles in degrees, applied XYZ).
        #[serde(default)]
        rotation_degrees: Vec3,
        /// Initial local scale.
        #[serde(default = "default_scale")]
        scale: Vec3,
    },
    /// Delete an entity and its children.
    DeleteEntity {
        /// Entity to delete.
        entity: EntityId,
    },
    /// Rename an entity.
    RenameEntity {
        /// Entity to rename.
        entity: EntityId,
        /// New name.
        name: String,
    },
    /// Add a component to an entity.
    AddComponent {
        /// Target entity.
        entity: EntityId,
        /// Component type ID (e.g., `"Camera"`, `"Rigidbody"`, `"Light"`).
        component_type: String,
        /// Serialized component data as JSON.
        data: serde_json::Value,
    },
    /// Update (upsert) a component on an entity.
    UpdateComponent {
        /// Target entity.
        entity: EntityId,
        /// Component type ID.
        component_type: String,
        /// New serialized component data as JSON.
        data: serde_json::Value,
    },
    /// Remove a component from an entity.
    RemoveComponent {
        /// Target entity.
        entity: EntityId,
        /// Component type ID.
        component_type: String,
    },
    /// Set the transform of an entity.
    SetTransform {
        /// Target entity.
        entity: EntityId,
        /// New local position (optional — unchanged fields use `None`).
        #[serde(default)]
        position: Option<Vec3>,
        /// New local rotation in Euler degrees (optional).
        #[serde(default)]
        rotation_degrees: Option<Vec3>,
        /// New local scale (optional).
        #[serde(default)]
        scale: Option<Vec3>,
    },
    /// Reparent an entity.
    SetParent {
        /// Child entity.
        entity: EntityId,
        /// New parent entity (or `None` to detach to root).
        parent: Option<EntityId>,
    },
    /// Attach a script component.
    AttachScript {
        /// Target entity.
        entity: EntityId,
        /// Script backend name (e.g., `"python"`, `"rhai"`).
        backend: String,
        /// Script module path.
        script: String,
    },
    /// Attach a known asset as a component (e.g., material, prefab).
    AttachAsset {
        /// Target entity.
        entity: EntityId,
        /// Asset GUID.
        asset: AssetId,
        /// Asset kind hint for routing to the correct component type.
        #[serde(default)]
        kind: Option<String>,
    },
}

fn default_scale() -> Vec3 {
    Vec3::ONE
}

/// Validation outcome of a [`SceneCommand`].
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CommandValidation {
    /// Whether the command can be applied.
    pub is_valid: bool,
    /// Machine-readable error code or `"ok"`.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Warnings that don't block application.
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl CommandValidation {
    fn ok() -> Self {
        Self {
            is_valid: true,
            code: "ok".to_string(),
            message: String::new(),
            warnings: Vec::new(),
        }
    }

    fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            is_valid: false,
            code: code.into(),
            message: message.into(),
            warnings: Vec::new(),
        }
    }

    fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

/// A low-level, entity-resolved scene mutation.
///
/// Generated by [`SceneCommand::validate`]. Unlike commands, patches operate
/// on concrete entity handles and contain fully resolved data.
#[derive(Clone, Debug)]
pub enum ScenePatch {
    /// Spawn a new entity with metadata.
    SpawnEntity {
        /// Entity display name.
        name: String,
        /// Optional parent entity handle.
        parent: Option<Entity>,
        /// Initial transform (translation, rotation, scale).
        transform: Transform,
    },
    /// Despawn an entity by handle.
    DespawnEntity {
        /// Entity handle to despawn.
        entity: Entity,
    },
    /// Rename an entity.
    RenameEntity {
        /// Entity handle to rename.
        entity: Entity,
        /// New display name.
        name: String,
    },
    /// Upsert a serialized component.
    UpsertComponent {
        /// Target entity handle.
        entity: Entity,
        /// Component data to upsert.
        component: ComponentData,
    },
    /// Remove a component by type ID.
    RemoveComponent {
        /// Target entity handle.
        entity: Entity,
        /// Component type ID string (e.g. `"Camera"`, `"Rigidbody"`).
        component_type: String,
    },
    /// Set local transform fields.
    SetTransform {
        /// Target entity handle.
        entity: Entity,
        /// New optional translation. `None` leaves current value unchanged.
        translation: Option<Vec3>,
        /// New optional rotation. `None` leaves current value unchanged.
        rotation: Option<Quat>,
        /// New optional scale. `None` leaves current value unchanged.
        scale: Option<Vec3>,
    },
    /// Set entity parent.
    SetParent {
        /// Child entity handle.
        entity: Entity,
        /// New parent entity handle. `None` detaches to root.
        parent: Option<Entity>,
    },
}

/// Result of applying a single [`ScenePatch`].
#[derive(Clone, Debug)]
pub struct PatchResult {
    /// Whether the patch was applied successfully.
    pub applied: bool,
    /// Human-readable description of the result.
    pub description: String,
    /// Entity affected (if any).
    pub entity: Option<Entity>,
    /// New entity created (only for spawn patches).
    pub spawned_entity: Option<Entity>,
}

impl SceneCommand {
    /// Validates this command against the current scene and produces
    /// resolved [`ScenePatch`] entries ready for application.
    ///
    /// Returns `Err` if the command is fundamentally invalid (e.g., entity
    /// doesn't exist). Returns `Ok` with validation details even for warnings.
    pub fn validate(&self, scene: &Scene) -> EngineResult<(CommandValidation, Vec<ScenePatch>)> {
        match self {
            Self::CreateEntity {
                name,
                parent,
                position,
                rotation_degrees,
                scale,
            } => {
                if name.is_empty() {
                    return Ok((
                        CommandValidation::error("invalid_name", "Entity name cannot be empty"),
                        Vec::new(),
                    ));
                }
                if name.len() > 256 {
                    return Ok((
                        CommandValidation::error(
                            "name_too_long",
                            "Entity name exceeds 256 characters",
                        ),
                        Vec::new(),
                    ));
                }

                // Check parent exists if specified
                if let Some(parent_id) = parent {
                    if scene.find_by_id(*parent_id).is_none() {
                        return Ok((
                            CommandValidation::error(
                                "parent_not_found",
                                format!("Parent entity {:?} not found in scene", parent_id),
                            ),
                            Vec::new(),
                        ));
                    }
                }

                let rotation = Quat::from_euler_deg(
                    rotation_degrees.x,
                    rotation_degrees.y,
                    rotation_degrees.z,
                );
                let transform = Transform {
                    translation: *position,
                    rotation,
                    scale: *scale,
                };

                Ok((
                    CommandValidation::ok(),
                    vec![ScenePatch::SpawnEntity {
                        name: name.clone(),
                        parent: parent.and_then(|id| scene.find_by_id(id)),
                        transform,
                    }],
                ))
            }

            Self::DeleteEntity { entity } => {
                let resolved = scene.find_by_id(*entity);
                match resolved {
                    Some(_) => Ok((
                        CommandValidation::ok()
                            .with_warning(format!("Deleting entity {:?} and all children", entity)),
                        vec![ScenePatch::DespawnEntity {
                            entity: resolved.unwrap(),
                        }],
                    )),
                    None => Ok((
                        CommandValidation::error(
                            "entity_not_found",
                            format!("Entity {:?} not found in scene", entity),
                        ),
                        Vec::new(),
                    )),
                }
            }

            Self::RenameEntity { entity, name } => {
                if name.is_empty() {
                    return Ok((
                        CommandValidation::error("invalid_name", "Entity name cannot be empty"),
                        Vec::new(),
                    ));
                }
                let resolved = scene.find_by_id(*entity);
                match resolved {
                    Some(_) => Ok((
                        CommandValidation::ok(),
                        vec![ScenePatch::RenameEntity {
                            entity: resolved.unwrap(),
                            name: name.clone(),
                        }],
                    )),
                    None => Ok((
                        CommandValidation::error(
                            "entity_not_found",
                            format!("Entity {:?} not found in scene", entity),
                        ),
                        Vec::new(),
                    )),
                }
            }

            Self::AddComponent {
                entity,
                component_type,
                data,
            }
            | Self::UpdateComponent {
                entity,
                component_type,
                data,
            } => {
                let resolved = scene.find_by_id(*entity);
                let resolved = match resolved {
                    Some(e) => e,
                    None => {
                        return Ok((
                            CommandValidation::error(
                                "entity_not_found",
                                format!("Entity {:?} not found in scene", entity),
                            ),
                            Vec::new(),
                        ));
                    }
                };

                let component = deserialize_component(component_type, data)?;
                let is_new = matches!(self, Self::AddComponent { .. });
                let patch = ScenePatch::UpsertComponent {
                    entity: resolved,
                    component,
                };

                let validation = if is_new {
                    // Check for duplicate
                    let existing_types: Vec<&str> = scene
                        .components(resolved)
                        .map(|cs| cs.iter().map(|c| c.type_id()).collect())
                        .unwrap_or_default();
                    if existing_types.contains(&component_type.as_str()) {
                        CommandValidation::error(
                            "duplicate_component",
                            format!(
                                "Entity already has a '{}' component. Use UpdateComponent instead.",
                                component_type
                            ),
                        )
                        .with_warning("Component already exists — use UpdateComponent")
                    } else {
                        CommandValidation::ok()
                    }
                } else {
                    // Check it actually exists to update
                    let has_component = scene
                        .components(resolved)
                        .map(|cs| cs.iter().any(|c| c.type_id() == component_type))
                        .unwrap_or(false);
                    if !has_component {
                        CommandValidation::error(
                            "component_not_found",
                            format!(
                                "Entity does not have a '{}' component to update",
                                component_type
                            ),
                        )
                    } else {
                        CommandValidation::ok()
                    }
                };

                Ok((validation, vec![patch]))
            }

            Self::RemoveComponent {
                entity,
                component_type,
            } => {
                let resolved = scene.find_by_id(*entity);
                let resolved = match resolved {
                    Some(e) => e,
                    None => {
                        return Ok((
                            CommandValidation::error(
                                "entity_not_found",
                                format!("Entity {:?} not found in scene", entity),
                            ),
                            Vec::new(),
                        ));
                    }
                };

                // Check component exists
                let has_component = scene
                    .components(resolved)
                    .map(|cs| cs.iter().any(|c| c.type_id() == component_type))
                    .unwrap_or(false);
                if !has_component {
                    return Ok((
                        CommandValidation::error(
                            "component_not_found",
                            format!("Entity does not have a '{}' component", component_type),
                        ),
                        Vec::new(),
                    ));
                }

                Ok((
                    CommandValidation::ok(),
                    vec![ScenePatch::RemoveComponent {
                        entity: resolved,
                        component_type: component_type.clone(),
                    }],
                ))
            }

            Self::SetTransform {
                entity,
                position,
                rotation_degrees,
                scale,
            } => {
                let resolved = scene.find_by_id(*entity);
                let resolved = match resolved {
                    Some(e) => e,
                    None => {
                        return Ok((
                            CommandValidation::error(
                                "entity_not_found",
                                format!("Entity {:?} not found in scene", entity),
                            ),
                            Vec::new(),
                        ));
                    }
                };

                let rotation = rotation_degrees.map(|r| Quat::from_euler_deg(r.x, r.y, r.z));
                Ok((
                    CommandValidation::ok(),
                    vec![ScenePatch::SetTransform {
                        entity: resolved,
                        translation: *position,
                        rotation,
                        scale: *scale,
                    }],
                ))
            }

            Self::SetParent { entity, parent } => {
                let resolved = scene.find_by_id(*entity);
                let resolved = match resolved {
                    Some(e) => e,
                    None => {
                        return Ok((
                            CommandValidation::error(
                                "entity_not_found",
                                format!("Entity {:?} not found in scene", entity),
                            ),
                            Vec::new(),
                        ));
                    }
                };

                // Check new parent exists if specified
                if let Some(parent_id) = parent {
                    let parent_resolved = scene.find_by_id(*parent_id);
                    if parent_resolved.is_none() {
                        return Ok((
                            CommandValidation::error(
                                "parent_not_found",
                                format!("Parent entity {:?} not found in scene", parent_id),
                            ),
                            Vec::new(),
                        ));
                    }
                }

                Ok((
                    CommandValidation::ok(),
                    vec![ScenePatch::SetParent {
                        entity: resolved,
                        parent: parent.and_then(|id| scene.find_by_id(id)),
                    }],
                ))
            }

            Self::AttachScript {
                entity,
                backend,
                script,
            } => {
                let resolved = scene.find_by_id(*entity);
                let resolved = match resolved {
                    Some(e) => e,
                    None => {
                        return Ok((
                            CommandValidation::error(
                                "entity_not_found",
                                format!("Entity {:?} not found in scene", entity),
                            ),
                            Vec::new(),
                        ));
                    }
                };

                if backend.is_empty() || script.is_empty() {
                    return Ok((
                        CommandValidation::error(
                            "invalid_script",
                            "Both backend and script path are required",
                        ),
                        Vec::new(),
                    ));
                }

                let component = ComponentData::Script(crate::scene::ScriptComponentProxy {
                    backend: backend.clone(),
                    script: script.clone(),
                    state_json: None,
                    pending_recovery: false,
                });

                Ok((
                    CommandValidation::ok(),
                    vec![ScenePatch::UpsertComponent {
                        entity: resolved,
                        component,
                    }],
                ))
            }

            Self::AttachAsset {
                entity,
                asset: _,
                kind: _,
            } => {
                let resolved = scene.find_by_id(*entity);
                let _resolved = match resolved {
                    Some(e) => e,
                    None => {
                        return Ok((
                            CommandValidation::error(
                                "entity_not_found",
                                format!("Entity {:?} not found in scene", entity),
                            ),
                            Vec::new(),
                        ));
                    }
                };

                // AttachAsset is a placeholder for future asset-to-component conversion.
                // For now it creates a no-op validation warning.
                Ok((
                    CommandValidation::error(
                        "not_implemented",
                        "AttachAsset is not yet implemented — use AttachScript or explicit AddComponent instead",
                    ),
                    Vec::new(),
                ))
            }
        }
    }
}

impl ScenePatch {
    /// Applies a single patch to the scene.
    ///
    /// Returns the result describing what happened.
    pub fn apply(&self, scene: &mut Scene) -> EngineResult<PatchResult> {
        match self {
            Self::SpawnEntity {
                name,
                parent,
                transform,
            } => {
                let entity = scene.create_object(name.clone())?;
                scene.transforms_mut().set_local(entity, *transform);
                if let Some(p) = parent {
                    scene.set_parent(entity, Some(*p))?;
                }
                Ok(PatchResult {
                    applied: true,
                    description: format!("Created entity '{}'", name),
                    entity: Some(entity),
                    spawned_entity: Some(entity),
                })
            }

            Self::DespawnEntity { entity } => {
                scene.destroy_deferred(*entity)?;
                scene.process_deferred_destroy()?;
                Ok(PatchResult {
                    applied: true,
                    description: "Deleted entity".to_string(),
                    entity: Some(*entity),
                    spawned_entity: None,
                })
            }

            Self::RenameEntity { entity, name } => {
                let obj = scene.object_mut(*entity).ok_or_else(|| {
                    EngineError::invalid_handle("cannot rename: entity not found")
                })?;
                obj.name = name.clone();
                Ok(PatchResult {
                    applied: true,
                    description: format!("Renamed entity to '{}'", name),
                    entity: Some(*entity),
                    spawned_entity: None,
                })
            }

            Self::UpsertComponent { entity, component } => {
                scene.upsert_component(*entity, component.clone())?;
                Ok(PatchResult {
                    applied: true,
                    description: format!("Upserted '{}' component", component.type_id()),
                    entity: Some(*entity),
                    spawned_entity: None,
                })
            }

            Self::RemoveComponent {
                entity,
                component_type,
            } => {
                scene.remove_component(*entity, component_type)?;
                Ok(PatchResult {
                    applied: true,
                    description: format!("Removed '{}' component", component_type),
                    entity: Some(*entity),
                    spawned_entity: None,
                })
            }

            Self::SetTransform {
                entity,
                translation,
                rotation,
                scale,
            } => {
                let current = scene
                    .transforms()
                    .local(*entity)
                    .unwrap_or(Transform::IDENTITY);
                let new_transform = Transform {
                    translation: translation.unwrap_or(current.translation),
                    rotation: rotation.unwrap_or(current.rotation),
                    scale: scale.unwrap_or(current.scale),
                };
                scene.transforms_mut().set_local(*entity, new_transform);
                Ok(PatchResult {
                    applied: true,
                    description: format!("Set transform on entity {:?}", entity),
                    entity: Some(*entity),
                    spawned_entity: None,
                })
            }

            Self::SetParent { entity, parent } => {
                scene.set_parent(*entity, *parent)?;
                Ok(PatchResult {
                    applied: true,
                    description: "Reparented entity".to_string(),
                    entity: Some(*entity),
                    spawned_entity: None,
                })
            }
        }
    }

    /// Applies a batch of patches transactionally.
    ///
    /// If any patch fails, all previously applied patches in this batch are
    /// rolled back.
    pub fn apply_batch(
        scene: &mut Scene,
        patches: &[ScenePatch],
    ) -> EngineResult<Vec<PatchResult>> {
        // Snapshot state before applying (for rollback)
        let before = scene.to_scene_file("rollback-snapshot")?;
        let mut results = Vec::with_capacity(patches.len());

        for patch in patches {
            match patch.apply(scene) {
                Ok(result) => results.push(result),
                Err(e) => {
                    // Rollback: reload the scene from snapshot
                    let rolled_back = Scene::from_scene_file(before)?;
                    *scene = rolled_back;
                    return Err(EngineError::other(format!(
                        "Patch failed at index {}: {}. All changes rolled back.",
                        results.len(),
                        e
                    )));
                }
            }
        }

        Ok(results)
    }

    /// Applies patches without transactional rollback.
    /// Use when the caller handles error recovery themselves.
    pub fn apply_batch_no_rollback(
        scene: &mut Scene,
        patches: &[ScenePatch],
    ) -> Vec<EngineResult<PatchResult>> {
        patches.iter().map(|p| p.apply(scene)).collect()
    }
}

// ─── Component deserialization ──────────────────────────────────────────

/// Helper: wraps a serde_json error into an EngineError.
fn json_err(component_type: &str, e: serde_json::Error) -> EngineError {
    EngineError::other(format!(
        "Failed to deserialize component '{component_type}': {e}"
    ))
}

/// Deserializes a component from type ID and JSON value.
fn deserialize_component(
    component_type: &str,
    data: &serde_json::Value,
) -> EngineResult<ComponentData> {
    match component_type {
        "Camera" => serde_json::from_value(data.clone())
            .map(ComponentData::Camera)
            .map_err(|e| json_err(component_type, e)),
        "MeshRenderer" => serde_json::from_value(data.clone())
            .map(ComponentData::MeshRenderer)
            .map_err(|e| json_err(component_type, e)),
        "Light" => serde_json::from_value(data.clone())
            .map(ComponentData::Light)
            .map_err(|e| json_err(component_type, e)),
        "Rigidbody" => serde_json::from_value(data.clone())
            .map(ComponentData::Rigidbody)
            .map_err(|e| json_err(component_type, e)),
        "Collider" => serde_json::from_value(data.clone())
            .map(ComponentData::Collider)
            .map_err(|e| json_err(component_type, e)),
        "FluidVolume" => serde_json::from_value(data.clone())
            .map(ComponentData::FluidVolume)
            .map_err(|e| json_err(component_type, e)),
        "WindZone" => serde_json::from_value(data.clone())
            .map(ComponentData::WindZone)
            .map_err(|e| json_err(component_type, e)),
        "AudioSource" => serde_json::from_value(data.clone())
            .map(ComponentData::AudioSource)
            .map_err(|e| json_err(component_type, e)),
        "AudioListener" => serde_json::from_value(data.clone())
            .map(ComponentData::AudioListener)
            .map_err(|e| json_err(component_type, e)),
        "Skybox" => serde_json::from_value(data.clone())
            .map(ComponentData::Skybox)
            .map_err(|e| json_err(component_type, e)),
        "Sprite2D" => serde_json::from_value(data.clone())
            .map(ComponentData::Sprite2D)
            .map_err(|e| json_err(component_type, e)),
        "TileMap" => serde_json::from_value(data.clone())
            .map(ComponentData::TileMap)
            .map_err(|e| json_err(component_type, e)),
        "Camera2D" => serde_json::from_value(data.clone())
            .map(ComponentData::Camera2D)
            .map_err(|e| json_err(component_type, e)),
        "Light2D" => serde_json::from_value(data.clone())
            .map(ComponentData::Light2D)
            .map_err(|e| json_err(component_type, e)),
        "Occluder2D" => serde_json::from_value(data.clone())
            .map(ComponentData::Occluder2D)
            .map_err(|e| json_err(component_type, e)),
        "AnimationPlayer" => serde_json::from_value(data.clone())
            .map(ComponentData::AnimationPlayer)
            .map_err(|e| json_err(component_type, e)),
        "SkinnedMeshRenderer" => serde_json::from_value(data.clone())
            .map(ComponentData::SkinnedMeshRenderer)
            .map_err(|e| json_err(component_type, e)),
        "AudioStreamPlayer2D" => serde_json::from_value(data.clone())
            .map(ComponentData::AudioStreamPlayer2D)
            .map_err(|e| json_err(component_type, e)),
        "AudioStreamPlayer3D" => serde_json::from_value(data.clone())
            .map(ComponentData::AudioStreamPlayer3D)
            .map_err(|e| json_err(component_type, e)),
        "ParticleEmitter" => serde_json::from_value(data.clone())
            .map(ComponentData::ParticleEmitter)
            .map_err(|e| json_err(component_type, e)),
        "AcousticMaterial" => serde_json::from_value(data.clone())
            .map(ComponentData::AcousticMaterial)
            .map_err(|e| json_err(component_type, e)),
        "AcousticGeometry" => serde_json::from_value(data.clone())
            .map(ComponentData::AcousticGeometry)
            .map_err(|e| json_err(component_type, e)),
        "AcousticRoom" => serde_json::from_value(data.clone())
            .map(ComponentData::AcousticRoom)
            .map_err(|e| json_err(component_type, e)),
        "AcousticPortal" => serde_json::from_value(data.clone())
            .map(ComponentData::AcousticPortal)
            .map_err(|e| json_err(component_type, e)),
        "AudioZone" => serde_json::from_value(data.clone())
            .map(ComponentData::AudioZone)
            .map_err(|e| json_err(component_type, e)),
        other => Err(EngineError::other(format!(
            "Unknown component type '{other}'"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::CameraComponentData;

    fn test_scene() -> Scene {
        Scene::new()
    }

    #[test]
    fn create_entity_command_is_valid() {
        let scene = test_scene();
        let cmd = SceneCommand::CreateEntity {
            name: "TestCube".to_string(),
            parent: None,
            position: Vec3::new(0.0, 0.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            scale: Vec3::ONE,
        };

        let (validation, patches) = cmd.validate(&scene).unwrap();
        assert!(validation.is_valid);
        assert_eq!(patches.len(), 1);
        assert!(matches!(patches[0], ScenePatch::SpawnEntity { .. }));
    }

    #[test]
    fn create_entity_empty_name_is_invalid() {
        let scene = test_scene();
        let cmd = SceneCommand::CreateEntity {
            name: "".to_string(),
            parent: None,
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            scale: Vec3::ONE,
        };

        let (validation, patches) = cmd.validate(&scene).unwrap();
        assert!(!validation.is_valid);
        assert!(patches.is_empty());
    }

    #[test]
    fn delete_nonexistent_entity_fails() {
        let scene = test_scene();
        let cmd = SceneCommand::DeleteEntity {
            entity: EntityId::from_u128(999),
        };

        let (validation, _) = cmd.validate(&scene).unwrap();
        assert!(!validation.is_valid);
        assert_eq!(validation.code, "entity_not_found");
    }

    #[test]
    fn set_transform_on_nonexistent_entity_fails() {
        let scene = test_scene();
        let cmd = SceneCommand::SetTransform {
            entity: EntityId::from_u128(42),
            position: Some(Vec3::new(1.0, 2.0, 3.0)),
            rotation_degrees: None,
            scale: None,
        };

        let (validation, _) = cmd.validate(&scene).unwrap();
        assert!(!validation.is_valid);
    }

    #[test]
    fn create_and_delete_entity_roundtrip() {
        let mut scene = test_scene();

        // Create entity
        let cmd = SceneCommand::CreateEntity {
            name: "Temp".to_string(),
            parent: None,
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            scale: Vec3::ONE,
        };

        let (validation, patches) = cmd.validate(&scene).unwrap();
        assert!(validation.is_valid);

        let results = ScenePatch::apply_batch(&mut scene, &patches).unwrap();
        assert_eq!(results.len(), 1);
        let entity = results[0].spawned_entity.unwrap();
        assert!(scene.world().is_alive(entity));

        // Now delete it
        let entity_id = scene.object(entity).unwrap().id;
        let delete_cmd = SceneCommand::DeleteEntity { entity: entity_id };
        let (_, delete_patches) = delete_cmd.validate(&scene).unwrap();
        ScenePatch::apply_batch(&mut scene, &delete_patches).unwrap();
        assert!(!scene.world().is_alive(entity));
    }

    #[test]
    fn add_component_roundtrip() {
        let mut scene = test_scene();

        // Create entity
        let entity = scene.create_object("CameraObj").unwrap();
        let entity_id = scene.object(entity).unwrap().id;

        // Add Camera component
        let camera_data = serde_json::json!({
            "vertical_fov_degrees": 60.0,
            "near": 0.01,
            "far": 1000.0,
            "primary": true,
            "clear_color": [0.1, 0.1, 0.1]
        });

        let cmd = SceneCommand::AddComponent {
            entity: entity_id,
            component_type: "Camera".to_string(),
            data: camera_data,
        };

        let (validation, patches) = cmd.validate(&scene).unwrap();
        assert!(validation.is_valid);

        ScenePatch::apply_batch(&mut scene, &patches).unwrap();
        let components = scene.components(entity).unwrap();
        assert!(components.iter().any(|c| c.type_id() == "Camera"));
    }

    #[test]
    fn remove_component_roundtrip() {
        let mut scene = test_scene();
        let entity = scene.create_object("LightObj").unwrap();
        let entity_id = scene.object(entity).unwrap().id;

        scene
            .upsert_component(entity, ComponentData::Light(Default::default()))
            .unwrap();

        let cmd = SceneCommand::RemoveComponent {
            entity: entity_id,
            component_type: "Light".to_string(),
        };

        let (validation, patches) = cmd.validate(&scene).unwrap();
        assert!(validation.is_valid);

        ScenePatch::apply_batch(&mut scene, &patches).unwrap();
        assert!(scene.components(entity).unwrap().is_empty());
    }

    #[test]
    fn set_parent_works_with_root_and_child() {
        let mut scene = test_scene();
        let parent = scene.create_object("Parent").unwrap();
        let parent_id = scene.object(parent).unwrap().id;
        let child = scene.create_object("Child").unwrap();
        let child_id = scene.object(child).unwrap().id;

        let cmd = SceneCommand::SetParent {
            entity: child_id,
            parent: Some(parent_id),
        };

        let (validation, patches) = cmd.validate(&scene).unwrap();
        assert!(validation.is_valid);

        ScenePatch::apply_batch(&mut scene, &patches).unwrap();
        assert_eq!(scene.transforms().parent(child), Some(parent));
    }

    #[test]
    fn rename_entity() {
        let mut scene = test_scene();
        let entity = scene.create_object("OldName").unwrap();
        let entity_id = scene.object(entity).unwrap().id;

        let cmd = SceneCommand::RenameEntity {
            entity: entity_id,
            name: "NewName".to_string(),
        };

        let (validation, patches) = cmd.validate(&scene).unwrap();
        assert!(validation.is_valid);
        ScenePatch::apply_batch(&mut scene, &patches).unwrap();
        assert_eq!(scene.object(entity).unwrap().name, "NewName");
    }

    #[test]
    fn set_transform_partial_update() {
        let mut scene = test_scene();
        let entity = scene.create_object("Mover").unwrap();
        let entity_id = scene.object(entity).unwrap().id;

        let cmd = SceneCommand::SetTransform {
            entity: entity_id,
            position: Some(Vec3::new(5.0, 10.0, 0.0)),
            rotation_degrees: None,
            scale: None,
        };

        let (validation, patches) = cmd.validate(&scene).unwrap();
        assert!(validation.is_valid);
        ScenePatch::apply_batch(&mut scene, &patches).unwrap();

        let t = scene.transforms().local(entity).unwrap();
        assert_eq!(t.translation, Vec3::new(5.0, 10.0, 0.0));
        assert_eq!(t.scale, Vec3::ONE); // unchanged
    }

    #[test]
    fn transactional_rollback_on_failure() {
        let mut scene = test_scene();
        let entity = scene.create_object("Survivor").unwrap();
        let entity_id = scene.object(entity).unwrap().id;

        // Apply one valid patch, then an invalid one should roll back
        let patches = vec![
            ScenePatch::RenameEntity {
                entity,
                name: "ShouldRollBack".to_string(),
            },
            // Invalid: entity handle 999 doesn't exist
            ScenePatch::DespawnEntity {
                entity: Entity::from_handle(engine_core::Handle::new(
                    999,
                    engine_core::Generation::FIRST,
                )),
            },
        ];

        let result = ScenePatch::apply_batch(&mut scene, &patches);
        assert!(result.is_err());

        // Must be rolled back to original state
        assert_eq!(scene.object(entity).unwrap().name, "Survivor");
    }

    #[test]
    fn attach_script_command() {
        let mut scene = test_scene();
        let entity = scene.create_object("Scripted").unwrap();
        let entity_id = scene.object(entity).unwrap().id;

        let cmd = SceneCommand::AttachScript {
            entity: entity_id,
            backend: "python".to_string(),
            script: "scripts/test.py".to_string(),
        };

        let (validation, patches) = cmd.validate(&scene).unwrap();
        assert!(validation.is_valid);
        ScenePatch::apply_batch(&mut scene, &patches).unwrap();

        let components = scene.components(entity).unwrap();
        let has_script = components.iter().any(|c| {
            if let crate::ComponentData::Script(s) = c {
                s.backend == "python" && s.script == "scripts/test.py"
            } else {
                false
            }
        });
        assert!(has_script);
    }

    #[test]
    fn create_entity_with_parent() {
        let mut scene = test_scene();
        let parent = scene.create_object("Parent").unwrap();
        let parent_id = scene.object(parent).unwrap().id;

        let cmd = SceneCommand::CreateEntity {
            name: "Child".to_string(),
            parent: Some(parent_id),
            position: Vec3::new(0.0, 1.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            scale: Vec3::ONE,
        };

        let (validation, patches) = cmd.validate(&scene).unwrap();
        assert!(validation.is_valid);
        let results = ScenePatch::apply_batch(&mut scene, &patches).unwrap();
        let child = results[0].spawned_entity.unwrap();
        assert_eq!(scene.transforms().parent(child), Some(parent));
    }
}
