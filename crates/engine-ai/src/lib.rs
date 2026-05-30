#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! AI agent service bridging LLMs with the Aster engine.
//!
//! Provides the [`AgentSession`] type that serializes project context,
//! sends it to an AI model, parses the response into [`AgentOperation`]s,
//! and executes them through the editor command system.

mod parser;
mod system_prompt;

pub use parser::parse_operations;

use std::path::PathBuf;

use engine_core::{EngineError, EngineResult};
use engine_editor::{
    agent::{AgentWriteMode, PermissionPolicy, TraceEntry, TraceRecorder},
    CommandContext, CommandRegistry, ConsoleEntry, ConsoleLevel, ConsoleService, ConsoleSource,
    ProjectContext, SelectionService, UndoRedoStack,
};

/// Abstract AI model backend.
///
/// Implementations handle the communication protocol with a specific
/// model provider (OpenAI, Anthropic, Ollama, etc.).
pub trait AiModel {
    /// Sends a chat request and returns the model's response text.
    fn chat(&self, request: AiRequest) -> EngineResult<AiResponse>;
}

/// Request sent to an AI model.
#[derive(Clone, Debug)]
pub struct AiRequest {
    /// System prompt describing the engine, available tools, and constraints.
    pub system: String,
    /// Serialized project context (scene graph, assets).
    pub context: serde_json::Value,
    /// The user's natural-language prompt.
    pub user: String,
}

/// Response returned by an AI model.
#[derive(Clone, Debug)]
pub struct AiResponse {
    /// Raw text content from the model.
    pub content: String,
}

/// An operation the AI agent requests the engine to perform.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentOperation {
    /// Execute a registered editor command.
    ExecuteCommand {
        /// Command identifier (e.g. "gameobject.create_empty").
        command: String,
        /// Optional parameters forwarded to the command handler.
        #[serde(default)]
        params: serde_json::Value,
    },
    /// Create or update a Rhai script file.
    WriteScript {
        /// Path relative to the asset root (e.g. "scripts/player.rhai").
        path: String,
        /// Rhai source code.
        source: String,
    },
    /// Create a new GameObject with optional components and position.
    CreateObject {
        /// Display name for the object.
        name: String,
        /// Component specifications.
        #[serde(default)]
        components: Vec<ComponentSpec>,
        /// Optional initial position [x, y, z].
        #[serde(default)]
        position: Option<[f32; 3]>,
    },
    /// Modify a component field on an entity.
    SetProperty {
        /// Entity identifier (e.g. "1:1").
        entity: String,
        /// Component type name (e.g. "Camera").
        component: String,
        /// Field name to modify.
        field: String,
        /// New value for the field.
        value: serde_json::Value,
    },
    /// Remove a component from an entity.
    RemoveComponent {
        /// Entity identifier.
        entity: String,
        /// Component type name to remove.
        component: String,
    },
    /// Delete an entity.
    DestroyObject {
        /// Entity identifier.
        entity: String,
    },
    /// Read a source file from the project.
    ReadFile {
        /// Path relative to the project root.
        path: String,
    },
    /// Report completion with an optional summary message.
    Complete {
        /// Optional summary of what was accomplished.
        #[serde(default)]
        summary: Option<String>,
    },
}

impl AgentOperation {
    /// Returns a human-readable action name for trace recording.
    pub fn action_name(&self) -> &'static str {
        match self {
            Self::ExecuteCommand { .. } => "execute_command",
            Self::WriteScript { .. } => "write_script",
            Self::CreateObject { .. } => "create_object",
            Self::SetProperty { .. } => "set_property",
            Self::RemoveComponent { .. } => "remove_component",
            Self::DestroyObject { .. } => "destroy_object",
            Self::ReadFile { .. } => "read_file",
            Self::Complete { .. } => "complete",
        }
    }
}

/// Component specification in an AI `CreateObject` operation.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct ComponentSpec {
    /// Component type name (e.g. "Camera", "Rigidbody", "Collider").
    #[serde(rename = "type")]
    pub component_type: String,
    /// Optional initial properties for the component.
    #[serde(default)]
    pub properties: serde_json::Value,
}

/// Outcome of an AI agent interaction.
#[derive(Clone, Debug)]
pub struct AgentOutcome {
    /// Number of operations performed.
    pub operations_performed: usize,
    /// Console entries produced during execution.
    pub console_entries: Vec<ConsoleEntry>,
    /// Trace entries produced during execution.
    pub trace_entries: Vec<TraceEntry>,
    /// Whether the agent signalled completion.
    pub completed: bool,
    /// Optional completion summary.
    pub summary: Option<String>,
}

impl Default for AgentOutcome {
    fn default() -> Self {
        Self {
            operations_performed: 0,
            console_entries: Vec::new(),
            trace_entries: Vec::new(),
            completed: false,
            summary: None,
        }
    }
}

/// A model-generated operation that has been validated but not yet applied.
#[derive(Clone, Debug)]
pub struct PlannedOperation {
    /// Operation to apply after user approval.
    pub operation: AgentOperation,
    /// Human-readable preview of the planned change.
    pub preview: String,
    /// Whether this operation requires write permission.
    pub requires_write: bool,
}

/// A validated agent plan ready for user preview.
#[derive(Clone, Debug)]
pub struct AgentPlan {
    /// Planned operations in model order.
    pub operations: Vec<PlannedOperation>,
    /// Whether all operations are read-only.
    pub read_only: bool,
    /// Whether any operation requires write permission.
    pub requires_write: bool,
    /// Permission policy used to validate this plan.
    pub policy: PermissionPolicy,
}

/// An AI agent session bound to a project.
///
/// Owns the project context, command registry, script backend, and undo stack
/// for the duration of an AI interaction.
pub struct AgentSession {
    /// Project state including scene, assets, and manifest.
    pub context: ProjectContext,
    /// Available editor commands.
    pub commands: CommandRegistry,
    /// Rhai script backend for compiling and creating scripts.
    pub script_backend: engine_script_rhai::RhaiScriptBackend,
    /// Undo/redo stack for reversible operations.
    pub undo_stack: UndoRedoStack,
    /// Console service for logging.
    pub console: ConsoleService,
    /// Current selection state.
    pub selection: SelectionService,
    /// Trace recorder for audited agent operations.
    pub trace: TraceRecorder,
    /// Asset root path for script creation.
    asset_root: PathBuf,
}

impl AgentSession {
    /// Creates a new agent session from a project context.
    ///
    /// Initializes the script backend, command registry with AI commands,
    /// and supporting services.
    pub fn new(context: ProjectContext) -> EngineResult<Self> {
        let mut commands = CommandRegistry::default();
        engine_editor::register_core_commands(&mut commands);
        engine_editor::register_ai_commands(&mut commands);

        let mut script_backend = engine_script_rhai::RhaiScriptBackend::new();
        let asset_root = context.root.join(&context.manifest.asset_root);

        // Pre-load any existing scripts from the scene
        script_backend.load_scene_scripts(&context.scene, &asset_root)?;

        Ok(Self {
            context,
            commands,
            script_backend,
            undo_stack: UndoRedoStack::default(),
            console: ConsoleService::default(),
            selection: SelectionService::default(),
            trace: TraceRecorder::default(),
            asset_root,
        })
    }

    /// Builds and validates a plan for one AI interaction cycle.
    ///
    /// 1. Builds project context JSON
    /// 2. Constructs the system prompt with available tools
    /// 3. Sends the request to the model
    /// 4. Parses the response into operations
    /// 5. Validates operations against the permission policy without applying them
    pub fn plan(
        &mut self,
        model: &dyn AiModel,
        user_prompt: &str,
        policy: PermissionPolicy,
    ) -> EngineResult<AgentPlan> {
        let project_context = self.context.to_ai_context();
        let available_commands: Vec<&engine_editor::EditorCommand> =
            self.commands.list_executable().collect();
        let system = system_prompt::build(&available_commands);

        let response = model.chat(AiRequest {
            system,
            context: project_context,
            user: user_prompt.to_string(),
        })?;

        let operations = match parser::parse_operations(&response.content) {
            Ok(operations) => operations,
            Err(error) => {
                self.push_agent_error(format!("parse_response: {error}"));
                return Err(error);
            }
        };

        self.build_plan(operations, policy)
    }

    /// Runs one AI interaction cycle using a transactional policy.
    ///
    /// This preserves the original convenience API while routing through plan
    /// validation before any operation is applied.
    pub fn run(&mut self, model: &dyn AiModel, user_prompt: &str) -> EngineResult<AgentOutcome> {
        let plan = self.plan(model, user_prompt, PermissionPolicy::transactional_write())?;
        self.apply_plan(&plan)
    }

    /// Applies an approved plan and records diagnostics and trace entries.
    pub fn apply_plan(&mut self, plan: &AgentPlan) -> EngineResult<AgentOutcome> {
        self.build_plan(
            plan.operations
                .iter()
                .map(|planned| planned.operation.clone())
                .collect(),
            plan.policy.clone(),
        )?;

        let mut outcome = AgentOutcome::default();
        for planned in &plan.operations {
            let op = &planned.operation;
            if matches!(op, AgentOperation::Complete { .. }) {
                outcome.completed = true;
                if let AgentOperation::Complete { summary } = op {
                    outcome.summary = summary.clone();
                }
                self.trace.record(
                    op.action_name(),
                    "completed",
                    "No recovery needed; completion does not mutate the project.",
                );
                break;
            }
            match self.execute_operation(op) {
                Ok(()) => {
                    outcome.operations_performed += 1;
                    self.trace
                        .record(op.action_name(), "applied", recovery_hint_for_success(op));
                }
                Err(error) => {
                    let entry = self.push_agent_error(format!("{}: {error}", op.action_name()));
                    outcome.console_entries.push(entry);
                    self.trace.record(
                        op.action_name(),
                        format!("failed: {error}"),
                        recovery_hint_for_failure(op),
                    );
                }
            }
        }
        outcome.trace_entries = self.trace.entries().to_vec();

        Ok(outcome)
    }

    /// Builds a validated plan from parsed operations.
    fn build_plan(
        &self,
        operations: Vec<AgentOperation>,
        policy: PermissionPolicy,
    ) -> EngineResult<AgentPlan> {
        let mut planned = Vec::with_capacity(operations.len());
        let mut requires_write = false;

        for operation in operations {
            let access = operation_access(&operation);
            if access.requires_write {
                requires_write = true;
            }
            validate_operation_policy(&operation, access, &policy)?;
            planned.push(PlannedOperation {
                preview: preview_operation(&operation),
                operation,
                requires_write: access.requires_write,
            });
        }

        Ok(AgentPlan {
            operations: planned,
            read_only: !requires_write,
            requires_write,
            policy,
        })
    }

    /// Executes a single agent operation against the engine state.
    fn execute_operation(&mut self, op: &AgentOperation) -> EngineResult<()> {
        match op {
            AgentOperation::ExecuteCommand { command, .. } => {
                let mut cmd_ctx = CommandContext {
                    project: &mut self.context,
                    selection: &self.selection,
                    console: &mut self.console,
                };
                let undo = self.commands.execute(command, &mut cmd_ctx)?;
                self.undo_stack.push(undo);
                Ok(())
            }
            AgentOperation::WriteScript { path, source } => {
                let relative = PathBuf::from(path);
                let full_path =
                    self.script_backend
                        .create_script(&self.asset_root, &relative, source)?;
                self.console.push(ConsoleEntry {
                    timestamp: "now".into(),
                    level: ConsoleLevel::Info,
                    source: ConsoleSource {
                        subsystem: "ai-agent".into(),
                        file: Some(full_path),
                        line: None,
                    },
                    message: format!("Created script: {path}"),
                });
                Ok(())
            }
            AgentOperation::CreateObject {
                name,
                components,
                position,
            } => {
                let entity = self.context.scene.create_object(name.as_str())?;
                if let Some(pos) = position {
                    use engine_core::math::Vec3;
                    if let Some(mut t) = self.context.scene.transforms().local(entity) {
                        t.translation = Vec3::new(pos[0], pos[1], pos[2]);
                        self.context.scene.transforms_mut().set_local(entity, t);
                    }
                }
                for spec in components {
                    let component = self.build_component(spec)?;
                    self.context.scene.upsert_component(entity, component)?;
                }
                let entity_id = format!(
                    "{}:{}",
                    entity.handle().slot(),
                    entity.handle().generation().get()
                );
                self.console.push(ConsoleEntry {
                    timestamp: "now".into(),
                    level: ConsoleLevel::Info,
                    source: ConsoleSource {
                        subsystem: "ai-agent".into(),
                        file: None,
                        line: None,
                    },
                    message: format!("Created object: {name} ({entity_id})"),
                });
                Ok(())
            }
            AgentOperation::SetProperty {
                entity,
                component,
                field,
                value,
            } => self.apply_property(entity, component, field, value),
            AgentOperation::RemoveComponent { entity, component } => {
                let parsed = parse_entity_id(entity)?;
                let removed = self.context.scene.remove_component(parsed, component)?;
                if removed {
                    self.console.push(ConsoleEntry {
                        timestamp: "now".into(),
                        level: ConsoleLevel::Info,
                        source: ConsoleSource {
                            subsystem: "ai-agent".into(),
                            file: None,
                            line: None,
                        },
                        message: format!("Removed {component} from {entity}"),
                    });
                }
                Ok(())
            }
            AgentOperation::DestroyObject { entity } => {
                let parsed = parse_entity_id(entity)?;
                self.context.scene.destroy_deferred(parsed)?;
                self.context.scene.process_deferred_destroy()?;
                self.console.push(ConsoleEntry {
                    timestamp: "now".into(),
                    level: ConsoleLevel::Info,
                    source: ConsoleSource {
                        subsystem: "ai-agent".into(),
                        file: None,
                        line: None,
                    },
                    message: format!("Destroyed object: {entity}"),
                });
                Ok(())
            }
            AgentOperation::ReadFile { path } => {
                let full_path = self.context.root.join(path);
                let content = std::fs::read_to_string(&full_path).map_err(|source| {
                    EngineError::Filesystem {
                        path: full_path,
                        source,
                    }
                })?;
                self.console.push(ConsoleEntry {
                    timestamp: "now".into(),
                    level: ConsoleLevel::Info,
                    source: ConsoleSource {
                        subsystem: "ai-agent".into(),
                        file: None,
                        line: None,
                    },
                    message: content,
                });
                Ok(())
            }
            AgentOperation::Complete { .. } => Ok(()),
        }
    }

    /// Converts a `ComponentSpec` into a `ComponentData` variant.
    fn build_component(&self, spec: &ComponentSpec) -> EngineResult<engine_ecs::ComponentData> {
        use engine_ecs::ComponentData;
        match spec.component_type.as_str() {
            "Camera" => Ok(ComponentData::Camera(
                engine_ecs::CameraComponentData::default(),
            )),
            "MeshRenderer" => Ok(ComponentData::MeshRenderer(
                engine_ecs::MeshRendererComponentData::default(),
            )),
            "Light" => Ok(ComponentData::Light(
                engine_ecs::LightComponentData::default(),
            )),
            "Rigidbody" => Ok(ComponentData::Rigidbody(
                engine_ecs::RigidbodyComponentData::default(),
            )),
            "Collider" => Ok(ComponentData::Collider(
                engine_ecs::ColliderComponentData::default(),
            )),
            "AudioSource" => Ok(ComponentData::AudioSource(
                engine_ecs::AudioSourceComponentData::default(),
            )),
            "ParticleEmitter" => Ok(ComponentData::ParticleEmitter(
                engine_ecs::ParticleEmitterComponentData::default(),
            )),
            "Sprite2D" => Ok(ComponentData::Sprite2D(
                engine_ecs::Sprite2DComponentData::default(),
            )),
            "Script" => Ok(ComponentData::Script(engine_ecs::ScriptComponentProxy {
                backend: "rhai".into(),
                script: spec
                    .properties
                    .get("script")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                state_json: None,
                pending_recovery: false,
            })),
            _ => Err(EngineError::config(format!(
                "unknown component type: {}",
                spec.component_type
            ))),
        }
    }

    /// Applies a property update to a component field on an entity.
    fn apply_property(
        &mut self,
        entity: &str,
        component: &str,
        field: &str,
        value: &serde_json::Value,
    ) -> EngineResult<()> {
        let parsed = parse_entity_id(entity)?;
        let components = self
            .context
            .scene
            .components(parsed)
            .ok_or_else(|| EngineError::config(format!("entity {entity} has no components")))?;

        // Find the component and read its current state as JSON
        let component_json = components
            .iter()
            .find(|c| c.type_id() == component)
            .map(|c| serde_json::to_value(c))
            .transpose()
            .map_err(|e| EngineError::other(e.to_string()))?
            .ok_or_else(|| {
                EngineError::config(format!("entity {entity} has no {component} component"))
            })?;

        // Apply the field update
        let mut updated = component_json;
        if let Some(obj) = updated.as_object_mut() {
            // Handle nested fields like "color.x" by setting top-level for now
            obj.insert(field.to_string(), value.clone());

            // Deserialize back to the correct ComponentData variant
            let new_component: engine_ecs::ComponentData = serde_json::from_value(updated)
                .map_err(|e| {
                    EngineError::config(format!("invalid value for {component}.{field}: {e}"))
                })?;

            self.context.scene.upsert_component(parsed, new_component)?;
        }

        self.console.push(ConsoleEntry {
            timestamp: "now".into(),
            level: ConsoleLevel::Info,
            source: ConsoleSource {
                subsystem: "ai-agent".into(),
                file: None,
                line: None,
            },
            message: format!("Set {component}.{field} on {entity}"),
        });
        Ok(())
    }

    fn push_agent_error(&mut self, message: String) -> ConsoleEntry {
        let entry = ConsoleEntry {
            timestamp: "now".into(),
            level: ConsoleLevel::Error,
            source: ConsoleSource {
                subsystem: "ai-agent".into(),
                file: None,
                line: None,
            },
            message,
        };
        self.console.push(entry.clone());
        entry
    }
}

#[derive(Clone, Copy, Debug)]
struct OperationAccess {
    requires_write: bool,
    requires_filesystem_write: bool,
}

fn operation_access(operation: &AgentOperation) -> OperationAccess {
    match operation {
        AgentOperation::ReadFile { .. } | AgentOperation::Complete { .. } => OperationAccess {
            requires_write: false,
            requires_filesystem_write: false,
        },
        AgentOperation::WriteScript { .. } => OperationAccess {
            requires_write: true,
            requires_filesystem_write: true,
        },
        AgentOperation::ExecuteCommand { .. }
        | AgentOperation::CreateObject { .. }
        | AgentOperation::SetProperty { .. }
        | AgentOperation::RemoveComponent { .. }
        | AgentOperation::DestroyObject { .. } => OperationAccess {
            requires_write: true,
            requires_filesystem_write: false,
        },
    }
}

fn validate_operation_policy(
    operation: &AgentOperation,
    access: OperationAccess,
    policy: &PermissionPolicy,
) -> EngineResult<()> {
    if !access.requires_write {
        return Ok(());
    }

    if policy.write_mode == AgentWriteMode::ReadOnly {
        return Err(EngineError::config(format!(
            "{} requires write permission",
            operation.action_name()
        )));
    }

    if access.requires_filesystem_write && !policy.filesystem_write {
        return Err(EngineError::config(format!(
            "{} requires filesystem write permission",
            operation.action_name()
        )));
    }

    if policy.write_mode == AgentWriteMode::Direct && !policy.direct_write {
        return Err(EngineError::config(format!(
            "{} requires explicit direct write permission",
            operation.action_name()
        )));
    }

    Ok(())
}

fn preview_operation(operation: &AgentOperation) -> String {
    match operation {
        AgentOperation::ExecuteCommand { command, .. } => {
            format!("Execute editor command `{command}`")
        }
        AgentOperation::WriteScript { path, .. } => {
            format!("Create or update Rhai script `{path}`")
        }
        AgentOperation::CreateObject { name, .. } => format!("Create object `{name}`"),
        AgentOperation::SetProperty {
            entity,
            component,
            field,
            ..
        } => format!("Set `{component}.{field}` on `{entity}`"),
        AgentOperation::RemoveComponent { entity, component } => {
            format!("Remove `{component}` from `{entity}`")
        }
        AgentOperation::DestroyObject { entity } => format!("Destroy object `{entity}`"),
        AgentOperation::ReadFile { path } => format!("Read project file `{path}`"),
        AgentOperation::Complete { summary } => summary
            .as_ref()
            .map(|summary| format!("Complete: {summary}"))
            .unwrap_or_else(|| "Complete agent session".to_string()),
    }
}

fn recovery_hint_for_success(operation: &AgentOperation) -> &'static str {
    match operation {
        AgentOperation::ExecuteCommand { .. }
        | AgentOperation::CreateObject { .. }
        | AgentOperation::SetProperty { .. }
        | AgentOperation::RemoveComponent { .. }
        | AgentOperation::DestroyObject { .. } => "Use editor undo to revert this operation.",
        AgentOperation::WriteScript { .. } => {
            "Review the generated script under the asset root and use version control or file history to revert it."
        }
        AgentOperation::ReadFile { .. } => "No recovery needed; this operation only read project data.",
        AgentOperation::Complete { .. } => "No recovery needed; completion does not mutate the project.",
    }
}

fn recovery_hint_for_failure(operation: &AgentOperation) -> &'static str {
    match operation {
        AgentOperation::WriteScript { .. } => {
            "Fix the script path or source, then regenerate or reapply the plan."
        }
        AgentOperation::ReadFile { .. } => "Check that the file path exists inside the project.",
        AgentOperation::ExecuteCommand { .. } => {
            "Check that the command is registered and available."
        }
        AgentOperation::CreateObject { .. }
        | AgentOperation::SetProperty { .. }
        | AgentOperation::RemoveComponent { .. }
        | AgentOperation::DestroyObject { .. } => {
            "Check entity identifiers, component names, and editor diagnostics before retrying."
        }
        AgentOperation::Complete { .. } => {
            "No recovery needed; completion does not mutate the project."
        }
    }
}

/// Parses an entity identifier string like "1:1" or "entity:1:1" into an Entity.
fn parse_entity_id(entity_str: &str) -> EngineResult<engine_ecs::Entity> {
    let id_part = entity_str.strip_prefix("entity:").unwrap_or(entity_str);
    let parts: Vec<&str> = id_part.split(':').collect();
    if parts.is_empty() {
        return Err(EngineError::config(format!(
            "invalid entity id: {entity_str}"
        )));
    }
    let slot = parts[0]
        .parse::<u32>()
        .map_err(|_| EngineError::config(format!("invalid entity id: {entity_str}")))?;
    Ok(engine_ecs::Entity::from_handle(engine_core::Handle::new(
        slot,
        engine_core::Generation::FIRST,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubModel {
        content: String,
    }

    impl StubModel {
        fn new(content: impl Into<String>) -> Self {
            Self {
                content: content.into(),
            }
        }
    }

    impl AiModel for StubModel {
        fn chat(&self, _request: AiRequest) -> EngineResult<AiResponse> {
            Ok(AiResponse {
                content: self.content.clone(),
            })
        }
    }

    fn temp_project_context() -> ProjectContext {
        use engine_ecs::ProjectManifest;

        let scene = engine_ecs::Scene::new();
        let database =
            engine_assets::AssetDatabase::new(std::env::temp_dir(), std::env::temp_dir());

        ProjectContext {
            manifest: ProjectManifest::example(),
            scene,
            database,
            registry: engine_assets::AssetRegistry::default(),
            assets: Vec::new(),
            asset_imports: Vec::new(),
            scene_dirty: false,
            root: std::env::temp_dir(),
            scene_path: std::env::temp_dir().join("main.aster_scene.json"),
        }
    }

    #[test]
    fn agent_session_initializes_with_project() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_root = manifest_dir.join("../engine-editor/../../examples/project");

        let ctx = ProjectContext::open(&project_root).unwrap();
        let session = AgentSession::new(ctx).unwrap();

        assert!(session.commands.list_executable().count() > 0);
        assert!(session.undo_stack.can_undo() == false);
    }

    #[test]
    fn parse_entity_id_handles_both_formats() {
        let entity = parse_entity_id("1:1").unwrap();
        assert_eq!(entity.handle().slot(), 1);

        let entity = parse_entity_id("entity:2:3").unwrap();
        assert_eq!(entity.handle().slot(), 2);
    }

    #[test]
    fn build_component_creates_known_types() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_root = manifest_dir.join("../engine-editor/../../examples/project");
        let ctx = ProjectContext::open(&project_root).unwrap();
        let session = AgentSession::new(ctx).unwrap();

        let spec = ComponentSpec {
            component_type: "Camera".into(),
            properties: serde_json::Value::Null,
        };
        let component = session.build_component(&spec).unwrap();
        assert_eq!(component.type_id(), "Camera");

        let spec = ComponentSpec {
            component_type: "Rigidbody".into(),
            properties: serde_json::Value::Null,
        };
        let component = session.build_component(&spec).unwrap();
        assert_eq!(component.type_id(), "Rigidbody");
    }

    #[test]
    fn execute_create_object_adds_to_scene() {
        use engine_ecs::ProjectManifest;

        let mut scene = engine_ecs::Scene::new();
        // Pre-populate with one object so the scene is non-trivial
        scene.create_object("Existing").unwrap();

        let database =
            engine_assets::AssetDatabase::new(std::env::temp_dir(), std::env::temp_dir());

        let ctx = ProjectContext {
            manifest: ProjectManifest::example(),
            scene,
            database,
            registry: engine_assets::AssetRegistry::default(),
            assets: Vec::new(),
            asset_imports: Vec::new(),
            scene_dirty: false,
            root: std::env::temp_dir(),
            scene_path: std::env::temp_dir().join("main.aster_scene.json"),
        };

        let mut session = AgentSession::new(ctx).unwrap();
        let op = AgentOperation::CreateObject {
            name: "AI_Player".into(),
            components: vec![ComponentSpec {
                component_type: "Rigidbody".into(),
                properties: serde_json::Value::Null,
            }],
            position: Some([0.0, 5.0, 0.0]),
        };

        session.execute_operation(&op).unwrap();

        let entity = session.context.scene.find_by_name("AI_Player").unwrap();
        let transform = session.context.scene.transforms().local(entity).unwrap();
        assert!((transform.translation.y - 5.0).abs() < 0.001);

        let components = session.context.scene.components(entity).unwrap();
        assert!(components.iter().any(|c| c.type_id() == "Rigidbody"));
    }

    #[test]
    fn plan_accepts_read_only_operations_under_read_only_policy() {
        let ctx = temp_project_context();
        let mut session = AgentSession::new(ctx).unwrap();
        let model = StubModel::new(
            r#"[
                {"action": "read_file", "path": "README.md"},
                {"action": "complete", "summary": "inspected project"}
            ]"#,
        );

        let plan = session
            .plan(
                &model,
                "what is in this project?",
                PermissionPolicy::read_only(),
            )
            .unwrap();

        assert!(plan.read_only);
        assert!(!plan.requires_write);
        assert_eq!(plan.operations.len(), 2);
        assert_eq!(plan.operations[0].preview, "Read project file `README.md`");
        assert!(session.context.scene.find_by_name("AI_Player").is_none());
    }

    #[test]
    fn plan_rejects_write_operations_under_read_only_policy() {
        let ctx = temp_project_context();
        let mut session = AgentSession::new(ctx).unwrap();
        let model = StubModel::new(r#"[{"action": "create_object", "name": "AI_Player"}]"#);

        let result = session.plan(&model, "create a player", PermissionPolicy::read_only());

        assert!(result.is_err());
        assert!(session.context.scene.find_by_name("AI_Player").is_none());
    }

    #[test]
    fn apply_plan_executes_only_after_approval() {
        let ctx = temp_project_context();
        let mut session = AgentSession::new(ctx).unwrap();
        let model = StubModel::new(
            r#"[
                {"action": "create_object", "name": "AI_Player"},
                {"action": "complete", "summary": "created player"}
            ]"#,
        );

        let plan = session
            .plan(
                &model,
                "create a player",
                PermissionPolicy::transactional_write(),
            )
            .unwrap();

        assert!(plan.requires_write);
        assert!(session.context.scene.find_by_name("AI_Player").is_none());

        let outcome = session.apply_plan(&plan).unwrap();

        assert_eq!(outcome.operations_performed, 1);
        assert!(outcome.completed);
        assert_eq!(outcome.summary.as_deref(), Some("created player"));
        assert!(session.context.scene.find_by_name("AI_Player").is_some());
        assert_eq!(outcome.trace_entries.len(), 2);
    }

    #[test]
    fn malformed_model_output_records_console_diagnostic() {
        let ctx = temp_project_context();
        let mut session = AgentSession::new(ctx).unwrap();
        let model = StubModel::new("not json");

        let result = session.plan(
            &model,
            "create a player",
            PermissionPolicy::transactional_write(),
        );

        assert!(result.is_err());
        assert_eq!(session.console.entries().len(), 1);
        assert!(session.console.entries()[0]
            .message
            .contains("parse_response"));
    }
}
