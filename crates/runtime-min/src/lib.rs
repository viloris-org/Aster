#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Minimal Aster runtime and first playable game runner.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use engine_core::{logging, EngineConfig, EngineError, EngineResult, FrameCounter};
use engine_ecs::{
    CameraComponentData, CameraRole, ComponentData, MeshRendererComponentData, ProjectManifest,
    Scene,
};
#[cfg(feature = "physics")]
use engine_physics::{
    BodyHandle, BodyKind, ColliderDesc, ColliderShape, PhysicsWorld, RigidbodyDesc,
    SimplePhysicsBackend,
};
use engine_platform::InputState;
use engine_render::{
    HeadlessRenderDevice, RenderDevice, RenderFrame, RenderGraph, RenderGraphBuilder, RenderWorld,
};

/// Explicit runtime services. There is no hidden global mutable state.
#[derive(Debug)]
pub struct RuntimeServices<R = HeadlessRenderDevice> {
    /// Runtime configuration.
    pub config: EngineConfig,
    /// Scene storage.
    pub scene: Scene,
    /// Render abstraction.
    pub renderer: R,
    /// Active render graph.
    pub render_graph: RenderGraph,
    /// Frame input state.
    pub input: InputState,
    /// Latest scene extraction submitted to rendering.
    pub render_world: RenderWorld,
    /// Whether the game simulation is paused.
    pub paused: bool,
    /// Latest runtime counters for diagnostics UI and smoke tests.
    pub stats: RuntimeStats,
    /// Diagnostics emitted by runtime subsystems.
    pub diagnostics: Vec<RuntimeDiagnostic>,
    #[cfg(feature = "physics")]
    /// Physics world used by runtime-game.
    pub physics: PhysicsWorld,
    fixed_timestep: FixedTimestep,
    frame_counter: FrameCounter,
    reported_script_errors: HashSet<String>,
    #[cfg(feature = "physics")]
    physics_bindings: Vec<PhysicsBinding>,
}

/// Runtime counters surfaced to editor and CLI diagnostics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeStats {
    /// Frame delta in seconds.
    pub frame_time_seconds: f32,
    /// Number of renderable objects submitted this frame.
    pub draw_calls: usize,
    /// Number of scene objects.
    pub entity_count: usize,
    /// Number of render resources known to the runtime.
    pub resource_count: usize,
    /// Number of fixed physics steps run this frame.
    pub physics_steps: u32,
}

/// Structured runtime diagnostic entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDiagnostic {
    /// Subsystem that emitted the diagnostic.
    pub source: String,
    /// Human-readable severity.
    pub level: String,
    /// Diagnostic message.
    pub message: String,
    /// Optional source file.
    pub file: Option<PathBuf>,
    /// Optional source line.
    pub line: Option<u32>,
}

#[cfg(feature = "physics")]
#[derive(Clone, Debug)]
struct PhysicsBinding {
    object: engine_core::EntityId,
    body: BodyHandle,
}

/// Fixed timestep accumulator used by the game loop.
#[derive(Clone, Copy, Debug)]
pub struct FixedTimestep {
    step: Duration,
    accumulator: Duration,
    max_steps_per_frame: u32,
}

impl Default for FixedTimestep {
    fn default() -> Self {
        Self {
            step: Duration::from_secs_f32(1.0 / 60.0),
            accumulator: Duration::ZERO,
            max_steps_per_frame: 5,
        }
    }
}

impl FixedTimestep {
    /// Adds elapsed wall-clock time to the accumulator.
    pub fn accumulate(&mut self, delta: Duration) {
        self.accumulator = self
            .accumulator
            .saturating_add(delta.min(Duration::from_millis(250)));
    }

    /// Returns whether another fixed step should run, consuming one step if so.
    pub fn consume_step(&mut self, steps_this_frame: u32) -> bool {
        if steps_this_frame >= self.max_steps_per_frame || self.accumulator < self.step {
            return false;
        }
        self.accumulator = self.accumulator.saturating_sub(self.step);
        true
    }
}

impl RuntimeServices<HeadlessRenderDevice> {
    /// Creates minimal runtime services with a headless renderer.
    pub fn minimal(config: EngineConfig) -> Self {
        let render_graph = build_default_render_graph();
        Self {
            config,
            scene: Scene::default(),
            renderer: HeadlessRenderDevice::default(),
            render_graph,
            input: {
                let mut input = InputState::default();
                input.bind_default_player_actions();
                input
            },
            render_world: RenderWorld::default(),
            paused: false,
            stats: RuntimeStats::default(),
            diagnostics: Vec::new(),
            #[cfg(feature = "physics")]
            physics: PhysicsWorld::new(SimplePhysicsBackend::new()),
            fixed_timestep: FixedTimestep::default(),
            frame_counter: FrameCounter::default(),
            reported_script_errors: HashSet::new(),
            #[cfg(feature = "physics")]
            physics_bindings: Vec::new(),
        }
    }
}

impl<R: RenderDevice> RuntimeServices<R> {
    /// Ticks one runtime frame.
    pub fn tick(&mut self) -> EngineResult<()> {
        logging::log_frame(self.frame_counter.get());
        let frame = RenderFrame {
            frame_index: self.frame_counter.get(),
        };
        self.renderer.execute_graph(&self.render_graph, frame)?;
        self.renderer
            .flush_destroy_queue(self.frame_counter.get().saturating_sub(2));
        self.frame_counter.advance();
        Ok(())
    }

    /// Ticks one game frame with explicit input, fixed update, scene, audio, render, and destroy order.
    pub fn tick_game_frame(&mut self, delta: Duration, single_step: bool) -> EngineResult<()> {
        logging::log_frame(self.frame_counter.get());
        self.stats.frame_time_seconds = delta.as_secs_f32();
        self.stats.physics_steps = 0;
        self.report_script_proxy_diagnostics();
        let should_simulate_variable = !self.paused || single_step;
        if should_simulate_variable {
            #[cfg(feature = "physics")]
            self.ensure_physics_bindings()?;
            self.fixed_timestep.accumulate(delta);
            let mut fixed_steps = 0;
            while self.fixed_timestep.consume_step(fixed_steps) {
                #[cfg(feature = "physics")]
                {
                    self.sync_scene_to_physics()?;
                    self.physics
                        .fixed_update(self.fixed_timestep.step.as_secs_f32());
                    self.sync_physics_to_scene()?;
                    self.stats.physics_steps = self.stats.physics_steps.saturating_add(1);
                }
                self.scene.tick_fixed_frame();
                fixed_steps += 1;
            }
            self.apply_builtin_player_controller();
            self.scene.tick_runtime_frame();
        }
        self.render_world = extract_render_world(&self.scene);
        let frame = RenderFrame {
            frame_index: self.frame_counter.get(),
        };
        self.renderer.execute_graph(&self.render_graph, frame)?;
        self.renderer
            .flush_destroy_queue(self.frame_counter.get().saturating_sub(2));
        self.scene.process_deferred_destroy()?;
        self.stats.draw_calls = self.render_world.objects.len();
        self.stats.entity_count = self.scene.objects().len();
        self.stats.resource_count = self.render_world.objects.len()
            + self.render_world.lights.len()
            + usize::from(self.render_world.camera.is_some());
        self.input.end_frame();
        self.frame_counter.advance();
        Ok(())
    }

    /// Current frame index.
    pub fn frame_index(&self) -> u64 {
        self.frame_counter.get()
    }

    /// Replaces the active render graph.
    pub fn set_render_graph(&mut self, graph: RenderGraph) {
        self.render_graph = graph;
    }

    fn apply_builtin_player_controller(&mut self) {
        let Some(player) = self.scene.find_by_name("Player") else {
            return;
        };
        let move_x = self.input.action_value("MoveX");
        let move_z = self.input.action_value("MoveY");
        if move_x == 0.0 && move_z == 0.0 {
            return;
        }
        if let Some(mut transform) = self.scene.transforms().local(player) {
            let speed = 0.08;
            transform.translation.x += move_x * speed;
            transform.translation.z += move_z * speed;
            self.scene.transforms_mut().set_local(player, transform);
        }
    }

    fn report_script_proxy_diagnostics(&mut self) {
        for (_, object) in self.scene.objects() {
            for script in object
                .scripts
                .iter()
                .chain(
                    object
                        .components
                        .iter()
                        .filter_map(|component| match component {
                            ComponentData::Script(script) => Some(script),
                            _ => None,
                        }),
                )
            {
                let key = format!("{}:{}", script.backend, script.script);
                if script.pending_recovery && self.reported_script_errors.insert(key) {
                    self.diagnostics.push(RuntimeDiagnostic {
                        source: "script".to_string(),
                        level: "error".to_string(),
                        message: format!(
                            "{} script `{}` is pending backend recovery",
                            script.backend, script.script
                        ),
                        file: Some(PathBuf::from(&script.script)),
                        line: None,
                    });
                }
            }
        }
    }

    #[cfg(feature = "physics")]
    fn ensure_physics_bindings(&mut self) -> EngineResult<()> {
        for (entity, object) in self.scene.objects() {
            if self
                .physics_bindings
                .iter()
                .any(|binding| binding.object == object.id)
            {
                continue;
            }
            let Some(rigidbody) = object
                .components
                .iter()
                .find_map(|component| match component {
                    ComponentData::Rigidbody(rigidbody) => Some(rigidbody),
                    _ => None,
                })
            else {
                continue;
            };
            let mut desc = RigidbodyDesc {
                transform: self.scene.transforms().local(entity).unwrap_or_default(),
                kind: match rigidbody.body_type.as_str() {
                    "static" => BodyKind::Static,
                    "kinematic" => BodyKind::Kinematic,
                    _ => BodyKind::Dynamic,
                },
                gravity_scale: if rigidbody.use_gravity { 1.0 } else { 0.0 },
                ..RigidbodyDesc::default()
            };
            desc.transform = self.scene.transforms().local(entity).unwrap_or_default();
            let body = self.physics.backend_mut().create_body(&desc)?;
            for collider in object
                .components
                .iter()
                .filter_map(|component| match component {
                    ComponentData::Collider(collider) => Some(collider),
                    _ => None,
                })
            {
                self.physics
                    .backend_mut()
                    .add_collider(body, &collider_desc_from_scene(collider, object.layer))?;
            }
            self.physics_bindings.push(PhysicsBinding {
                object: object.id,
                body,
            });
        }
        Ok(())
    }

    #[cfg(feature = "physics")]
    fn sync_scene_to_physics(&mut self) -> EngineResult<()> {
        for binding in &self.physics_bindings {
            if let Some(entity) = self.scene.find_by_id(binding.object) {
                let transform = self.scene.transforms().local(entity).unwrap_or_default();
                self.physics
                    .backend_mut()
                    .set_body_transform(binding.body, transform)?;
            }
        }
        Ok(())
    }

    #[cfg(feature = "physics")]
    fn sync_physics_to_scene(&mut self) -> EngineResult<()> {
        for binding in &self.physics_bindings {
            if let Some(entity) = self.scene.find_by_id(binding.object) {
                let transform = self.physics.backend().body_transform(binding.body)?;
                self.scene.transforms_mut().set_local(entity, transform);
            }
        }
        Ok(())
    }
}

#[cfg(feature = "physics")]
fn collider_desc_from_scene(
    collider: &engine_ecs::ColliderComponentData,
    layer: u32,
) -> ColliderDesc {
    let half = collider.size * 0.5;
    ColliderDesc {
        shape: match collider.shape.as_str() {
            "sphere" => ColliderShape::Sphere {
                radius: half.x.max(half.y).max(half.z),
            },
            "capsule" => ColliderShape::Capsule {
                half_height: half.y,
                radius: half.x.max(half.z),
            },
            _ => ColliderShape::Box { half_extents: half },
        },
        is_trigger: collider.is_trigger,
        layer,
        ..ColliderDesc::default()
    }
}

/// Loaded project context used by runtime-game.
#[derive(Debug)]
pub struct RuntimeProject {
    /// Project root directory.
    pub root: PathBuf,
    /// Parsed project manifest.
    pub manifest: ProjectManifest,
    /// Default scene loaded from the manifest.
    pub scene: Scene,
}

/// Loads a project manifest and default scene.
pub fn load_runtime_project(project: impl AsRef<Path>) -> EngineResult<RuntimeProject> {
    let project = project.as_ref();
    let manifest_path = if project.is_dir() {
        project.join("aster.project.toml")
    } else {
        project.to_path_buf()
    };
    let root = manifest_path
        .parent()
        .ok_or_else(|| EngineError::config("project manifest must have a parent directory"))?
        .to_path_buf();
    let manifest_text =
        fs::read_to_string(&manifest_path).map_err(|source| EngineError::Filesystem {
            path: manifest_path.clone(),
            source,
        })?;
    let manifest = toml::from_str::<ProjectManifest>(&manifest_text)
        .map_err(|error| EngineError::config(format!("project manifest parse failed: {error}")))?;
    if let Some(diagnostic) = manifest.diagnostics().into_iter().next() {
        return Err(EngineError::config(format!(
            "{}: {}",
            diagnostic.path, diagnostic.message
        )));
    }
    let scene_path = root.join(&manifest.default_scene);
    let scene_text = fs::read_to_string(&scene_path).map_err(|source| EngineError::Filesystem {
        path: scene_path.clone(),
        source,
    })?;
    let scene = Scene::from_json(&scene_text)?;
    Ok(RuntimeProject {
        root,
        manifest,
        scene,
    })
}

/// Extracts the active scene into a minimal render queue.
pub fn extract_render_world(scene: &Scene) -> RenderWorld {
    let mut world = RenderWorld::default();
    for (entity, object) in scene.objects() {
        let transform = scene.transforms().local(entity).unwrap_or_default();
        for component in &object.components {
            match component {
                ComponentData::Camera(camera) => {
                    if world.camera.is_none() || camera.primary {
                        world.camera = Some(camera_to_render(object.id, transform, camera));
                    }
                }
                ComponentData::MeshRenderer(renderer) => {
                    world
                        .objects
                        .push(mesh_to_render(object.id, transform, renderer));
                }
                ComponentData::Light(light) => {
                    world.lights.push(engine_render::RenderLight {
                        object: object.id,
                        transform,
                        kind: light.kind.clone(),
                        intensity: light.intensity,
                    });
                }
                ComponentData::Rigidbody(_)
                | ComponentData::Collider(_)
                | ComponentData::AudioSource(_)
                | ComponentData::Script(_) => {}
            }
        }
        if object.camera_role == Some(CameraRole::Main) && world.camera.is_none() {
            world.camera = Some(camera_to_render(
                object.id,
                transform,
                &CameraComponentData::default(),
            ));
        }
        if object.name == "Player" && world.objects.is_empty() {
            world.objects.push(mesh_to_render(
                object.id,
                transform,
                &MeshRendererComponentData::default(),
            ));
        }
    }
    world
}

fn camera_to_render(
    object: engine_core::EntityId,
    transform: engine_core::math::Transform,
    camera: &CameraComponentData,
) -> engine_render::RenderCamera {
    engine_render::RenderCamera {
        object,
        transform,
        vertical_fov_degrees: camera.vertical_fov_degrees,
        near: camera.near,
        far: camera.far,
    }
}

fn mesh_to_render(
    object: engine_core::EntityId,
    transform: engine_core::math::Transform,
    renderer: &MeshRendererComponentData,
) -> engine_render::RenderObject {
    engine_render::RenderObject {
        object,
        transform,
        mesh: renderer
            .builtin_mesh
            .clone()
            .unwrap_or_else(|| "asset-mesh".to_string()),
        material: renderer
            .material
            .builtin
            .clone()
            .unwrap_or_else(|| "asset-material".to_string()),
    }
}

/// Builds the default forward render graph used by the minimal runtime.
pub fn build_default_render_graph() -> RenderGraph {
    let mut builder = RenderGraphBuilder::new();
    let shadow = builder.add_pass("shadow");
    let forward = builder.add_pass("forward");
    let post = builder.add_pass("post");
    builder.order_before(shadow, forward);
    builder.order_before(forward, post);
    builder.build()
}

/// Runs a one-frame native smoke path for the minimal runtime.
pub fn smoke_runtime_min() -> EngineResult<u64> {
    let config = EngineConfig::default();
    logging::log_runtime_start(&config.app_name, config.profile.as_str());
    let mut services = RuntimeServices::minimal(config);
    services.tick()?;
    Ok(services.frame_index())
}

/// Runs a project with the runtime-game windowed loop.
#[cfg(feature = "runtime-game")]
pub fn run_project(project: impl AsRef<Path>) -> EngineResult<()> {
    use engine_platform::{InputEvent, KeyCode};
    use std::{sync::Arc, time::Instant};
    use winit::{
        application::ApplicationHandler,
        event::{ElementState, MouseScrollDelta, WindowEvent},
        event_loop::{ActiveEventLoop, EventLoop},
        keyboard::{KeyCode as WinitKeyCode, PhysicalKey},
        window::{Window, WindowId},
    };

    struct GameApp {
        services: RuntimeServices,
        window: Option<Arc<Window>>,
        last_frame: Instant,
        single_step: bool,
    }

    impl ApplicationHandler for GameApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() {
                return;
            }
            let window = event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Aster Runtime")
                        .with_inner_size(winit::dpi::LogicalSize::new(960_u32, 540_u32)),
                )
                .expect("create runtime window");
            self.window = Some(Arc::new(window));
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::KeyboardInput { event, .. } => {
                    if let Some(key) = convert_winit_key(event.physical_key) {
                        match event.state {
                            ElementState::Pressed => {
                                self.services.input.apply_event(InputEvent::KeyDown(key));
                                if key == KeyCode::Escape {
                                    event_loop.exit();
                                } else if key == KeyCode::Space {
                                    self.services.paused = !self.services.paused;
                                } else if key == KeyCode::Enter {
                                    self.single_step = true;
                                }
                            }
                            ElementState::Released => {
                                self.services.input.apply_event(InputEvent::KeyUp(key));
                            }
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.services.input.apply_event(InputEvent::MouseMove {
                        x: position.x as f32,
                        y: position.y as f32,
                    });
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let (x, y) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (x, y),
                        MouseScrollDelta::PixelDelta(position) => {
                            (position.x as f32, position.y as f32)
                        }
                    };
                    self.services
                        .input
                        .apply_event(InputEvent::MouseWheel { x, y });
                }
                WindowEvent::Resized(size) => {
                    let title = format!(
                        "Aster Runtime - {}x{}",
                        size.width.max(1),
                        size.height.max(1)
                    );
                    if let Some(window) = &self.window {
                        window.set_title(&title);
                    }
                }
                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    let delta = now.saturating_duration_since(self.last_frame);
                    self.last_frame = now;
                    if let Err(error) = self.services.tick_game_frame(delta, self.single_step) {
                        eprintln!("runtime error: {error}");
                        event_loop.exit();
                        return;
                    }
                    self.single_step = false;
                    if let Some(window) = &self.window {
                        let status = if self.services.render_world.is_visible() {
                            "rendering"
                        } else {
                            "empty"
                        };
                        window.set_title(&format!(
                            "Aster Runtime - frame {} - {status}",
                            self.services.frame_index()
                        ));
                    }
                }
                _ => {}
            }
        }

        fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn convert_winit_key(key: PhysicalKey) -> Option<KeyCode> {
        match key {
            PhysicalKey::Code(WinitKeyCode::Escape) => Some(KeyCode::Escape),
            PhysicalKey::Code(WinitKeyCode::Enter) => Some(KeyCode::Enter),
            PhysicalKey::Code(WinitKeyCode::Space) => Some(KeyCode::Space),
            PhysicalKey::Code(WinitKeyCode::ArrowUp) => Some(KeyCode::ArrowUp),
            PhysicalKey::Code(WinitKeyCode::ArrowDown) => Some(KeyCode::ArrowDown),
            PhysicalKey::Code(WinitKeyCode::ArrowLeft) => Some(KeyCode::ArrowLeft),
            PhysicalKey::Code(WinitKeyCode::ArrowRight) => Some(KeyCode::ArrowRight),
            PhysicalKey::Code(WinitKeyCode::KeyW) => Some(KeyCode::Character('w')),
            PhysicalKey::Code(WinitKeyCode::KeyA) => Some(KeyCode::Character('a')),
            PhysicalKey::Code(WinitKeyCode::KeyS) => Some(KeyCode::Character('s')),
            PhysicalKey::Code(WinitKeyCode::KeyD) => Some(KeyCode::Character('d')),
            _ => None,
        }
    }

    let project = load_runtime_project(project)?;
    let mut services = RuntimeServices::minimal(EngineConfig::default());
    services.scene = project.scene;
    services.render_world = extract_render_world(&services.scene);
    let event_loop = EventLoop::new().map_err(|error| EngineError::other(error.to_string()))?;
    let mut app = GameApp {
        services,
        window: None,
        last_frame: Instant::now(),
        single_step: false,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|error| EngineError::other(error.to_string()))
}

/// Reports that runtime-game support is not compiled into this binary.
#[cfg(not(feature = "runtime-game"))]
pub fn run_project(_project: impl AsRef<Path>) -> EngineResult<()> {
    Err(EngineError::UnsupportedCapability {
        capability: "runtime-game",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_min_ticks_one_frame() {
        assert_eq!(smoke_runtime_min().unwrap(), 1);
    }

    #[test]
    fn default_render_graph_has_three_passes() {
        let graph = build_default_render_graph();
        assert_eq!(graph.pass_count(), 3);
        assert_eq!(graph.passes[0].name, "shadow");
        assert_eq!(graph.passes[1].name, "forward");
        assert_eq!(graph.passes[2].name, "post");
    }

    #[test]
    fn runtime_services_can_replace_render_graph() {
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        let mut builder = RenderGraphBuilder::new();
        builder.add_pass("custom");
        services.set_render_graph(builder.build());
        assert_eq!(services.render_graph.pass_count(), 1);
        services.tick().unwrap();
    }

    #[test]
    fn loads_example_project_and_extracts_render_world() {
        let project = load_runtime_project(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/project"),
        )
        .unwrap();
        let render_world = extract_render_world(&project.scene);

        assert!(project.scene.find_by_name("Player").is_some());
        assert!(render_world.is_visible());
    }

    #[test]
    fn game_frame_updates_stats_and_script_diagnostics() {
        let project = load_runtime_project(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/project"),
        )
        .unwrap();
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.scene = project.scene;

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert!(services.stats.entity_count >= 2);
        assert!(services.stats.draw_calls >= 1);
        assert!(services
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source == "script"));
    }

    #[cfg(feature = "physics")]
    #[test]
    fn game_frame_creates_physics_bindings_for_scene_components() {
        let project = load_runtime_project(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/project"),
        )
        .unwrap();
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.scene = project.scene;

        services
            .tick_game_frame(Duration::from_millis(20), false)
            .unwrap();

        assert_eq!(services.physics_bindings.len(), 1);
        assert!(services.stats.physics_steps >= 1);
    }
}
