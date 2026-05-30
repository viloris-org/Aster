#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Native Hub and editor shell state for the first Aster UI surface.
//!
//! With the Tauri migration, the state types (`HubState`, `EditorShell`,
//! `ProjectContext`, etc.) now live in `engine-editor`. This crate re-exports
//! them for backward compatibility and provides the egui rendering shell
//! when compiled with the `editor` feature.

#[cfg(feature = "editor")]
pub mod hub;
#[cfg(feature = "editor")]
pub mod shell;

#[cfg(feature = "editor")]
pub mod fonts;
#[cfg(feature = "editor")]
pub use fonts::setup_egui_fonts;
#[cfg(feature = "editor")]
pub use hub::draw_hub;
#[cfg(feature = "editor")]
pub use shell::{build_camera_preview_render_world, build_editor_render_world, draw_shell};

// Re-export state types from engine-editor for backward compatibility.
// New consumers should import directly from engine-editor.
pub use engine_editor::{
    resource_kind_label, ConfirmDeleteDialog, DesignTokens, EditorAction,
    EditorSceneViewOrientation, EditorSceneViewProjection, EditorShell, EditorSnapSettings,
    EditorTransformSpace, EditorTransformTool, HubAction, HubPage, HubState, NewProjectDialog,
    PlayModeRequest, ProjectContext, ProjectDeletionDecision, ProjectDeletionMode,
    ScriptEditorState, ScriptTemplateBackend, ViewportTargetState, ViewportTexture,
    ViewportTransformDragMode, ViewportTransformDragState,
};

// ShellUiState and Copilot types stay in engine-editor-ui
// (ShellUiState references CopilotPanelState which references engine-ai).
pub use shell::types::{CopilotPanelState, CopilotStatus, ShellUiState};
