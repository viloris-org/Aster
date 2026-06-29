#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Varg language parser and diagnostics.
//!
//! This crate owns the public Varg authoring surface and the MVP runtime
//! interpreter used by the engine while the full compiler is built out.

mod api;
mod ast;
mod behavior;
mod diagnostics;
mod parser;
mod runtime;
mod scene_context;
mod syntax;
mod vscene;

pub use api::{
    VargScriptApiItem, VargScriptApiKind, VargScriptApiModule, varg_script_api_registry,
};
pub use ast::{VargDeclaration, VargExport, VargFileAst, VargFileRole, VargImport};
pub use behavior::{VargBehavior, VargBehaviorNode, compile_behavior_source};
pub use diagnostics::{VargDiagnostic, VargDiagnosticSeverity};
pub use parser::{diagnose_source, parse_source, parse_source_lossy};
pub use runtime::{
    VargAudioCommand, VargDestroyNearestRequest, VargHookMetadata, VargRenderCommand,
    VargRuntimeContext, VargRuntimeContextRef, VargRuntimeOutput, VargScript, VargScriptMetadata,
    VargSpawnRequest, VargUiCommand, compile_script_source,
};
pub use scene_context::{VargSceneBounds, VargSceneContext};
pub use vscene::{
    compile_vscene_source_to_scene, compile_vscene_source_to_scene_file,
    serialize_scene_file_to_vscene, serialize_scene_to_vscene,
};

#[cfg(test)]
mod tests;
