use super::*;

/// Runs a project with the runtime-game windowed loop.
#[cfg(feature = "runtime-game")]
pub fn run_project(project: impl AsRef<Path>) -> EngineResult<()> {
    use engine_platform::KeyCode;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use winit::{
        application::ApplicationHandler,
        event::{DeviceEvent, DeviceId, ElementState, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId},
    };

    #[cfg(feature = "wgpu")]
    type GameServices = RuntimeServices<WgpuRenderDevice>;
    #[cfg(not(feature = "wgpu"))]
    type GameServices = RuntimeServices;

    struct GameApp {
        services: Option<GameServices>,
        project: Option<RuntimeProject>,
        window: Option<Arc<Window>>,
        last_frame: Instant,
        single_step: bool,
        project_name: String,
        target_frame_time: Duration,
        applied_input_capture: Option<RuntimeInputCapture>,
    }

    impl ApplicationHandler for GameApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() {
                return;
            }
            let width = std::env::var("VARG_OUTPUT_WIDTH")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1920_u32);
            let height = std::env::var("VARG_OUTPUT_HEIGHT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1080_u32);
            let window = event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(&self.project_name)
                        .with_inner_size(winit::dpi::PhysicalSize::new(width, height)),
                )
                .expect("create runtime window");
            let window = Arc::new(window);
            let size = window.inner_size();
            let Some(project) = self.project.take() else {
                eprintln!("runtime error: project was already consumed");
                event_loop.exit();
                return;
            };
            match create_game_services(EngineConfig::default(), window.clone(), size, project) {
                Ok(services) => self.services = Some(services),
                Err(error) => {
                    eprintln!("runtime error: {error}");
                    event_loop.exit();
                    return;
                }
            }
            self.window = Some(window);
            event_loop.set_control_flow(ControlFlow::Wait);
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            match &event {
                WindowEvent::KeyboardInput { event, .. } => {
                    if let Some(key) = convert_winit_key_static(event.physical_key) {
                        if event.state == ElementState::Pressed {
                            if key == KeyCode::Space {
                                if let Some(services) = self.services.as_mut() {
                                    services.paused = !services.paused;
                                }
                            } else if key == KeyCode::Enter {
                                self.single_step = true;
                            }
                        }
                    }
                }
                WindowEvent::Resized(size) => {
                    self.resize_surface(size.width, size.height);
                    let title = format!(
                        "Varg Runtime - {}x{}",
                        size.width.max(1),
                        size.height.max(1)
                    );
                    if let Some(window) = &self.window {
                        window.set_title(&title);
                    }
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    self.sync_surface_to_window();
                }
                WindowEvent::RedrawRequested => {
                    self.sync_surface_to_window();
                    let Some(services) = self.services.as_mut() else {
                        return;
                    };
                    let now = Instant::now();
                    let delta = now.saturating_duration_since(self.last_frame);
                    self.last_frame = now;
                    if let Err(error) = services.tick_game_frame(delta, self.single_step) {
                        eprintln!("runtime error: {error}");
                        event_loop.exit();
                        return;
                    }
                    if services.take_exit_requested() {
                        event_loop.exit();
                        return;
                    }
                    if let Some(window) = &self.window {
                        let capture = services.input_capture();
                        if self.applied_input_capture != Some(capture) {
                            if let Err(error) = apply_winit_input_capture(window, capture) {
                                eprintln!("runtime input capture error: {error}");
                            } else {
                                self.applied_input_capture = Some(capture);
                            }
                        }
                    }
                    self.single_step = false;
                    if let Some(window) = &self.window {
                        let status = if services.render_world.is_visible() {
                            "rendering"
                        } else {
                            "empty"
                        };
                        window.set_title(&format!(
                            "Varg Runtime - frame {} - {status}",
                            services.frame_index()
                        ));
                    }
                }
                _ => {}
            }
            if let Some(services) = self.services.as_mut() {
                if services.process_winit_event(&event) {
                    event_loop.exit();
                }
            }
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            if let Some(window) = &self.window {
                window.request_redraw();
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + self.target_frame_time,
                ));
            } else {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }

        fn device_event(
            &mut self,
            _event_loop: &ActiveEventLoop,
            _device_id: DeviceId,
            event: DeviceEvent,
        ) {
            if let Some(services) = self.services.as_mut() {
                services.process_winit_device_event(&event);
            }
        }
    }

    impl GameApp {
        fn resize_surface(&mut self, width: u32, height: u32) {
            #[cfg(feature = "wgpu")]
            if let Some(services) = self.services.as_mut() {
                services.renderer.resize_surface(width, height);
            }
        }

        fn sync_surface_to_window(&mut self) {
            let Some(window) = self.window.as_ref() else {
                return;
            };
            let size = window.inner_size();
            self.resize_surface(size.width, size.height);
        }
    }

    let project = load_runtime_project(project)?;
    let project_name = project.manifest.name.clone();
    let target_fps = project.build.render.target_fps.max(1);
    let target_frame_time = Duration::from_secs_f64(1.0 / f64::from(target_fps));
    let event_loop = EventLoop::new().map_err(|error| EngineError::other(error.to_string()))?;
    let mut app = GameApp {
        services: None,
        project: Some(project),
        window: None,
        last_frame: Instant::now(),
        single_step: false,
        project_name,
        target_frame_time,
        applied_input_capture: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|error| EngineError::other(error.to_string()))
}

#[cfg(all(feature = "runtime-game", feature = "wgpu"))]
fn create_game_services(
    config: EngineConfig,
    window: std::sync::Arc<winit::window::Window>,
    size: winit::dpi::PhysicalSize<u32>,
    project: RuntimeProject,
) -> EngineResult<RuntimeServices<WgpuRenderDevice>> {
    let instance = engine_render_wgpu::wgpu::Instance::default();
    let surface = instance
        .create_surface(window)
        .map_err(|error| EngineError::other(format!("create wgpu surface failed: {error}")))?;
    let mut renderer =
        WgpuRenderDevice::new_surface(surface, size.width.max(1), size.height.max(1))?;
    renderer.configure_performance(runtime_performance_config_from_env());
    let scaling_settings = render_scaling_settings_from_build(&project.build);
    let mut services = RuntimeServices::with_renderer(config, renderer);
    services.set_render_scaling(scaling_settings, runtime_scaling_context());
    #[cfg(feature = "audio")]
    services.enable_default_audio_output();
    services.set_project_root(project.root.clone());
    services.set_script_roots(
        project
            .manifest
            .script_roots
            .iter()
            .map(|root| PathBuf::from(root.as_str())),
    );
    let asset_root = project.root.join(&project.manifest.asset_root);
    services.load_project_assets(asset_root)?;
    services.scene = project.scene;
    services.render_world = extract_render_world(&services.scene);
    Ok(services)
}

#[cfg(all(feature = "runtime-game", not(feature = "wgpu")))]
fn create_game_services(
    config: EngineConfig,
    _window: std::sync::Arc<winit::window::Window>,
    _size: winit::dpi::PhysicalSize<u32>,
    project: RuntimeProject,
) -> EngineResult<RuntimeServices> {
    let mut services = RuntimeServices::minimal(config);
    #[cfg(feature = "audio")]
    services.enable_default_audio_output();
    services.set_project_root(project.root.clone());
    services.set_script_roots(
        project
            .manifest
            .script_roots
            .iter()
            .map(|root| PathBuf::from(root.as_str())),
    );
    let asset_root = project.root.join(&project.manifest.asset_root);
    services.load_project_assets(asset_root)?;
    services.scene = project.scene;
    services.render_world = extract_render_world(&services.scene);
    Ok(services)
}

/// Reports that runtime-game support is not compiled into this binary.
#[cfg(not(feature = "runtime-game"))]
pub fn run_project(_project: impl AsRef<Path>) -> EngineResult<()> {
    Err(EngineError::UnsupportedCapability {
        capability: "runtime-game",
    })
}
