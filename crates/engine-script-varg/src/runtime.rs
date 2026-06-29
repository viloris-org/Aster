use std::collections::HashMap;

use engine_core::math::{Quat, Transform, Vec3};

use crate::ast::VargExport;
use crate::diagnostics::{VargDiagnostic, VargDiagnosticSeverity};
use crate::parser::parse_source;
use crate::scene_context::VargSceneContext;
use crate::syntax::{parse_function_signature, strip_line_comment};

/// Compiled Varg script summary used by the MVP runtime.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VargScript {
    /// Script declaration name.
    pub name: String,
    /// Editor-exposed properties.
    pub exports: Vec<VargExport>,
    /// Mutable script state variable defaults.
    pub state_defaults: HashMap<String, serde_json::Value>,
    /// Lifecycle hook bodies keyed by reserved hook name.
    hooks: HashMap<String, Vec<RuntimeStatement>>,
}

/// Compiles a `.varg` script into the MVP executable runtime summary.
pub fn compile_script_source(
    path: impl AsRef<std::path::Path>,
    source: &str,
) -> (Option<VargScript>, Vec<VargDiagnostic>) {
    let (ast, mut diagnostics) = parse_source(path, source);
    let Some(ast) = ast else {
        return (None, diagnostics);
    };
    let Some(declaration) = ast
        .declarations
        .iter()
        .find(|declaration| declaration.kind == "script")
    else {
        diagnostics.push(VargDiagnostic {
            code: "VARG3003".to_string(),
            severity: VargDiagnosticSeverity::Error,
            line: Some(1),
            column: Some(1),
            message: "logic file does not contain a script declaration".to_string(),
            expected: "`script Name { ... }`".to_string(),
            suggestion: "Add a script declaration or attach a file that contains one.".to_string(),
            blocking: true,
            source_line: source.lines().next().map(str::to_string),
        });
        return (None, diagnostics);
    };
    let mut script = VargScript {
        name: declaration
            .name
            .clone()
            .unwrap_or_else(|| "UnnamedScript".to_string()),
        exports: declaration.exports.clone(),
        state_defaults: HashMap::new(),
        hooks: HashMap::new(),
    };
    compile_runtime_blocks(source, &mut script);
    (Some(script), diagnostics)
}

/// Public metadata for a compiled Varg script.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct VargScriptMetadata {
    /// Script declaration name.
    pub name: String,
    /// Editor-exposed properties available for inspector overrides.
    pub exports: Vec<VargExport>,
    /// Lifecycle hooks implemented by this script.
    pub hooks: Vec<VargHookMetadata>,
}

/// Public metadata for one implemented script lifecycle hook.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct VargHookMetadata {
    /// Stable hook name, such as `start`, `update`, `fixedUpdate`, or `lateUpdate`.
    pub name: String,
}

/// Per-invocation context passed to the Varg script runtime.
#[derive(Clone, Debug)]
pub struct VargRuntimeContext {
    /// Local transform for the entity this script is attached to.
    pub transform: Transform,
    /// Frame input state.
    pub input: engine_platform::InputState,
    /// Delta time for the lifecycle call.
    pub delta_time: f32,
    /// Total elapsed runtime time in seconds.
    pub total_time: f32,
    /// Monotonic runtime frame index.
    pub frame_index: u64,
    /// Runtime output size in pixels.
    pub screen_size: (f32, f32),
    /// Editor-exposed overrides keyed by exported property name.
    pub exported_values: HashMap<String, serde_json::Value>,
    /// Persistent script state keyed by state variable name.
    pub state: HashMap<String, serde_json::Value>,
    /// Read-only scene facts exposed to migrated declarative gameplay APIs.
    pub scene: VargSceneContext,
}

/// Borrowed per-invocation context for hot runtime dispatch.
pub struct VargRuntimeContextRef<'a> {
    /// Local transform for the entity this script is attached to.
    pub transform: Transform,
    /// Frame input state.
    pub input: &'a engine_platform::InputState,
    /// Screen-space pointer positions that began this frame.
    pub pointer_pressed: &'a [(f32, f32)],
    /// Screen-space pointer positions that ended this frame.
    pub pointer_released: &'a [(f32, f32)],
    /// Delta time for the lifecycle call.
    pub delta_time: f32,
    /// Total elapsed runtime time in seconds.
    pub total_time: f32,
    /// Monotonic runtime frame index.
    pub frame_index: u64,
    /// Runtime output size in pixels.
    pub screen_size: (f32, f32),
    /// Editor-exposed overrides keyed by exported property name.
    pub exported_values: &'a HashMap<String, serde_json::Value>,
    /// Persistent script state keyed by state variable name.
    pub state: HashMap<String, serde_json::Value>,
    /// Read-only scene facts exposed to migrated declarative gameplay APIs.
    pub scene: VargSceneContext,
}

/// Result of executing one lifecycle hook.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VargRuntimeOutput {
    /// Updated local transform.
    pub transform: Transform,
    /// Updated persistent state.
    pub state: HashMap<String, serde_json::Value>,
    /// Log entries emitted by `log(...)`.
    pub logs: Vec<String>,
    /// UI draw commands emitted by `ui.*(...)` calls during this hook.
    pub ui_commands: Vec<VargUiCommand>,
    /// Audio commands emitted by script during this hook.
    pub audio_commands: Vec<VargAudioCommand>,
    /// Render environment commands emitted by script during this hook.
    pub render_commands: Vec<VargRenderCommand>,
    /// Scene object creation requests emitted during this hook.
    pub spawn_requests: Vec<VargSpawnRequest>,
    /// Scene object destruction requests emitted during this hook.
    pub destroy_nearest_requests: Vec<VargDestroyNearestRequest>,
    /// Whether the script requested deferred destruction of its owning entity.
    pub destroy_self: bool,
    /// Optional request to capture or release the game window mouse.
    pub mouse_capture: Option<bool>,
}

/// Render environment request emitted by Varg gameplay scripts.
#[derive(Clone, Debug, PartialEq)]
pub enum VargRenderCommand {
    /// Switch to screen-space global illumination.
    UseScreenSpaceGi,
    /// Configure probe-volume global illumination.
    UseProbeVolumeGi {
        /// World-space center.
        center: Vec3,
        /// World-space extents.
        extent: Vec3,
        /// Probe counts on x/y/z axes, represented as floats at the script boundary.
        counts: Vec3,
        /// Indirect lighting multiplier.
        intensity: f32,
    },
    /// Change GI intensity without replacing the current GI mode.
    SetGiIntensity(f32),
    /// Enable and select a runtime weather preset.
    SetWeatherPreset(String),
    /// Set normalized time of day in hours `[0.0, 24.0)`.
    SetWeatherTimeOfDay(f32),
    /// Set cloud cover in `[0.0, 1.0]`.
    SetWeatherCloudCover(f32),
    /// Set precipitation in `[0.0, 1.0]`.
    SetWeatherPrecipitation(f32),
    /// Set global weather wind velocity.
    SetWeatherWind(Vec3),
}

/// A lightweight procedural audio request emitted by Varg gameplay scripts.
#[derive(Clone, Debug, PartialEq)]
pub enum VargAudioCommand {
    /// Generate and play a one-shot oscillator tone.
    PlayTone {
        /// Waveform name: `sine`, `square`, `sawtooth`, `triangle`, or `noise`.
        waveform: String,
        /// Oscillator frequency in Hz.
        frequency_hz: f32,
        /// Tone duration in seconds.
        duration_seconds: f32,
        /// Linear gain in `[0.0, 1.0]`.
        volume: f32,
        /// Whether to place the sound at the script entity's position.
        spatial: bool,
        /// Source position used when `spatial` is true.
        position: Vec3,
    },
    /// Generate and loop a simple procedural note pattern.
    StartLoop {
        /// Stable script-provided loop id.
        id: String,
        /// Waveform name: `sine`, `square`, `sawtooth`, `triangle`, or `noise`.
        waveform: String,
        /// Whitespace/comma-separated notes, rests, or Hz values.
        pattern: String,
        /// Tempo in beats per minute.
        bpm: f32,
        /// Duration of each pattern token in beats.
        beats_per_note: f32,
        /// Linear gain in `[0.0, 1.0]`.
        volume: f32,
    },
    /// Stop a running procedural loop.
    StopLoop {
        /// Stable script-provided loop id.
        id: String,
    },
}

/// A primitive scene object creation request emitted by Varg gameplay scripts.
#[derive(Clone, Debug, PartialEq)]
pub struct VargSpawnRequest {
    /// User-visible object name.
    pub name: String,
    /// User-visible object tag.
    pub tag: String,
    /// Built-in mesh identifier such as `debug/cube` or `debug/sphere`.
    pub builtin_mesh: String,
    /// Collider primitive shape such as `box` or `sphere`.
    pub collider_shape: String,
    /// Local position for the spawned object.
    pub position: Vec3,
    /// Local shape size. Spheres use equal XYZ diameter.
    pub size: Vec3,
    /// Optional Varg script to attach to the spawned object.
    pub script: Option<String>,
}

/// A scene object destruction request emitted by Varg gameplay scripts.
#[derive(Clone, Debug, PartialEq)]
pub struct VargDestroyNearestRequest {
    /// User-visible tag to match.
    pub tag: String,
    /// Maximum local-space distance from `origin`.
    pub radius: f32,
    /// Local-space origin used for nearest-object selection.
    pub origin: Vec3,
}

/// A retained UI draw request emitted by Varg scripts.
#[derive(Clone, Debug, PartialEq)]
pub enum VargUiCommand {
    /// Draws text at a screen-space position.
    Label {
        /// Stable script-provided widget id.
        id: String,
        /// Text to draw.
        text: String,
        /// Screen-space x position in pixels.
        x: f32,
        /// Screen-space y position in pixels.
        y: f32,
    },
    /// Draws a flat colored rectangle in screen space.
    Rect {
        /// Stable script-provided widget id.
        id: String,
        /// Screen-space x position in pixels.
        x: f32,
        /// Screen-space y position in pixels.
        y: f32,
        /// Width in pixels.
        width: f32,
        /// Height in pixels.
        height: f32,
        /// RGBA color in linear float channels.
        color: [f32; 4],
    },
    /// Draws a textured rectangle in screen space.
    Texture {
        /// Stable script-provided widget id.
        id: String,
        /// Runtime GUI texture key.
        texture: String,
        /// Screen-space x position in pixels.
        x: f32,
        /// Screen-space y position in pixels.
        y: f32,
        /// Width in pixels.
        width: f32,
        /// Height in pixels.
        height: f32,
        /// RGBA tint in linear float channels.
        color: [f32; 4],
    },
}

#[derive(Clone, Debug, PartialEq)]
enum RuntimeStatement {
    Log(String),
    Translate(Expression),
    SetPosition(Expression),
    SetPositionAxis {
        axis: Axis,
        value: Expression,
    },
    AddToPosition {
        axis: Axis,
        value: Expression,
    },
    SetRotation(Expression),
    SetRotationAxis {
        axis: Axis,
        value: Expression,
    },
    AddToRotation {
        axis: Axis,
        value: Expression,
    },
    DeclareLocal {
        name: String,
        value: Expression,
    },
    AssignBinding {
        name: String,
        value: Expression,
    },
    AddToBinding {
        name: String,
        value: Expression,
    },
    SubFromBinding {
        name: String,
        value: Expression,
    },
    AssignState {
        name: String,
        value: Expression,
    },
    AddToState {
        name: String,
        value: Expression,
    },
    SubFromState {
        name: String,
        value: Expression,
    },
    If {
        condition: ConditionExpression,
        statements: Vec<RuntimeStatement>,
        else_statements: Vec<RuntimeStatement>,
    },
    ForLoop {
        variable: String,
        range: RangeExpression,
        body: Vec<RuntimeStatement>,
    },
    WhileLoop {
        condition: ConditionExpression,
        body: Vec<RuntimeStatement>,
    },
    CallFunction(String),
    Return(Expression),
    Break,
    Continue,
    Wait(Expression),
    DestroySelf,
    SpawnBox {
        name: Expression,
        tag: Expression,
        position: Expression,
        size: Expression,
        script: Expression,
    },
    SpawnSphere {
        name: Expression,
        tag: Expression,
        position: Expression,
        radius: Expression,
        script: Expression,
    },
    DestroyNearestWithTag {
        tag: Expression,
        radius: Expression,
    },
    PlayTone {
        waveform: Expression,
        frequency: Expression,
        duration: Expression,
        volume: Expression,
        spatial: bool,
    },
    StartAudioLoop {
        id: Expression,
        waveform: Expression,
        pattern: Expression,
        bpm: Expression,
        beats_per_note: Expression,
        volume: Expression,
    },
    StopAudioLoop {
        id: Expression,
    },
    UseScreenSpaceGi,
    UseProbeVolumeGi {
        center: Expression,
        extent: Expression,
        counts: Expression,
        intensity: Expression,
    },
    SetGiIntensity(Expression),
    SetWeatherPreset(Expression),
    SetWeatherTimeOfDay(Expression),
    SetWeatherCloudCover(Expression),
    SetWeatherPrecipitation(Expression),
    SetWeatherWind(Expression),
    SetMouseCapture(Expression),
    UiLabel {
        id: Expression,
        text: Expression,
        x: Expression,
        y: Expression,
    },
    UiRect {
        id: Expression,
        x: Expression,
        y: Expression,
        width: Expression,
        height: Expression,
        color: [Expression; 4],
    },
    UiTexture {
        id: Expression,
        texture: Expression,
        x: Expression,
        y: Expression,
        width: Expression,
        height: Expression,
        color: [Expression; 4],
    },
}

#[derive(Clone, Debug, PartialEq)]
enum ConditionExpression {
    InputDown(String),
    InputJustPressed(String),
    InputJustReleased(String),
    ActionDown(String),
    ActionJustPressed(String),
    ActionJustReleased(String),
    ActionUp(String),
    Not(Box<ConditionExpression>),
    And(Box<ConditionExpression>, Box<ConditionExpression>),
    Or(Box<ConditionExpression>, Box<ConditionExpression>),
    Compare {
        lhs: Expression,
        op: CompareOp,
        rhs: Expression,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum Expression {
    Number(f32),
    String(String),
    Bool(bool),
    Variable(String),
    Member(String, String),
    Call {
        function: String,
        args: Vec<Expression>,
    },
    Vec3(Box<Expression>, Box<Expression>, Box<Expression>),
    Binary {
        op: BinaryOp,
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompareOp {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
}

#[derive(Clone, Debug, PartialEq)]
enum RangeExpression {
    Range(Expression, Expression),          // i in 0..10
    RangeInclusive(Expression, Expression), // i in 0..=10
    Count(Expression),                      // i in count(10)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Axis {
    X,
    Y,
    Z,
}

impl VargScript {
    /// Returns public metadata for editor, AI, hot reload, and runtime dispatch.
    pub fn metadata(&self) -> VargScriptMetadata {
        VargScriptMetadata {
            name: self.name.clone(),
            exports: self.exports.clone(),
            hooks: self
                .hook_names()
                .into_iter()
                .map(|name| VargHookMetadata {
                    name: name.to_string(),
                })
                .collect(),
        }
    }

    /// Returns whether this script implements a lifecycle hook.
    pub fn has_hook(&self, hook: &str) -> bool {
        self.hooks.contains_key(hook)
    }

    /// Returns implemented lifecycle hook names in stable lexical order.
    pub fn hook_names(&self) -> Vec<&str> {
        let mut hooks = self.hooks.keys().map(String::as_str).collect::<Vec<_>>();
        hooks.sort_unstable();
        hooks
    }

    /// Executes a lifecycle hook if the script defines it.
    pub fn run_hook(&self, hook: &str, context: VargRuntimeContext) -> VargRuntimeOutput {
        self.run_hook_borrowed(
            hook,
            VargRuntimeContextRef {
                transform: context.transform,
                input: &context.input,
                pointer_pressed: &[],
                pointer_released: &[],
                delta_time: context.delta_time,
                total_time: context.total_time,
                frame_index: context.frame_index,
                screen_size: context.screen_size,
                exported_values: &context.exported_values,
                state: context.state,
                scene: context.scene,
            },
        )
    }

    /// Executes a lifecycle hook using borrowed immutable frame inputs.
    pub fn run_hook_borrowed(
        &self,
        hook: &str,
        mut context: VargRuntimeContextRef<'_>,
    ) -> VargRuntimeOutput {
        self.run_hook_inner(hook, &mut context)
    }

    fn run_hook_inner(
        &self,
        hook: &str,
        context: &mut VargRuntimeContextRef<'_>,
    ) -> VargRuntimeOutput {
        for (name, value) in &self.state_defaults {
            context
                .state
                .entry(name.clone())
                .or_insert_with(|| value.clone());
        }

        let mut output = VargRuntimeOutput {
            transform: context.transform,
            state: std::mem::take(&mut context.state),
            logs: Vec::new(),
            ui_commands: Vec::new(),
            audio_commands: Vec::new(),
            render_commands: Vec::new(),
            spawn_requests: Vec::new(),
            destroy_nearest_requests: Vec::new(),
            destroy_self: false,
            mouse_capture: None,
        };
        let Some(statements) = self.hooks.get(hook) else {
            return output;
        };
        let mut env = RuntimeEnvironment {
            script: self,
            input: context.input,
            pointer_pressed: context.pointer_pressed,
            pointer_released: context.pointer_released,
            delta_time: context.delta_time,
            total_time: context.total_time,
            frame_index: context.frame_index,
            screen_size: context.screen_size,
            exported_values: context.exported_values,
            scene: &context.scene,
            transform: &mut output.transform,
            state: &mut output.state,
            locals: HashMap::new(),
            logs: &mut output.logs,
            ui_commands: &mut output.ui_commands,
            audio_commands: &mut output.audio_commands,
            render_commands: &mut output.render_commands,
            spawn_requests: &mut output.spawn_requests,
            destroy_nearest_requests: &mut output.destroy_nearest_requests,
            destroy_self: &mut output.destroy_self,
            mouse_capture: &mut output.mouse_capture,
            should_return: false,
            should_break: false,
            should_continue: false,
        };
        for statement in statements {
            env.execute(statement);
            if env.should_return {
                break;
            }
        }
        output
    }
}
fn quat_from_script_rotation(rotation: Vec3) -> Quat {
    Quat::from_euler_deg(rotation.z, rotation.y, rotation.x)
}

fn script_rotation_from_quat(rotation: Quat) -> Vec3 {
    let (yaw, pitch, roll) = rotation.to_euler_deg();
    Vec3::new(pitch, yaw, roll)
}
struct RuntimeEnvironment<'a> {
    script: &'a VargScript,
    input: &'a engine_platform::InputState,
    pointer_pressed: &'a [(f32, f32)],
    pointer_released: &'a [(f32, f32)],
    delta_time: f32,
    total_time: f32,
    frame_index: u64,
    screen_size: (f32, f32),
    exported_values: &'a HashMap<String, serde_json::Value>,
    scene: &'a VargSceneContext,
    transform: &'a mut Transform,
    state: &'a mut HashMap<String, serde_json::Value>,
    locals: HashMap<String, serde_json::Value>,
    logs: &'a mut Vec<String>,
    ui_commands: &'a mut Vec<VargUiCommand>,
    audio_commands: &'a mut Vec<VargAudioCommand>,
    render_commands: &'a mut Vec<VargRenderCommand>,
    spawn_requests: &'a mut Vec<VargSpawnRequest>,
    destroy_nearest_requests: &'a mut Vec<VargDestroyNearestRequest>,
    destroy_self: &'a mut bool,
    mouse_capture: &'a mut Option<bool>,
    /// When true, the current function should return.
    should_return: bool,
    /// When true, the current loop should break.
    should_break: bool,
    /// When true, skip to the next loop iteration.
    should_continue: bool,
}

impl RuntimeEnvironment<'_> {
    fn execute(&mut self, statement: &RuntimeStatement) {
        if self.should_return {
            return;
        }
        match statement {
            RuntimeStatement::Log(message) => self.logs.push(message.clone()),
            RuntimeStatement::Translate(expression) => {
                let delta = self.eval_vec3(expression);
                self.transform.translation += delta;
            }
            RuntimeStatement::SetPosition(expression) => {
                self.transform.translation = self.eval_vec3(expression);
            }
            RuntimeStatement::SetPositionAxis { axis, value } => {
                let value = self.eval_number(value);
                match axis {
                    Axis::X => self.transform.translation.x = value,
                    Axis::Y => self.transform.translation.y = value,
                    Axis::Z => self.transform.translation.z = value,
                }
            }
            RuntimeStatement::AddToPosition { axis, value } => {
                let value = self.eval_number(value);
                match axis {
                    Axis::X => self.transform.translation.x += value,
                    Axis::Y => self.transform.translation.y += value,
                    Axis::Z => self.transform.translation.z += value,
                }
            }
            RuntimeStatement::SetRotation(expression) => {
                self.transform.rotation = quat_from_script_rotation(self.eval_vec3(expression));
            }
            RuntimeStatement::SetRotationAxis { axis, value } => {
                let mut rotation = script_rotation_from_quat(self.transform.rotation);
                let value = self.eval_number(value);
                match axis {
                    Axis::X => rotation.x = value,
                    Axis::Y => rotation.y = value,
                    Axis::Z => rotation.z = value,
                }
                self.transform.rotation = quat_from_script_rotation(rotation);
            }
            RuntimeStatement::AddToRotation { axis, value } => {
                let mut rotation = script_rotation_from_quat(self.transform.rotation);
                let value = self.eval_number(value);
                match axis {
                    Axis::X => rotation.x += value,
                    Axis::Y => rotation.y += value,
                    Axis::Z => rotation.z += value,
                }
                self.transform.rotation = quat_from_script_rotation(rotation);
            }
            RuntimeStatement::AssignState { name, value } => {
                let value = self.eval_json(value);
                self.state.insert(name.clone(), value);
            }
            RuntimeStatement::AddToState { name, value } => {
                let current = self.state_number(name);
                let next = current + self.eval_number(value);
                self.state
                    .insert(name.clone(), serde_json::Value::from(next as f64));
            }
            RuntimeStatement::SubFromState { name, value } => {
                let current = self.state_number(name);
                let next = current - self.eval_number(value);
                self.state
                    .insert(name.clone(), serde_json::Value::from(next as f64));
            }
            RuntimeStatement::DeclareLocal { name, value } => {
                let value = self.eval_json(value);
                self.locals.insert(name.clone(), value);
            }
            RuntimeStatement::AssignBinding { name, value } => {
                let value = self.eval_json(value);
                if self.locals.contains_key(name) {
                    self.locals.insert(name.clone(), value);
                } else {
                    self.state.insert(name.clone(), value);
                }
            }
            RuntimeStatement::AddToBinding { name, value } => {
                let current = self.binding_number(name);
                let next = current + self.eval_number(value);
                self.assign_number_binding(name, next);
            }
            RuntimeStatement::SubFromBinding { name, value } => {
                let current = self.binding_number(name);
                let next = current - self.eval_number(value);
                self.assign_number_binding(name, next);
            }
            RuntimeStatement::If {
                condition,
                statements,
                else_statements,
            } => {
                let branch = if self.eval_condition(condition) {
                    statements
                } else {
                    else_statements
                };
                for statement in branch {
                    self.execute(statement);
                    if self.should_return || self.should_break || self.should_continue {
                        break;
                    }
                }
            }
            RuntimeStatement::ForLoop {
                variable,
                range,
                body,
            } => {
                let (start, end, inclusive) = match range {
                    RangeExpression::Range(s, e) => (
                        self.eval_number(s) as i32,
                        self.eval_number(e) as i32,
                        false,
                    ),
                    RangeExpression::RangeInclusive(s, e) => {
                        (self.eval_number(s) as i32, self.eval_number(e) as i32, true)
                    }
                    RangeExpression::Count(n) => (0, self.eval_number(n) as i32, false),
                };

                let limit = if inclusive { end + 1 } else { end };
                for i in start..limit {
                    self.locals
                        .insert(variable.clone(), serde_json::Value::from(i as f64));
                    self.should_continue = false;

                    for statement in body {
                        self.execute(statement);
                        if self.should_return || self.should_break || self.should_continue {
                            break;
                        }
                    }

                    if self.should_return || self.should_break {
                        break;
                    }
                }
                self.should_break = false;
                self.locals.remove(variable);
            }
            RuntimeStatement::WhileLoop { condition, body } => {
                const MAX_ITERATIONS: usize = 10000;
                let mut iterations = 0;

                while self.eval_condition(condition) && iterations < MAX_ITERATIONS {
                    iterations += 1;
                    self.should_continue = false;

                    for statement in body {
                        self.execute(statement);
                        if self.should_return || self.should_break || self.should_continue {
                            break;
                        }
                    }

                    if self.should_return || self.should_break {
                        break;
                    }
                }
                self.should_break = false;
            }
            RuntimeStatement::CallFunction(name) => {
                self.execute_function(name);
            }
            RuntimeStatement::Return(_) => {
                self.should_return = true;
            }
            RuntimeStatement::Break => {
                self.should_break = true;
            }
            RuntimeStatement::Continue => {
                self.should_continue = true;
            }
            RuntimeStatement::Wait(duration) => {
                let seconds = self.eval_number(duration);
                if seconds > 0.0 {
                    let timer_key = "__wait_timer";

                    // Check if we're already waiting
                    if let Some(remaining) = self.state.get(timer_key).and_then(|v| v.as_f64()) {
                        let remaining = remaining as f32;
                        let new_remaining = remaining - self.delta_time;

                        if new_remaining > 0.0 {
                            // Still waiting
                            self.state.insert(
                                timer_key.to_string(),
                                serde_json::Value::from(new_remaining as f64),
                            );
                            self.should_return = true;
                        } else {
                            // Wait finished, clear timer and continue
                            self.state.remove(timer_key);
                        }
                    } else {
                        // Start new wait
                        self.state.insert(
                            timer_key.to_string(),
                            serde_json::Value::from(seconds as f64),
                        );
                        self.should_return = true;
                    }
                }
            }
            RuntimeStatement::DestroySelf => {
                *self.destroy_self = true;
                self.should_return = true;
            }
            RuntimeStatement::SpawnBox {
                name,
                tag,
                position,
                size,
                script,
            } => {
                let name = self
                    .eval_string(name)
                    .unwrap_or_else(|| "Spawned Box".to_string());
                let tag = self.eval_string(tag).unwrap_or_default();
                let position = self.eval_vec3(position);
                let size = self.eval_vec3(size);
                let script = self.empty_string_as_none(script);
                self.spawn_requests.push(VargSpawnRequest {
                    name,
                    tag,
                    builtin_mesh: "debug/cube".to_string(),
                    collider_shape: "box".to_string(),
                    position,
                    size,
                    script,
                });
            }
            RuntimeStatement::SpawnSphere {
                name,
                tag,
                position,
                radius,
                script,
            } => {
                let diameter = self.eval_number(radius).max(0.0) * 2.0;
                let name = self
                    .eval_string(name)
                    .unwrap_or_else(|| "Spawned Sphere".to_string());
                let tag = self.eval_string(tag).unwrap_or_default();
                let position = self.eval_vec3(position);
                let script = self.empty_string_as_none(script);
                self.spawn_requests.push(VargSpawnRequest {
                    name,
                    tag,
                    builtin_mesh: "debug/sphere".to_string(),
                    collider_shape: "sphere".to_string(),
                    position,
                    size: Vec3::new(diameter, diameter, diameter),
                    script,
                });
            }
            RuntimeStatement::DestroyNearestWithTag { tag, radius } => {
                let tag = self.eval_string(tag).unwrap_or_default();
                let radius = self.eval_number(radius).max(0.0);
                self.destroy_nearest_requests
                    .push(VargDestroyNearestRequest {
                        tag,
                        radius,
                        origin: self.transform.translation,
                    });
            }
            RuntimeStatement::PlayTone {
                waveform,
                frequency,
                duration,
                volume,
                spatial,
            } => {
                let waveform = self
                    .eval_string(waveform)
                    .unwrap_or_else(|| "sine".to_string());
                let frequency_hz = self.eval_number(frequency);
                let duration_seconds = self.eval_number(duration);
                let volume = self.eval_number(volume);
                self.audio_commands.push(VargAudioCommand::PlayTone {
                    waveform,
                    frequency_hz,
                    duration_seconds,
                    volume,
                    spatial: *spatial,
                    position: self.transform.translation,
                });
            }
            RuntimeStatement::StartAudioLoop {
                id,
                waveform,
                pattern,
                bpm,
                beats_per_note,
                volume,
            } => {
                let id = self.eval_string(id).unwrap_or_else(|| "main".to_string());
                let waveform = self
                    .eval_string(waveform)
                    .unwrap_or_else(|| "sine".to_string());
                let pattern = self.eval_string(pattern).unwrap_or_default();
                let bpm = self.eval_number(bpm);
                let beats_per_note = self.eval_number(beats_per_note);
                let volume = self.eval_number(volume);
                self.audio_commands.push(VargAudioCommand::StartLoop {
                    id,
                    waveform,
                    pattern,
                    bpm,
                    beats_per_note,
                    volume,
                });
            }
            RuntimeStatement::StopAudioLoop { id } => {
                self.audio_commands.push(VargAudioCommand::StopLoop {
                    id: self.eval_string(id).unwrap_or_else(|| "main".to_string()),
                });
            }
            RuntimeStatement::UseScreenSpaceGi => {
                self.render_commands
                    .push(VargRenderCommand::UseScreenSpaceGi);
            }
            RuntimeStatement::UseProbeVolumeGi {
                center,
                extent,
                counts,
                intensity,
            } => {
                let center = self.eval_vec3(center);
                let extent = self.eval_vec3(extent);
                let counts = self.eval_vec3(counts);
                let intensity = self.eval_number(intensity);
                self.render_commands
                    .push(VargRenderCommand::UseProbeVolumeGi {
                        center,
                        extent,
                        counts,
                        intensity,
                    });
            }
            RuntimeStatement::SetGiIntensity(intensity) => {
                let intensity = self.eval_number(intensity);
                self.render_commands
                    .push(VargRenderCommand::SetGiIntensity(intensity));
            }
            RuntimeStatement::SetWeatherPreset(preset) => {
                let preset = self
                    .eval_string(preset)
                    .unwrap_or_else(|| "clear".to_string());
                self.render_commands
                    .push(VargRenderCommand::SetWeatherPreset(preset));
            }
            RuntimeStatement::SetWeatherTimeOfDay(time_of_day) => {
                let time_of_day = self.eval_number(time_of_day);
                self.render_commands
                    .push(VargRenderCommand::SetWeatherTimeOfDay(time_of_day));
            }
            RuntimeStatement::SetWeatherCloudCover(cloud_cover) => {
                let cloud_cover = self.eval_number(cloud_cover);
                self.render_commands
                    .push(VargRenderCommand::SetWeatherCloudCover(cloud_cover));
            }
            RuntimeStatement::SetWeatherPrecipitation(precipitation) => {
                let precipitation = self.eval_number(precipitation);
                self.render_commands
                    .push(VargRenderCommand::SetWeatherPrecipitation(precipitation));
            }
            RuntimeStatement::SetWeatherWind(wind) => {
                let wind = self.eval_vec3(wind);
                self.render_commands
                    .push(VargRenderCommand::SetWeatherWind(wind));
            }
            RuntimeStatement::SetMouseCapture(expression) => {
                *self.mouse_capture = Some(self.eval_bool(expression));
            }
            RuntimeStatement::UiLabel { id, text, x, y } => {
                let id = self.eval_string(id).unwrap_or_default();
                let text = self.eval_display_string(text);
                let x = self.eval_number(x);
                let y = self.eval_number(y);
                self.ui_commands
                    .push(VargUiCommand::Label { id, text, x, y });
            }
            RuntimeStatement::UiRect {
                id,
                x,
                y,
                width,
                height,
                color,
            } => {
                let id = self.eval_string(id).unwrap_or_default();
                let x = self.eval_number(x);
                let y = self.eval_number(y);
                let width = self.eval_number(width).max(0.0);
                let height = self.eval_number(height).max(0.0);
                let color = [
                    self.eval_number(&color[0]).clamp(0.0, 1.0),
                    self.eval_number(&color[1]).clamp(0.0, 1.0),
                    self.eval_number(&color[2]).clamp(0.0, 1.0),
                    self.eval_number(&color[3]).clamp(0.0, 1.0),
                ];
                self.ui_commands.push(VargUiCommand::Rect {
                    id,
                    x,
                    y,
                    width,
                    height,
                    color,
                });
            }
            RuntimeStatement::UiTexture {
                id,
                texture,
                x,
                y,
                width,
                height,
                color,
            } => {
                let id = self.eval_string(id).unwrap_or_default();
                let texture = self.eval_string(texture).unwrap_or_default();
                let x = self.eval_number(x);
                let y = self.eval_number(y);
                let width = self.eval_number(width).max(0.0);
                let height = self.eval_number(height).max(0.0);
                let color = [
                    self.eval_number(&color[0]).clamp(0.0, 1.0),
                    self.eval_number(&color[1]).clamp(0.0, 1.0),
                    self.eval_number(&color[2]).clamp(0.0, 1.0),
                    self.eval_number(&color[3]).clamp(0.0, 1.0),
                ];
                self.ui_commands.push(VargUiCommand::Texture {
                    id,
                    texture,
                    x,
                    y,
                    width,
                    height,
                    color,
                });
            }
        }
    }

    fn eval_condition(&mut self, condition: &ConditionExpression) -> bool {
        match condition {
            ConditionExpression::InputDown(action) | ConditionExpression::ActionDown(action) => {
                input_action_down(self.input, action)
            }
            ConditionExpression::InputJustPressed(action) => {
                input_action_pressed(self.input, action)
            }
            ConditionExpression::InputJustReleased(action) => {
                input_action_released(self.input, action)
            }
            ConditionExpression::ActionUp(action) => !input_action_down(self.input, action),
            ConditionExpression::ActionJustPressed(action) => {
                input_action_pressed(self.input, action)
            }
            ConditionExpression::ActionJustReleased(action) => {
                input_action_released(self.input, action)
            }
            ConditionExpression::Not(condition) => !self.eval_condition(condition),
            ConditionExpression::And(lhs, rhs) => {
                self.eval_condition(lhs) && self.eval_condition(rhs)
            }
            ConditionExpression::Or(lhs, rhs) => {
                self.eval_condition(lhs) || self.eval_condition(rhs)
            }
            ConditionExpression::Compare { lhs, op, rhs } => {
                let lhs = self.eval_number(lhs);
                let rhs = self.eval_number(rhs);
                match op {
                    CompareOp::Equal => (lhs - rhs).abs() <= f32::EPSILON,
                    CompareOp::NotEqual => (lhs - rhs).abs() > f32::EPSILON,
                    CompareOp::GreaterThan => lhs > rhs,
                    CompareOp::GreaterThanOrEqual => lhs >= rhs,
                    CompareOp::LessThan => lhs < rhs,
                    CompareOp::LessThanOrEqual => lhs <= rhs,
                }
            }
        }
    }

    fn eval_vec3(&mut self, expression: &Expression) -> Vec3 {
        match expression {
            Expression::Vec3(x, y, z) => Vec3::new(
                self.eval_number(x),
                self.eval_number(y),
                self.eval_number(z),
            ),
            _ => Vec3::new(self.eval_number(expression), 0.0, 0.0),
        }
    }

    fn eval_json(&mut self, expression: &Expression) -> serde_json::Value {
        match expression {
            Expression::String(value) => serde_json::Value::String(value.clone()),
            Expression::Bool(value) => serde_json::Value::Bool(*value),
            Expression::Call { function, args }
                if matches!(function.as_str(), "ui.input" | "UI.input") =>
            {
                serde_json::Value::String(self.eval_ui_input_string(args))
            }
            _ => serde_json::Value::from(self.eval_number(expression) as f64),
        }
    }

    fn eval_bool(&mut self, expression: &Expression) -> bool {
        match expression {
            Expression::Bool(value) => *value,
            Expression::String(value) => !value.is_empty(),
            _ => self.eval_number(expression).abs() > f32::EPSILON,
        }
    }

    fn eval_number(&mut self, expression: &Expression) -> f32 {
        match expression {
            Expression::Number(value) => *value,
            Expression::String(_) => 0.0,
            Expression::Bool(value) => {
                if *value {
                    1.0
                } else {
                    0.0
                }
            }
            Expression::Variable(name) => self.variable_number(name),
            Expression::Member(owner, field) => self.member_number(owner, field),
            Expression::Call { function, args } => self.call_number(function, args),
            Expression::Vec3(_, _, _) => 0.0,
            Expression::Binary { op, lhs, rhs } => {
                let lhs = self.eval_number(lhs);
                let rhs = self.eval_number(rhs);
                match op {
                    BinaryOp::Add => lhs + rhs,
                    BinaryOp::Sub => lhs - rhs,
                    BinaryOp::Mul => lhs * rhs,
                    BinaryOp::Div => {
                        if rhs.abs() <= f32::EPSILON {
                            0.0
                        } else {
                            lhs / rhs
                        }
                    }
                }
            }
        }
    }

    fn variable_number(&self, name: &str) -> f32 {
        if name == "dt" {
            return self.delta_time;
        }
        if name == "time" {
            return self.total_time;
        }
        if let Some(value) = self
            .exported_values
            .get(name)
            .or_else(|| self.locals.get(name))
            .or_else(|| self.state.get(name))
            .and_then(json_number)
        {
            return value;
        }
        self.script
            .exports
            .iter()
            .find(|export| export.name == name)
            .and_then(|export| export.default_value.as_ref())
            .and_then(|value| parse_default_literal(value))
            .and_then(|value| json_number(&value))
            .unwrap_or(0.0)
    }

    fn member_number(&self, owner: &str, field: &str) -> f32 {
        match (owner, field) {
            ("entity.position", "x") | ("position", "x") => self.transform.translation.x,
            ("entity.position", "y") | ("position", "y") => self.transform.translation.y,
            ("entity.position", "z") | ("position", "z") => self.transform.translation.z,
            ("Input", "moveX") => self.input.action_value("MoveX"),
            ("Input", "moveY") => self.input.action_value("MoveY"),
            ("Input", "mouseDeltaX") | ("Input", "mouseDx") => self.input.mouse_delta().0,
            ("Input", "mouseDeltaY") | ("Input", "mouseDy") => self.input.mouse_delta().1,
            ("Input", "wheelX") => self.input.wheel_delta().0,
            ("Input", "wheelY") => self.input.wheel_delta().1,
            ("Input", "cursorX") => self
                .input
                .cursor_position()
                .map(|position| position.0)
                .unwrap_or(0.0),
            ("Input", "cursorY") => self
                .input
                .cursor_position()
                .map(|position| position.1)
                .unwrap_or(0.0),
            ("InputAction", action) => self.input.action_value(action),
            ("Time", "time") | ("Time", "elapsed") => self.total_time,
            ("Time", "delta") | ("Time", "dt") => self.delta_time,
            ("Time", "frame") => self.frame_index as f32,
            ("state", name) => self.state.get(name).and_then(json_number).unwrap_or(0.0),
            _ => self
                .state
                .get(owner)
                .and_then(|value| value.get(field))
                .and_then(json_number)
                .unwrap_or(0.0),
        }
    }

    fn state_number(&self, name: &str) -> f32 {
        self.state.get(name).and_then(json_number).unwrap_or(0.0)
    }

    fn binding_number(&self, name: &str) -> f32 {
        self.locals
            .get(name)
            .or_else(|| self.state.get(name))
            .and_then(json_number)
            .unwrap_or(0.0)
    }

    fn assign_number_binding(&mut self, name: &str, value: f32) {
        let value = serde_json::Value::from(value as f64);
        if self.locals.contains_key(name) {
            self.locals.insert(name.to_string(), value);
        } else {
            self.state.insert(name.to_string(), value);
        }
    }

    fn call_number(&mut self, function: &str, args: &[Expression]) -> f32 {
        match function {
            "entity.hasTag" => {
                if args.len() != 1 {
                    return 0.0;
                }
                self.eval_string(&args[0])
                    .is_some_and(|tag| self.scene.entity_has_tag(&tag)) as u8 as f32
            }
            "scene.distanceTo" | "distanceTo" => {
                if args.len() != 1 {
                    return 0.0;
                }
                self.eval_string(&args[0])
                    .and_then(|name| {
                        self.scene
                            .distance_to_name(self.transform.translation, &name)
                    })
                    .unwrap_or(0.0)
            }
            "scene.distanceToTag" | "distanceToTag" => {
                if args.len() != 1 {
                    return 0.0;
                }
                self.eval_string(&args[0])
                    .and_then(|tag| self.scene.distance_to_tag(self.transform.translation, &tag))
                    .unwrap_or(0.0)
            }
            "scene.distanceToTagBounds" | "distanceToTagBounds" => {
                if args.len() != 1 {
                    return 0.0;
                }
                self.eval_string(&args[0])
                    .and_then(|tag| {
                        self.scene
                            .distance_to_tag_bounds(self.transform.translation, &tag)
                    })
                    .unwrap_or(0.0)
            }
            "scene.horizontalDistanceToTagBounds" | "horizontalDistanceToTagBounds" => {
                if args.len() != 1 {
                    return 0.0;
                }
                self.eval_string(&args[0])
                    .and_then(|tag| {
                        self.scene
                            .horizontal_distance_to_tag_bounds(self.transform.translation, &tag)
                    })
                    .unwrap_or(0.0)
            }
            "playerDistance" | "scene.playerDistance" => self
                .scene
                .distance_to_tag(self.transform.translation, "Player")
                .or_else(|| {
                    self.scene
                        .distance_to_name(self.transform.translation, "Player")
                })
                .unwrap_or(0.0),
            "scene.xOf" | "xOf" => {
                if args.len() != 1 {
                    return 0.0;
                }
                self.eval_string(&args[0])
                    .and_then(|name| self.scene.x_of_name(&name))
                    .unwrap_or(0.0)
            }
            "scene.yOf" | "yOf" => {
                if args.len() != 1 {
                    return 0.0;
                }
                self.eval_string(&args[0])
                    .and_then(|name| self.scene.y_of_name(&name))
                    .unwrap_or(0.0)
            }
            "scene.zOf" | "zOf" => {
                if args.len() != 1 {
                    return 0.0;
                }
                self.eval_string(&args[0])
                    .and_then(|name| self.scene.z_of_name(&name))
                    .unwrap_or(0.0)
            }
            "Input.mouseDeltaX" | "Input.mouseDx" => self.input.mouse_delta().0,
            "Input.mouseDeltaY" | "Input.mouseDy" => self.input.mouse_delta().1,
            "Input.wheelX" => self.input.wheel_delta().0,
            "Input.wheelY" => self.input.wheel_delta().1,
            "Input.cursorX" => self
                .input
                .cursor_position()
                .map(|position| position.0)
                .unwrap_or(0.0),
            "Input.cursorY" => self
                .input
                .cursor_position()
                .map(|position| position.1)
                .unwrap_or(0.0),
            "ui.screenWidth" | "UI.screenWidth" => self.screen_size.0.max(1.0),
            "ui.screenHeight" | "UI.screenHeight" => self.screen_size.1.max(1.0),
            "Input.pointerDown" | "Input.touchDown" => self
                .input
                .mouse_button_down(engine_platform::MouseButton::Left)
                as u8 as f32,
            "Input.pointerPressed" | "Input.touchPressed" => {
                (!self.pointer_pressed.is_empty()) as u8 as f32
            }
            "Input.pointerReleased" | "Input.touchReleased" => {
                (!self.pointer_released.is_empty()) as u8 as f32
            }
            "sin" | "Math.sin" => self.unary_math(args, f32::sin),
            "cos" | "Math.cos" => self.unary_math(args, f32::cos),
            "tan" | "Math.tan" => self.unary_math(args, f32::tan),
            "abs" | "Math.abs" => self.unary_math(args, f32::abs),
            "sqrt" | "Math.sqrt" => self.unary_math(args, |value| value.max(0.0).sqrt()),
            "floor" | "Math.floor" => self.unary_math(args, f32::floor),
            "ceil" | "Math.ceil" => self.unary_math(args, f32::ceil),
            "round" | "Math.round" => self.unary_math(args, f32::round),
            "min" | "Math.min" => args
                .iter()
                .map(|arg| self.eval_number(arg))
                .reduce(f32::min)
                .unwrap_or(0.0),
            "max" | "Math.max" => args
                .iter()
                .map(|arg| self.eval_number(arg))
                .reduce(f32::max)
                .unwrap_or(0.0),
            "clamp" | "Math.clamp" => {
                if args.len() != 3 {
                    return 0.0;
                }
                self.eval_number(&args[0])
                    .clamp(self.eval_number(&args[1]), self.eval_number(&args[2]))
            }
            "lerp" | "Math.lerp" => {
                if args.len() != 3 {
                    return 0.0;
                }
                let from = self.eval_number(&args[0]);
                let to = self.eval_number(&args[1]);
                let t = self.eval_number(&args[2]);
                from + (to - from) * t
            }
            "smoothstep" | "Math.smoothstep" => {
                if args.len() != 3 {
                    return 0.0;
                }
                let edge0 = self.eval_number(&args[0]);
                let edge1 = self.eval_number(&args[1]);
                if (edge1 - edge0).abs() <= f32::EPSILON {
                    return 0.0;
                }
                let t = ((self.eval_number(&args[2]) - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            }
            "easeIn" | "Math.easeIn" => {
                let t = args
                    .first()
                    .map(|arg| self.eval_number(arg).clamp(0.0, 1.0))
                    .unwrap_or(0.0);
                t * t
            }
            "easeOut" | "Math.easeOut" => {
                let t = args
                    .first()
                    .map(|arg| self.eval_number(arg).clamp(0.0, 1.0))
                    .unwrap_or(0.0);
                1.0 - (1.0 - t) * (1.0 - t)
            }
            "easeInOut" | "Math.easeInOut" => {
                let t = args
                    .first()
                    .map(|arg| self.eval_number(arg).clamp(0.0, 1.0))
                    .unwrap_or(0.0);
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) * 0.5
                }
            }
            "pulse" | "Math.pulse" => {
                let time = args
                    .first()
                    .map(|arg| self.eval_number(arg))
                    .unwrap_or(self.total_time);
                let frequency = args.get(1).map(|arg| self.eval_number(arg)).unwrap_or(1.0);
                (time * frequency * std::f32::consts::TAU).sin() * 0.5 + 0.5
            }
            "ui.button" | "UI.button" => {
                if args.len() != 6 {
                    return 0.0;
                }
                let id = self.eval_string(&args[0]).unwrap_or_default();
                let text = self.eval_display_string(&args[1]);
                let x = self.eval_number(&args[2]);
                let y = self.eval_number(&args[3]);
                let width = self.eval_number(&args[4]).max(0.0);
                let height = self.eval_number(&args[5]).max(0.0);
                let hot = self.pointer_over_rect(x, y, width, height);
                let pressed = hot
                    && self
                        .input
                        .mouse_button_down(engine_platform::MouseButton::Left);
                self.ui_commands.push(VargUiCommand::Rect {
                    id: format!("{id}:bg"),
                    x,
                    y: if pressed { y + 1.0 } else { y },
                    width,
                    height,
                    color: if pressed {
                        [0.12, 0.24, 0.32, 0.98]
                    } else if hot {
                        [0.18, 0.32, 0.42, 0.94]
                    } else {
                        [0.08, 0.1, 0.14, 0.92]
                    },
                });
                self.ui_commands.push(VargUiCommand::Label {
                    id: format!("{id}:label"),
                    text,
                    x: x + 16.0,
                    y: y + height * 0.5 - 6.0,
                });
                self.pointer_released
                    .iter()
                    .any(|(px, py)| point_in_rect(*px, *py, x, y, width, height))
                    as u8 as f32
            }
            "ui.toggle" | "UI.toggle" => {
                if args.len() != 6 {
                    return 0.0;
                }
                let id = self.eval_string(&args[0]).unwrap_or_default();
                let current = self.eval_bool(&args[1]);
                let x = self.eval_number(&args[2]);
                let y = self.eval_number(&args[3]);
                let width = self.eval_number(&args[4]).max(0.0);
                let height = self.eval_number(&args[5]).max(0.0);
                let clicked = self
                    .pointer_released
                    .iter()
                    .any(|(px, py)| point_in_rect(*px, *py, x, y, width, height));
                let next = if clicked { !current } else { current };
                self.ui_commands.push(VargUiCommand::Rect {
                    id: format!("{id}:track"),
                    x,
                    y,
                    width,
                    height,
                    color: if next {
                        [0.14, 0.48, 0.36, 0.95]
                    } else {
                        [0.12, 0.14, 0.18, 0.92]
                    },
                });
                let knob_size = (height - 6.0).max(0.0);
                let knob_x = if next {
                    x + width - knob_size - 3.0
                } else {
                    x + 3.0
                };
                self.ui_commands.push(VargUiCommand::Rect {
                    id: format!("{id}:knob"),
                    x: knob_x,
                    y: y + 3.0,
                    width: knob_size,
                    height: knob_size,
                    color: [0.95, 0.97, 1.0, 1.0],
                });
                next as u8 as f32
            }
            "ui.slider" | "UI.slider" => {
                if args.len() != 8 {
                    return 0.0;
                }
                let id = self.eval_string(&args[0]).unwrap_or_default();
                let current = self.eval_number(&args[1]);
                let x = self.eval_number(&args[2]);
                let y = self.eval_number(&args[3]);
                let width = self.eval_number(&args[4]).max(1.0);
                let height = self.eval_number(&args[5]).max(1.0);
                let min = self.eval_number(&args[6]);
                let max = self.eval_number(&args[7]);
                let active = self.ui_drag_active(&id, x, y, width, height);
                let next = if active {
                    let cursor_x = self
                        .input
                        .cursor_position()
                        .map(|position| position.0)
                        .unwrap_or(x);
                    let t = ((cursor_x - x) / width).clamp(0.0, 1.0);
                    min + (max - min) * t
                } else {
                    current
                };
                let range = max - min;
                let t = if range.abs() <= f32::EPSILON {
                    0.0
                } else {
                    ((next - min) / range).clamp(0.0, 1.0)
                };
                let track_y = y + height * 0.5 - 3.0;
                self.ui_commands.push(VargUiCommand::Rect {
                    id: format!("{id}:track"),
                    x,
                    y: track_y,
                    width,
                    height: 6.0,
                    color: [0.12, 0.14, 0.18, 0.92],
                });
                self.ui_commands.push(VargUiCommand::Rect {
                    id: format!("{id}:fill"),
                    x,
                    y: track_y,
                    width: width * t,
                    height: 6.0,
                    color: [0.18, 0.5, 0.78, 0.96],
                });
                self.ui_commands.push(VargUiCommand::Rect {
                    id: format!("{id}:thumb"),
                    x: x + width * t - 5.0,
                    y: y + height * 0.5 - 8.0,
                    width: 10.0,
                    height: 16.0,
                    color: if active {
                        [1.0, 1.0, 1.0, 1.0]
                    } else {
                        [0.82, 0.88, 0.95, 1.0]
                    },
                });
                next
            }
            "ui.dragArea" | "UI.dragArea" => {
                if args.len() != 5 {
                    return 0.0;
                }
                let id = self.eval_string(&args[0]).unwrap_or_default();
                let x = self.eval_number(&args[1]);
                let y = self.eval_number(&args[2]);
                let width = self.eval_number(&args[3]).max(0.0);
                let height = self.eval_number(&args[4]).max(0.0);
                self.ui_drag_active(&id, x, y, width, height) as u8 as f32
            }
            "ui.dragX" | "UI.dragX" => {
                if args.len() != 5 {
                    return 0.0;
                }
                let id = self.eval_string(&args[0]).unwrap_or_default();
                let x = self.eval_number(&args[1]);
                let y = self.eval_number(&args[2]);
                let width = self.eval_number(&args[3]).max(0.0);
                let height = self.eval_number(&args[4]).max(0.0);
                if self.ui_drag_active(&id, x, y, width, height) {
                    self.input.mouse_delta().0
                } else {
                    0.0
                }
            }
            "ui.dragY" | "UI.dragY" => {
                if args.len() != 5 {
                    return 0.0;
                }
                let id = self.eval_string(&args[0]).unwrap_or_default();
                let x = self.eval_number(&args[1]);
                let y = self.eval_number(&args[2]);
                let width = self.eval_number(&args[3]).max(0.0);
                let height = self.eval_number(&args[4]).max(0.0);
                if self.ui_drag_active(&id, x, y, width, height) {
                    self.input.mouse_delta().1
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }

    fn pointer_over_rect(&self, x: f32, y: f32, width: f32, height: f32) -> bool {
        self.input
            .cursor_position()
            .is_some_and(|(px, py)| point_in_rect(px, py, x, y, width, height))
    }

    fn ui_drag_active(&mut self, id: &str, x: f32, y: f32, width: f32, height: f32) -> bool {
        let active_key = "__ui_drag_active";
        let pointer_down = self
            .input
            .mouse_button_down(engine_platform::MouseButton::Left);
        if !pointer_down {
            if self.state.get(active_key).and_then(|value| value.as_str()) == Some(id) {
                self.state.remove(active_key);
            }
            return false;
        }
        if self.state.get(active_key).and_then(|value| value.as_str()) == Some(id) {
            return true;
        }
        if self
            .pointer_pressed
            .iter()
            .any(|(px, py)| point_in_rect(*px, *py, x, y, width, height))
        {
            self.state.insert(
                active_key.to_string(),
                serde_json::Value::String(id.to_string()),
            );
            return true;
        }
        false
    }

    fn eval_ui_input_string(&mut self, args: &[Expression]) -> String {
        if args.len() != 6 {
            return String::new();
        }
        let id = self.eval_string(&args[0]).unwrap_or_default();
        let placeholder = self.eval_display_string(&args[1]);
        let x = self.eval_number(&args[2]);
        let y = self.eval_number(&args[3]);
        let width = self.eval_number(&args[4]).max(0.0);
        let height = self.eval_number(&args[5]).max(0.0);
        let value_key = format!("__ui_input:{id}");
        let focus_key = "__ui_focus";
        if !self.pointer_released.is_empty() {
            let hit = self
                .pointer_released
                .iter()
                .any(|(px, py)| point_in_rect(*px, *py, x, y, width, height));
            if hit {
                self.state
                    .insert(focus_key.to_string(), serde_json::Value::String(id.clone()));
            } else if self.state.get(focus_key).and_then(|value| value.as_str())
                == Some(id.as_str())
            {
                self.state.remove(focus_key);
            }
        }
        let focused =
            self.state.get(focus_key).and_then(|value| value.as_str()) == Some(id.as_str());
        let mut text = self
            .state
            .get(&value_key)
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_default();
        if focused {
            for key in self.input.pressed_keys() {
                match key {
                    engine_platform::KeyCode::Backspace => {
                        text.pop();
                    }
                    engine_platform::KeyCode::Enter => {
                        self.state.remove(focus_key);
                    }
                    engine_platform::KeyCode::Space => text.push(' '),
                    engine_platform::KeyCode::Character(ch) if !ch.is_control() => text.push(ch),
                    _ => {}
                }
            }
            self.state
                .insert(value_key, serde_json::Value::String(text.clone()));
        }
        self.ui_commands.push(VargUiCommand::Rect {
            id: format!("{id}:input_bg"),
            x,
            y,
            width,
            height,
            color: if focused {
                [0.1, 0.16, 0.24, 0.96]
            } else {
                [0.08, 0.1, 0.14, 0.92]
            },
        });
        let display = if text.is_empty() {
            placeholder
        } else if focused && (self.frame_index / 30).is_multiple_of(2) {
            format!("{text}|")
        } else {
            text.clone()
        };
        self.ui_commands.push(VargUiCommand::Label {
            id: format!("{id}:input_text"),
            text: display,
            x: x + 10.0,
            y: y + height * 0.5 - 6.0,
        });
        text
    }

    fn eval_string(&self, expression: &Expression) -> Option<String> {
        match expression {
            Expression::String(value) => Some(value.clone()),
            Expression::Variable(name) => self
                .locals
                .get(name)
                .or_else(|| self.state.get(name))
                .and_then(|value| value.as_str())
                .map(str::to_string),
            Expression::Member(owner, field) => self
                .state
                .get(owner)
                .and_then(|value| value.get(field))
                .and_then(|value| value.as_str())
                .map(str::to_string),
            _ => None,
        }
    }

    fn empty_string_as_none(&mut self, expression: &Expression) -> Option<String> {
        self.eval_string(expression)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn eval_display_string(&mut self, expression: &Expression) -> String {
        match expression {
            Expression::String(value) => value.clone(),
            Expression::Number(value) => format_display_number(*value),
            Expression::Bool(value) => value.to_string(),
            Expression::Variable(name) => self
                .exported_values
                .get(name)
                .or_else(|| self.locals.get(name))
                .or_else(|| self.state.get(name))
                .map(json_display_string)
                .or_else(|| {
                    self.script
                        .exports
                        .iter()
                        .find(|export| export.name == *name)
                        .and_then(|export| export.default_value.as_ref())
                        .and_then(|value| parse_default_literal(value))
                        .map(|value| json_display_string(&value))
                })
                .unwrap_or_else(|| format_display_number(self.eval_number(expression))),
            Expression::Member(owner, field) => self
                .state
                .get(owner)
                .and_then(|value| value.get(field))
                .map(json_display_string)
                .unwrap_or_else(|| format_display_number(self.member_number(owner, field))),
            Expression::Call { function, args }
                if matches!(function.as_str(), "ui.input" | "UI.input") =>
            {
                self.eval_ui_input_string(args)
            }
            Expression::Call { .. } => format_display_number(self.eval_number(expression)),
            Expression::Vec3(_, _, _) => format_display_number(self.eval_number(expression)),
            Expression::Binary { op, lhs, rhs } => match op {
                BinaryOp::Add
                    if self.expression_prefers_text(lhs) || self.expression_prefers_text(rhs) =>
                {
                    format!(
                        "{}{}",
                        self.eval_display_string(lhs),
                        self.eval_display_string(rhs)
                    )
                }
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                    format_display_number(self.eval_number(expression))
                }
            },
        }
    }

    fn expression_prefers_text(&self, expression: &Expression) -> bool {
        match expression {
            Expression::String(_) => true,
            Expression::Variable(name) => self
                .exported_values
                .get(name)
                .or_else(|| self.locals.get(name))
                .or_else(|| self.state.get(name))
                .is_some_and(serde_json::Value::is_string),
            Expression::Member(owner, field) => self
                .state
                .get(owner)
                .and_then(|value| value.get(field))
                .is_some_and(serde_json::Value::is_string),
            Expression::Binary {
                op: BinaryOp::Add,
                lhs,
                rhs,
            } => self.expression_prefers_text(lhs) || self.expression_prefers_text(rhs),
            _ => false,
        }
    }

    fn unary_math(&mut self, args: &[Expression], op: impl FnOnce(f32) -> f32) -> f32 {
        args.first()
            .map(|arg| op(self.eval_number(arg)))
            .unwrap_or(0.0)
    }

    fn execute_function(&mut self, name: &str) {
        let Some(statements) = self.script.hooks.get(name).cloned() else {
            return;
        };
        let was_returning = self.should_return;
        self.should_return = false;
        for statement in &statements {
            self.execute(statement);
            if self.should_return || self.should_break || self.should_continue {
                break;
            }
        }
        self.should_return = was_returning;
    }
}

fn compile_runtime_blocks(source: &str, script: &mut VargScript) {
    let lines = source.lines().collect::<Vec<_>>();
    let functions = collect_runtime_function_names(&lines);
    let mut index = 0usize;
    while index < lines.len() {
        let trimmed = strip_line_comment(lines[index]).trim();
        if trimmed.starts_with("var ") {
            if let Some((name, value)) = parse_state_default(trimmed) {
                script.state_defaults.insert(name, value);
            }
        }
        if let Some(signature) = parse_function_signature(trimmed) {
            let (body, next) = collect_block(&lines, index);
            let mut body_index = 0usize;
            let statements = parse_runtime_statements(&body, &mut body_index, &functions);
            script.hooks.insert(signature.name, statements);
            index = next;
            continue;
        }
        index += 1;
    }
}

pub(crate) fn diagnose_runtime_blocks(source: &str) -> Vec<VargDiagnostic> {
    let lines = source.lines().collect::<Vec<_>>();
    let functions = collect_runtime_function_names(&lines);
    let mut diagnostics = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let trimmed = strip_line_comment(lines[index]).trim();
        if let Some(_signature) = parse_function_signature(trimmed) {
            let (body, next) = collect_block(&lines, index);
            let mut body_index = 0usize;
            let _ = parse_runtime_statements_with_diagnostics(
                &body,
                &mut body_index,
                &mut diagnostics,
                source,
                &functions,
            );
            index = next;
            continue;
        }
        index += 1;
    }
    diagnostics
}

fn parse_runtime_statements(
    lines: &[RuntimeLine],
    index: &mut usize,
    functions: &std::collections::HashSet<String>,
) -> Vec<RuntimeStatement> {
    let mut diagnostics = Vec::new();
    parse_runtime_statements_with_diagnostics(lines, index, &mut diagnostics, "", functions)
}

fn parse_runtime_statements_with_diagnostics(
    lines: &[RuntimeLine],
    index: &mut usize,
    diagnostics: &mut Vec<VargDiagnostic>,
    source: &str,
    functions: &std::collections::HashSet<String>,
) -> Vec<RuntimeStatement> {
    let mut statements = Vec::new();
    while *index < lines.len() {
        let line = &lines[*index];
        let trimmed = strip_line_comment(&line.text).trim();
        *index += 1;
        if trimmed.is_empty() || trimmed == "}" {
            continue;
        }
        if let Some(condition) = parse_if_condition(trimmed) {
            let nested = collect_inline_or_block(lines, index);
            let mut nested_index = 0usize;
            let else_nested = collect_else_block(lines, index);
            let mut else_index = 0usize;
            statements.push(RuntimeStatement::If {
                condition,
                statements: parse_runtime_statements_with_diagnostics(
                    &nested,
                    &mut nested_index,
                    diagnostics,
                    source,
                    functions,
                ),
                else_statements: parse_runtime_statements_with_diagnostics(
                    &else_nested,
                    &mut else_index,
                    diagnostics,
                    source,
                    functions,
                ),
            });
            continue;
        }
        if let Some((variable, range)) = parse_for_loop(trimmed) {
            let body = collect_inline_or_block(lines, index);
            let mut body_index = 0usize;
            statements.push(RuntimeStatement::ForLoop {
                variable,
                range,
                body: parse_runtime_statements_with_diagnostics(
                    &body,
                    &mut body_index,
                    diagnostics,
                    source,
                    functions,
                ),
            });
            continue;
        }
        if let Some(condition) = parse_while_loop(trimmed) {
            let body = collect_inline_or_block(lines, index);
            let mut body_index = 0usize;
            statements.push(RuntimeStatement::WhileLoop {
                condition,
                body: parse_runtime_statements_with_diagnostics(
                    &body,
                    &mut body_index,
                    diagnostics,
                    source,
                    functions,
                ),
            });
            continue;
        }
        if let Some(statement) = parse_runtime_statement(trimmed, functions) {
            statements.push(statement);
        } else {
            diagnostics.push(unsupported_runtime_statement_diagnostic(
                source,
                line.line_no,
                &line.text,
                trimmed,
            ));
        }
    }
    statements
}

fn collect_runtime_function_names(lines: &[&str]) -> std::collections::HashSet<String> {
    lines
        .iter()
        .filter_map(|line| parse_function_signature(strip_line_comment(line).trim()))
        .map(|signature| signature.name)
        .collect()
}

fn parse_runtime_statement(
    line: &str,
    functions: &std::collections::HashSet<String>,
) -> Option<RuntimeStatement> {
    if line.trim() == "break" {
        return Some(RuntimeStatement::Break);
    }
    if line.trim() == "continue" {
        return Some(RuntimeStatement::Continue);
    }
    if let Some(expr) = line.strip_prefix("return ") {
        return Some(RuntimeStatement::Return(parse_expression(expr.trim())?));
    }
    if line.trim() == "return" {
        return Some(RuntimeStatement::Return(Expression::Number(0.0)));
    }
    if let Some(content) = function_args(line, "wait") {
        return Some(RuntimeStatement::Wait(parse_expression(content)?));
    }
    if line.trim() == "entity.destroy()" || line.trim() == "destroySelf()" {
        return Some(RuntimeStatement::DestroySelf);
    }
    if let Some((name, args)) = parse_expression_call(line) {
        if args.trim().is_empty() && functions.contains(name) {
            return Some(RuntimeStatement::CallFunction(name.to_string()));
        }
    }
    if let Some(content) = method_args(line, "scene.spawnBox") {
        let args = split_top_level_commas(content);
        if args.len() == 5 {
            return Some(RuntimeStatement::SpawnBox {
                name: parse_expression(args[0])?,
                tag: parse_expression(args[1])?,
                position: parse_expression(args[2])?,
                size: parse_expression(args[3])?,
                script: parse_expression(args[4])?,
            });
        }
    }
    if let Some(content) = method_args(line, "scene.spawnSphere") {
        let args = split_top_level_commas(content);
        if args.len() == 5 {
            return Some(RuntimeStatement::SpawnSphere {
                name: parse_expression(args[0])?,
                tag: parse_expression(args[1])?,
                position: parse_expression(args[2])?,
                radius: parse_expression(args[3])?,
                script: parse_expression(args[4])?,
            });
        }
    }
    for method in [
        "scene.destroyNearestWithTag",
        "scene.destroyNearestTag",
        "destroyNearestWithTag",
    ] {
        if let Some(content) = method_args(line, method) {
            let args = split_top_level_commas(content);
            if args.len() == 2 {
                return Some(RuntimeStatement::DestroyNearestWithTag {
                    tag: parse_expression(args[0])?,
                    radius: parse_expression(args[1])?,
                });
            }
        }
    }
    for (method, spatial) in [
        ("Audio.playTone", false),
        ("audio.playTone", false),
        ("Audio.playTone3D", true),
        ("audio.playTone3D", true),
    ] {
        if let Some(content) = method_args(line, method) {
            let args = split_top_level_commas(content);
            if args.len() == 4 {
                return Some(RuntimeStatement::PlayTone {
                    waveform: parse_expression(args[0])?,
                    frequency: parse_expression(args[1])?,
                    duration: parse_expression(args[2])?,
                    volume: parse_expression(args[3])?,
                    spatial,
                });
            }
        }
    }
    for method in ["Audio.startLoop", "audio.startLoop"] {
        if let Some(content) = method_args(line, method) {
            let args = split_top_level_commas(content);
            if args.len() == 6 {
                return Some(RuntimeStatement::StartAudioLoop {
                    id: parse_expression(args[0])?,
                    waveform: parse_expression(args[1])?,
                    pattern: parse_expression(args[2])?,
                    bpm: parse_expression(args[3])?,
                    beats_per_note: parse_expression(args[4])?,
                    volume: parse_expression(args[5])?,
                });
            }
        }
    }
    for method in ["Audio.stopLoop", "audio.stopLoop"] {
        if let Some(content) = method_args(line, method) {
            return Some(RuntimeStatement::StopAudioLoop {
                id: parse_expression(content)?,
            });
        }
    }
    for method in ["render.gi.useScreenSpace", "Render.gi.useScreenSpace"] {
        if method_args(line, method).is_some() {
            return Some(RuntimeStatement::UseScreenSpaceGi);
        }
    }
    for method in ["render.gi.useProbeVolume", "Render.gi.useProbeVolume"] {
        if let Some(content) = method_args(line, method) {
            let args = split_top_level_commas(content);
            if args.len() == 4 {
                return Some(RuntimeStatement::UseProbeVolumeGi {
                    center: parse_expression(args[0])?,
                    extent: parse_expression(args[1])?,
                    counts: parse_expression(args[2])?,
                    intensity: parse_expression(args[3])?,
                });
            }
        }
    }
    for method in ["render.gi.setIntensity", "Render.gi.setIntensity"] {
        if let Some(content) = method_args(line, method) {
            return Some(RuntimeStatement::SetGiIntensity(parse_expression(content)?));
        }
    }
    for method in ["render.weather.set", "Render.weather.set"] {
        if let Some(content) = method_args(line, method) {
            return Some(RuntimeStatement::SetWeatherPreset(parse_expression(
                content,
            )?));
        }
    }
    for method in ["render.weather.setTimeOfDay", "Render.weather.setTimeOfDay"] {
        if let Some(content) = method_args(line, method) {
            return Some(RuntimeStatement::SetWeatherTimeOfDay(parse_expression(
                content,
            )?));
        }
    }
    for method in [
        "render.weather.setCloudCover",
        "Render.weather.setCloudCover",
    ] {
        if let Some(content) = method_args(line, method) {
            return Some(RuntimeStatement::SetWeatherCloudCover(parse_expression(
                content,
            )?));
        }
    }
    for method in [
        "render.weather.setPrecipitation",
        "Render.weather.setPrecipitation",
    ] {
        if let Some(content) = method_args(line, method) {
            return Some(RuntimeStatement::SetWeatherPrecipitation(parse_expression(
                content,
            )?));
        }
    }
    for method in ["render.weather.setWind", "Render.weather.setWind"] {
        if let Some(content) = method_args(line, method) {
            return Some(RuntimeStatement::SetWeatherWind(parse_expression(content)?));
        }
    }
    if line.trim() == "Input.captureMouse()" {
        return Some(RuntimeStatement::SetMouseCapture(Expression::Bool(true)));
    }
    if line.trim() == "Input.releaseMouse()" {
        return Some(RuntimeStatement::SetMouseCapture(Expression::Bool(false)));
    }
    if let Some(content) = function_args(line, "Input.captureMouse") {
        let expression = if content.trim().is_empty() {
            Expression::Bool(true)
        } else {
            parse_expression(content)?
        };
        return Some(RuntimeStatement::SetMouseCapture(expression));
    }
    if let Some(content) = function_args(line, "Input.setMouseCapture") {
        return Some(RuntimeStatement::SetMouseCapture(parse_expression(
            content,
        )?));
    }
    if let Some(content) = function_args(line, "Input.setCursorCaptured") {
        return Some(RuntimeStatement::SetMouseCapture(parse_expression(
            content,
        )?));
    }
    if let Some(content) = function_args(line, "log") {
        return parse_string_literal(content).map(RuntimeStatement::Log);
    }
    if let Some(content) = method_args(line, "entity.translate") {
        return parse_expression(content).map(RuntimeStatement::Translate);
    }
    if let Some(content) = method_args(line, "ui.label") {
        let args = split_top_level_commas(content);
        if args.len() == 4 {
            return Some(RuntimeStatement::UiLabel {
                id: parse_expression(args[0])?,
                text: parse_expression(args[1])?,
                x: parse_expression(args[2])?,
                y: parse_expression(args[3])?,
            });
        }
    }
    if let Some(content) = method_args(line, "ui.rect") {
        let args = split_top_level_commas(content);
        if args.len() == 9 {
            return Some(RuntimeStatement::UiRect {
                id: parse_expression(args[0])?,
                x: parse_expression(args[1])?,
                y: parse_expression(args[2])?,
                width: parse_expression(args[3])?,
                height: parse_expression(args[4])?,
                color: [
                    parse_expression(args[5])?,
                    parse_expression(args[6])?,
                    parse_expression(args[7])?,
                    parse_expression(args[8])?,
                ],
            });
        }
    }
    if let Some(content) = method_args(line, "ui.texture") {
        let args = split_top_level_commas(content);
        if args.len() == 6 || args.len() == 10 {
            let color = if args.len() == 10 {
                [
                    parse_expression(args[6])?,
                    parse_expression(args[7])?,
                    parse_expression(args[8])?,
                    parse_expression(args[9])?,
                ]
            } else {
                [
                    Expression::Number(1.0),
                    Expression::Number(1.0),
                    Expression::Number(1.0),
                    Expression::Number(1.0),
                ]
            };
            return Some(RuntimeStatement::UiTexture {
                id: parse_expression(args[0])?,
                texture: parse_expression(args[1])?,
                x: parse_expression(args[2])?,
                y: parse_expression(args[3])?,
                width: parse_expression(args[4])?,
                height: parse_expression(args[5])?,
                color,
            });
        }
    }
    if let Some((name, value)) = parse_local_declaration(line) {
        return Some(RuntimeStatement::DeclareLocal {
            name: name.to_string(),
            value: parse_expression(value)?,
        });
    }
    if let Some(value) = parse_position_assignment(line) {
        return Some(RuntimeStatement::SetPosition(parse_expression(value)?));
    }
    if let Some((axis, value)) = parse_position_axis_assignment(line) {
        return Some(RuntimeStatement::SetPositionAxis {
            axis,
            value: parse_expression(value)?,
        });
    }
    if let Some((axis, value)) = parse_position_add(line) {
        return Some(RuntimeStatement::AddToPosition {
            axis,
            value: parse_expression(value)?,
        });
    }
    if let Some((name, value)) = parse_state_add(line) {
        return Some(RuntimeStatement::AddToState {
            name: name.to_string(),
            value: parse_expression(value)?,
        });
    }
    if let Some((name, value)) = parse_binding_add(line) {
        return Some(RuntimeStatement::AddToBinding {
            name: name.to_string(),
            value: parse_expression(value)?,
        });
    }
    if let Some((name, value)) = parse_state_sub(line) {
        return Some(RuntimeStatement::SubFromState {
            name: name.to_string(),
            value: parse_expression(value)?,
        });
    }
    if let Some((name, value)) = parse_binding_sub(line) {
        return Some(RuntimeStatement::SubFromBinding {
            name: name.to_string(),
            value: parse_expression(value)?,
        });
    }
    if let Some(value) = parse_rotation_assignment(line) {
        return Some(RuntimeStatement::SetRotation(parse_expression(value)?));
    }
    if let Some((axis, value)) = parse_rotation_axis_assignment(line) {
        return Some(RuntimeStatement::SetRotationAxis {
            axis,
            value: parse_expression(value)?,
        });
    }
    if let Some((axis, value)) = parse_rotation_add(line) {
        return Some(RuntimeStatement::AddToRotation {
            axis,
            value: parse_expression(value)?,
        });
    }
    if let Some((name, value)) = parse_state_assignment(line) {
        return Some(RuntimeStatement::AssignState {
            name: name.to_string(),
            value: parse_expression(value)?,
        });
    }
    if let Some((name, value)) = parse_binding_assignment(line) {
        return Some(RuntimeStatement::AssignBinding {
            name: name.to_string(),
            value: parse_expression(value)?,
        });
    }
    None
}

fn parse_if_condition(line: &str) -> Option<ConditionExpression> {
    let rest = line.strip_prefix("if ")?.trim();
    let condition = rest.strip_suffix('{').unwrap_or(rest).trim();
    parse_condition_expression(condition)
}

fn parse_condition_expression(condition: &str) -> Option<ConditionExpression> {
    let condition = strip_wrapping_parens(condition.trim());
    if let Some((lhs, rhs)) = split_logical(condition, "||") {
        return Some(ConditionExpression::Or(
            Box::new(parse_condition_expression(lhs)?),
            Box::new(parse_condition_expression(rhs)?),
        ));
    }
    if let Some((lhs, rhs)) = split_logical(condition, "&&") {
        return Some(ConditionExpression::And(
            Box::new(parse_condition_expression(lhs)?),
            Box::new(parse_condition_expression(rhs)?),
        ));
    }
    if let Some(inner) = condition.strip_prefix('!') {
        return Some(ConditionExpression::Not(Box::new(
            parse_condition_expression(inner.trim())?,
        )));
    }
    if let Some(action) = function_args(condition, "Input.pressed") {
        return parse_string_literal(action).map(ConditionExpression::InputJustPressed);
    }
    if let Some(action) = function_args(condition, "Input.down") {
        return parse_string_literal(action).map(ConditionExpression::InputDown);
    }
    if let Some(action) = function_args(condition, "Input.justPressed") {
        return parse_string_literal(action).map(ConditionExpression::InputJustPressed);
    }
    if let Some(action) = function_args(condition, "Input.pressedThisFrame") {
        return parse_string_literal(action).map(ConditionExpression::InputJustPressed);
    }
    if let Some(action) = function_args(condition, "Input.justReleased") {
        return parse_string_literal(action).map(ConditionExpression::InputJustReleased);
    }
    if let Some(action) = function_args(condition, "Input.released") {
        return parse_string_literal(action).map(ConditionExpression::InputJustReleased);
    }
    if let Some(action) = function_args(condition, "Input.actionDown") {
        return parse_string_literal(action).map(ConditionExpression::ActionDown);
    }
    if let Some(action) = function_args(condition, "Input.actionPressed") {
        return parse_string_literal(action).map(ConditionExpression::ActionJustPressed);
    }
    if let Some(action) = function_args(condition, "Input.actionReleased") {
        return parse_string_literal(action).map(ConditionExpression::ActionJustReleased);
    }
    if let Some(action) = function_args(condition, "Input.actionUp") {
        return parse_string_literal(action).map(ConditionExpression::ActionUp);
    }
    if let Some((lhs, op, rhs)) = split_comparison(condition) {
        return Some(ConditionExpression::Compare {
            lhs: parse_expression(lhs)?,
            op,
            rhs: parse_expression(rhs)?,
        });
    }
    if parse_expression_call(condition).is_some_and(|(function, _)| {
        matches!(
            function,
            "entity.hasTag"
                | "scene.distanceTo"
                | "distanceTo"
                | "scene.distanceToTag"
                | "distanceToTag"
                | "scene.distanceToTagBounds"
                | "distanceToTagBounds"
                | "scene.horizontalDistanceToTagBounds"
                | "horizontalDistanceToTagBounds"
                | "playerDistance"
                | "scene.playerDistance"
        )
    }) {
        return Some(ConditionExpression::Compare {
            lhs: parse_expression(condition)?,
            op: CompareOp::NotEqual,
            rhs: Expression::Number(0.0),
        });
    }
    if is_truthy_condition_source(condition) {
        return Some(ConditionExpression::Compare {
            lhs: parse_expression(condition)?,
            op: CompareOp::NotEqual,
            rhs: Expression::Number(0.0),
        });
    }

    None
}

fn parse_expression(source: &str) -> Option<Expression> {
    let source = source.trim().trim_end_matches(';').trim();
    parse_binary_expression(source, &[('+', BinaryOp::Add), ('-', BinaryOp::Sub)])
        .or_else(|| parse_binary_expression(source, &[('*', BinaryOp::Mul), ('/', BinaryOp::Div)]))
        .or_else(|| parse_atom(source))
}

fn parse_binary_expression(source: &str, ops: &[(char, BinaryOp)]) -> Option<Expression> {
    let mut depth = 0usize;
    let mut in_string = false;
    let chars = source.char_indices().collect::<Vec<_>>();
    for (index, ch) in chars.into_iter().rev() {
        match ch {
            '"' => in_string = !in_string,
            ')' if !in_string => depth += 1,
            '(' if !in_string => depth = depth.saturating_sub(1),
            _ => {}
        }
        if in_string || depth != 0 {
            continue;
        }
        if let Some((_, op)) = ops.iter().find(|(candidate, _)| *candidate == ch) {
            if index == 0 {
                continue;
            }
            let lhs = parse_expression(&source[..index])?;
            let rhs = parse_expression(&source[index + ch.len_utf8()..])?;
            return Some(Expression::Binary {
                op: *op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            });
        }
    }
    None
}

fn parse_atom(source: &str) -> Option<Expression> {
    if let Ok(number) = source.parse::<f32>() {
        return Some(Expression::Number(number));
    }
    if source == "true" {
        return Some(Expression::Bool(true));
    }
    if source == "false" {
        return Some(Expression::Bool(false));
    }
    if let Some(value) = parse_string_literal(source) {
        return Some(Expression::String(value));
    }
    if let Some(content) = function_args(source, "Vec3") {
        let parts = split_top_level_commas(content);
        if parts.len() == 3 {
            return Some(Expression::Vec3(
                Box::new(parse_expression(parts[0])?),
                Box::new(parse_expression(parts[1])?),
                Box::new(parse_expression(parts[2])?),
            ));
        }
    }
    if let Some(action) = function_args(source, "Input.axis") {
        let action = parse_string_literal(action)?;
        return Some(match action.as_str() {
            "move" => Expression::Member("Input".to_string(), "moveY".to_string()),
            "moveX" => Expression::Member("Input".to_string(), "moveX".to_string()),
            "moveY" => Expression::Member("Input".to_string(), "moveY".to_string()),
            _ => Expression::Variable(action),
        });
    }
    if let Some(action) = function_args(source, "Input.actionValue") {
        return parse_string_literal(action)
            .map(|action| Expression::Member("InputAction".to_string(), action));
    }
    if let Some(action) = function_args(source, "Input.value") {
        return parse_string_literal(action)
            .map(|action| Expression::Member("InputAction".to_string(), action));
    }
    if let Some((function, args)) = parse_expression_call(source) {
        return Some(Expression::Call {
            function: function.to_string(),
            args: split_top_level_commas(args)
                .into_iter()
                .map(parse_expression)
                .collect::<Option<Vec<_>>>()?,
        });
    }
    if let Some((owner, field)) = source.rsplit_once('.') {
        return Some(Expression::Member(
            owner.trim().to_string(),
            field.trim().to_string(),
        ));
    }
    Some(Expression::Variable(source.to_string()))
}

fn parse_state_default(line: &str) -> Option<(String, serde_json::Value)> {
    let rest = line.strip_prefix("var ")?.trim();
    let (name, after_name) = rest.split_once(':')?;
    let (_, value) = after_name.split_once('=')?;
    Some((
        name.trim().to_string(),
        parse_default_literal(value.trim())?,
    ))
}

fn parse_default_literal(value: &str) -> Option<serde_json::Value> {
    let value = value.trim().trim_end_matches(';').trim();
    if let Ok(number) = value.parse::<f64>() {
        return Some(serde_json::Value::from(number));
    }
    if value == "true" {
        return Some(serde_json::Value::Bool(true));
    }
    if value == "false" {
        return Some(serde_json::Value::Bool(false));
    }
    parse_string_literal(value).map(serde_json::Value::String)
}

fn parse_local_declaration(line: &str) -> Option<(&str, &str)> {
    let rest = line
        .strip_prefix("let ")
        .or_else(|| line.strip_prefix("var "))?
        .trim();
    let (name_part, value) = rest.split_once('=')?;
    let name = name_part
        .split_once(':')
        .map_or(name_part, |(name, _)| name)
        .trim();
    if is_valid_runtime_binding_name(name) {
        Some((name, value.trim()))
    } else {
        None
    }
}

fn parse_position_assignment(line: &str) -> Option<&str> {
    let (lhs, rhs) = line.split_once('=')?;
    if lhs.contains('+') || lhs.contains('-') || lhs.contains('*') || lhs.contains('/') {
        return None;
    }
    match lhs.trim() {
        "entity.position" | "position" => Some(rhs.trim()),
        _ => None,
    }
}

fn parse_position_axis_assignment(line: &str) -> Option<(Axis, &str)> {
    let (lhs, rhs) = line.split_once('=')?;
    if lhs.contains('+') || lhs.contains('-') || lhs.contains('*') || lhs.contains('/') {
        return None;
    }
    let axis = parse_position_axis(lhs.trim())?;
    Some((axis, rhs.trim()))
}

fn parse_position_add(line: &str) -> Option<(Axis, &str)> {
    let (lhs, rhs) = line.split_once("+=")?;
    let axis = parse_position_axis(lhs.trim())?;
    Some((axis, rhs.trim()))
}

fn parse_position_axis(lhs: &str) -> Option<Axis> {
    match lhs {
        "entity.position.x" | "position.x" => Some(Axis::X),
        "entity.position.y" | "position.y" => Some(Axis::Y),
        "entity.position.z" | "position.z" => Some(Axis::Z),
        _ => None,
    }
}

fn parse_rotation_assignment(line: &str) -> Option<&str> {
    let (lhs, rhs) = line.split_once('=')?;
    if lhs.contains('+') || lhs.contains('-') || lhs.contains('*') || lhs.contains('/') {
        return None;
    }
    match lhs.trim() {
        "entity.rotation" | "rotation" => Some(rhs.trim()),
        _ => None,
    }
}

fn parse_rotation_axis_assignment(line: &str) -> Option<(Axis, &str)> {
    let (lhs, rhs) = line.split_once('=')?;
    if lhs.contains('+') || lhs.contains('-') || lhs.contains('*') || lhs.contains('/') {
        return None;
    }
    let axis = parse_rotation_axis(lhs.trim())?;
    Some((axis, rhs.trim()))
}

fn parse_rotation_add(line: &str) -> Option<(Axis, &str)> {
    let (lhs, rhs) = line.split_once("+=")?;
    let axis = parse_rotation_axis(lhs.trim())?;
    Some((axis, rhs.trim()))
}

fn parse_rotation_axis(lhs: &str) -> Option<Axis> {
    match lhs {
        "entity.rotation.x" | "rotation.x" => Some(Axis::X),
        "entity.rotation.y" | "rotation.y" => Some(Axis::Y),
        "entity.rotation.z" | "rotation.z" => Some(Axis::Z),
        _ => None,
    }
}

fn parse_state_add(line: &str) -> Option<(&str, &str)> {
    let (lhs, rhs) = line.split_once("+=")?;
    lhs.trim()
        .strip_prefix("state.")
        .map(|name| (name, rhs.trim()))
        .or_else(|| Some((lhs.trim(), rhs.trim())))
}

fn parse_state_sub(line: &str) -> Option<(&str, &str)> {
    let (lhs, rhs) = line.split_once("-=")?;
    lhs.trim()
        .strip_prefix("state.")
        .map(|name| (name, rhs.trim()))
        .or_else(|| Some((lhs.trim(), rhs.trim())))
}

fn parse_state_assignment(line: &str) -> Option<(&str, &str)> {
    let (lhs, rhs) = line.split_once('=')?;
    if lhs.contains('+') || lhs.contains('-') || lhs.contains('*') || lhs.contains('/') {
        return None;
    }
    lhs.trim()
        .strip_prefix("state.")
        .map(|name| (name, rhs.trim()))
}

fn parse_binding_add(line: &str) -> Option<(&str, &str)> {
    let (lhs, rhs) = line.split_once("+=")?;
    let name = lhs.trim();
    is_valid_runtime_binding_name(name).then_some((name, rhs.trim()))
}

fn parse_binding_sub(line: &str) -> Option<(&str, &str)> {
    let (lhs, rhs) = line.split_once("-=")?;
    let name = lhs.trim();
    is_valid_runtime_binding_name(name).then_some((name, rhs.trim()))
}

fn parse_binding_assignment(line: &str) -> Option<(&str, &str)> {
    let (lhs, rhs) = line.split_once('=')?;
    if lhs.contains('+') || lhs.contains('-') || lhs.contains('*') || lhs.contains('/') {
        return None;
    }
    let name = lhs.trim();
    is_valid_runtime_binding_name(name).then_some((name, rhs.trim()))
}

fn is_valid_runtime_binding_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        && !matches!(
            name,
            "if" | "else" | "for" | "while" | "return" | "true" | "false"
        )
}

fn is_truthy_condition_source(source: &str) -> bool {
    let source = source.trim();
    if is_valid_runtime_binding_name(source) {
        return true;
    }
    if parse_expression_call(source).is_some_and(|(function, _)| {
        matches!(
            function,
            "ui.button" | "UI.button" | "ui.toggle" | "UI.toggle" | "ui.dragArea" | "UI.dragArea"
        )
    }) {
        return true;
    }
    source
        .strip_prefix("state.")
        .is_some_and(is_valid_runtime_binding_name)
}

fn unsupported_runtime_statement_diagnostic(
    source: &str,
    line_no: usize,
    raw_line: &str,
    trimmed: &str,
) -> VargDiagnostic {
    let column = raw_line
        .find(trimmed)
        .map(|index| index + 1)
        .unwrap_or_else(|| {
            raw_line
                .chars()
                .position(|ch| !ch.is_whitespace())
                .map(|index| index + 1)
                .unwrap_or(1)
        });
    let (message, expected, suggestion) = unsupported_runtime_statement_help(trimmed);
    VargDiagnostic {
        code: "VARG4100".to_string(),
        severity: VargDiagnosticSeverity::Error,
        line: Some(line_no),
        column: Some(column),
        message,
        expected,
        suggestion,
        blocking: true,
        source_line: Some(
            source
                .lines()
                .nth(line_no.saturating_sub(1))
                .unwrap_or(raw_line)
                .to_string(),
        ),
    }
}

fn unsupported_runtime_statement_help(trimmed: &str) -> (String, String, String) {
    if trimmed.starts_with("emit(") || trimmed.starts_with("emit ") {
        return (
            "unsupported runtime API `emit`".to_string(),
            "The MVP runtime supports local script state, transform position changes, Input, mouse capture, Time, Math/easing helpers, `log(...)`, `wait(...)`, and basic interactive `ui.*(...)` controls."
                .to_string(),
            "`emit(...)` is in the target language direction but is not wired into this runtime yet. Store a value in `state.*` or use `log(...)` for now."
                .to_string(),
        );
    }

    if trimmed.contains("entity.velocity") {
        return (
            "unsupported entity API `entity.velocity`".to_string(),
            "Use `entity.translate(Vec3(...))`, `position = Vec3(...)`, or `position.x/y/z` assignment in the MVP runtime."
                .to_string(),
            "For transform-only motion, replace velocity mutation with `position.y += jumpForce * dt` or `entity.translate(Vec3(0, jumpForce * dt, 0))`."
                .to_string(),
        );
    }

    if trimmed.starts_with("if ") {
        return (
            "unsupported or malformed `if` condition".to_string(),
            "Supported conditions use Input checks, numeric comparisons, `!`, `&&`, and `||`."
                .to_string(),
            "Rewrite the condition with supported bindings such as `Input.down(\"jump\")`, `state.count > 0`, or `position.y <= 1.0`."
                .to_string(),
        );
    }

    if trimmed.starts_with("for ") {
        return (
            "unsupported or malformed `for` loop".to_string(),
            "Supported loops are `for i in 0..10`, `for i in 0..=10`, and `for i in count(n)`."
                .to_string(),
            "Rewrite the loop range using one of the supported range forms.".to_string(),
        );
    }

    if trimmed.starts_with("while ") {
        return (
            "unsupported or malformed `while` condition".to_string(),
            "Supported conditions use Input checks, numeric comparisons, `!`, `&&`, and `||`."
                .to_string(),
            "Rewrite the condition with supported numeric state, local, Time, Input, or position bindings."
                .to_string(),
        );
    }

    (
        "unsupported runtime statement".to_string(),
        "Supported statements are `let`/`var` locals, state assignment, position assignment, `entity.translate(...)`, `scene.spawnBox(...)`, `scene.spawnSphere(...)`, `scene.destroyNearestWithTag(...)`, `Audio.playTone(...)`, `Audio.playTone3D(...)`, `Audio.startLoop(...)`, `Audio.stopLoop(...)`, `Input.captureMouse(...)`, `Input.releaseMouse()`, `ui.label(...)`, `ui.rect(...)`, interactive `ui.button/toggle/slider/drag/input` expression calls, `if`, `for`, `while`, `return`, `break`, `continue`, `wait(...)`, and `log(...)`."
            .to_string(),
        "Rewrite this line using the supported MVP script API, or add runtime support before using this language construct."
            .to_string(),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeLine {
    line_no: usize,
    text: String,
}

fn collect_inline_or_block(lines: &[RuntimeLine], index: &mut usize) -> Vec<RuntimeLine> {
    let mut collected = Vec::new();
    let mut depth = 1isize;
    while *index < lines.len() {
        let line = lines[*index].clone();
        *index += 1;
        let trimmed = strip_line_comment(&line.text).trim();
        if depth == 1 && trimmed.starts_with("} else") {
            *index = (*index).saturating_sub(1);
            break;
        }
        depth += trimmed.matches('{').count() as isize;
        depth -= trimmed.matches('}').count() as isize;
        if depth <= 0 {
            break;
        }
        collected.push(line);
    }
    collected
}

fn collect_else_block(lines: &[RuntimeLine], index: &mut usize) -> Vec<RuntimeLine> {
    if *index >= lines.len() {
        return Vec::new();
    }
    let trimmed = strip_line_comment(&lines[*index].text).trim();
    if trimmed == "else {" || trimmed == "} else {" {
        *index += 1;
        return collect_inline_or_block(lines, index);
    }
    Vec::new()
}

fn collect_block(lines: &[&str], start: usize) -> (Vec<RuntimeLine>, usize) {
    let mut body = Vec::new();
    let mut depth =
        lines[start].matches('{').count() as isize - lines[start].matches('}').count() as isize;
    let mut index = start + 1;
    while index < lines.len() {
        let line = lines[index];
        depth += line.matches('{').count() as isize;
        depth -= line.matches('}').count() as isize;
        if depth <= 0 {
            return (body, index + 1);
        }
        body.push(RuntimeLine {
            line_no: index + 1,
            text: line.to_string(),
        });
        index += 1;
    }
    (body, index)
}

fn function_args<'a>(line: &'a str, function: &str) -> Option<&'a str> {
    let rest = line.trim().strip_prefix(function)?.trim();
    Some(rest.strip_prefix('(')?.strip_suffix(')')?.trim())
}

fn method_args<'a>(line: &'a str, method: &str) -> Option<&'a str> {
    function_args(line.trim_end_matches(';'), method)
}

fn parse_string_literal(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value.strip_prefix('"')?;
    let end = value.rfind('"')?;
    Some(value[..end].to_string())
}

fn point_in_rect(px: f32, py: f32, x: f32, y: f32, width: f32, height: f32) -> bool {
    px >= x && px <= x + width && py >= y && py <= y + height
}

fn parse_expression_call(source: &str) -> Option<(&str, &str)> {
    let open = source.find('(')?;
    let function = source[..open].trim();
    if function.is_empty()
        || !function
            .chars()
            .all(|ch| ch == '_' || ch == '.' || ch.is_ascii_alphanumeric())
    {
        return None;
    }
    let args = source[open + 1..].strip_suffix(')')?;
    spans_whole_call(source).then_some((function, args))
}

fn spans_whole_call(source: &str) -> bool {
    let mut depth = 0usize;
    let mut in_string = false;
    for (index, ch) in source.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string => {
                depth = depth.saturating_sub(1);
                if depth == 0 && index + ch.len_utf8() < source.len() {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn split_top_level_commas(source: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut in_string = false;
    for (index, ch) in source.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth = depth.saturating_sub(1),
            ',' if !in_string && depth == 0 => {
                parts.push(source[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(source[start..].trim());
    parts
}

fn strip_wrapping_parens(source: &str) -> &str {
    let mut current = source.trim();
    loop {
        let Some(inner) = current
            .strip_prefix('(')
            .and_then(|value| value.strip_suffix(')'))
        else {
            return current;
        };
        if spans_whole_expression(current) {
            current = inner.trim();
        } else {
            return current;
        }
    }
}

fn spans_whole_expression(source: &str) -> bool {
    let mut depth = 0usize;
    let mut in_string = false;
    for (index, ch) in source.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string => {
                depth = depth.saturating_sub(1);
                if depth == 0 && index + ch.len_utf8() < source.len() {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn split_logical<'a>(source: &'a str, operator: &str) -> Option<(&'a str, &'a str)> {
    let mut depth = 0usize;
    let mut in_string = false;
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let ch = source[index..].chars().next()?;
        match ch {
            '"' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth = depth.saturating_sub(1),
            _ => {}
        }
        if !in_string && depth == 0 && source[index..].starts_with(operator) {
            let lhs = source[..index].trim();
            let rhs = source[index + operator.len()..].trim();
            if !lhs.is_empty() && !rhs.is_empty() {
                return Some((lhs, rhs));
            }
        }
        index += ch.len_utf8();
    }
    None
}

fn split_comparison(source: &str) -> Option<(&str, CompareOp, &str)> {
    let mut depth = 0usize;
    let mut in_string = false;
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let ch = source[index..].chars().next()?;
        match ch {
            '"' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth = depth.saturating_sub(1),
            _ => {}
        }
        if !in_string && depth == 0 {
            for (symbol, op) in [
                ("==", CompareOp::Equal),
                ("!=", CompareOp::NotEqual),
                (">=", CompareOp::GreaterThanOrEqual),
                ("<=", CompareOp::LessThanOrEqual),
                (">", CompareOp::GreaterThan),
                ("<", CompareOp::LessThan),
            ] {
                if source[index..].starts_with(symbol) {
                    let lhs = source[..index].trim();
                    let rhs = source[index + symbol.len()..].trim();
                    if !lhs.is_empty() && !rhs.is_empty() {
                        return Some((lhs, op, rhs));
                    }
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

fn json_number(value: &serde_json::Value) -> Option<f32> {
    if let Some(number) = value.as_f64() {
        return Some(number as f32);
    }
    value.as_bool().map(|value| if value { 1.0 } else { 0.0 })
}

fn json_display_string(value: &serde_json::Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(value) = value.as_bool() {
        return value.to_string();
    }
    if let Some(number) = value.as_f64() {
        return format_display_number(number as f32);
    }
    value.to_string()
}

fn format_display_number(value: f32) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    if (value.fract()).abs() < 0.0001 {
        return format!("{}", value as i64);
    }
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn input_action_down(input: &engine_platform::InputState, action: &str) -> bool {
    if input.action_down(action) {
        return true;
    }
    if let Some(keys) = default_action_keys(action) {
        return keys.iter().any(|key| input.key_down(*key));
    }
    false
}

fn input_action_pressed(input: &engine_platform::InputState, action: &str) -> bool {
    if input.action_pressed(action) {
        return true;
    }
    if let Some(keys) = default_action_keys(action) {
        return keys.iter().any(|key| input.key_pressed(*key));
    }
    false
}

fn input_action_released(input: &engine_platform::InputState, action: &str) -> bool {
    if input.action_released(action) {
        return true;
    }
    if let Some(keys) = default_action_keys(action) {
        return keys.iter().any(|key| input.key_released(*key));
    }
    false
}

fn default_action_keys(action: &str) -> Option<&'static [engine_platform::KeyCode]> {
    use engine_platform::KeyCode;

    match action {
        "jump" | "Jump" | "Space" => Some(&[KeyCode::Space]),
        "fire" | "Fire" => Some(&[KeyCode::Character('f'), KeyCode::Character('e')]),
        "interact" | "Interact" => Some(&[KeyCode::Character('e')]),
        "pause" | "Pause" | "Escape" | "Esc" => Some(&[KeyCode::Escape]),
        "moveForward" | "MoveForward" => Some(&[KeyCode::Character('w'), KeyCode::ArrowUp]),
        "moveBackward" | "MoveBackward" | "MoveBack" => {
            Some(&[KeyCode::Character('s'), KeyCode::ArrowDown])
        }
        "moveLeft" | "MoveLeft" => Some(&[KeyCode::Character('a'), KeyCode::ArrowLeft]),
        "moveRight" | "MoveRight" => Some(&[KeyCode::Character('d'), KeyCode::ArrowRight]),
        _ => None,
    }
}

fn parse_for_loop(line: &str) -> Option<(String, RangeExpression)> {
    let rest = line.strip_prefix("for ")?.trim();
    let (loop_part, _) = rest.split_once('{').unwrap_or((rest, ""));
    let loop_part = loop_part.trim();

    let (variable, range_part) = loop_part.split_once(" in ")?;
    let variable = variable.trim().to_string();
    let range_part = range_part.trim();

    // Parse count(n) syntax
    if let Some(count_expr) = function_args(range_part, "count") {
        let expr = parse_expression(count_expr)?;
        return Some((variable, RangeExpression::Count(expr)));
    }

    // Parse range expressions: a..b or a..=b
    if let Some((start_str, end_str)) = range_part.split_once("..=") {
        let start = parse_expression(start_str.trim())?;
        let end = parse_expression(end_str.trim())?;
        return Some((variable, RangeExpression::RangeInclusive(start, end)));
    }

    if let Some((start_str, end_str)) = range_part.split_once("..") {
        let start = parse_expression(start_str.trim())?;
        let end = parse_expression(end_str.trim())?;
        return Some((variable, RangeExpression::Range(start, end)));
    }

    None
}

fn parse_while_loop(line: &str) -> Option<ConditionExpression> {
    let rest = line.strip_prefix("while ")?.trim();
    let condition = rest.strip_suffix('{').unwrap_or(rest).trim();
    parse_condition_expression(condition)
}
