use super::*;

/// Explicit runtime services used by the headless and game runners.
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
    /// Logical action bindings for high-level input queries.
    pub action_map: ActionMap,
    /// Latest scene extraction submitted to rendering.
    pub render_world: RenderWorld,
    /// Whether the game simulation is paused.
    pub paused: bool,
    /// Whether the built-in pause menu overlay is open.
    pub pause_menu_open: bool,
    /// Whether the runtime requested the host game window to close.
    pub exit_requested: bool,
    /// Aggregated time state (delta, fixed delta, total time, frame index, time scale).
    pub time: TimeState,
    /// Latest runtime counters for diagnostics UI and smoke tests.
    pub stats: RuntimeStats,
    /// Current player-facing render scaling settings.
    pub render_scaling_settings: RenderScalingSettings,
    /// Last successfully negotiated render scaling selection.
    pub render_scaling_selection: Option<engine_render::RenderScalingSelection>,
    /// Runtime render environment overrides emitted by gameplay scripts.
    pub render_environment: RuntimeRenderEnvironment,
    /// User-facing runtime preferences controlled by the pause menu.
    pub user_preferences: RuntimeUserPreferences,
    /// Diagnostics emitted by runtime subsystems.
    pub diagnostics: Vec<RuntimeDiagnostic>,
    /// UI draw commands emitted by scripts during the latest game frame.
    pub ui_commands: Vec<VargUiCommand>,
    /// Current input capture request emitted by runtime scripts.
    pub input_capture: RuntimeInputCapture,
    /// Screen-space pointer positions that began this frame.
    pub pointer_pressed: Vec<(f32, f32)>,
    /// Screen-space pointer positions that ended this frame.
    pub pointer_released: Vec<(f32, f32)>,
    #[cfg(feature = "physics")]
    /// Physics world used by runtime-game.
    pub physics: PhysicsWorld,
    #[cfg(feature = "runtime-game")]
    /// Platform gamepad backend used by runtime-game.
    pub gamepad_provider: Box<dyn GamepadProvider>,
    #[cfg(feature = "audio")]
    /// Audio context used by runtime-game.
    pub audio: AudioContext,
    /// Project root used to resolve script and asset paths.
    pub project_root: Option<PathBuf>,
    /// Script workspace roots used to resolve relative script references.
    pub script_roots: Vec<PathBuf>,
    /// Asset database used to resolve project GUIDs to runtime resources.
    pub asset_database: AssetDatabase,
    /// Runtime asset registry and cache state.
    pub asset_registry: AssetRegistry,
    /// Asset folder used by scan/import and hot reload.
    pub asset_root: Option<PathBuf>,
    /// GPU textures resolved from project asset GUIDs.
    pub texture_resources: HashMap<engine_core::AssetId, ImageHandle>,
    /// GUI textures available to script-authored HUDs.
    pub gui_textures: HashMap<String, GuiTextureId>,
    /// CPU mesh resources resolved from project asset GUIDs.
    pub mesh_resources: HashMap<engine_core::AssetId, ModelResource>,
    /// Material resources resolved from project asset GUIDs.
    pub material_resources: HashMap<engine_core::AssetId, MaterialFormat>,
    frame_counter: FrameCounter,
    #[cfg(feature = "asset-import")]
    hot_reload: HotReloadTracker,
    script_index: RuntimeScriptIndex,
    scene_snapshot: RuntimeSceneSnapshot,
    varg_script_cache: HashMap<PathBuf, Arc<VargScript>>,
    #[cfg(feature = "audio")]
    reported_script_errors: HashSet<String>,
    #[cfg(feature = "audio")]
    audio_bindings: Vec<AudioBinding>,
    #[cfg(feature = "audio")]
    audio_clips: HashMap<engine_core::AssetId, ClipHandle>,
    #[cfg(feature = "audio")]
    audio_listener_position: Option<engine_core::math::Vec3>,
    #[cfg(feature = "audio")]
    transient_audio: Vec<(SourceHandle, ClipHandle)>,
    #[cfg(feature = "audio")]
    procedural_loops: HashMap<String, (SourceHandle, ClipHandle)>,
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
    /// Number of indexed triangles submitted by the render backend.
    pub triangles: u64,
    /// Number of render objects considered before visibility selection.
    pub submitted_render_objects: u32,
    /// Number of render objects selected into the latest Visibility Set.
    pub visible_render_objects: u32,
    /// Number of render objects rejected by visibility selection.
    pub culled_render_objects: u32,
    /// Number of enabled Frame Pipeline passes.
    pub render_pipeline_passes: u32,
    /// Number of scene objects.
    pub entity_count: usize,
    /// Number of render resources known to the runtime.
    pub resource_count: usize,
    /// Number of fixed physics steps run this frame.
    pub physics_steps: u32,
    /// CPU time spent preparing and submitting the render frame.
    pub render_cpu_ms: f32,
    /// Native output dimensions.
    pub output_size: (u32, u32),
    /// Internal rendering dimensions after dynamic resolution.
    pub internal_render_size: (u32, u32),
    /// Active internal linear rendering scale.
    pub render_scale: f32,
    /// Active upscaler implementation.
    pub upscaler: engine_render::UpscalerKind,
    /// Active frame generation implementation.
    pub frame_generation: engine_render::FrameGenerationKind,
    /// Latest GPU frame time, when supported.
    pub gpu_frame_ms: Option<f32>,
    /// Estimated input latency, when supported.
    pub estimated_latency_ms: Option<f32>,
    /// Dropped presentation frame count.
    pub dropped_frames: u64,
    /// Number of logical audio sources.
    pub audio_sources: u32,
    /// Number of physical voices rendered in the latest audio block.
    pub audio_physical_voices: u32,
    /// Number of virtualized voices.
    pub audio_virtual_voices: u32,
    /// Number of audio backend underruns or stream errors.
    pub audio_underruns: u64,
}

/// Runtime render environment values that scripts may drive between frames.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeRenderEnvironment {
    /// Requested global illumination strategy.
    pub global_illumination: RenderGlobalIllumination,
    /// Runtime weather simulation state.
    pub weather: RuntimeWeatherState,
}

impl Default for RuntimeRenderEnvironment {
    fn default() -> Self {
        Self {
            global_illumination: RenderGlobalIllumination::default(),
            weather: RuntimeWeatherState::default(),
        }
    }
}

/// Runtime weather state that scripts may drive between frames.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeWeatherState {
    /// Whether runtime weather should override the extracted environment.
    pub enabled: bool,
    /// Weather preset name: `clear`, `overcast`, `rain`, `storm`, or `night`.
    pub preset: String,
    /// Local time of day in hours `[0.0, 24.0)`.
    pub time_of_day: f32,
    /// Cloud cover in `[0.0, 1.0]`.
    pub cloud_cover: f32,
    /// Precipitation intensity in `[0.0, 1.0]`.
    pub precipitation: f32,
    /// Global weather wind velocity.
    pub wind: Vec3,
}

impl Default for RuntimeWeatherState {
    fn default() -> Self {
        Self {
            enabled: false,
            preset: "clear".to_string(),
            time_of_day: 12.0,
            cloud_cover: 0.0,
            precipitation: 0.0,
            wind: Vec3::ZERO,
        }
    }
}

/// Runtime-owned input capture state requested by gameplay scripts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeInputCapture {
    /// Whether the game window should capture and hide the mouse cursor.
    pub mouse: bool,
}

/// Player-facing runtime preferences.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeUserPreferences {
    /// Whether horizontal mouse look is inverted before gameplay scripts read it.
    pub invert_mouse_x: bool,
    /// Whether vertical mouse look is inverted before gameplay scripts read it.
    pub invert_mouse_y: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VargLifecycleHook {
    Start,
    FixedUpdate,
    Update,
    LateUpdate,
}

impl VargLifecycleHook {
    fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::FixedUpdate => "fixedUpdate",
            Self::Update => "update",
            Self::LateUpdate => "lateUpdate",
        }
    }

    fn runs_once(self) -> bool {
        matches!(self, Self::Start)
    }
}

#[derive(Clone, Debug)]
struct RuntimeScriptInvocation {
    entity: engine_ecs::Entity,
    source: String,
}

#[derive(Clone, Debug, Default)]
struct RuntimeScriptIndex {
    scene_version: Option<u64>,
    start: Vec<RuntimeScriptInvocation>,
    fixed_update: Vec<RuntimeScriptInvocation>,
    update: Vec<RuntimeScriptInvocation>,
    late_update: Vec<RuntimeScriptInvocation>,
}

impl RuntimeScriptIndex {
    fn refresh(&mut self, scene: &Scene) {
        let scene_version = scene.structure_version();
        if self.scene_version == Some(scene_version) {
            return;
        }

        self.start.clear();
        self.fixed_update.clear();
        self.update.clear();
        self.late_update.clear();

        for (entity, object) in scene.iter_objects() {
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
                let invocation = RuntimeScriptInvocation {
                    entity,
                    source: script.source.clone(),
                };
                self.start.push(invocation.clone());
                self.fixed_update.push(invocation.clone());
                self.update.push(invocation.clone());
                self.late_update.push(invocation);
            }
        }

        self.scene_version = Some(scene_version);
    }

    fn invocations(&self, hook: VargLifecycleHook) -> &[RuntimeScriptInvocation] {
        match hook {
            VargLifecycleHook::Start => &self.start,
            VargLifecycleHook::FixedUpdate => &self.fixed_update,
            VargLifecycleHook::Update => &self.update,
            VargLifecycleHook::LateUpdate => &self.late_update,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct RuntimeSceneSnapshot {
    entity_names: HashMap<engine_ecs::Entity, String>,
    entity_tags: HashMap<engine_ecs::Entity, String>,
    positions_by_entity: HashMap<engine_ecs::Entity, Vec3>,
    bounds_by_entity: HashMap<engine_ecs::Entity, VargSceneBounds>,
    shared_positions_by_name: Arc<HashMap<String, Vec3>>,
    shared_positions_by_tag: Arc<HashMap<String, Vec<Vec3>>>,
    shared_bounds_by_name: Arc<HashMap<String, VargSceneBounds>>,
    shared_bounds_by_tag: Arc<HashMap<String, Vec<VargSceneBounds>>>,
}

impl RuntimeSceneSnapshot {
    fn refresh(&mut self, scene: &Scene) {
        self.entity_names.clear();
        self.entity_tags.clear();
        self.positions_by_entity.clear();
        self.bounds_by_entity.clear();
        let mut positions_by_name: HashMap<String, Vec3> = HashMap::new();
        let mut positions_by_tag: HashMap<String, Vec<Vec3>> = HashMap::new();
        let mut bounds_by_name: HashMap<String, VargSceneBounds> = HashMap::new();
        let mut bounds_by_tag: HashMap<String, Vec<VargSceneBounds>> = HashMap::new();

        for (entity, object) in scene.iter_objects() {
            self.entity_names.insert(entity, object.name.clone());
            self.entity_tags.insert(entity, object.tag.clone());
            if let Some(transform) = scene.transforms().local(entity) {
                self.positions_by_entity
                    .insert(entity, transform.translation);
                positions_by_name.insert(object.name.clone(), transform.translation);
                positions_by_tag
                    .entry(object.tag.clone())
                    .or_default()
                    .push(transform.translation);
                let bounds = script_bounds_for_object(object, transform);
                self.bounds_by_entity.insert(entity, bounds);
                bounds_by_name.insert(object.name.clone(), bounds);
                bounds_by_tag
                    .entry(object.tag.clone())
                    .or_default()
                    .push(bounds);
            }
        }
        self.shared_positions_by_name = Arc::new(positions_by_name);
        self.shared_positions_by_tag = Arc::new(positions_by_tag);
        self.shared_bounds_by_name = Arc::new(bounds_by_name);
        self.shared_bounds_by_tag = Arc::new(bounds_by_tag);
    }

    fn sync_entity_transform(&mut self, scene: &Scene, entity: engine_ecs::Entity) {
        let Some(object) = scene.object(entity) else {
            return;
        };
        let Some(transform) = scene.transforms().local(entity) else {
            return;
        };
        let position = transform.translation;
        self.entity_names.insert(entity, object.name.clone());
        self.entity_tags.insert(entity, object.tag.clone());
        Arc::make_mut(&mut self.shared_positions_by_name).insert(object.name.clone(), position);
        if let Some(previous) = self.positions_by_entity.insert(entity, position) {
            if let Some(tag_positions) =
                Arc::make_mut(&mut self.shared_positions_by_tag).get_mut(&object.tag)
            {
                if let Some(index) = tag_positions
                    .iter()
                    .position(|candidate| *candidate == previous)
                {
                    tag_positions.remove(index);
                }
            }
        }
        Arc::make_mut(&mut self.shared_positions_by_tag)
            .entry(object.tag.clone())
            .or_default()
            .push(position);
        let bounds = script_bounds_for_object(object, transform);
        if let Some(previous) = self.bounds_by_entity.insert(entity, bounds) {
            if let Some(tag_bounds) =
                Arc::make_mut(&mut self.shared_bounds_by_tag).get_mut(&object.tag)
            {
                if let Some(index) = tag_bounds
                    .iter()
                    .position(|candidate| *candidate == previous)
                {
                    tag_bounds.remove(index);
                }
            }
        }
        Arc::make_mut(&mut self.shared_bounds_by_name).insert(object.name.clone(), bounds);
        Arc::make_mut(&mut self.shared_bounds_by_tag)
            .entry(object.tag.clone())
            .or_default()
            .push(bounds);
    }

    fn context_for(&self, entity: engine_ecs::Entity) -> VargSceneContext {
        VargSceneContext::from_shared_scene(
            self.entity_names.get(&entity).cloned().unwrap_or_default(),
            self.entity_tags.get(&entity).cloned().unwrap_or_default(),
            Arc::clone(&self.shared_positions_by_name),
            Arc::clone(&self.shared_positions_by_tag),
            Arc::clone(&self.shared_bounds_by_name),
            Arc::clone(&self.shared_bounds_by_tag),
        )
    }
}

fn script_bounds_for_object(
    object: &engine_ecs::GameObject,
    transform: Transform,
) -> VargSceneBounds {
    let size = object
        .components
        .iter()
        .find_map(|component| match component {
            ComponentData::Collider(collider) => Some(collider.size * transform.scale),
            _ => None,
        })
        .unwrap_or(Vec3::ZERO);
    VargSceneBounds::from_center_size(transform.translation, size)
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
    last_position: engine_core::math::Vec3,
    last_rotation: engine_core::math::Quat,
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug)]
struct AudioBinding {
    object: engine_core::EntityId,
    source: SourceHandle,
    last_position: engine_core::math::Vec3,
}

impl RuntimeServices<HeadlessRenderDevice> {
    /// Creates minimal runtime services with a headless renderer.
    pub fn minimal(config: EngineConfig) -> Self {
        Self::with_renderer(config, HeadlessRenderDevice::default())
    }
}

/// Creates headless runtime services from a cloned edit-time scene snapshot.
pub fn headless_services_from_scene(
    config: EngineConfig,
    project_root: impl Into<PathBuf>,
    scene: &Scene,
) -> EngineResult<RuntimeServices> {
    let file = scene.to_scene_file("play-copy")?;
    let mut services = RuntimeServices::minimal(config);
    services.set_project_root(project_root);
    services.scene = Scene::from_scene_file(file)?;
    services.render_world = extract_render_world(&services.scene);
    Ok(services)
}

#[cfg(feature = "asset-import")]
fn modified_time(path: &Path) -> EngineResult<std::time::SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|source| EngineError::Filesystem {
            path: path.to_path_buf(),
            source,
        })
}

impl<R: RenderDevice> RuntimeServices<R> {
    /// Creates runtime services with an explicit render backend.
    pub fn with_renderer(config: EngineConfig, renderer: R) -> Self {
        let render_graph = build_default_render_graph();
        Self {
            config,
            scene: Scene::default(),
            renderer,
            render_graph,
            input: {
                let mut input = InputState::default();
                input.bind_default_player_actions();
                input
            },
            action_map: ActionMap::new(),
            render_world: RenderWorld::default(),
            paused: false,
            pause_menu_open: false,
            exit_requested: false,
            time: TimeState::new(),
            stats: RuntimeStats::default(),
            render_scaling_settings: RenderScalingSettings::default(),
            render_scaling_selection: None,
            render_environment: RuntimeRenderEnvironment::default(),
            user_preferences: RuntimeUserPreferences::default(),
            diagnostics: Vec::new(),
            ui_commands: Vec::new(),
            input_capture: RuntimeInputCapture::default(),
            pointer_pressed: Vec::new(),
            pointer_released: Vec::new(),
            #[cfg(feature = "physics")]
            physics: PhysicsWorld::new(RapierPhysicsBackend::new()),
            #[cfg(feature = "runtime-game")]
            gamepad_provider: engine_platform::GilrsGamepadProvider::new()
                .map(|provider| Box::new(provider) as Box<dyn GamepadProvider>)
                .unwrap_or_else(|_| Box::new(engine_platform::NullGamepadProvider)),
            #[cfg(feature = "audio")]
            audio: AudioContext::new(MemoryAudioBackend::new()),
            project_root: None,
            script_roots: Vec::new(),
            asset_database: AssetDatabase::new("assets", "builtin"),
            asset_registry: AssetRegistry::default(),
            asset_root: None,
            texture_resources: HashMap::new(),
            gui_textures: HashMap::new(),
            mesh_resources: HashMap::new(),
            material_resources: HashMap::new(),
            frame_counter: FrameCounter::default(),
            #[cfg(feature = "asset-import")]
            hot_reload: HotReloadTracker::default(),
            script_index: RuntimeScriptIndex::default(),
            scene_snapshot: RuntimeSceneSnapshot::default(),
            varg_script_cache: HashMap::new(),
            #[cfg(feature = "audio")]
            reported_script_errors: HashSet::new(),
            #[cfg(feature = "audio")]
            audio_bindings: Vec::new(),
            #[cfg(feature = "audio")]
            audio_clips: HashMap::new(),
            #[cfg(feature = "audio")]
            audio_listener_position: None,
            #[cfg(feature = "audio")]
            transient_audio: Vec::new(),
            #[cfg(feature = "audio")]
            procedural_loops: HashMap::new(),
            #[cfg(feature = "physics")]
            physics_bindings: Vec::new(),
        }
    }

    fn ensure_builtin_gui_textures(&mut self) -> EngineResult<()> {
        for (key, width, height, pixels) in builtin_gui_texture_sources() {
            if self.gui_textures.contains_key(key) {
                continue;
            }
            let id = self.renderer.upload_gui_texture(
                ImageDesc {
                    width,
                    height,
                    mip_levels: 1,
                    samples: 1,
                    format: ImageFormat::Rgba8Srgb,
                    usage: ImageUsage::SAMPLED.or(ImageUsage::TRANSFER_DST),
                    label: Some("runtime gui texture"),
                },
                &pixels,
            )?;
            self.gui_textures.insert(key.to_string(), id);
        }
        Ok(())
    }

    /// Applies player-facing render settings immediately without recreating the renderer.
    pub fn set_render_scaling(
        &mut self,
        settings: RenderScalingSettings,
        context: RenderScalingContext,
    ) -> engine_render::RenderScalingSelection {
        let settings = settings.normalized();
        let selection = self.renderer.configure_render_scaling(&settings, context);
        self.render_scaling_settings = settings;
        self.render_scaling_selection = Some(selection.clone());
        self.stats.render_scale = selection.render_scale;
        self.stats.upscaler = selection.upscaler;
        self.stats.frame_generation = selection.frame_generation;
        selection
    }

    /// Returns the renderer's currently available scaling capabilities.
    pub fn render_scaling_capabilities(&self) -> engine_render::RenderScalingCapabilities {
        self.renderer.render_scaling_capabilities()
    }

    #[cfg(all(feature = "audio", feature = "runtime-game"))]
    /// Attempts to route runtime audio to the operating system's default output device.
    ///
    /// If no device can be opened, the runtime keeps the deterministic memory backend
    /// and records a warning diagnostic so preview hosts can surface the failure.
    pub fn enable_default_audio_output(&mut self) {
        match AudioContext::device_default() {
            Ok(audio) => self.audio = audio,
            Err(error) => self.diagnostics.push(RuntimeDiagnostic {
                source: "audio".to_string(),
                level: "warning".to_string(),
                message: format!("default audio output unavailable; using memory backend: {error}"),
                file: None,
                line: None,
            }),
        }
    }

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
        self.run_frame(delta, single_step)
    }

    /// Returns and clears the current runtime exit request.
    pub fn take_exit_requested(&mut self) -> bool {
        let requested = self.exit_requested;
        self.exit_requested = false;
        requested
    }

    /// Executes one game frame in well-ordered phases:
    ///
    /// 1. **begin_frame** — reset transient input state
    /// 2. **fixed_timestep_loop** — physics step + fixed-update scripts
    /// 3. **update** — player controller + update scripts
    /// 4. **late_update** — scene runtime tick + audio
    /// 5. **render_submit** — extract render world, submit, execute graph
    /// 6. **deferred_destroy** — flush GPU destroy queue, process scene deferred destroys
    /// 7. **end_frame** — update stats, advance frame counter
    pub fn run_frame(&mut self, delta: Duration, single_step: bool) -> EngineResult<()> {
        let dt = delta.as_secs_f32();

        // ── begin_frame ────────────────────────────────────────────────
        logging::log_frame(self.frame_counter.get());
        #[cfg(feature = "runtime-game")]
        {
            let gamepads = self.gamepad_provider.poll_gamepads();
            self.input.apply_gamepad_states(gamepads);
        }
        self.time.update(dt);
        self.stats.frame_time_seconds = self.time.delta_seconds;
        self.input.evaluate_mapped_actions(self.time.delta_seconds);
        self.stats.physics_steps = 0;
        self.ui_commands.clear();
        self.handle_pause_menu_input();
        let should_simulate = (!self.paused && !self.pause_menu_open) || single_step;

        // ── script startup ───────────────────────────────────────────
        if should_simulate {
            #[cfg(feature = "physics")]
            self.ensure_physics_bindings()?;
            #[cfg(feature = "audio")]
            self.ensure_audio_bindings()?;
            self.run_varg_scripts_start();

            // ── fixed_timestep_loop ────────────────────────────────────
            let mut fixed_steps = 0;
            while self.time.consume_fixed_step(fixed_steps) {
                #[cfg(feature = "physics")]
                {
                    let fixed_dt = self.time.fixed_delta_seconds;
                    self.sync_scene_to_physics()?;
                    self.apply_environment_forces(fixed_dt)?;
                    self.physics.fixed_update(fixed_dt);
                    self.report_physics_events();
                    self.sync_physics_to_scene()?;
                    self.stats.physics_steps = self.stats.physics_steps.saturating_add(1);
                }
                let fixed_dt = self.time.fixed_delta_seconds;
                self.run_varg_scripts_fixed_update(fixed_dt);
                self.scene.tick_fixed_frame();
                fixed_steps += 1;
            }

            // ── update ─────────────────────────────────────────────────
            self.apply_builtin_player_controller();
            self.run_varg_scripts_update(dt);

            // ── late_update ────────────────────────────────────────────
            self.scene.tick_runtime_frame();
            self.run_varg_scripts_late_update(dt);
        }

        #[cfg(feature = "audio")]
        self.update_audio(dt)?;

        // ── render_submit ──────────────────────────────────────────────
        self.render_world = extract_render_world(&self.scene);
        self.render_world.global_illumination = self.render_environment.global_illumination.clone();
        apply_runtime_weather_environment(&mut self.render_world, &self.render_environment.weather);
        Self::resolve_render_materials(&mut self.render_world, &self.mesh_resources);
        let frame = RenderFrame {
            frame_index: self.frame_counter.get(),
        };
        self.ensure_builtin_gui_textures()?;
        let ui_draw_list = build_script_ui_draw_list(&self.ui_commands, &self.gui_textures);
        let ui_draw_list = if self.pause_menu_open {
            build_pause_menu_draw_list(ui_draw_list, self.user_preferences, self.stats.output_size)
        } else {
            ui_draw_list
        };
        if !ui_draw_list.commands.is_empty() {
            self.renderer.queue_surface_gui(ui_draw_list.clone());
        }
        self.renderer.submit_render_world_with_graph(
            &self.render_world,
            &self.render_graph,
            frame,
        )?;
        if !ui_draw_list.commands.is_empty() {
            self.renderer.draw_gui(&ui_draw_list)?;
        }
        self.renderer.record_frame_time(dt * 1000.0);
        let render_metrics = self.renderer.performance_metrics();

        // ── deferred_destroy ───────────────────────────────────────────
        self.renderer
            .flush_destroy_queue(self.frame_counter.get().saturating_sub(2));
        self.scene.process_deferred_destroy()?;

        // ── end_frame ──────────────────────────────────────────────────
        self.stats.draw_calls = if self.renderer.api() == RenderApi::Headless {
            self.render_world.objects.len()
        } else {
            render_metrics.draw_calls as usize
        };
        if self.renderer.api() == RenderApi::Headless {
            self.stats.triangles = 0;
            self.stats.submitted_render_objects = self.render_world.objects.len() as u32;
            self.stats.visible_render_objects = self.render_world.objects.len() as u32;
            self.stats.culled_render_objects = 0;
            self.stats.render_pipeline_passes = self.render_graph.pass_count() as u32;
        } else {
            self.stats.triangles = render_metrics.triangles;
            self.stats.submitted_render_objects = render_metrics.submitted_objects;
            self.stats.visible_render_objects = render_metrics.visible_objects;
            self.stats.culled_render_objects = render_metrics.culled_objects;
            self.stats.render_pipeline_passes = render_metrics.pipeline_passes;
        }
        self.stats.entity_count = self.scene.object_count();
        self.stats.resource_count = self.render_world.objects.len()
            + self.render_world.lights.len()
            + usize::from(self.render_world.camera.is_some());
        self.stats.render_cpu_ms = render_metrics.render_cpu_ms;
        self.stats.output_size = (render_metrics.output_width, render_metrics.output_height);
        self.stats.internal_render_size = (
            render_metrics.internal_width,
            render_metrics.internal_height,
        );
        self.stats.render_scale = render_metrics.render_scale;
        self.stats.upscaler = render_metrics.upscaler;
        self.stats.frame_generation = render_metrics.frame_generation;
        self.stats.gpu_frame_ms = render_metrics.gpu_frame_ms;
        self.stats.estimated_latency_ms = render_metrics.estimated_latency_ms;
        self.stats.dropped_frames = render_metrics.dropped_frames;
        self.input.end_frame();
        self.pointer_pressed.clear();
        self.pointer_released.clear();
        self.frame_counter.advance();
        Ok(())
    }

    fn handle_pause_menu_input(&mut self) {
        use engine_platform::KeyCode;

        if self.input.key_pressed(KeyCode::Escape) {
            self.pause_menu_open = !self.pause_menu_open;
            if !self.pause_menu_open {
                self.paused = false;
                return;
            }
        }
        if !self.pause_menu_open {
            return;
        }

        self.input_capture.mouse = false;
        self.paused = true;
        let pause_layout = PauseMenuLayout::for_output(self.stats.output_size);

        if self.input.key_pressed(KeyCode::Enter)
            || self.input.key_pressed(KeyCode::Space)
            || self.input.key_pressed(KeyCode::Character('e'))
            || self
                .pointer_released
                .iter()
                .any(|(x, y)| pause_layout.continue_button.contains(*x, *y))
        {
            self.pause_menu_open = false;
            self.paused = false;
        } else if self.input.key_pressed(KeyCode::Character('q'))
            || self
                .pointer_released
                .iter()
                .any(|(x, y)| pause_layout.exit_button.contains(*x, *y))
        {
            self.exit_requested = true;
        } else if self.input.key_pressed(KeyCode::Character('x'))
            || self
                .pointer_released
                .iter()
                .any(|(x, y)| pause_layout.invert_x_button.contains(*x, *y))
        {
            self.user_preferences.invert_mouse_x = !self.user_preferences.invert_mouse_x;
        } else if self.input.key_pressed(KeyCode::Character('y'))
            || self
                .pointer_released
                .iter()
                .any(|(x, y)| pause_layout.invert_y_button.contains(*x, *y))
        {
            self.user_preferences.invert_mouse_y = !self.user_preferences.invert_mouse_y;
        }
    }

    /// Resolves material names on render objects using imported model
    /// materials. When a render object references a model but has no explicit
    /// material asset assignment, the first material from the model is used.
    fn resolve_render_materials(
        world: &mut RenderWorld,
        mesh_resources: &HashMap<engine_core::AssetId, ModelResource>,
    ) {
        for obj in &mut world.objects {
            if !obj.material.is_empty() && obj.material != "debug/default" {
                continue;
            }
            let Some(model_guid) = parse_asset_guid(&obj.mesh) else {
                continue;
            };
            let Some(model) = mesh_resources.get(&model_guid) else {
                continue;
            };
            if model.materials.is_empty() {
                continue;
            }
            let material_index = model_material_index_for_mesh(&obj.mesh, model).unwrap_or(0);
            obj.material = format!("material:{:032x}:{material_index}", model_guid.as_u128());
        }
    }

    /// Current frame index.
    pub fn frame_index(&self) -> u64 {
        self.frame_counter.get()
    }

    fn run_varg_scripts_start(&mut self) {
        self.run_varg_lifecycle(VargLifecycleHook::Start, 0.0);
    }

    fn run_varg_scripts_fixed_update(&mut self, fixed_dt: f32) {
        self.run_varg_lifecycle(VargLifecycleHook::FixedUpdate, fixed_dt);
    }

    fn run_varg_scripts_update(&mut self, dt: f32) {
        self.run_varg_lifecycle(VargLifecycleHook::Update, dt);
    }

    fn run_varg_scripts_late_update(&mut self, dt: f32) {
        self.run_varg_lifecycle(VargLifecycleHook::LateUpdate, dt);
    }

    fn run_varg_lifecycle(&mut self, hook: VargLifecycleHook, dt: f32) {
        self.script_index.refresh(&self.scene);
        self.scene_snapshot.refresh(&self.scene);
        let invocations = self.script_index.invocations(hook).to_vec();

        for invocation in invocations {
            let entity = invocation.entity;
            let Some(script) = self.varg_script_component(entity, &invocation.source) else {
                continue;
            };
            if hook.runs_once() && script.state.contains_key("__varg_started") {
                continue;
            }
            let script_path = self.resolve_project_path(&script.source);
            let compiled = match self.load_varg_script(&script_path) {
                Ok(script) => script,
                Err(error) => {
                    self.diagnostics.push(RuntimeDiagnostic {
                        source: "script".to_string(),
                        level: "error".to_string(),
                        message: format!("varg load error: {error}"),
                        file: Some(script_path),
                        line: None,
                    });
                    continue;
                }
            };
            if !compiled.has_hook(hook.name()) {
                continue;
            }
            let transform = self.scene.transforms().local(entity).unwrap_or_default();
            let scene_context = self.scene_snapshot.context_for(entity);
            let script_input = self.input_for_scripts();
            let screen_size = (
                self.stats.output_size.0.max(800) as f32,
                self.stats.output_size.1.max(600) as f32,
            );
            let mut output = compiled.run_hook_borrowed(
                hook.name(),
                VargRuntimeContextRef {
                    transform,
                    input: &script_input,
                    pointer_pressed: &self.pointer_pressed,
                    pointer_released: &self.pointer_released,
                    delta_time: dt,
                    total_time: self.time.total_time,
                    frame_index: self.time.frame_index,
                    screen_size,
                    exported_values: &script.exported_values,
                    state: script.state.clone(),
                    scene: scene_context,
                },
            );
            if hook.runs_once() {
                output
                    .state
                    .insert("__varg_started".to_string(), serde_json::Value::Bool(true));
            }
            self.scene
                .transforms_mut()
                .set_local(entity, output.transform);
            self.scene_snapshot
                .sync_entity_transform(&self.scene, entity);
            self.apply_varg_script_state(entity, &script.source, output.state);
            if output.destroy_self {
                if let Err(error) = self.scene.destroy_deferred(entity) {
                    self.diagnostics.push(RuntimeDiagnostic {
                        source: "script".to_string(),
                        level: "error".to_string(),
                        message: format!("varg destroy error: {error}"),
                        file: Some(script_path.clone()),
                        line: None,
                    });
                }
            }
            for request in output.spawn_requests {
                if let Err(error) = self.apply_varg_spawn_request(request) {
                    self.diagnostics.push(RuntimeDiagnostic {
                        source: "script".to_string(),
                        level: "error".to_string(),
                        message: format!("varg spawn error: {error}"),
                        file: Some(script_path.clone()),
                        line: None,
                    });
                } else {
                    self.scene_snapshot.refresh(&self.scene);
                }
            }
            for request in output.destroy_nearest_requests {
                if let Err(error) = self.apply_varg_destroy_nearest_request(entity, request) {
                    self.diagnostics.push(RuntimeDiagnostic {
                        source: "script".to_string(),
                        level: "error".to_string(),
                        message: format!("varg destroy nearest error: {error}"),
                        file: Some(script_path.clone()),
                        line: None,
                    });
                }
            }
            if let Some(mouse_capture) = output.mouse_capture {
                self.input_capture.mouse = mouse_capture;
            }
            for command in output.audio_commands {
                if let Err(error) = self.apply_varg_audio_command(command) {
                    self.diagnostics.push(RuntimeDiagnostic {
                        source: "audio".to_string(),
                        level: "warning".to_string(),
                        message: format!("varg audio error: {error}"),
                        file: Some(script_path.clone()),
                        line: None,
                    });
                }
            }
            for command in output.render_commands {
                self.apply_varg_render_command(command);
            }
            for message in output.logs {
                self.diagnostics.push(RuntimeDiagnostic {
                    source: "script".to_string(),
                    level: "info".to_string(),
                    message,
                    file: Some(script_path.clone()),
                    line: None,
                });
            }
            self.ui_commands.extend(output.ui_commands);
        }
    }

    fn apply_varg_render_command(&mut self, command: VargRenderCommand) {
        match command {
            VargRenderCommand::UseScreenSpaceGi => {
                self.render_environment.global_illumination =
                    RenderGlobalIllumination::ScreenSpace { intensity: 1.0 };
            }
            VargRenderCommand::UseProbeVolumeGi {
                center,
                extent,
                counts,
                intensity,
            } => {
                self.render_environment.global_illumination =
                    RenderGlobalIllumination::ProbeVolume(RenderProbeVolume {
                        center,
                        extent: Vec3::new(
                            extent.x.abs().max(0.001),
                            extent.y.abs().max(0.001),
                            extent.z.abs().max(0.001),
                        ),
                        counts: [
                            script_probe_count(counts.x),
                            script_probe_count(counts.y),
                            script_probe_count(counts.z),
                        ],
                        intensity: intensity.max(0.0),
                    });
            }
            VargRenderCommand::SetGiIntensity(intensity) => {
                let intensity = intensity.max(0.0);
                match &mut self.render_environment.global_illumination {
                    RenderGlobalIllumination::ProbeVolume(volume) => {
                        volume.intensity = intensity;
                    }
                    RenderGlobalIllumination::ScreenSpace {
                        intensity: screen_space_intensity,
                    } => {
                        *screen_space_intensity = intensity;
                    }
                }
            }
            VargRenderCommand::SetWeatherPreset(preset) => {
                self.render_environment.weather.enabled = true;
                self.render_environment.weather.preset = normalize_weather_preset(&preset);
                apply_weather_preset_defaults(&mut self.render_environment.weather);
            }
            VargRenderCommand::SetWeatherTimeOfDay(time_of_day) => {
                self.render_environment.weather.enabled = true;
                self.render_environment.weather.time_of_day = normalize_time_of_day(time_of_day);
            }
            VargRenderCommand::SetWeatherCloudCover(cloud_cover) => {
                self.render_environment.weather.enabled = true;
                self.render_environment.weather.cloud_cover = cloud_cover.clamp(0.0, 1.0);
            }
            VargRenderCommand::SetWeatherPrecipitation(precipitation) => {
                self.render_environment.weather.enabled = true;
                self.render_environment.weather.precipitation = precipitation.clamp(0.0, 1.0);
            }
            VargRenderCommand::SetWeatherWind(wind) => {
                self.render_environment.weather.enabled = true;
                self.render_environment.weather.wind = wind;
            }
        }
    }

    fn varg_script_component(
        &self,
        entity: engine_ecs::Entity,
        source: &str,
    ) -> Option<ScriptComponent> {
        let object = self.scene.object(entity)?;
        object
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
            .find(|script| script.source == source)
            .cloned()
    }

    fn apply_varg_spawn_request(&mut self, request: VargSpawnRequest) -> EngineResult<()> {
        let entity = self.scene.create_object(request.name)?;
        if let Some(object) = self.scene.object_mut(entity) {
            object.tag = request.tag;
        }
        self.scene.transforms_mut().set_local(
            entity,
            Transform {
                translation: request.position,
                scale: request.size,
                ..Transform::IDENTITY
            },
        );
        self.scene.upsert_component(
            entity,
            ComponentData::MeshRenderer(MeshRendererComponentData {
                mesh: None,
                builtin_mesh: Some(request.builtin_mesh),
                material: MaterialRef::debug(),
                casts_shadows: true,
                receive_shadows: true,
            }),
        )?;
        self.scene.upsert_component(
            entity,
            ComponentData::Collider(ColliderComponentData {
                shape: request.collider_shape,
                size: Vec3::ONE,
                is_trigger: false,
                mask: !0,
                physics_material: "default".to_string(),
            }),
        )?;
        if let Some(script) = request.script {
            self.scene
                .upsert_component(entity, ComponentData::Script(ScriptComponent::new(script)))?;
        }
        Ok(())
    }

    fn apply_varg_destroy_nearest_request(
        &mut self,
        source_entity: engine_ecs::Entity,
        request: VargDestroyNearestRequest,
    ) -> EngineResult<()> {
        if request.tag.is_empty() || request.radius <= 0.0 {
            return Ok(());
        }

        let nearest = self
            .scene
            .iter_objects()
            .filter(|(entity, object)| *entity != source_entity && object.tag == request.tag)
            .filter_map(|(entity, _)| {
                let transform = self.scene.transforms().local(entity)?;
                let distance = (transform.translation - request.origin).length();
                (distance <= request.radius).then_some((entity, distance))
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(entity, _)| entity);

        if let Some(entity) = nearest {
            self.scene.destroy_deferred(entity)?;
        }
        Ok(())
    }

    #[cfg(feature = "audio")]
    fn apply_varg_audio_command(&mut self, command: VargAudioCommand) -> EngineResult<()> {
        match command {
            VargAudioCommand::PlayTone {
                waveform,
                frequency_hz,
                duration_seconds,
                volume,
                spatial,
                position,
            } => {
                const SAMPLE_RATE: u32 = 44_100;
                let waveform = parse_synth_waveform(&waveform);
                let frequency = frequency_hz.clamp(20.0, 20_000.0);
                let duration = duration_seconds.clamp(0.005, 5.0);
                let volume = volume.clamp(0.0, 1.0);
                let samples = generate_tone(waveform, frequency, duration, volume, SAMPLE_RATE);
                let clip =
                    self.audio
                        .backend_mut()
                        .load_clip("script-tone", &samples, 1, SAMPLE_RATE)?;
                let source = self.audio.backend_mut().spawn_source(&AudioSourceDesc {
                    clip,
                    volume: 1.0,
                    pitch: 1.0,
                    looping: false,
                    position: spatial.then_some(position),
                    auto_play: true,
                    bus: "SFX".to_string(),
                    spatial_mode: if spatial {
                        SpatialMode::Object
                    } else {
                        SpatialMode::Direct
                    },
                    shape: AudioSourceShape::Point,
                    attenuation: AttenuationModel::default(),
                    priority: 160,
                    virtualization: VirtualizationPolicy::Virtualize,
                    category: VoiceCategory::Sfx,
                    critical: false,
                    doppler_scale: 0.0,
                    spread: 1.0,
                    use_hrtf: true,
                })?;
                self.transient_audio.push((source, clip));
            }
            VargAudioCommand::StartLoop {
                id,
                waveform,
                pattern,
                bpm,
                beats_per_note,
                volume,
            } => {
                let id = id.trim();
                if id.is_empty() || self.procedural_loops.contains_key(id) {
                    return Ok(());
                }
                const SAMPLE_RATE: u32 = 44_100;
                let waveform = parse_synth_waveform(&waveform);
                let samples = generate_loop_pattern(
                    waveform,
                    &pattern,
                    bpm,
                    beats_per_note,
                    volume,
                    SAMPLE_RATE,
                )?;
                let clip =
                    self.audio
                        .backend_mut()
                        .load_clip("script-loop", &samples, 1, SAMPLE_RATE)?;
                let source = self.audio.backend_mut().spawn_source(&AudioSourceDesc {
                    clip,
                    volume: 1.0,
                    pitch: 1.0,
                    looping: true,
                    position: None,
                    auto_play: true,
                    bus: "Music".to_string(),
                    spatial_mode: SpatialMode::Direct,
                    shape: AudioSourceShape::Point,
                    attenuation: AttenuationModel::default(),
                    priority: 96,
                    virtualization: VirtualizationPolicy::Virtualize,
                    category: VoiceCategory::Music,
                    critical: false,
                    doppler_scale: 0.0,
                    spread: 1.0,
                    use_hrtf: false,
                })?;
                self.procedural_loops.insert(id.to_string(), (source, clip));
            }
            VargAudioCommand::StopLoop { id } => {
                if let Some((source, clip)) = self.procedural_loops.remove(id.trim()) {
                    let _ = self.audio.backend_mut().destroy_source(source);
                    let _ = self.audio.backend_mut().unload_clip(clip);
                }
            }
        }
        Ok(())
    }

    #[cfg(not(feature = "audio"))]
    fn apply_varg_audio_command(&mut self, _command: VargAudioCommand) -> EngineResult<()> {
        Err(EngineError::config(
            "script audio requires the runtime `audio` feature",
        ))
    }

    fn load_varg_script(&mut self, path: &Path) -> EngineResult<Arc<VargScript>> {
        if !self.varg_script_cache.contains_key(path) {
            let source = fs::read_to_string(path).map_err(|source| EngineError::Filesystem {
                path: path.to_path_buf(),
                source,
            })?;
            let (script, diagnostics) = compile_script_source(path, &source);
            if !diagnostics.is_empty() {
                let details = diagnostics
                    .iter()
                    .map(|diagnostic| {
                        format!(
                            "{} at {}:{}: {}",
                            diagnostic.code,
                            diagnostic.line.unwrap_or(1),
                            diagnostic.column.unwrap_or(1),
                            diagnostic.message
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(EngineError::other(details));
            }
            let Some(script) = script else {
                return Err(EngineError::other("Varg script did not compile"));
            };
            self.varg_script_cache
                .insert(path.to_path_buf(), Arc::new(script));
        }
        Ok(Arc::clone(
            self.varg_script_cache.get(path).expect("script inserted"),
        ))
    }

    fn apply_varg_script_state(
        &mut self,
        entity: engine_ecs::Entity,
        source: &str,
        state: HashMap<String, serde_json::Value>,
    ) {
        let _ = self.scene.update_script_state(entity, source, state);
    }

    fn resolve_project_path(&self, path: &str) -> PathBuf {
        let path = path.strip_prefix("project:/").unwrap_or(path);
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            let Some(project_root) = self.project_root.as_ref() else {
                return path;
            };
            let project_path = project_root.join(&path);
            if project_path.is_file() {
                return project_path;
            }
            for script_root in &self.script_roots {
                let root = if script_root.is_absolute() {
                    script_root.clone()
                } else {
                    project_root.join(script_root)
                };
                let candidate = root.join(&path);
                if candidate.is_file() {
                    return candidate;
                }
            }
            project_path
        }
    }

    /// Returns true if any key bound to `action_name` was pressed this frame.
    pub fn action_pressed(&self, action_name: &str) -> bool {
        self.input.action_pressed(action_name)
            || self.action_map.action_pressed(&self.input, action_name)
    }

    /// Returns true if any key bound to `action_name` is held (including the first frame).
    pub fn action_held(&self, action_name: &str) -> bool {
        self.input.action_down(action_name) || self.action_map.action_held(&self.input, action_name)
    }

    /// Loads action bindings from a TOML file at the given path.
    ///
    /// The TOML format is:
    /// ```toml
    /// [actions]
    /// MoveForward = ["W", "ArrowUp"]
    /// Jump = ["Space"]
    ///
    /// [contexts.Gameplay]
    /// priority = 10
    ///
    /// [[contexts.Gameplay.bindings]]
    /// action = "Jump"
    /// keys = ["Space"]
    /// triggers = ["Pressed"]
    /// ```
    /// Key names must match `KeyCode` variant names (Escape, Enter, Space,
    /// ArrowUp, ArrowDown, ArrowLeft, ArrowRight) or be a single character
    /// for `Character` keys.
    pub fn load_action_bindings(&mut self, path: &Path) -> EngineResult<()> {
        let content = fs::read_to_string(path).map_err(|source| EngineError::Filesystem {
            path: path.to_path_buf(),
            source,
        })?;
        let doc: toml::Value = content.parse().map_err(|_| {
            EngineError::other(format!(
                "failed to parse action bindings TOML at {}",
                path.display()
            ))
        })?;
        if let Some(actions) = doc.get("actions").and_then(|value| value.as_table()) {
            for (name, keys) in actions {
                if let Some(arr) = keys.as_array() {
                    for key_val in arr {
                        if let Some(key_str) = key_val.as_str() {
                            if let Some(key) = ActionMap::parse_key_name(key_str) {
                                self.action_map.bind(name, key);
                            }
                        }
                    }
                }
            }
        }

        if let Some(contexts) = doc.get("contexts").and_then(|value| value.as_table()) {
            for (name, context) in contexts {
                let Some(context) = context.as_table() else {
                    continue;
                };
                let mut map = InputMap {
                    name: name.clone(),
                    priority: context
                        .get("priority")
                        .and_then(toml::Value::as_integer)
                        .unwrap_or_default() as i32,
                    ..Default::default()
                };

                if let Some(bindings) = context.get("bindings").and_then(toml::Value::as_array) {
                    for binding in bindings {
                        let Some(binding) = binding.as_table() else {
                            continue;
                        };
                        let Some(action) = binding.get("action").and_then(toml::Value::as_str)
                        else {
                            continue;
                        };
                        map.mappings
                            .push((action.to_string(), parse_input_binding_table(binding)));
                    }
                }

                self.input.set_input_map(map);
            }
        }
        Ok(())
    }

    /// Replaces the active render graph.
    pub fn set_render_graph(&mut self, graph: RenderGraph) {
        self.render_graph = graph;
    }

    /// Processes a winit window event, dispatching it to the appropriate
    /// input state or renderer method.
    ///
    /// Handles: KeyboardInput, MouseInput, CursorMoved, MouseWheel, Resized.
    /// Returns `true` if the event was CloseRequested (caller should exit).
    #[cfg(feature = "runtime-game")]
    pub fn process_winit_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        use engine_platform::InputEvent;
        use winit::event::{ElementState, MouseScrollDelta, WindowEvent};

        match event {
            WindowEvent::CloseRequested => return true,
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(key) = convert_winit_key_static(event.physical_key) {
                    match event.state {
                        ElementState::Pressed => {
                            self.input.apply_event(InputEvent::KeyDown(key));
                        }
                        ElementState::Released => {
                            self.input.apply_event(InputEvent::KeyUp(key));
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(btn) = convert_winit_mouse_button_static(*button) {
                    match state {
                        ElementState::Pressed => {
                            if btn == engine_platform::MouseButton::Left
                                && let Some(position) = self.input.cursor_position()
                            {
                                self.pointer_pressed.push(position);
                            }
                            self.input.apply_event(InputEvent::MouseButtonDown(btn));
                        }
                        ElementState::Released => {
                            if btn == engine_platform::MouseButton::Left
                                && let Some(position) = self.input.cursor_position()
                            {
                                self.pointer_released.push(position);
                            }
                            self.input.apply_event(InputEvent::MouseButtonUp(btn));
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.apply_event(InputEvent::MouseMove {
                    x: position.x as f32,
                    y: position.y as f32,
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(position) => {
                        (position.x as f32, position.y as f32)
                    }
                };
                self.input.apply_event(InputEvent::MouseWheel { x, y });
            }
            WindowEvent::Touch(touch) => {
                use engine_platform::MouseButton;
                use winit::event::TouchPhase;

                let position = (touch.location.x as f32, touch.location.y as f32);
                self.input.apply_event(InputEvent::MouseMove {
                    x: position.0,
                    y: position.1,
                });
                match touch.phase {
                    TouchPhase::Started => {
                        self.pointer_pressed.push(position);
                        self.input
                            .apply_event(InputEvent::MouseButtonDown(MouseButton::Left));
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        self.pointer_released.push(position);
                        self.input
                            .apply_event(InputEvent::MouseButtonUp(MouseButton::Left));
                    }
                    TouchPhase::Moved => {}
                }
            }
            _ => {}
        }
        false
    }

    /// Processes a winit device event for relative input such as raw mouse motion.
    #[cfg(feature = "runtime-game")]
    pub fn process_winit_device_event(&mut self, event: &winit::event::DeviceEvent) {
        use engine_platform::InputEvent;
        use winit::event::DeviceEvent;

        if self.input_capture.mouse
            && let DeviceEvent::MouseMotion { delta } = event
        {
            self.input.apply_event(InputEvent::MouseDelta {
                x: delta.0 as f32,
                y: delta.1 as f32,
            });
        }
    }

    /// Returns the latest script-requested input capture state.
    pub fn input_capture(&self) -> RuntimeInputCapture {
        self.input_capture
    }

    fn input_for_scripts(&self) -> InputState {
        let scale_x = if self.user_preferences.invert_mouse_x {
            -1.0
        } else {
            1.0
        };
        let scale_y = if self.user_preferences.invert_mouse_y {
            -1.0
        } else {
            1.0
        };
        self.input.with_mouse_delta_scale(scale_x, scale_y)
    }

    /// Sets the project root used by runtime backends to resolve relative files.
    pub fn set_project_root(&mut self, root: impl Into<PathBuf>) {
        self.project_root = Some(root.into());
    }

    /// Sets the script workspace roots used to resolve relative script references.
    pub fn set_script_roots<I, P>(&mut self, roots: I)
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.script_roots = roots.into_iter().map(Into::into).collect();
    }

    /// Scans, imports, and binds all supported project assets for runtime use.
    #[cfg(feature = "asset-import")]
    pub fn load_project_assets(&mut self, asset_root: impl Into<PathBuf>) -> EngineResult<()> {
        let asset_root = asset_root.into();
        self.asset_database = AssetDatabase::new(&asset_root, "builtin");
        self.asset_root = Some(asset_root.clone());
        let report = scan_project_assets(&asset_root, &mut self.asset_database)?;
        for meta in report.metas {
            self.import_runtime_asset(
                &asset_root,
                meta.guid,
                &meta.source_path,
                meta.kind,
                &meta.importer,
            )?;
            if let Ok(modified) = modified_time(&asset_root.join(&meta.source_path)) {
                self.hot_reload.observe(meta.guid, modified);
            }
        }
        self.register_loaded_model_materials();
        Ok(())
    }

    /// Reimports files whose modification stamp changed and refreshes runtime handles.
    #[cfg(feature = "asset-import")]
    pub fn reload_changed_project_assets(&mut self) -> EngineResult<Vec<engine_core::AssetId>> {
        let asset_root = self
            .asset_root
            .clone()
            .ok_or_else(|| EngineError::config("project assets are not loaded"))?;
        let report = scan_project_assets(&asset_root, &mut self.asset_database)?;
        let mut changed = Vec::new();
        for meta in report.metas {
            let path = asset_root.join(&meta.source_path);
            let Ok(modified) = modified_time(&path) else {
                continue;
            };
            if self.hot_reload.observe(meta.guid, modified) {
                self.import_runtime_asset(
                    &asset_root,
                    meta.guid,
                    &meta.source_path,
                    meta.kind,
                    &meta.importer,
                )?;
                changed.push(meta.guid.as_asset_id());
            }
        }
        self.register_loaded_model_materials();
        Ok(changed)
    }

    #[cfg(feature = "asset-import")]
    fn import_runtime_asset(
        &mut self,
        asset_root: &Path,
        guid: AssetGuid,
        source_path: &Path,
        kind: ResourceKind,
        importer: &str,
    ) -> EngineResult<()> {
        let outcome = import_builtin_asset(
            asset_root,
            &mut self.asset_registry,
            ImportTask {
                guid,
                source_path: source_path.to_path_buf(),
                kind,
                importer: importer.to_string(),
            },
        )?;
        for diagnostic in outcome.diagnostics {
            self.diagnostics.push(RuntimeDiagnostic {
                source: "assets".to_string(),
                level: "warning".to_string(),
                message: diagnostic.message,
                file: diagnostic.path,
                line: None,
            });
        }
        let Some(handle) = self.asset_registry.handle_for_guid(guid) else {
            return Ok(());
        };
        let Some(cpu) = self.asset_registry.cpu_resource(handle).cloned() else {
            return Ok(());
        };
        match kind {
            ResourceKind::Texture => {
                let gpu = if let Ok(cubemap) = DecodedCubemapResource::from_bytes(&cpu.bytes) {
                    let gpu = self.renderer.upload_cubemap(
                        ImageDesc::cubemap(cubemap.face_size, ImageFormat::Rgba8Srgb),
                        &cubemap.pixels,
                    )?;
                    self.renderer
                        .register_skybox_cubemap(&format!("asset:{:032x}", guid.as_u128()), gpu);
                    gpu
                } else {
                    let texture = DecodedTextureResource::from_bytes(&cpu.bytes)
                        .map_err(EngineError::from)?;
                    self.renderer.upload_texture(
                        ImageDesc {
                            width: texture.width,
                            height: texture.height,
                            mip_levels: 1,
                            samples: 1,
                            format: ImageFormat::Rgba8Srgb,
                            usage: ImageUsage::SAMPLED.or(ImageUsage::TRANSFER_DST),
                            label: Some("project texture"),
                        },
                        &texture.pixels,
                    )?
                };
                self.texture_resources.insert(guid.as_asset_id(), gpu);
                self.asset_registry.put_gpu(
                    handle,
                    GpuResource {
                        kind,
                        backend_token: gpu.raw().slot() as u64,
                    },
                )?;
            }
            ResourceKind::Model | ResourceKind::SkinnedModel => {
                let model = ModelResource::from_bytes(&cpu.bytes).map_err(EngineError::from)?;
                // Upload each mesh sub-object to the GPU
                for (i, mesh) in model.meshes.iter().enumerate() {
                    let mesh_name = if model.meshes.len() == 1 {
                        format!("asset:{:032x}", guid.as_u128())
                    } else {
                        format!("asset:{:032x}:{}", guid.as_u128(), i)
                    };
                    self.renderer.upload_mesh_data(
                        &mesh_name,
                        &mesh.positions,
                        &mesh.normals,
                        &mesh.texcoords,
                        &mesh.indices,
                    )?;
                }
                // Register PBR materials from the model so the renderer can
                // look up parameters by material name at render time.
                for (mat_idx, material) in model.materials.iter().enumerate() {
                    let material_name = format!("material:{:032x}:{}", guid.as_u128(), mat_idx);
                    self.renderer.register_material_params(
                        &material_name,
                        material.base_color,
                        material.metallic,
                        material.roughness,
                        material.emissive,
                    );
                    let textures = self.material_textures_from_refs(source_path, material);
                    self.renderer
                        .register_material_textures(&material_name, &textures);
                }
                self.mesh_resources.insert(guid.as_asset_id(), model);
            }
            ResourceKind::Material => {
                let text = std::str::from_utf8(&cpu.bytes)
                    .map_err(|error| EngineError::other(error.to_string()))?;
                let material = match importer {
                    "material-toml" => MaterialFormat::from_toml(text),
                    "vasset" => MaterialFormat::from_vasset(text),
                    _ => MaterialFormat::from_json(text),
                }
                .map_err(EngineError::from)?;
                let material_name = format!("asset:{:032x}", guid.as_u128());
                let metallic = material.parameters.get("metallic").copied().unwrap_or(0.0);
                let roughness = material.parameters.get("roughness").copied().unwrap_or(0.5);
                self.renderer.register_material_params(
                    &material_name,
                    [1.0, 1.0, 1.0, 1.0],
                    metallic,
                    roughness,
                    [0.0, 0.0, 0.0],
                );
                self.material_resources.insert(guid.as_asset_id(), material);
            }
            ResourceKind::Audio => {
                #[cfg(feature = "audio")]
                {
                    self.load_audio_clip(
                        guid.as_asset_id(),
                        &source_path.to_string_lossy(),
                        &cpu.bytes,
                    )?;
                }
            }
            ResourceKind::Shader
            | ResourceKind::Animation
            | ResourceKind::Script
            | ResourceKind::Prefab
            | ResourceKind::Scene => {}
        }
        Ok(())
    }

    #[cfg(feature = "asset-import")]
    fn material_textures_from_refs(
        &self,
        model_source_path: &Path,
        material: &engine_assets::CpuMaterialResource,
    ) -> RenderMaterialTextures {
        RenderMaterialTextures {
            base_color: self.texture_handle_for_material_ref(
                model_source_path,
                material.base_color_texture_ref.as_deref(),
            ),
            normal: self.texture_handle_for_material_ref(
                model_source_path,
                material.normal_texture_ref.as_deref(),
            ),
            metallic_roughness: self.texture_handle_for_material_ref(
                model_source_path,
                material.metallic_roughness_texture_ref.as_deref(),
            ),
            emissive: None,
            occlusion: None,
        }
    }

    #[cfg(feature = "asset-import")]
    fn texture_handle_for_material_ref(
        &self,
        model_source_path: &Path,
        texture_ref: Option<&str>,
    ) -> Option<ImageHandle> {
        let relative = resolve_model_texture_ref(model_source_path, texture_ref?);
        let guid = self.asset_database.get_guid_for_path(&relative)?;
        self.texture_resources.get(&guid.as_asset_id()).copied()
    }

    #[cfg(feature = "asset-import")]
    fn register_loaded_model_materials(&mut self) {
        let models: Vec<(engine_core::AssetId, PathBuf, ModelResource)> = self
            .mesh_resources
            .iter()
            .filter_map(|(asset_id, model)| {
                let guid = AssetGuid::from_asset_id(*asset_id);
                let path = self
                    .asset_database
                    .resolve_guid(guid)
                    .ok()?
                    .as_path()
                    .to_path_buf();
                Some((*asset_id, path, model.clone()))
            })
            .collect();

        for (asset_id, source_path, model) in models {
            for (mat_idx, material) in model.materials.iter().enumerate() {
                let material_name = format!("material:{:032x}:{mat_idx}", asset_id.as_u128());
                self.renderer.register_material_params(
                    &material_name,
                    material.base_color,
                    material.metallic,
                    material.roughness,
                    material.emissive,
                );
                let textures = self.material_textures_from_refs(&source_path, material);
                self.renderer
                    .register_material_textures(&material_name, &textures);
            }
        }
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
        #[cfg(feature = "physics")]
        if let Some(binding) = self.physics_binding_for_entity(player).cloned() {
            let translation = engine_core::math::Vec3::new(move_x * 0.08, 0.0, move_z * 0.08);
            if self
                .physics
                .backend_mut()
                .move_character(
                    binding.body,
                    CharacterControllerDesc {
                        shape: ColliderShapeRef::Capsule {
                            half_height: 0.5,
                            radius: 0.25,
                        },
                        translation,
                        dt: self.stats.frame_time_seconds.max(1.0 / 60.0),
                        filter: QueryFilter::default(),
                    },
                )
                .is_ok()
            {
                if let Ok(world_transform) = self.physics.backend().body_transform(binding.body) {
                    self.scene
                        .transforms_mut()
                        .set_world(player, world_transform);
                    return;
                }
            }
        }
        if let Some(mut transform) = self.scene.transforms().local(player) {
            let speed = 0.08;
            transform.translation.x += move_x * speed;
            transform.translation.z += move_z * speed;
            self.scene.transforms_mut().set_local(player, transform);
        }
    }

    #[cfg(feature = "physics")]
    fn physics_binding_for_entity(&self, entity: engine_ecs::Entity) -> Option<&PhysicsBinding> {
        let object = self.scene.object(entity)?;
        self.physics_bindings
            .iter()
            .find(|binding| binding.object == object.id)
    }

    #[cfg(feature = "physics")]
    fn ensure_physics_bindings(&mut self) -> EngineResult<()> {
        for (entity, object) in self.scene.iter_objects() {
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
            let world_transform = self.scene.transforms().world(entity).unwrap_or_default();
            let desc = RigidbodyDesc {
                transform: world_transform,
                kind: match rigidbody.body_type.as_str() {
                    "static" => BodyKind::Static,
                    "kinematic" => BodyKind::Kinematic,
                    _ => BodyKind::Dynamic,
                },
                gravity_scale: if rigidbody.use_gravity { 1.0 } else { 0.0 },
                ..RigidbodyDesc::default()
            };
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
                last_position: world_transform.translation,
                last_rotation: world_transform.rotation,
            });
        }
        Ok(())
    }

    #[cfg(feature = "physics")]
    fn sync_scene_to_physics(&mut self) -> EngineResult<()> {
        for binding in &self.physics_bindings {
            if let Some(entity) = self.scene.find_by_id(binding.object) {
                let transform = self.scene.transforms().world(entity).unwrap_or_default();
                self.physics
                    .backend_mut()
                    .set_body_transform(binding.body, transform)?;
            }
        }
        Ok(())
    }

    #[cfg(feature = "physics")]
    fn apply_environment_forces(&mut self, dt: f32) -> EngineResult<()> {
        if dt <= f32::EPSILON {
            return Ok(());
        }

        let time_seconds = self.time.total_time;
        let mut fluid_volumes = Vec::new();
        let mut wind_zones = Vec::new();
        for (entity, object) in self.scene.iter_objects() {
            let transform = self.scene.transforms().world(entity).unwrap_or_default();
            for component in &object.components {
                match component {
                    ComponentData::FluidVolume(fluid) => {
                        fluid_volumes.push(FluidVolumeSample::new(
                            fluid_volume_desc_from_scene(fluid),
                            transform,
                        ));
                    }
                    ComponentData::WindZone(wind) => {
                        let half_extents = (wind.size * transform.scale) * 0.5;
                        let min = transform.translation - half_extents;
                        let max = transform.translation + half_extents;
                        wind_zones.push((wind.clone(), min, max));
                    }
                    _ => {}
                }
            }
        }

        if fluid_volumes.is_empty() && wind_zones.is_empty() {
            return Ok(());
        }

        let gravity = 9.81;
        for binding in &mut self.physics_bindings {
            let Some(entity) = self.scene.find_by_id(binding.object) else {
                continue;
            };
            let Some(object) = self.scene.object(entity) else {
                continue;
            };
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
            if rigidbody.body_type != "dynamic" {
                continue;
            }

            let transform = self.scene.transforms().world(entity).unwrap_or_default();
            let collider = object
                .components
                .iter()
                .find_map(|component| match component {
                    ComponentData::Collider(collider) => Some(collider),
                    _ => None,
                });
            let probe_set = object
                .components
                .iter()
                .find_map(|component| match component {
                    ComponentData::BuoyancyProbeSet(probe_set) => Some(probe_set),
                    _ => None,
                });
            let collider_size = collider.map_or(Vec3::ONE, |collider| collider.size);
            let body_half = (collider_size * transform.scale) * 0.5;
            let body_min = transform.translation - body_half;
            let body_max = transform.translation + body_half;
            let velocity = (transform.translation - binding.last_position) / dt;
            let collider_volume = collider
                .map(|collider| {
                    collider_displacement_volume(&collider_shape_from_scene(
                        collider,
                        transform.scale,
                    ))
                })
                .unwrap_or(1.0)
                .max(0.001);

            for fluid in &fluid_volumes {
                let fluid_force = if let Some(probe_set) = probe_set {
                    let previous_transform = engine_core::math::Transform {
                        translation: binding.last_position,
                        rotation: binding.last_rotation,
                        scale: transform.scale,
                    };
                    solve_probe_buoyancy(
                        fluid,
                        &buoyancy_probe_set_from_scene(probe_set),
                        transform,
                        previous_transform,
                        rigidbody.mass.max(0.001),
                        gravity,
                        dt,
                        time_seconds,
                    )
                } else {
                    solve_volume_buoyancy(
                        fluid,
                        BuoyancyBodySample {
                            min: body_min,
                            max: body_max,
                            velocity,
                            collider_volume,
                            mass: rigidbody.mass.max(0.001),
                        },
                        gravity,
                        time_seconds,
                    )
                };

                apply_fluid_force(self.physics.backend_mut(), binding.body, fluid_force)?;
            }

            for (wind, zone_min, zone_max) in &wind_zones {
                if transform.translation.x < zone_min.x
                    || transform.translation.x > zone_max.x
                    || transform.translation.y < zone_min.y
                    || transform.translation.y > zone_max.y
                    || transform.translation.z < zone_min.z
                    || transform.translation.z > zone_max.z
                {
                    continue;
                }

                let mass = rigidbody.mass.max(0.001);
                let relative_velocity = velocity - wind.wind_velocity;
                let force = relative_velocity
                    * (-wind.linear_drag.max(0.0) * wind.strength.max(0.0) * mass);
                if force.length_squared() > f32::EPSILON {
                    self.physics
                        .backend_mut()
                        .apply_force(binding.body, force)?;
                }
            }
        }

        Ok(())
    }

    #[cfg(feature = "physics")]
    fn sync_physics_to_scene(&mut self) -> EngineResult<()> {
        for binding in &mut self.physics_bindings {
            if let Some(entity) = self.scene.find_by_id(binding.object) {
                let transform = self.physics.backend().body_transform(binding.body)?;
                self.scene.transforms_mut().set_world(entity, transform);
                binding.last_position = transform.translation;
                binding.last_rotation = transform.rotation;
            }
        }
        Ok(())
    }

    #[cfg(feature = "physics")]
    fn report_physics_events(&mut self) {
        for event in self.physics.drain_contacts() {
            self.diagnostics.push(RuntimeDiagnostic {
                source: "physics".to_string(),
                level: "info".to_string(),
                message: format!(
                    "{} {} between body {} and body {}",
                    if event.is_trigger {
                        "trigger"
                    } else {
                        "collision"
                    },
                    if event.entered { "entered" } else { "exited" },
                    event.body_a.0,
                    event.body_b.0
                ),
                file: None,
                line: None,
            });
        }
    }

    #[cfg(feature = "audio")]
    /// Loads an encoded WAV/OGG asset for scene AudioSource components.
    pub fn load_audio_clip(
        &mut self,
        asset: engine_core::AssetId,
        name: &str,
        bytes: &[u8],
    ) -> EngineResult<ClipHandle> {
        const STREAMING_THRESHOLD_BYTES: usize = 1024 * 1024;
        let clip = if bytes.len() >= STREAMING_THRESHOLD_BYTES {
            self.audio
                .backend_mut()
                .load_streamed_clip(name, std::sync::Arc::from(bytes))?
        } else {
            let (samples, channels, sample_rate) = engine_audio::decode_audio_bytes(name, bytes)?;
            self.audio
                .backend_mut()
                .load_clip(name, &samples, channels, sample_rate)?
        };
        self.audio_clips.insert(asset, clip);
        Ok(clip)
    }
    #[cfg(feature = "audio")]
    fn ensure_audio_bindings(&mut self) -> EngineResult<()> {
        for (entity, object) in self.scene.iter_objects() {
            if self
                .audio_bindings
                .iter()
                .any(|binding| binding.object == object.id)
            {
                continue;
            }
            let Some(audio_source) =
                object
                    .components
                    .iter()
                    .find_map(|component| match component {
                        ComponentData::AudioSource(source) => Some(source),
                        _ => None,
                    })
            else {
                continue;
            };
            let Some(asset) = audio_source.clip else {
                continue;
            };
            let Some(clip) = self.audio_clips.get(&asset).copied() else {
                let key = format!("audio:{}", asset.as_u128());
                if self.reported_script_errors.insert(key) {
                    self.diagnostics.push(RuntimeDiagnostic {
                        source: "audio".to_string(),
                        level: "warning".to_string(),
                        message: format!("audio clip {} is not loaded", asset.as_u128()),
                        file: None,
                        line: None,
                    });
                }
                continue;
            };
            let transform = self.scene.transforms().world(entity).unwrap_or_default();
            let source = self.audio.backend_mut().spawn_source(&AudioSourceDesc {
                clip,
                volume: audio_source.volume,
                pitch: audio_source.pitch,
                looping: audio_source.looping,
                position: Some(transform.translation),
                auto_play: audio_source.play_on_start,
                bus: audio_source.bus.clone(),
                spatial_mode: parse_spatial_mode(&audio_source.spatial_mode),
                shape: parse_audio_source_shape(audio_source),
                attenuation: parse_attenuation_model(audio_source),
                priority: audio_source.priority,
                virtualization: parse_virtualization_policy(&audio_source.virtualization),
                category: parse_voice_category(&audio_source.category),
                critical: audio_source.critical,
                doppler_scale: audio_source.doppler_scale,
                spread: audio_source.spread,
                use_hrtf: audio_source.use_hrtf,
            })?;
            self.audio_bindings.push(AudioBinding {
                object: object.id,
                source,
                last_position: transform.translation,
            });
        }
        Ok(())
    }

    #[cfg(feature = "audio")]
    fn update_audio(&mut self, dt: f32) -> EngineResult<()> {
        let mut index = 0;
        while index < self.audio_bindings.len() {
            if self
                .scene
                .find_by_id(self.audio_bindings[index].object)
                .is_none()
            {
                let binding = self.audio_bindings.swap_remove(index);
                self.audio.backend_mut().destroy_source(binding.source)?;
            } else {
                index += 1;
            }
        }
        let listener_entity = self
            .scene
            .iter_objects()
            .find(|(_, object)| {
                object
                    .components
                    .iter()
                    .any(|component| matches!(component, ComponentData::AudioListener(_)))
            })
            .map(|(entity, _)| entity)
            .or_else(|| {
                self.scene
                    .main_camera()
                    .or_else(|| self.scene.game_camera())
            });

        if let Some(listener_entity) = listener_entity {
            let transform = self
                .scene
                .transforms()
                .world(listener_entity)
                .unwrap_or_default();
            let listener_data = self
                .scene
                .object(listener_entity)
                .and_then(|object| {
                    object
                        .components
                        .iter()
                        .find_map(|component| match component {
                            ComponentData::AudioListener(listener) => Some(listener.clone()),
                            _ => None,
                        })
                })
                .unwrap_or_default();
            let velocity = self
                .audio_listener_position
                .filter(|_| dt > f32::EPSILON)
                .map(|previous| (transform.translation - previous) / dt)
                .unwrap_or(engine_core::math::Vec3::ZERO);
            let output_mode = parse_output_mode(&listener_data.output_mode);
            let hrtf_quality = parse_hrtf_quality(&listener_data.hrtf_quality);
            let _ = self.audio.backend_mut().set_output_mode(output_mode);
            let _ = self.audio.backend_mut().set_hrtf_quality(hrtf_quality);
            self.audio.backend_mut().set_listener(&AudioListenerDesc {
                position: transform.translation,
                forward: transform
                    .rotation
                    .rotate(engine_core::math::Vec3::new(0.0, 0.0, -1.0))
                    .normalized(),
                up: transform
                    .rotation
                    .rotate(engine_core::math::Vec3::new(0.0, 1.0, 0.0))
                    .normalized(),
                velocity,
                output_mode,
                hrtf_quality,
                hrtf_enabled: listener_data.hrtf_enabled,
            });
            self.audio_listener_position = Some(transform.translation);
        }
        let mut acoustic_sources = Vec::with_capacity(self.audio_bindings.len());
        for binding in &mut self.audio_bindings {
            if let Some(entity) = self.scene.find_by_id(binding.object) {
                let transform = self.scene.transforms().world(entity).unwrap_or_default();
                let Some(object) = self.scene.object(entity) else {
                    continue;
                };
                if let Some(audio_source) =
                    object
                        .components
                        .iter()
                        .find_map(|component| match component {
                            ComponentData::AudioSource(source) => Some(source),
                            _ => None,
                        })
                {
                    self.audio
                        .backend_mut()
                        .set_volume(binding.source, audio_source.volume)?;
                    self.audio
                        .backend_mut()
                        .set_looping(binding.source, audio_source.looping)?;
                    let velocity = if dt > f32::EPSILON {
                        (transform.translation - binding.last_position) / dt
                    } else {
                        engine_core::math::Vec3::ZERO
                    };
                    self.audio.backend_mut().set_source_transform(
                        binding.source,
                        AudioObjectTransform {
                            position: transform.translation,
                            forward: transform
                                .rotation
                                .rotate(engine_core::math::Vec3::new(0.0, 0.0, -1.0))
                                .normalized(),
                            velocity,
                        },
                    )?;
                    acoustic_sources.push(AcousticSourceSample {
                        handle: binding.source,
                        position: transform.translation,
                    });
                    binding.last_position = transform.translation;
                }
            }
        }
        if let Some(listener_position) = self.audio_listener_position {
            let snapshot = AcousticSceneSnapshot {
                listener_position,
                sources: acoustic_sources,
                blockers: extract_acoustic_blockers(&self.scene),
            };
            for (source, propagation) in
                solve_direct_propagation(&snapshot, AcousticSolverConfig::default())
            {
                let _ = self
                    .audio
                    .backend_mut()
                    .set_source_propagation(source, propagation);
            }
        }
        self.audio.update(dt);
        self.cleanup_finished_transient_audio();
        let diagnostics = self.audio.diagnostics();
        self.stats.audio_sources = diagnostics.logical_sources;
        self.stats.audio_physical_voices = diagnostics.physical_voices;
        self.stats.audio_virtual_voices = diagnostics.virtual_voices;
        self.stats.audio_underruns = diagnostics.underruns;
        Ok(())
    }

    #[cfg(feature = "audio")]
    fn cleanup_finished_transient_audio(&mut self) {
        let mut index = 0;
        while index < self.transient_audio.len() {
            let (source, _) = self.transient_audio[index];
            let finished = self
                .audio
                .backend()
                .playback_state(source)
                .is_ok_and(|state| state != engine_audio::PlaybackState::Playing);
            if finished {
                let (source, clip) = self.transient_audio.swap_remove(index);
                let _ = self.audio.backend_mut().destroy_source(source);
                let _ = self.audio.backend_mut().unload_clip(clip);
            } else {
                index += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_min_ticks_one_frame() {
        assert_eq!(smoke_runtime_min().unwrap(), 1);
    }

    #[test]
    fn run_frame_60_frames_headless() {
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        let delta = Duration::from_secs_f32(1.0 / 60.0);
        for _ in 0..60 {
            services.run_frame(delta, false).unwrap();
        }
        assert_eq!(services.frame_index(), 60);
        assert_eq!(services.time.frame_index, 60);
        assert!(
            services.time.total_time > 0.0,
            "total_time should accumulate across frames"
        );
    }

    #[test]
    fn run_frame_input_reset_at_begin_frame() {
        use engine_platform::{InputEvent, KeyCode};
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services
            .input
            .apply_event(InputEvent::KeyDown(KeyCode::Space));
        assert!(
            services.input.key_pressed(KeyCode::Space),
            "key should be pressed before frame"
        );
        services
            .run_frame(Duration::from_millis(16), false)
            .unwrap();
        assert!(
            !services.input.key_pressed(KeyCode::Space),
            "pressed state should be cleared by begin_frame"
        );
        assert!(
            services.input.key_down(KeyCode::Space),
            "held state should persist after begin_frame"
        );
    }

    #[cfg(feature = "physics")]
    #[test]
    fn runtime_physics_diagnostics_respect_contact_filter_chain() {
        use engine_physics::{ColliderDesc, ContactFilter, RigidbodyDesc, Transform, Vec3};

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        let ignored = services
            .physics
            .backend_mut()
            .create_body(&RigidbodyDesc::default())
            .unwrap();
        let other = services
            .physics
            .backend_mut()
            .create_body(&RigidbodyDesc {
                transform: Transform {
                    translation: Vec3::new(0.25, 0.0, 0.0),
                    ..Transform::IDENTITY
                },
                ..RigidbodyDesc::default()
            })
            .unwrap();
        services
            .physics
            .backend_mut()
            .add_collider(ignored, &ColliderDesc::default())
            .unwrap();
        services
            .physics
            .backend_mut()
            .add_collider(other, &ColliderDesc::default())
            .unwrap();
        services
            .physics
            .contact_filter_chain
            .push(ContactFilter::IgnoreBody { body: ignored });

        services.physics.fixed_update(0.0);
        services.report_physics_events();

        assert!(services.diagnostics.is_empty());
    }

    #[cfg(feature = "physics")]
    #[test]
    fn fluid_collider_displacement_volume_matches_basic_shapes() {
        let box_volume = collider_displacement_volume(&collider_shape_from_scene(
            &ColliderComponentData {
                shape: "box".to_string(),
                size: Vec3::new(2.0, 3.0, 4.0),
                ..ColliderComponentData::default()
            },
            Vec3::ONE,
        ));
        assert!((box_volume - 24.0).abs() < 1e-5);

        let sphere_volume = collider_displacement_volume(&collider_shape_from_scene(
            &ColliderComponentData {
                shape: "sphere".to_string(),
                size: Vec3::new(2.0, 2.0, 2.0),
                ..ColliderComponentData::default()
            },
            Vec3::ONE,
        ));
        assert!((sphere_volume - ((4.0 / 3.0) * std::f32::consts::PI)).abs() < 1e-5);

        let capsule_volume = collider_displacement_volume(&collider_shape_from_scene(
            &ColliderComponentData {
                shape: "capsule".to_string(),
                size: Vec3::new(2.0, 2.0, 2.0),
                ..ColliderComponentData::default()
            },
            Vec3::ONE,
        ));
        let expected_capsule = std::f32::consts::PI * 2.0 + (4.0 / 3.0) * std::f32::consts::PI;
        assert!((capsule_volume - expected_capsule).abs() < 1e-5);
    }

    #[test]
    fn model_material_resolution_keeps_explicit_materials() {
        let model_guid = engine_core::AssetId::from_u128(0x1234);
        let explicit_guid = engine_core::AssetId::from_u128(0x5678);
        let mut mesh_resources = HashMap::new();
        mesh_resources.insert(
            model_guid,
            ModelResource {
                materials: vec![engine_assets::CpuMaterialResource::default()],
                ..ModelResource::default()
            },
        );
        let model_mesh = format!("asset:{:032x}", model_guid.as_u128());
        let mut world = RenderWorld {
            objects: vec![
                engine_render::RenderObject {
                    object: engine_core::EntityId::from_u128(1),
                    transform: engine_core::math::Transform::IDENTITY,
                    mesh: model_mesh.clone(),
                    material: format!("asset:{:032x}", explicit_guid.as_u128()),
                    casts_shadows: true,
                    receive_shadows: true,
                    bounds: engine_render::RenderBounds::default(),
                    lods: Vec::new(),
                },
                engine_render::RenderObject {
                    object: engine_core::EntityId::from_u128(2),
                    transform: engine_core::math::Transform::IDENTITY,
                    mesh: model_mesh,
                    material: "debug/default".to_string(),
                    casts_shadows: true,
                    receive_shadows: true,
                    bounds: engine_render::RenderBounds::default(),
                    lods: Vec::new(),
                },
            ],
            ..RenderWorld::default()
        };

        RuntimeServices::<HeadlessRenderDevice>::resolve_render_materials(
            &mut world,
            &mesh_resources,
        );

        assert_eq!(
            world.objects[0].material,
            format!("asset:{:032x}", explicit_guid.as_u128())
        );
        assert_eq!(
            world.objects[1].material,
            format!("material:{:032x}:0", model_guid.as_u128())
        );
    }

    #[test]
    fn model_material_resolution_uses_mesh_primitive_material_index() {
        let model_guid = engine_core::AssetId::from_u128(0x1234);
        let mut mesh_resources = HashMap::new();
        mesh_resources.insert(
            model_guid,
            ModelResource {
                meshes: vec![
                    engine_assets::BasicMeshResource {
                        positions: Vec::new(),
                        normals: Vec::new(),
                        texcoords: Vec::new(),
                        indices: Vec::new(),
                        material_index: Some(2),
                    },
                    engine_assets::BasicMeshResource {
                        positions: Vec::new(),
                        normals: Vec::new(),
                        texcoords: Vec::new(),
                        indices: Vec::new(),
                        material_index: Some(1),
                    },
                ],
                materials: vec![
                    engine_assets::CpuMaterialResource::default(),
                    engine_assets::CpuMaterialResource::default(),
                    engine_assets::CpuMaterialResource::default(),
                ],
            },
        );
        let mut world = RenderWorld {
            objects: vec![engine_render::RenderObject {
                object: engine_core::EntityId::from_u128(1),
                transform: engine_core::math::Transform::IDENTITY,
                mesh: format!("asset:{:032x}:1", model_guid.as_u128()),
                material: String::new(),
                casts_shadows: true,
                receive_shadows: true,
                bounds: engine_render::RenderBounds::default(),
                lods: Vec::new(),
            }],
            ..RenderWorld::default()
        };

        RuntimeServices::<HeadlessRenderDevice>::resolve_render_materials(
            &mut world,
            &mesh_resources,
        );

        assert_eq!(
            world.objects[0].material,
            format!("material:{:032x}:1", model_guid.as_u128())
        );
    }

    #[cfg(feature = "asset-import")]
    #[test]
    fn model_texture_refs_resolve_relative_to_model_source() {
        assert_eq!(
            resolve_model_texture_ref(Path::new("models/ship/ship.gltf"), "textures/albedo.png"),
            PathBuf::from("models/ship/textures/albedo.png")
        );
        assert_eq!(
            resolve_model_texture_ref(Path::new("models/ship/ship.gltf"), "../shared/normal.png"),
            PathBuf::from("models/shared/normal.png")
        );
    }

    #[test]
    fn default_render_graph_has_expected_passes() {
        let graph = build_default_render_graph();
        assert_eq!(graph.pass_count(), 6);
        assert_eq!(graph.passes[0].name, "shadow");
        assert_eq!(graph.passes[1].name, "forward");
        assert_eq!(graph.passes[2].name, "temporal-inputs");
        assert_eq!(graph.passes[3].name, "upscale");
        assert_eq!(graph.passes[4].name, "post");
        assert_eq!(graph.passes[5].name, "ui");
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
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples/project/fps_arena"),
        )
        .unwrap();
        let render_world = extract_render_world(&project.scene);

        assert!(project.scene.find_by_name("Player").is_some());
        assert!(render_world.is_visible());
    }

    #[test]
    fn game_frame_updates_stats_and_script_diagnostics() {
        let project = load_runtime_project(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples/project/fps_arena"),
        )
        .unwrap();
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.scene = project.scene;

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert!(services.stats.entity_count >= 2);
        assert!(services.stats.draw_calls >= 1);
        assert!(!services.diagnostics.iter().any(|diagnostic| {
            diagnostic.source == "script" && diagnostic.message.contains("pending backend recovery")
        }));
    }

    #[test]
    fn script_ui_commands_build_gui_draw_list() {
        let mut gui_textures = HashMap::new();
        gui_textures.insert("test:slot".to_string(), GuiTextureId(7));
        let draw_list = build_script_ui_draw_list(
            &[
                VargUiCommand::Rect {
                    id: "panel".to_string(),
                    x: 8.0,
                    y: 10.0,
                    width: 24.0,
                    height: 12.0,
                    color: [0.25, 0.5, 0.75, 1.0],
                },
                VargUiCommand::Label {
                    id: "label".to_string(),
                    text: "Hi!".to_string(),
                    x: 40.0,
                    y: 12.0,
                },
                VargUiCommand::Texture {
                    id: "slot".to_string(),
                    texture: "test:slot".to_string(),
                    x: 64.0,
                    y: 18.0,
                    width: 16.0,
                    height: 16.0,
                    color: [1.0, 1.0, 1.0, 0.75],
                },
            ],
            &gui_textures,
        );

        assert!(draw_list.vertices.len() > 16);
        assert_eq!(draw_list.vertices.len() % 4, 0);
        assert_eq!(draw_list.indices.len(), draw_list.commands.len() * 6);
        assert!(draw_list.commands.len() > 4);
        assert_eq!(draw_list.commands[0].scissor, [8, 10, 24, 12]);
        assert_eq!(draw_list.commands[0].index_offset, 0);
        assert_eq!(draw_list.commands[0].index_count, 6);
        assert_eq!(draw_list.vertices[0].color, 0xffbf8040);
        assert_eq!(draw_list.vertices[4].pos, [40.0, 12.0]);
        assert_eq!(draw_list.vertices[4].color, 0xffffffff);
        assert_eq!(draw_list.commands[1].scissor, [40, 12, 2, 2]);
        assert_eq!(draw_list.commands.last().unwrap().texture, GuiTextureId(7));
        assert_eq!(draw_list.vertices.last().unwrap().color, 0xbfffffff);
    }

    #[test]
    fn varg_script_start_and_update_can_move_transform() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("move.varg"),
            r#"script Move {
    @export var speed: Float = 2.0
    var started: Int = 0

    func start() {
        entity.translate(Vec3(1.0, 0.0, 0.0))
        state.started = 1
    }

    func update(_ dt: Float) {
        entity.translate(Vec3(0.0, 0.0, Input.actionValue("MoveY") * speed))
        state.ticks += 1
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Scripted").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/move.varg")),
            )
            .unwrap();
        services
            .input
            .apply_event(engine_platform::InputEvent::KeyDown(
                engine_platform::KeyCode::Character('w'),
            ));

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        let transform = services.scene.transforms().local(entity).unwrap();
        assert_eq!(transform.translation.x, 1.0);
        assert_eq!(transform.translation.z, 2.0);
        let object = services.scene.object(entity).unwrap();
        let script = object
            .components
            .iter()
            .find_map(|component| match component {
                ComponentData::Script(script) => Some(script),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            script.state.get("started").and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            script.state.get("ticks").and_then(|value| value.as_f64()),
            Some(1.0)
        );
    }

    #[test]
    fn varg_script_without_start_hook_does_not_record_started_state() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("tick.varg"),
            r#"script TickOnly {
    var ticks: Int = 0

    func update(_ dt: Float) {
        state.ticks += 1
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Scripted").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/tick.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        let object = services.scene.object(entity).unwrap();
        let script = object
            .components
            .iter()
            .find_map(|component| match component {
                ComponentData::Script(script) => Some(script),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            script.state.get("ticks").and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert!(!script.state.contains_key("__varg_started"));
    }

    #[test]
    fn varg_script_ui_commands_are_collected_per_frame() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("hud.varg"),
            r#"script Hud {
    func update(_ dt: Float) {
        ui.rect("panel", 8.0, 8.0, 160.0, 40.0, 0.0, 0.0, 0.0, 0.7)
        ui.label("score", "Score: 10", 16.0, 20.0)
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Hud").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/hud.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert_eq!(
            services.ui_commands,
            vec![
                VargUiCommand::Rect {
                    id: "panel".to_string(),
                    x: 8.0,
                    y: 8.0,
                    width: 160.0,
                    height: 40.0,
                    color: [0.0, 0.0, 0.0, 0.7],
                },
                VargUiCommand::Label {
                    id: "score".to_string(),
                    text: "Score: 10".to_string(),
                    x: 16.0,
                    y: 20.0,
                },
            ]
        );

        services.paused = true;
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();
        assert!(services.ui_commands.is_empty());
    }

    #[test]
    fn varg_script_ui_buttons_receive_runtime_pointer_releases() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("menu.varg"),
            r#"script Menu {
    var clicked: Int = 0

    func update(_ dt: Float) {
        if ui.button("continue", "Continue", 100.0, 80.0, 200.0, 48.0) {
            state.clicked = 1
        }
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Menu").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/menu.varg")),
            )
            .unwrap();
        services.pointer_released.push((140.0, 100.0));

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        let object = services.scene.object(entity).unwrap();
        let script = object
            .components
            .iter()
            .find_map(|component| match component {
                ComponentData::Script(script) => Some(script),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            script.state.get("clicked").and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(services.ui_commands.len(), 2);
        assert!(services.pointer_released.is_empty());
    }

    #[test]
    fn varg_script_can_drive_runtime_global_illumination() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("gi.varg"),
            r#"script GiController {
    func update(_ dt: Float) {
        render.gi.useProbeVolume(Vec3(1.0, 2.0, 3.0), Vec3(12.0, 6.0, 10.0), Vec3(4.0, 3.0, 2.0), 1.4)
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Lighting").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/gi.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert_eq!(
            services.render_world.global_illumination,
            RenderGlobalIllumination::ProbeVolume(RenderProbeVolume {
                center: Vec3::new(1.0, 2.0, 3.0),
                extent: Vec3::new(12.0, 6.0, 10.0),
                counts: [4, 3, 2],
                intensity: 1.4,
            })
        );
    }

    #[test]
    fn varg_script_can_drive_screen_space_gi_intensity() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("ssgi.varg"),
            r#"script SsgiController {
    func update(_ dt: Float) {
        render.gi.useScreenSpace()
        render.gi.setIntensity(0.35)
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Lighting").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/ssgi.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert_eq!(
            services.render_world.global_illumination,
            RenderGlobalIllumination::ScreenSpace { intensity: 0.35 }
        );
    }

    #[test]
    fn varg_script_can_drive_runtime_weather_environment() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("weather.varg"),
            r#"script WeatherController {
    func update(_ dt: Float) {
        render.weather.set("storm")
        render.weather.setTimeOfDay(18.0)
        render.weather.setCloudCover(0.9)
        render.weather.setPrecipitation(0.8)
        render.weather.setWind(Vec3(4.0, 0.0, -2.0))
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Weather").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/weather.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        let weather = &services.render_environment.weather;
        assert!(weather.enabled);
        assert_eq!(weather.preset, "storm");
        assert_eq!(weather.time_of_day, 18.0);
        assert_eq!(weather.cloud_cover, 0.9);
        assert_eq!(weather.precipitation, 0.8);
        assert_eq!(weather.wind, Vec3::new(4.0, 0.0, -2.0));

        let environment = services
            .render_world
            .environment
            .as_ref()
            .expect("weather should author render environment");
        assert!(environment.sky_enabled);
        assert!(environment.fog.enabled);
        assert!(environment.fog.density > 0.001);
        assert!(environment.ssr_intensity > 0.22);
        assert!(services.render_world.active_skybox().is_some());
    }

    #[test]
    fn runtime_weather_is_inactive_by_default() {
        let mut world = RenderWorld::default();
        apply_runtime_weather_environment(&mut world, &RuntimeWeatherState::default());

        assert!(world.environment.is_none());
        assert!(world.skybox.is_none());
        assert!(world.fog.is_none());
    }

    #[test]
    fn varg_script_references_can_resolve_from_configured_script_roots() {
        let root = tempfile::tempdir().unwrap();
        let gameplay = root.path().join("packages/gameplay/src");
        std::fs::create_dir_all(&gameplay).unwrap();
        std::fs::write(
            gameplay.join("move.varg"),
            r#"script Move {
    func update(_ dt: Float) {
        entity.translate(Vec3(0.0, 1.0, 0.0))
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        services.set_script_roots([PathBuf::from("packages/gameplay/src")]);
        let entity = services.scene.create_object("Scripted").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("move.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        let transform = services.scene.transforms().local(entity).unwrap();
        assert_eq!(transform.translation.y, 1.0);
    }

    #[test]
    fn varg_script_sees_transient_input_and_late_update() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("input.varg"),
            r#"script InputProbe {
    var pressed: Int = 0
    var released: Int = 0
    var lateTicks: Int = 0

    func update(_ dt: Float) {
        if Input.justPressed("jump") {
            state.pressed += 1
        }

        if Input.justReleased("jump") {
            state.released += 1
        }
    }

    func lateUpdate(_ dt: Float) {
        state.lateTicks += 1
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Scripted").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/input.varg")),
            )
            .unwrap();

        services
            .input
            .apply_event(engine_platform::InputEvent::KeyDown(
                engine_platform::KeyCode::Space,
            ));
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        services
            .input
            .apply_event(engine_platform::InputEvent::KeyUp(
                engine_platform::KeyCode::Space,
            ));
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        let object = services.scene.object(entity).unwrap();
        let script = object
            .components
            .iter()
            .find_map(|component| match component {
                ComponentData::Script(script) => Some(script),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            script.state.get("pressed").and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            script
                .state
                .get("released")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            script
                .state
                .get("lateTicks")
                .and_then(|value| value.as_f64()),
            Some(2.0)
        );
    }

    #[test]
    fn varg_script_can_spawn_scene_objects() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("spawner.varg"),
            r#"script Spawner {
    func update(_ dt: Float) {
        scene.spawnBox("Runtime Platform", "Platform", Vec3(4.0, 0.0, 8.0), Vec3(2.0, 0.5, 2.0), "")
        scene.spawnSphere("Runtime Crystal", "Collectible", Vec3(4.0, 1.2, 8.0), 0.35, "scripts/bobber.varg")
    }
}
"#,
        )
        .unwrap();
        std::fs::write(
            scripts.join("bobber.varg"),
            r#"script Bobber {
    func update(_ dt: Float) {
        position.y += 0.0
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let spawner = services.scene.create_object("Spawner").unwrap();
        services
            .scene
            .upsert_component(
                spawner,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/spawner.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        let platform = services
            .scene
            .objects()
            .into_iter()
            .find(|(_, object)| object.name == "Runtime Platform")
            .map(|(entity, _)| entity)
            .expect("platform spawned");
        let crystal = services
            .scene
            .objects()
            .into_iter()
            .find(|(_, object)| object.name == "Runtime Crystal")
            .map(|(entity, _)| entity)
            .expect("crystal spawned");

        let platform_object = services.scene.object(platform).unwrap();
        assert_eq!(platform_object.tag, "Platform");
        assert!(platform_object.components.iter().any(|component| matches!(
            component,
            ComponentData::MeshRenderer(mesh) if mesh.builtin_mesh.as_deref() == Some("debug/cube")
        )));
        assert!(platform_object.components.iter().any(|component| matches!(
            component,
            ComponentData::Collider(collider) if collider.shape == "box"
        )));
        assert_eq!(
            services
                .scene
                .transforms()
                .local(platform)
                .unwrap()
                .translation,
            Vec3::new(4.0, 0.0, 8.0)
        );
        assert_eq!(
            services.scene.transforms().local(platform).unwrap().scale,
            Vec3::new(2.0, 0.5, 2.0)
        );

        let crystal_object = services.scene.object(crystal).unwrap();
        assert_eq!(crystal_object.tag, "Collectible");
        assert!(crystal_object.components.iter().any(|component| matches!(
            component,
            ComponentData::MeshRenderer(mesh) if mesh.builtin_mesh.as_deref() == Some("debug/sphere")
        )));
        assert!(crystal_object.components.iter().any(|component| matches!(
            component,
            ComponentData::Collider(collider) if collider.shape == "sphere"
        )));
        assert!(crystal_object.components.iter().any(|component| matches!(
            component,
            ComponentData::Script(script) if script.source == "scripts/bobber.varg"
        )));
    }

    #[test]
    fn varg_script_can_query_scene_tags_distance_and_destroy_self() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("enemy.varg"),
            r#"script Enemy {
    func update(_ dt: Float) {
        if entity.hasTag("Enemy") && playerDistance() <= 5.0 {
            state.sawPlayer = 1
        }

        if scene.distanceToTag("Hazard") < 2.0 {
            entity.destroy()
        }
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());

        let player = services.scene.create_object("Player").unwrap();
        services.scene.object_mut(player).unwrap().tag = "Player".to_string();
        let mut player_transform = engine_core::math::Transform::default();
        player_transform.translation = engine_core::math::Vec3::new(3.0, 0.0, 4.0);
        services
            .scene
            .transforms_mut()
            .set_local(player, player_transform);

        let hazard = services.scene.create_object("Spike").unwrap();
        services.scene.object_mut(hazard).unwrap().tag = "Hazard".to_string();
        let mut hazard_transform = engine_core::math::Transform::default();
        hazard_transform.translation = engine_core::math::Vec3::new(1.0, 0.0, 0.0);
        services
            .scene
            .transforms_mut()
            .set_local(hazard, hazard_transform);

        let enemy = services.scene.create_object("Enemy").unwrap();
        services.scene.object_mut(enemy).unwrap().tag = "Enemy".to_string();
        services
            .scene
            .upsert_component(
                enemy,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/enemy.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert!(
            services.scene.object(enemy).is_none(),
            "enemy should be destroyed at the frame-safe destroy point"
        );
    }

    #[test]
    fn varg_script_can_destroy_nearest_tagged_scene_object() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("collector.varg"),
            r#"script Collector {
    func update(_ dt: Float) {
        scene.destroyNearestWithTag("Collectible", 1.5)
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());

        let player = services.scene.create_object("Player").unwrap();
        services.scene.object_mut(player).unwrap().tag = "Player".to_string();
        services.scene.transforms_mut().set_local(
            player,
            Transform {
                translation: Vec3::new(0.0, 0.0, 0.0),
                ..Transform::default()
            },
        );
        services
            .scene
            .upsert_component(
                player,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/collector.varg")),
            )
            .unwrap();

        let near = services.scene.create_object("Near Crystal").unwrap();
        services.scene.object_mut(near).unwrap().tag = "Collectible".to_string();
        services.scene.transforms_mut().set_local(
            near,
            Transform {
                translation: Vec3::new(0.5, 0.0, 0.0),
                ..Transform::default()
            },
        );

        let far = services.scene.create_object("Far Crystal").unwrap();
        services.scene.object_mut(far).unwrap().tag = "Collectible".to_string();
        services.scene.transforms_mut().set_local(
            far,
            Transform {
                translation: Vec3::new(3.0, 0.0, 0.0),
                ..Transform::default()
            },
        );

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert!(
            services.scene.object(near).is_none(),
            "nearest crystal should be destroyed"
        );
        assert!(
            services.scene.object(far).is_some(),
            "far crystal should remain outside the pickup radius"
        );
        assert!(
            services.scene.object(player).is_some(),
            "request source should not destroy itself"
        );
    }

    #[cfg(feature = "audio")]
    #[test]
    fn game_frame_spawns_loaded_audio_source() {
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        let asset = engine_core::AssetId::from_u128(7);
        services
            .load_audio_clip(asset, "tone.wav", &test_wav_bytes())
            .unwrap();
        let entity = services.scene.create_object("Speaker").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::AudioSource(engine_ecs::AudioSourceComponentData {
                    clip: Some(asset),
                    volume: 0.5,
                    looping: true,
                    play_on_start: true,
                    spatial_blend: 0.0,
                    ..engine_ecs::AudioSourceComponentData::default()
                }),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert_eq!(services.audio_bindings.len(), 1);
    }

    #[cfg(feature = "audio")]
    #[test]
    fn acoustic_geometry_drives_source_propagation() {
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        let asset = engine_core::AssetId::from_u128(8);
        services
            .load_audio_clip(asset, "tone.wav", &test_wav_bytes())
            .unwrap();

        let listener = services.scene.create_object("Listener").unwrap();
        services
            .scene
            .upsert_component(listener, ComponentData::AudioListener(Default::default()))
            .unwrap();

        let speaker = services.scene.create_object("Speaker").unwrap();
        services.scene.transforms_mut().set_local(
            speaker,
            engine_core::math::Transform {
                translation: engine_core::math::Vec3::new(0.0, 0.0, -10.0),
                ..engine_core::math::Transform::IDENTITY
            },
        );
        services
            .scene
            .upsert_component(
                speaker,
                ComponentData::AudioSource(engine_ecs::AudioSourceComponentData {
                    clip: Some(asset),
                    volume: 1.0,
                    looping: true,
                    play_on_start: true,
                    spatial_mode: "object".to_string(),
                    attenuation: "none".to_string(),
                    ..engine_ecs::AudioSourceComponentData::default()
                }),
            )
            .unwrap();

        let wall = services.scene.create_object("Wall").unwrap();
        services.scene.transforms_mut().set_local(
            wall,
            engine_core::math::Transform {
                translation: engine_core::math::Vec3::new(0.0, 0.0, -5.0),
                ..engine_core::math::Transform::IDENTITY
            },
        );
        services
            .scene
            .upsert_component(
                wall,
                ComponentData::AcousticGeometry(engine_ecs::AcousticGeometryComponentData {
                    size: engine_core::math::Vec3::new(4.0, 4.0, 0.5),
                    material: Some(engine_ecs::AcousticMaterialComponentData {
                        transmission: [0.3, 0.15, 0.02],
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert_eq!(services.audio_bindings.len(), 1);
        assert_eq!(services.audio.diagnostics().acoustics_sources, 1);
    }

    #[cfg(feature = "audio")]
    #[test]
    fn varg_script_can_play_procedural_tone() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("sfx.varg"),
            r#"script Sfx {
    func update(_ dt: Float) {
        if state.played == 0 {
            Audio.playTone("square", 660.0, 0.08, 0.25)
            state.played = 1
        }
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Sfx").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/sfx.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert_eq!(services.audio.diagnostics().loaded_clips, 1);
        assert_eq!(services.audio.diagnostics().logical_sources, 1);
        assert_eq!(services.transient_audio.len(), 1);

        services
            .tick_game_frame(Duration::from_millis(100), false)
            .unwrap();

        assert_eq!(services.audio.diagnostics().loaded_clips, 0);
        assert_eq!(services.audio.diagnostics().logical_sources, 0);
        assert!(services.transient_audio.is_empty());
    }

    #[cfg(feature = "audio")]
    #[test]
    fn varg_script_can_start_and_stop_procedural_bgm_loop() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("bgm.varg"),
            r#"script Bgm {
    func start() {
        Audio.startLoop("main", "triangle", "C4 E4 G4 R", 120.0, 0.5, 0.18)
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Bgm").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/bgm.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert_eq!(services.audio.diagnostics().loaded_clips, 1);
        assert_eq!(services.audio.diagnostics().logical_sources, 1);
        assert_eq!(services.audio.diagnostics().physical_voices, 1);
        assert!(services.procedural_loops.contains_key("main"));

        services
            .apply_varg_audio_command(VargAudioCommand::StopLoop {
                id: "main".to_string(),
            })
            .unwrap();

        assert_eq!(services.audio.diagnostics().loaded_clips, 0);
        assert_eq!(services.audio.diagnostics().logical_sources, 0);
        assert!(services.procedural_loops.is_empty());
    }

    #[cfg(feature = "physics")]
    #[test]
    fn game_frame_creates_physics_bindings_for_scene_components() {
        let project = load_runtime_project(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples/project/fps_arena"),
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

    #[cfg(feature = "audio")]
    fn test_wav_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&44_100u32.to_le_bytes());
        bytes.extend_from_slice(&88_200u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&0i16.to_le_bytes());
        bytes.extend_from_slice(&i16::MAX.to_le_bytes());
        bytes
    }

    #[test]
    fn action_map_default_bindings_on_runtime_services() {
        let services = RuntimeServices::minimal(EngineConfig::default());
        // The default ActionMap should have MoveForward, MoveBack, MoveLeft,
        // MoveRight, Jump, Fire, Interact, Pause
        assert!(
            services.action_pressed("MoveForward") || !services.action_pressed("MoveForward"),
            "action_pressed should not panic for known action"
        );
        assert!(
            services.action_held("MoveForward") || !services.action_held("MoveForward"),
            "action_held should not panic for known action"
        );
        // Unknown actions return false
        assert!(!services.action_pressed("DoesNotExist"));
        assert!(!services.action_held("DoesNotExist"));
    }

    #[test]
    fn action_pressed_delegates_to_action_map() {
        use engine_platform::{InputEvent, KeyCode};
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services
            .input
            .apply_event(InputEvent::KeyDown(KeyCode::Space));
        assert!(
            services.action_pressed("Jump"),
            "Jump should be pressed when Space is pressed"
        );
        assert!(
            services.action_held("Jump"),
            "Jump should be held when Space is down"
        );
    }

    #[test]
    fn action_map_fire_and_interact_bindings() {
        use engine_platform::{InputEvent, KeyCode};
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services
            .input
            .apply_event(InputEvent::KeyDown(KeyCode::Character('f')));
        assert!(
            services.action_pressed("Fire"),
            "Fire should be pressed when F is pressed"
        );
        assert!(!services.action_pressed("Interact"));
    }

    #[test]
    fn action_map_pause_binding() {
        use engine_platform::{InputEvent, KeyCode};
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services
            .input
            .apply_event(InputEvent::KeyDown(KeyCode::Escape));
        assert!(
            services.action_pressed("Pause"),
            "Pause should be pressed when Escape is pressed"
        );
    }

    #[test]
    fn action_map_axis_value() {
        use engine_platform::{InputEvent, KeyCode};
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        // No input → 0.0
        assert_eq!(
            services
                .action_map
                .axis_value(&services.input, "MoveLeft", "MoveRight"),
            0.0
        );
        // Press D → MoveRight → +1.0
        services
            .input
            .apply_event(InputEvent::KeyDown(KeyCode::Character('d')));
        assert_eq!(
            services
                .action_map
                .axis_value(&services.input, "MoveLeft", "MoveRight"),
            1.0
        );
        // Press A too → both held → 0.0
        services
            .input
            .apply_event(InputEvent::KeyDown(KeyCode::Character('a')));
        assert_eq!(
            services
                .action_map
                .axis_value(&services.input, "MoveLeft", "MoveRight"),
            0.0
        );
    }

    #[cfg(feature = "runtime-game")]
    #[derive(Debug)]
    struct FixedGamepadProvider {
        states: Vec<engine_platform::GamepadState>,
    }

    #[cfg(feature = "runtime-game")]
    impl engine_platform::GamepadProvider for FixedGamepadProvider {
        fn poll_gamepads(&mut self) -> Vec<engine_platform::GamepadState> {
            self.states.clone()
        }

        fn gamepad_count(&self) -> usize {
            self.states.len()
        }
    }

    #[cfg(feature = "runtime-game")]
    #[test]
    fn runtime_game_frame_polls_gamepads() {
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        let mut gamepad = engine_platform::GamepadState::connected(0, "Xbox Wireless Controller");
        gamepad.press_button(engine_platform::GamepadButton::A);
        gamepad.left_stick_x = 0.75;
        services.gamepad_provider = Box::new(FixedGamepadProvider {
            states: vec![gamepad],
        });

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert!(
            services
                .input
                .gamepad_button_down(engine_platform::GamepadButton::A)
        );
        assert_eq!(services.input.gamepad_states().len(), 1);
        assert_eq!(services.input.gamepad_states()[0].left_stick_x, 0.75);
    }

    #[test]
    fn load_action_bindings_from_toml() {
        use engine_platform::KeyCode;
        let dir =
            std::env::temp_dir().join(format!("varg-action-bindings-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let toml_path = dir.join("action_bindings.toml");
        std::fs::write(
            &toml_path,
            "[actions]\nCrouch = [\"C\"]\nSprint = [\"Shift\"]\n",
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.load_action_bindings(&toml_path).unwrap();

        // Verify the Crouch binding was added
        assert!(services.action_map.bindings.get("Crouch").is_some());
        assert!(
            services
                .action_map
                .bindings
                .get("Crouch")
                .unwrap()
                .contains(&KeyCode::Character('c')),
            "Crouch should be bound to C"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_input_context_bindings_from_toml() {
        let dir =
            std::env::temp_dir().join(format!("varg-input-context-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let toml_path = dir.join("input_bindings.toml");
        std::fs::write(
            &toml_path,
            r#"
[contexts.Gameplay]
priority = 10

[[contexts.Gameplay.bindings]]
action = "Dash"
keys = ["C"]
triggers = ["Pressed"]

[[contexts.Gameplay.bindings]]
action = "LookX"
axis = "Axis1D"
gamepad_axes = ["RightStickX"]
deadzone = 0.1
modifiers = ["Scalar:0.5"]
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.load_action_bindings(&toml_path).unwrap();

        services
            .input
            .apply_event(engine_platform::InputEvent::KeyDown(
                engine_platform::KeyCode::Character('c'),
            ));
        services.input.evaluate_mapped_actions(1.0 / 60.0);
        assert!(services.action_pressed("Dash"));
        assert!(services.action_held("Dash"));

        services.input.end_frame();
        services
            .input
            .apply_event(engine_platform::InputEvent::KeyUp(
                engine_platform::KeyCode::Character('c'),
            ));
        services.input.evaluate_mapped_actions(1.0 / 60.0);
        assert!(!services.action_held("Dash"));

        let mut gamepad = engine_platform::GamepadState::connected(0, "Test Controller");
        gamepad.right_stick_x = 0.6;
        services.input.apply_gamepad_state(gamepad);
        services.input.evaluate_mapped_actions(1.0 / 60.0);
        let look_x = services.input.action_value("LookX");
        assert!(
            (0.25..0.35).contains(&look_x),
            "LookX should apply deadzone and scalar modifier, got {look_x}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn varg_script_mouse_capture_request_updates_runtime_state() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("capture.varg"),
            r#"script Capture {
    func update(_ dt: Float) {
        Input.captureMouse(true)
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Player").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/capture.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert_eq!(
            services.input_capture(),
            RuntimeInputCapture { mouse: true }
        );
    }

    #[test]
    fn varg_script_mouse_capture_request_can_release_runtime_state() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(
            scripts.join("capture.varg"),
            r#"script Capture {
    func update(_ dt: Float) {
        if Input.pressed("Escape") {
            Input.captureMouse(false)
        } else {
            Input.captureMouse(true)
        }
    }
}
"#,
        )
        .unwrap();

        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.set_project_root(root.path());
        let entity = services.scene.create_object("Player").unwrap();
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(engine_ecs::ScriptComponent::new("scripts/capture.varg")),
            )
            .unwrap();

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();
        assert_eq!(
            services.input_capture(),
            RuntimeInputCapture { mouse: true }
        );

        services
            .input
            .apply_event(engine_platform::InputEvent::KeyDown(
                engine_platform::KeyCode::Escape,
            ));
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();
        assert_eq!(
            services.input_capture(),
            RuntimeInputCapture { mouse: false }
        );
    }

    #[test]
    fn escape_opens_pause_menu_and_menu_actions_control_runtime() {
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.stats.output_size = (800, 600);
        services.input_capture.mouse = true;
        services
            .input
            .apply_event(engine_platform::InputEvent::KeyDown(
                engine_platform::KeyCode::Escape,
            ));

        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert!(services.pause_menu_open);
        assert!(services.paused);
        assert_eq!(
            services.input_capture(),
            RuntimeInputCapture { mouse: false }
        );
        assert!(
            !build_pause_menu_draw_list(
                GuiDrawList::default(),
                services.user_preferences,
                (1920, 1080),
            )
            .commands
            .is_empty()
        );

        services.stats.output_size = (800, 600);
        let layout = PauseMenuLayout::for_output(services.stats.output_size);
        services.pointer_released.push((
            layout.invert_x_button.x + 40.0,
            layout.invert_x_button.y + 20.0,
        ));
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert!(services.pause_menu_open);
        assert!(services.user_preferences.invert_mouse_x);

        services.stats.output_size = (800, 600);
        services.pointer_released.push((
            layout.invert_y_button.x + 40.0,
            layout.invert_y_button.y + 20.0,
        ));
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert!(services.user_preferences.invert_mouse_y);

        services
            .input
            .apply_event(engine_platform::InputEvent::KeyUp(
                engine_platform::KeyCode::Escape,
            ));
        services.stats.output_size = (800, 600);
        services.pointer_released.push((
            layout.continue_button.x + 40.0,
            layout.continue_button.y + 20.0,
        ));
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert!(!services.pause_menu_open);
        assert!(!services.paused);

        services
            .input
            .apply_event(engine_platform::InputEvent::KeyDown(
                engine_platform::KeyCode::Escape,
            ));
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();
        services
            .input
            .apply_event(engine_platform::InputEvent::KeyUp(
                engine_platform::KeyCode::Escape,
            ));
        services.stats.output_size = (800, 600);
        services
            .pointer_released
            .push((layout.exit_button.x + 40.0, layout.exit_button.y + 20.0));
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert!(services.take_exit_requested());
        assert!(!services.take_exit_requested());
    }

    #[test]
    fn pause_menu_layout_centers_in_small_outputs() {
        let layout = PauseMenuLayout::for_output((800, 600));

        assert_eq!(layout.panel.width, 600.0);
        assert_eq!(layout.panel.height, 410.0);
        assert_eq!(layout.panel.x, 100.0);
        assert_eq!(layout.panel.y, 95.0);
        assert!(layout.continue_button.contains(200.0, 225.0));
    }

    #[test]
    fn pause_menu_click_targets_follow_output_size() {
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services.stats.output_size = (800, 600);
        services.pause_menu_open = true;
        services.paused = true;
        let layout = PauseMenuLayout::for_output(services.stats.output_size);

        services.pointer_released.push((
            layout.continue_button.x + 8.0,
            layout.continue_button.y + 8.0,
        ));
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .unwrap();

        assert!(!services.pause_menu_open);
        assert!(!services.paused);
    }

    #[test]
    fn runtime_user_preferences_scale_script_mouse_delta() {
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        services
            .input
            .apply_event(engine_platform::InputEvent::MouseDelta { x: 6.0, y: -4.0 });

        assert_eq!(services.input_for_scripts().mouse_delta(), (6.0, -4.0));

        services.user_preferences.invert_mouse_x = true;
        assert_eq!(services.input_for_scripts().mouse_delta(), (-6.0, -4.0));

        services.user_preferences.invert_mouse_y = true;
        assert_eq!(services.input_for_scripts().mouse_delta(), (-6.0, 4.0));
    }

    #[test]
    fn load_action_bindings_missing_file_returns_error() {
        let mut services = RuntimeServices::minimal(EngineConfig::default());
        let result = services.load_action_bindings(Path::new("/nonexistent/action_bindings.toml"));
        assert!(result.is_err());
    }
}
