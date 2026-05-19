#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Atomic ECS and base scene storage.

pub mod scene;
pub mod schema;
pub mod transform;
pub mod world;

#[cfg(feature = "physics")]
pub mod physics;

#[cfg(feature = "audio")]
pub mod audio;

pub use scene::{
    AudioSourceComponentData, CameraComponentData, CameraRole, ColliderComponentData,
    ComponentData, GameObject, LifecycleStage, LightComponentData, MaterialRef,
    MeshRendererComponentData, ObjectIdAllocator, RigidbodyComponentData, Scene, SceneFile,
    SceneMode, ScriptComponentProxy,
};
pub use schema::{
    BuildConfiguration, ComponentFieldKind, ComponentFieldSchema, ComponentSchema,
    ComponentSchemaRegistry, EditorPreferences, FormatDiagnostic, FormatVersion, PrefabFile,
    ProjectManifest, SchemaEvolution,
};
pub use transform::TransformHierarchy;
pub use world::{Component, ComponentStorage, Entity, World};
