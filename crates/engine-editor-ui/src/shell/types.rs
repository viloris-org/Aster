//! Shared types for the editor shell UI.
//!
//! Egui-dependent types stay here. All other types are re-exported from
//! `engine-editor` via the crate root.

use std::{collections::BTreeSet, path::PathBuf};

use egui::Color32;
use engine_assets::AssetGuid;
use engine_core::EntityId;
use engine_render::RenderWorld;

// Re-export types moved to engine-editor for backward compat with egui panel code.
pub use crate::{
    resource_kind_label, ConfirmDeleteDialog, DesignTokens, EditorAction,
    EditorSceneViewOrientation, EditorSceneViewProjection, EditorShell, EditorSnapSettings,
    EditorTransformSpace, EditorTransformTool, HubAction, HubPage, HubState, NewProjectDialog,
    PlayModeRequest, ProjectContext, ProjectDeletionDecision, ProjectDeletionMode,
    ScriptEditorState, ScriptTemplateBackend, ViewportTargetState, ViewportTexture,
    ViewportTransformDragMode, ViewportTransformDragState,
};

/// RGB color helper.
pub fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Dark theme color palette for the Infernux editor.
#[derive(Clone, Copy)]
pub struct InfernuxPalette {
    /// Primary text color.
    pub text: Color32,
    /// Dimmed text color.
    pub text_dim: Color32,
    /// Disabled text color.
    pub text_disabled: Color32,
    /// Window background color.
    pub window_bg: Color32,
    /// Panel background color.
    pub panel_bg: Color32,
    /// Menu bar background color.
    pub menu_bar: Color32,
    /// Status bar background color.
    pub status_bar: Color32,
    /// Viewport background color.
    pub viewport_bg: Color32,
    /// Frame background color.
    pub frame_bg: Color32,
    /// Input field background color.
    pub input_bg: Color32,
    /// Frame hover color.
    pub frame_hover: Color32,
    /// Frame active/pressed color.
    pub frame_active: Color32,
    /// Header background color.
    pub header: Color32,
    /// Header hover color.
    pub header_hover: Color32,
    /// Header active color.
    pub header_active: Color32,
    /// Border color.
    pub border: Color32,
    /// Border highlight color (for focused elements).
    pub border_highlight: Color32,
    /// Subtle separator color.
    pub separator: Color32,
    /// Alternate row color.
    pub row_alt: Color32,
    /// Selection color.
    pub selection: Color32,
    /// Selection hover color.
    pub selection_hover: Color32,
    /// Accent color.
    pub accent: Color32,
    /// Accent hover color.
    pub accent_hover: Color32,
    /// Play button color.
    pub play: Color32,
    /// Play button hover color.
    pub play_hover: Color32,
    /// Pause button color.
    pub pause: Color32,
    /// Pause button hover color.
    pub pause_hover: Color32,
    /// Warning color.
    pub warning: Color32,
    /// Error color.
    pub error: Color32,
    /// Success/info color.
    pub success: Color32,
    /// Overlay background (for modals/dialogs).
    pub overlay_bg: Color32,
}

impl InfernuxPalette {
    /// Creates a dark theme palette.
    pub const fn dark() -> Self {
        Self {
            text: Color32::from_rgb(220, 220, 220),
            text_dim: Color32::from_rgb(150, 150, 150),
            text_disabled: Color32::from_rgb(100, 100, 100),
            window_bg: Color32::from_rgb(32, 32, 32),
            panel_bg: Color32::from_rgb(40, 40, 40),
            menu_bar: Color32::from_rgb(35, 35, 35),
            status_bar: Color32::from_rgb(30, 30, 30),
            viewport_bg: Color32::from_rgb(28, 28, 28),
            frame_bg: Color32::from_rgb(50, 50, 50),
            input_bg: Color32::from_rgb(38, 38, 38),
            frame_hover: Color32::from_rgb(60, 60, 60),
            frame_active: Color32::from_rgb(55, 55, 55),
            header: Color32::from_rgb(48, 48, 48),
            header_hover: Color32::from_rgb(58, 58, 58),
            header_active: Color32::from_rgb(52, 52, 52),
            border: Color32::from_rgb(60, 60, 60),
            border_highlight: Color32::from_rgb(80, 120, 160),
            separator: Color32::from_rgb(55, 55, 55),
            row_alt: Color32::from_rgba_premultiplied(8, 8, 8, 8),
            selection: Color32::from_rgb(50, 100, 150),
            selection_hover: Color32::from_rgb(60, 110, 160),
            accent: Color32::from_rgb(220, 80, 80),
            accent_hover: Color32::from_rgb(235, 95, 95),
            play: Color32::from_rgb(60, 130, 90),
            play_hover: Color32::from_rgb(70, 145, 105),
            pause: Color32::from_rgb(140, 115, 50),
            pause_hover: Color32::from_rgb(155, 130, 65),
            warning: Color32::from_rgb(220, 170, 70),
            error: Color32::from_rgb(220, 80, 80),
            success: Color32::from_rgb(80, 180, 120),
            overlay_bg: Color32::from_rgba_premultiplied(0, 0, 0, 180),
        }
    }
}

/// Status of the current Copilot operation.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum CopilotStatus {
    /// Idle — waiting for user input.
    #[default]
    Idle,
    /// Agent is planning (sending to model, parsing response).
    Planning,
    /// Plan is ready for user review.
    ReadyForReview,
    /// Agent is executing approved operations.
    Executing,
    /// Execution complete.
    Complete,
    /// An error occurred.
    Error(String),
}

/// A single message in the Copilot chat history.
#[derive(Clone, Debug)]
pub struct CopilotChatMessage {
    /// Message role ("user" or "assistant").
    pub role: String,
    /// Message content.
    pub content: String,
}

/// A single planned operation shown in the review UI.
#[derive(Clone, Debug)]
pub struct PlanPreviewItem {
    /// Index in the plan.
    pub index: usize,
    /// Human-readable preview text.
    pub preview: String,
    /// Whether the operation is write-capable (requires approval).
    pub requires_write: bool,
    /// User has approved this operation.
    pub approved: bool,
}

/// Transient Copilot panel state.
#[derive(Clone, Debug)]
pub struct CopilotPanelState {
    /// Whether the panel is visible.
    pub visible: bool,
    /// User input text.
    pub input: String,
    /// Chat history.
    pub messages: Vec<CopilotChatMessage>,
    /// Current operation status.
    pub status: CopilotStatus,
    /// Whether auto-accept is enabled for low/medium risk operations.
    pub auto_accept: bool,
    /// Whether the trace section is expanded.
    pub trace_expanded: bool,
    /// Cached plan preview lines (one per planned operation).
    pub plan_preview: Vec<PlanPreviewItem>,
    /// The parsed AgentOperation values backing the plan preview.
    /// Indexed by the same position as `plan_preview`.
    pub cached_operations: Vec<engine_ai::AgentOperation>,
    /// Cached trace entries from the last execution.
    pub trace_entries: Vec<String>,
    /// Console entry count from the last execution.
    pub console_entry_count: usize,
    /// Error count from the last execution.
    pub console_error_count: usize,
    /// Status message to show (e.g. "Applied 4 operations").
    pub status_message: Option<String>,
    /// Error message to display.
    pub error_message: Option<String>,
}

impl Default for CopilotPanelState {
    fn default() -> Self {
        Self {
            visible: true,
            input: String::new(),
            messages: Vec::new(),
            status: CopilotStatus::Idle,
            auto_accept: false,
            trace_expanded: false,
            plan_preview: Vec::new(),
            cached_operations: Vec::new(),
            trace_entries: Vec::new(),
            console_entry_count: 0,
            console_error_count: 0,
            status_message: None,
            error_message: None,
        }
    }
}

/// Transient UI state for the editor shell.
///
/// Lives in engine-editor-ui because it contains `copilot: CopilotPanelState`
/// which references `engine_ai::AgentOperation`.
#[derive(Debug, Default)]
pub struct ShellUiState {
    /// Whether the Hierarchy panel is visible.
    pub show_hierarchy: bool,
    /// Whether the Inspector panel is visible.
    pub show_inspector: bool,
    /// Whether the Project panel is visible.
    pub show_project: bool,
    /// Whether the Console panel is visible.
    pub show_console: bool,
    /// Whether the Scene View panel is visible.
    pub show_scene_view: bool,
    /// Whether the Game View panel is visible.
    pub show_game_view: bool,
    /// Whether the engine is in play mode.
    pub playing: bool,
    /// Whether the engine is paused.
    pub paused: bool,
    /// Hierarchy object-name filter.
    pub hierarchy_filter: String,
    /// Project asset-name filter.
    pub project_filter: String,
    /// Console message filter.
    pub console_filter: String,
    /// Whether repeated console rows are collapsed by message.
    pub console_collapse: bool,
    /// Path typed by the user for Project panel import.
    pub project_import_path: String,
    /// Script file name typed by the user for Project panel script creation.
    pub project_new_script_name: String,
    /// Script backend selected for newly-created script assets.
    pub project_new_script_backend: ScriptTemplateBackend,
    /// Last Project panel import or rescan status.
    pub project_import_status: Option<String>,
    /// Scene object IDs selected in Hierarchy.
    pub hierarchy_selection: Vec<EntityId>,
    /// Dragged hierarchy object, if any.
    pub hierarchy_dragging: Option<EntityId>,
    /// Entity currently being renamed in hierarchy: (EntityId, edit text).
    pub hierarchy_rename: Option<(EntityId, String)>,
    /// Asset dragged from Project panel.
    pub dragged_asset: Option<AssetGuid>,
    /// Last requested Scene View render target.
    pub scene_view_target: Option<ViewportTargetState>,
    /// Last requested Game View render target.
    pub game_view_target: Option<ViewportTargetState>,
    /// Last requested selected-camera preview render target.
    pub camera_preview_target: Option<ViewportTargetState>,
    /// Copilot panel transient state.
    pub copilot: CopilotPanelState,
    /// Rendered scene view texture.
    pub scene_view_texture: Option<ViewportTexture>,
    /// Rendered game view texture.
    pub game_view_texture: Option<ViewportTexture>,
    /// Rendered selected-camera preview texture.
    pub camera_preview_texture: Option<ViewportTexture>,
    /// Latest Game View render-world produced by Play Mode runtime ticking.
    pub runtime_game_world: Option<RenderWorld>,
    /// Whether the command palette popup is open.
    pub command_palette_open: bool,
    /// Command palette text filter.
    pub command_filter: String,
    /// Last command dispatch status shown in the command palette.
    pub command_status: Option<String>,
    /// Editor camera orbit state: yaw angle in radians.
    pub editor_camera_yaw: f32,
    /// Editor camera orbit state: pitch angle in radians.
    pub editor_camera_pitch: f32,
    /// Editor camera orbit distance from target.
    pub editor_camera_distance: f32,
    /// Smoothed editor camera orbit distance target.
    pub editor_camera_target_distance: f32,
    /// Editor camera look-at target in world space.
    pub editor_camera_target: [f32; 3],
    /// Pending action for the native host to execute.
    pub pending_action: Option<EditorAction>,
    /// Action to execute after the unsaved-changes dialog is resolved (Save or Discard).
    pub pending_action_after_close: Option<EditorAction>,
    /// Scene snapshot captured before a Scene View guide drag began.
    pub scene_guide_drag_before: Option<(EntityId, String)>,
    /// Full viewport transform drag state used for stable cumulative edits.
    pub viewport_transform_drag: Option<ViewportTransformDragState>,
    /// Scene snapshot captured before a transform drag began (batches undo to drag session).
    pub inspector_drag_before: Option<String>,
    /// Whether the unsaved-changes close dialog is visible.
    pub show_close_dialog: bool,
    /// Whether closing the dialog should exit the app (true) or return to hub (false).
    pub close_dialog_exit_app: bool,
    /// Scene snapshot captured before a viewport transform drag began.
    pub viewport_transform_drag_before: Option<(EntityId, String)>,
    /// Pending Play Mode request for the native editor host to execute.
    pub play_mode_request: Option<PlayModeRequest>,
    /// Current Scene View projection mode.
    pub editor_scene_view_projection: EditorSceneViewProjection,
    /// Whether axis presets automatically switch Scene View to orthographic mode.
    pub editor_scene_view_auto_orthographic: bool,
    /// Current named Scene View orientation.
    pub editor_scene_view_orientation: EditorSceneViewOrientation,
    /// Current transform tool coordinate space.
    pub editor_transform_space: EditorTransformSpace,
    /// Current transform tool mode.
    pub editor_transform_tool: EditorTransformTool,
    /// Status message shown in the status bar.
    pub status_toast: Option<String>,
    /// Frames remaining before the status toast is cleared.
    pub status_toast_frames: u32,
    /// Inspector component type IDs that are currently collapsed.
    pub inspector_collapsed: Vec<String>,
    /// Filter text for the Add Component searchable dropdown.
    pub add_component_filter: String,
    /// Component type ID awaiting removal confirmation (two-click delete).
    pub remove_confirm: Option<String>,
    /// Editor snap settings for transform gizmos.
    pub editor_snap_settings: EditorSnapSettings,
    /// Folder paths (relative) that are expanded in the Project panel tree.
    pub expanded_folders: BTreeSet<String>,
    /// Asset path currently being renamed in the Project panel: (relative_path, edit_text).
    pub asset_rename: Option<(PathBuf, String)>,
    /// Asset path awaiting delete confirmation (two-click delete).
    pub asset_delete_confirm: Option<PathBuf>,
    /// Current in-editor script editing session.
    pub script_editor: Option<ScriptEditorState>,
    /// Whether snapping is temporarily toggled via Ctrl key.
    pub snap_toggle: bool,
    /// Whether the current drag has produced any accumulated movement.
    pub drag_dirty: bool,
    /// Text label shown near the gizmo during drag.
    pub drag_delta_label: Option<String>,
}

impl ShellUiState {
    /// Creates a default state with all editor panels open.
    pub fn all_open() -> Self {
        Self {
            show_hierarchy: true,
            show_inspector: true,
            show_project: true,
            show_console: true,
            show_scene_view: true,
            show_game_view: true,
            ..Self::default()
        }
    }
}
