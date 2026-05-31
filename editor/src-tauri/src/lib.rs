//! Tauri backend for the Aster Editor.
//!
//! Single `rpc` command that dispatches to EditorHost methods,
//! mirroring the original stdin/stdout JSON-RPC protocol.

use std::{
    cell::UnsafeCell,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

use engine_core::{EngineConfig, EngineError, EngineResult, RuntimeProfile};
use engine_editor::{
    ConsoleEntry, ConsoleLevel, ConsoleService, DurableEditorState, EditorPreferences,
    FileEditorStore, ProjectMetadata, ThemePreference, UndoCommand,
};
use engine_editor::{EditorShell, HubState, ProjectDeletionDecision, ProjectDeletionMode};
use engine_i18n::{Locale, Translations};
use engine_render::ImageFormat;
use engine_render_wgpu::{WgpuOffscreenConfig, WgpuRenderDevice};
use runtime_min::{headless_services_from_scene, RuntimeServices};
use serde_json::Value;
use tauri::{utils::config::Color, Manager, State};

const WINDOW_BACKGROUND: &str = "#181818";

#[derive(Clone, Debug, Eq, PartialEq)]
enum DesktopEnvironment {
    Gnome,
    Kde,
    Xfce,
    Cinnamon,
    Mate,
    Unknown,
}

impl DesktopEnvironment {
    fn detect() -> Self {
        let candidates = [
            std::env::var("XDG_CURRENT_DESKTOP").ok(),
            std::env::var("XDG_SESSION_DESKTOP").ok(),
            std::env::var("DESKTOP_SESSION").ok(),
            std::env::var("KDE_FULL_SESSION")
                .ok()
                .filter(|v| v == "true"),
            std::env::var("GNOME_DESKTOP_SESSION_ID").ok(),
        ];
        let desktop = candidates
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(":")
            .to_ascii_lowercase();

        if desktop.contains("kde") || desktop.contains("plasma") {
            Self::Kde
        } else if desktop.contains("gnome") {
            Self::Gnome
        } else if desktop.contains("xfce") {
            Self::Xfce
        } else if desktop.contains("cinnamon") {
            Self::Cinnamon
        } else if desktop.contains("mate") {
            Self::Mate
        } else {
            Self::Unknown
        }
    }

    fn id(&self) -> &'static str {
        match self {
            Self::Gnome => "gnome",
            Self::Kde => "kde",
            Self::Xfce => "xfce",
            Self::Cinnamon => "cinnamon",
            Self::Mate => "mate",
            Self::Unknown => "unknown",
        }
    }

    fn prefers_native_chrome(&self) -> bool {
        true
    }

    #[cfg(test)]
    fn prefers_native_chrome_for_backend(&self, native_wayland_preferred: bool) -> bool {
        let _ = native_wayland_preferred;
        true
    }
}

#[derive(Clone, Debug)]
struct DesktopIntegration {
    desktop: DesktopEnvironment,
}

impl DesktopIntegration {
    fn detect() -> Self {
        Self {
            desktop: DesktopEnvironment::detect(),
        }
    }

    fn prefers_native_chrome(&self) -> bool {
        self.desktop.prefers_native_chrome()
    }

    fn as_json(&self) -> Value {
        serde_json::json!({
            "desktop_environment": self.desktop.id(),
            "prefers_native_chrome": self.prefers_native_chrome(),
            "window_background": WINDOW_BACKGROUND,
            "window_backend": std::env::var("GDK_BACKEND").unwrap_or_else(|_| "default".to_owned()),
        })
    }
}

// ─── Editor host state ───────────────────────────────────────────────────────

pub struct EditorHost {
    /// Hub state (project picker screen).
    hub: HubState,
    /// Editor shell (active editor when a project is open).
    shell: EditorShell,
    /// Durable state loaded from disk.
    durable_state: DurableEditorState,
    /// File-based preference store.
    store: FileEditorStore,
    /// Console service (shared between hub and shell).
    console: ConsoleService,
    /// WGPU render device for offscreen viewport rendering (lazily created).
    render_device: Option<WgpuRenderDevice>,
    /// Desktop/window integration policy detected on the Rust side.
    desktop_integration: DesktopIntegration,
    /// Cached translations for the current locale.
    translations: Translations,
    /// Monotonic version counter incremented on every scene mutation.
    /// Used by the frontend to skip viewport re-renders when nothing changed.
    scene_version: u64,
    /// Runtime snapshot used by Game View play mode.
    play_runtime: Option<RuntimeServices>,
    /// Last wall-clock frame timestamp for play mode deltas.
    play_last_frame: Option<Instant>,
    /// Monotonic version counter for simulated play frames.
    play_version: u64,
}

impl EditorHost {
    pub fn new(store: FileEditorStore) -> EngineResult<Self> {
        let durable_state = store.load().unwrap_or_default();
        let hub = HubState::from_durable_state(durable_state.clone());
        let locale = hub.preferences().locale;
        let mut host = Self {
            hub,
            shell: EditorShell::with_core_services(EditorPreferences::default()),
            durable_state,
            store,
            console: ConsoleService::default(),
            render_device: None,
            desktop_integration: DesktopIntegration::detect(),
            translations: Translations::load(locale),
            scene_version: 1,
            play_runtime: None,
            play_last_frame: None,
            play_version: 1,
        };

        host.reopen_last_project_if_needed();
        Ok(host)
    }

    /// Dispatch an RPC method call.
    pub fn handle(&mut self, method: &str, params: &Value) -> EngineResult<Value> {
        match method {
            // ── Hub ──
            "app/get_desktop_integration" => self.app_get_desktop_integration(params),
            "app/open_folder" => self.app_open_folder(params),
            "hub/get_state" => self.hub_get_state(params),
            "hub/get_translations" => self.hub_get_translations(params),
            "hub/list_projects" => self.hub_list_projects(params),
            "hub/open_project" => self.hub_open_project(params),
            "hub/create_project" => self.hub_create_project(params),
            "hub/delete_project" => self.hub_delete_project(params),
            "hub/set_theme" => self.hub_set_theme(params),
            "hub/set_page" => self.hub_set_page(params),
            "hub/set_locale" => self.hub_set_locale(params),

            // ── Console ──
            "console/get_entries" => self.console_get_entries(params),
            "console/clear" => self.console_clear(params),
            "console/push_entry" => self.console_push_entry(params),

            // ── Viewport ──
            "viewport/readback" => self.viewport_readback(params),

            // ── Play mode ──
            "play/start" => self.play_start(params),
            "play/stop" => self.play_stop(params),
            "play/get_state" => self.play_get_state(params),

            // ── Shell ──
            "shell/get_state" => self.shell_get_state(params),
            "shell/get_scene_tree" => self.shell_get_scene_tree(params),
            "shell/get_entity" => self.shell_get_entity(params),
            "shell/select_entity" => self.shell_select_entity(params),
            "shell/save_scene" => self.shell_save_scene(params),
            "shell/open_scene" => self.shell_open_scene(params),
            "shell/save_scene_as" => self.shell_save_scene_as(params),
            "shell/update_transform" => self.shell_update_transform(params),
            "shell/add_component" => self.shell_add_component(params),
            "shell/remove_component" => self.shell_remove_component(params),
            "shell/undo" => self.shell_undo(params),
            "shell/redo" => self.shell_redo(params),
            "shell/create_object" => self.shell_create_object(params),
            "shell/delete_object" => self.shell_delete_object(params),
            "shell/duplicate_object" => self.shell_duplicate_object(params),
            "shell/close_project" => self.shell_close_project(params),

            _ => Err(EngineError::config(format!("unknown method: {method}"))),
        }
    }

    fn app_get_desktop_integration(&mut self, _params: &Value) -> EngineResult<Value> {
        Ok(self.desktop_integration.as_json())
    }

    fn app_open_folder(&mut self, params: &Value) -> EngineResult<Value> {
        use std::process::Command;

        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;

        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open")
                .arg(path)
                .spawn()
                .map_err(|e| EngineError::other(format!("failed to open folder: {e}")))?;
        }
        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .arg(path)
                .spawn()
                .map_err(|e| EngineError::other(format!("failed to open folder: {e}")))?;
        }
        #[cfg(target_os = "windows")]
        {
            Command::new("explorer")
                .arg(path)
                .spawn()
                .map_err(|e| EngineError::other(format!("failed to open folder: {e}")))?;
        }

        Ok(serde_json::json!({ "opened": true }))
    }

    // ── Hub handlers ──

    fn hub_get_state(&mut self, _params: &Value) -> EngineResult<Value> {
        Ok(serde_json::json!({
            "page": match self.hub.page() {
                engine_editor::ui_state::HubPage::Projects => "projects",
                engine_editor::ui_state::HubPage::Installs => "installs",
                engine_editor::ui_state::HubPage::Settings => "settings",
            },
            "theme": match self.hub.preferences().theme {
                ThemePreference::Dark => "dark",
                ThemePreference::Light => "light",
                ThemePreference::System => "system",
            },
            "recent_projects": self.hub.filtered_projects().iter().map(|p| serde_json::json!({
                "name": p.name,
                "path": p.path.to_string_lossy(),
                "last_touched": p.last_touched,
                "toolchain_version": p.toolchain_version,
            })).collect::<Vec<_>>(),
            "locale": match self.hub.preferences().locale {
                engine_i18n::Locale::Zh => "zh",
                _ => "en",
            },
            "installs": self.hub.installs().iter().map(|i| serde_json::json!({
                "version": i.version,
                "path": i.path.to_string_lossy(),
                "editor_available": i.editor_available,
                "runtime_available": i.runtime_available,
            })).collect::<Vec<_>>(),
            "open_project": self.shell.project().map(|p| p.root.to_string_lossy()),
            "desktop_integration": self.desktop_integration.as_json(),
        }))
    }

    fn hub_list_projects(&mut self, _params: &Value) -> EngineResult<Value> {
        let projects: Vec<Value> = self
            .hub
            .filtered_projects()
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "path": p.path.to_string_lossy(),
                    "last_touched": p.last_touched,
                    "toolchain_version": p.toolchain_version,
                })
            })
            .collect();
        Ok(serde_json::json!({ "projects": projects }))
    }

    fn hub_open_project(&mut self, params: &Value) -> EngineResult<Value> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path' parameter"))?;
        let project_path = PathBuf::from(path);

        // Load the project into the editor shell
        self.shell.open_project(&project_path)?;

        // Mark as recent
        let name = self
            .shell
            .project()
            .map(|p| p.name().to_owned())
            .unwrap_or_else(|| {
                project_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            });
        let metadata = ProjectMetadata::new(&name, &project_path, timestamp_now(), "0.1.0");
        self.hub.upsert_project(metadata);

        // Persist state
        self.hub.mark_project_open(project_path.clone());
        self.sync_durable_state();

        // Forward console entries from shell open
        self.drain_shell_console();

        Ok(serde_json::json!({
            "name": name,
            "path": project_path.to_string_lossy(),
        }))
    }

    fn hub_create_project(&mut self, params: &Value) -> EngineResult<Value> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'name' parameter"))?;
        let location = params
            .get("location")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'location' parameter"))?;

        let request = engine_editor::NewProjectRequest {
            name: name.to_owned(),
            location: Some(PathBuf::from(location)),
            template_id: params
                .get("template_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned()),
            toolchain_version: params
                .get("toolchain_version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned()),
        };

        let plan = self.hub.create_project_plan(&request)?;
        self.hub.create_project_files(&plan)?;

        let metadata = ProjectMetadata::new(
            &plan.name,
            &plan.path,
            timestamp_now(),
            &plan.toolchain_version,
        );
        self.hub.upsert_project(metadata);
        self.sync_durable_state();

        Ok(serde_json::json!({
            "name": plan.name,
            "path": plan.path.to_string_lossy(),
        }))
    }

    fn hub_delete_project(&mut self, params: &Value) -> EngineResult<Value> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path' parameter"))?;
        let confirmed = params
            .get("confirmed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let project_path = PathBuf::from(path);
        let decision = self.hub.request_project_deletion(
            &project_path,
            ProjectDeletionMode::RemoveRecent,
            confirmed,
        );

        match decision {
            ProjectDeletionDecision::RemovedFromRecent { .. } => {
                self.sync_durable_state();
                Ok(serde_json::json!({ "status": "removed" }))
            }
            ProjectDeletionDecision::NeedsConfirmation { .. } => {
                Ok(serde_json::json!({ "status": "needs_confirmation" }))
            }
            ProjectDeletionDecision::RefusedOpenProject { .. } => {
                Err(EngineError::config("cannot delete an open project"))
            }
            ProjectDeletionDecision::DeleteFilesApproved { .. } => Err(EngineError::config(
                "file deletion not supported through IPC",
            )),
        }
    }

    fn hub_set_theme(&mut self, params: &Value) -> EngineResult<Value> {
        let theme = params
            .get("theme")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'theme' parameter"))?;
        let pref = match theme {
            "light" => ThemePreference::Light,
            "dark" => ThemePreference::Dark,
            _ => ThemePreference::System,
        };
        self.hub.set_theme(pref);
        self.sync_durable_state();
        Ok(serde_json::json!({ "theme": theme }))
    }

    fn hub_set_page(&mut self, params: &Value) -> EngineResult<Value> {
        let page = params
            .get("page")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'page' parameter"))?;
        use engine_editor::ui_state::HubPage;
        let p = match page {
            "installs" => HubPage::Installs,
            "settings" => HubPage::Settings,
            _ => HubPage::Projects,
        };
        self.hub.set_page(p);
        self.sync_durable_state();
        Ok(serde_json::json!({ "page": page }))
    }

    fn hub_get_translations(&mut self, _params: &Value) -> EngineResult<Value> {
        let entries: Vec<serde_json::Value> = self
            .translations
            .entries()
            .into_iter()
            .map(|(k, v)| serde_json::json!({ "key": k, "value": v }))
            .collect();
        Ok(serde_json::json!({
            "locale": match self.translations.locale() {
                Locale::En => "en",
                Locale::Zh => "zh",
            },
            "entries": entries,
        }))
    }

    fn hub_set_locale(&mut self, params: &Value) -> EngineResult<Value> {
        let locale_str = params
            .get("locale")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'locale' parameter"))?;
        let locale = match locale_str {
            "zh" => Locale::Zh,
            _ => Locale::En,
        };
        self.hub.set_locale(locale);
        // Reload translations for the new locale
        self.translations = Translations::load(locale);
        self.sync_durable_state();
        Ok(serde_json::json!({ "locale": locale_str }))
    }

    // ── Console handlers ──

    fn console_get_entries(&mut self, _params: &Value) -> EngineResult<Value> {
        let entries: Vec<Value> = self
            .console
            .entries()
            .iter()
            .map(|e| {
                serde_json::json!({
                    "timestamp": e.timestamp,
                    "level": format!("{:?}", e.level).to_lowercase(),
                    "subsystem": e.source.subsystem,
                    "file": e.source.file.as_ref().map(|f| f.to_string_lossy()),
                    "line": e.source.line,
                    "message": e.message,
                })
            })
            .collect();
        Ok(serde_json::json!({ "entries": entries }))
    }

    fn console_clear(&mut self, _params: &Value) -> EngineResult<Value> {
        self.console.clear();
        Ok(serde_json::json!({}))
    }

    fn console_push_entry(&mut self, params: &Value) -> EngineResult<Value> {
        let level = match params
            .get("level")
            .and_then(|v| v.as_str())
            .unwrap_or("info")
        {
            "trace" => ConsoleLevel::Trace,
            "debug" => ConsoleLevel::Debug,
            "warn" => ConsoleLevel::Warn,
            "error" => ConsoleLevel::Error,
            _ => ConsoleLevel::Info,
        };
        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        let subsystem = params
            .get("subsystem")
            .and_then(|v| v.as_str())
            .unwrap_or("editor")
            .to_owned();
        self.console.push(ConsoleEntry {
            timestamp: timestamp_now(),
            level,
            source: engine_editor::ConsoleSource {
                subsystem,
                file: params
                    .get("file")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from),
                line: params
                    .get("line")
                    .and_then(|v| v.as_u64())
                    .map(|l| l as u32),
            },
            message,
        });
        Ok(serde_json::json!({}))
    }

    /// Increment the scene version counter so the frontend can skip redundant renders.
    fn bump_scene_version(&mut self) {
        self.scene_version = self.scene_version.wrapping_add(1);
    }

    // ── Viewport handlers ──

    /// Render the current scene to an offscreen buffer and return raw RGBA pixels.
    /// Returns `(width, height, rgba_bytes)`.
    /// If `last_version` param matches the current `scene_version`, skips rendering
    /// and returns `(0, 0, empty_vec)` as a no-change signal.
    fn render_viewport(&mut self, params: &Value) -> EngineResult<(u32, u32, Vec<u8>)> {
        use engine_core::math::Vec3;
        use runtime_min::extract_render_world;

        let play_mode = params
            .get("play_mode")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Lazy rendering: if the scene version hasn't changed, skip the full pipeline
        if !play_mode {
            if let Some(last_ver) = params.get("last_version").and_then(|v| v.as_u64()) {
                if last_ver == self.scene_version {
                    return Ok((0, 0, Vec::new()));
                }
            }
        } else if let Some(last_ver) = params.get("last_version").and_then(|v| v.as_u64()) {
            if last_ver == self.play_version {
                return Ok((0, 0, Vec::new()));
            }
        }

        let (width, height) = (
            params.get("width").and_then(|v| v.as_u64()).unwrap_or(640) as u32,
            params.get("height").and_then(|v| v.as_u64()).unwrap_or(480) as u32,
        );

        // Extract render world from the scene
        let mut world = if play_mode {
            self.tick_play_runtime()?;
            let Some(runtime) = self.play_runtime.as_ref() else {
                return Err(EngineError::config("play mode is not running"));
            };
            extract_render_world(&runtime.scene)
        } else {
            let Some(project) = self.shell.project() else {
                return Err(EngineError::config("no project open"));
            };
            extract_render_world(&project.scene)
        };

        // Set up editor camera if we have one
        if let Some(ref mut camera) = world.camera {
            let camera_yaw = params.get("yaw").and_then(|v| v.as_f64()).unwrap_or(-0.5) as f32;
            let camera_pitch = params.get("pitch").and_then(|v| v.as_f64()).unwrap_or(0.3) as f32;
            let camera_dist = params
                .get("distance")
                .and_then(|v| v.as_f64())
                .unwrap_or(6.0) as f32;
            let target_x = params
                .get("target_x")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;
            let target_y = params
                .get("target_y")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;
            let target_z = params
                .get("target_z")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;

            let eye_x = target_x + camera_dist * camera_pitch.cos() * camera_yaw.sin();
            let eye_y = target_y + camera_dist * camera_pitch.sin();
            let eye_z = target_z + camera_dist * camera_pitch.cos() * camera_yaw.cos();

            camera.transform.translation = Vec3::new(eye_x, eye_y, eye_z);
        }

        // Lazily create the wgpu render device (with proper error handling)
        if self.render_device.is_none() {
            let config = WgpuOffscreenConfig {
                width: width.max(1),
                height: height.max(1),
                format: ImageFormat::Rgba8Srgb,
            };
            self.render_device =
                Some(WgpuRenderDevice::new_offscreen(config).map_err(|e| {
                    EngineError::other(format!("failed to create wgpu device: {e}"))
                })?);
        }
        let device = self.render_device.as_mut().unwrap();

        // Resize if needed
        let (cur_w, cur_h) = device.default_target_size();
        if cur_w != width || cur_h != height {
            device
                .resize_default_target(width.max(1), height.max(1))
                .map_err(|e| EngineError::other(format!("resize failed: {e}")))?;
        }

        // Render
        device.render_world_offscreen(&world)?;

        // Readback raw RGBA
        let (w, h, rgba) = device.readback_default_target()?;
        Ok((w, h, rgba))
    }

    /// Legacy JSON viewport readback — encodes as PNG + base64.
    /// Prefer `viewport_readback_raw` for performance.
    fn viewport_readback(&mut self, params: &Value) -> EngineResult<Value> {
        let (width, height, rgba) = self.render_viewport(params)?;

        // Encode as PNG
        use image::EncodableLayout;
        let img = image::RgbaImage::from_raw(width.max(1), height.max(1), rgba)
            .ok_or_else(|| EngineError::other("failed to create RGBA image"))?;
        let mut png_bytes = Vec::new();
        {
            use image::codecs::png::PngEncoder;
            use image::ImageEncoder;
            let encoder = PngEncoder::new(&mut png_bytes);
            encoder
                .write_image(
                    img.as_bytes(),
                    img.width(),
                    img.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(|e| EngineError::other(format!("PNG encode failed: {e}")))?;
        }
        let b64 = base64_encode(&png_bytes);

        Ok(serde_json::json!({
            "width": width,
            "height": height,
            "png_base64": b64,
        }))
    }

    /// Binary viewport readback — returns raw RGBA bytes with
    /// [width: u32 LE][height: u32 LE][pixels...] layout.
    /// Frontend receives this as ArrayBuffer via Tauri binary IPC.
    fn viewport_readback_raw(&mut self, params: &Value) -> EngineResult<Vec<u8>> {
        let (width, height, rgba) = self.render_viewport(params)?;

        // Prepend dimensions as u32 LE headers, then raw RGBA pixels
        let mut result = Vec::with_capacity(8 + rgba.len());
        result.extend_from_slice(&(width as u32).to_le_bytes());
        result.extend_from_slice(&(height as u32).to_le_bytes());
        result.extend_from_slice(&rgba);
        Ok(result)
    }

    // ── Shell handlers ──

    fn shell_get_state(&mut self, _params: &Value) -> EngineResult<Value> {
        Ok(serde_json::json!({
            "has_project": self.shell.project().is_some(),
            "project_name": self.shell.project().map(|p| p.name()),
            "scene_dirty": self.shell.is_scene_dirty(),
            "can_undo": self.shell.undo_stack().can_undo(),
            "can_redo": self.shell.undo_stack().can_redo(),
            "scene_version": self.scene_version,
            "desktop_integration": self.desktop_integration.as_json(),
        }))
    }

    fn shell_get_scene_tree(&mut self, _params: &Value) -> EngineResult<Value> {
        let Some(project) = self.shell.project() else {
            return Ok(serde_json::json!({ "objects": [] }));
        };
        let objects: Vec<Value> = project
            .scene
            .objects()
            .iter()
            .map(|(entity, obj)| {
                let transform = project
                    .scene
                    .transforms()
                    .world(*entity)
                    .unwrap_or_default();
                serde_json::json!({
                    "id": format!("{:032x}", obj.id.as_u128()),
                    "name": obj.name,
                    "tag": obj.tag,
                    "position": [
                        transform.translation.x,
                        transform.translation.y,
                        transform.translation.z,
                    ],
                })
            })
            .collect();
        Ok(serde_json::json!({ "objects": objects }))
    }

    fn shell_get_entity(&mut self, params: &Value) -> EngineResult<Value> {
        let id_str = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'id' parameter"))?;
        let entity_id_val = u128::from_str_radix(id_str, 16)
            .map_err(|_| EngineError::config("invalid entity id"))?;
        let entity_id = engine_core::EntityId::from_u128(entity_id_val);

        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };
        let entity = project
            .scene
            .find_by_id(entity_id)
            .ok_or_else(|| EngineError::config("entity not found"))?;
        let Some(obj) = project.scene.object(entity) else {
            return Err(EngineError::config("entity not found"));
        };
        let transform = project.scene.transforms().world(entity).unwrap_or_default();
        let components: Vec<Value> = obj
            .components
            .iter()
            .map(|c| {
                serde_json::json!({
                    "type": format!("{:?}", c),
                })
            })
            .collect();

        Ok(serde_json::json!({
            "id": id_str,
            "name": obj.name,
            "tag": obj.tag,
            "transform": {
                "position": [transform.translation.x, transform.translation.y, transform.translation.z],
                "rotation": [transform.rotation.x, transform.rotation.y, transform.rotation.z, transform.rotation.w],
                "scale": [transform.scale.x, transform.scale.y, transform.scale.z],
            },
            "components": components,
        }))
    }

    fn shell_select_entity(&mut self, params: &Value) -> EngineResult<Value> {
        let id_str = params.get("id").and_then(|v| v.as_str());
        match id_str {
            Some(id) => {
                self.shell
                    .select_entity_id(engine_core::EntityId::from_u128(
                        u128::from_str_radix(id, 16)
                            .map_err(|_| EngineError::config("invalid entity id"))?,
                    ));
                Ok(serde_json::json!({ "selected": id }))
            }
            None => {
                self.shell.selection_mut().clear();
                Ok(serde_json::json!({ "selected": null }))
            }
        }
    }

    fn shell_save_scene(&mut self, _params: &Value) -> EngineResult<Value> {
        let path = self.shell.save_scene()?;
        self.drain_shell_console();
        Ok(serde_json::json!({ "path": path }))
    }

    /// Open a scene from an arbitrary JSON file path.
    /// Reads the file, parses it as a scene, and replaces the current project's scene.
    fn shell_open_scene(&mut self, params: &Value) -> EngineResult<Value> {
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;
        let path = std::path::PathBuf::from(path_str);

        let text = std::fs::read_to_string(&path).map_err(|e| EngineError::Filesystem {
            path: path.clone(),
            source: e,
        })?;
        let new_scene = engine_ecs::Scene::from_json(&text)?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };
        project.scene = new_scene;
        project.scene_path = path.clone();
        project.scene_dirty = false;
        self.bump_scene_version();

        self.console.push(engine_editor::ConsoleEntry {
            timestamp: timestamp_now(),
            level: engine_editor::ConsoleLevel::Info,
            source: engine_editor::ConsoleSource {
                subsystem: "editor".to_string(),
                file: None,
                line: None,
            },
            message: format!("opened scene {}", path.display()),
        });

        Ok(serde_json::json!({
            "path": path.to_string_lossy(),
        }))
    }

    /// Save the scene to a specified path (Save As).
    fn shell_save_scene_as(&mut self, params: &Value) -> EngineResult<Value> {
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;
        let path = std::path::PathBuf::from(path_str);

        let display_path = self.shell.save_scene_as(&path)?;
        self.drain_shell_console();
        self.bump_scene_version();

        Ok(serde_json::json!({ "path": display_path }))
    }

    fn shell_undo(&mut self, _params: &Value) -> EngineResult<Value> {
        let ok = self.shell.undo_scene_command()?;
        self.drain_shell_console();
        self.bump_scene_version();
        Ok(serde_json::json!({ "applied": ok }))
    }

    fn shell_redo(&mut self, _params: &Value) -> EngineResult<Value> {
        let ok = self.shell.redo_scene_command()?;
        self.drain_shell_console();
        self.bump_scene_version();
        Ok(serde_json::json!({ "applied": ok }))
    }

    fn shell_close_project(&mut self, _params: &Value) -> EngineResult<Value> {
        self.stop_play_runtime();
        self.shell.close_project();
        self.durable_state = self.hub.durable_state();
        self.durable_state.last_open_project = None;
        self.hub = HubState::from_durable_state(self.durable_state.clone());
        self.persist_state();
        Ok(serde_json::json!({}))
    }

    // ── Scene CRUD ──

    fn shell_create_object(&mut self, params: &Value) -> EngineResult<Value> {
        let before = self.scene_snapshot()?;
        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        // Optional parent lookup
        let parent_entity = params
            .get("parent_id")
            .and_then(|v| v.as_str())
            .map(|id_str| {
                let pid = engine_core::EntityId::from_u128(
                    u128::from_str_radix(id_str, 16)
                        .map_err(|_| EngineError::config("invalid parent id"))?,
                );
                project
                    .scene
                    .find_by_id(pid)
                    .ok_or_else(|| EngineError::config("parent entity not found"))
            })
            .transpose()?;

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("New Object");

        let entity = project.scene.create_object(name)?;

        if let Some(parent) = parent_entity {
            project.scene.set_parent(entity, Some(parent))?;
        }

        project.scene_dirty = true;
        let after = self.scene_snapshot()?;
        self.shell
            .push_undo(UndoCommand::new("Create Object", "", before, after));
        self.bump_scene_version();

        let project = self.shell.project().unwrap();
        let obj = project.scene.object(entity).unwrap();
        let transform = project.scene.transforms().world(entity).unwrap_or_default();

        Ok(serde_json::json!({
            "id": format!("{:032x}", obj.id.as_u128()),
            "name": obj.name,
            "tag": obj.tag,
            "position": [
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
            ],
        }))
    }

    fn shell_delete_object(&mut self, params: &Value) -> EngineResult<Value> {
        let id_str = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'id'"))?;
        let entity_id = engine_core::EntityId::from_u128(
            u128::from_str_radix(id_str, 16)
                .map_err(|_| EngineError::config("invalid entity id"))?,
        );

        let before = self.scene_snapshot()?;
        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        let entity = project
            .scene
            .find_by_id(entity_id)
            .ok_or_else(|| EngineError::config("entity not found"))?;

        project.scene.destroy_deferred(entity)?;
        project.scene.process_deferred_destroy()?;
        project.scene_dirty = true;
        let after = self.scene_snapshot()?;
        self.shell
            .push_undo(UndoCommand::new("Delete Object", id_str, before, after));
        self.bump_scene_version();
        Ok(serde_json::json!({ "deleted": true }))
    }

    fn shell_duplicate_object(&mut self, params: &Value) -> EngineResult<Value> {
        let id_str = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'id'"))?;
        let entity_id = engine_core::EntityId::from_u128(
            u128::from_str_radix(id_str, 16)
                .map_err(|_| EngineError::config("invalid entity id"))?,
        );

        let before = self.scene_snapshot()?;
        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        let entity = project
            .scene
            .find_by_id(entity_id)
            .ok_or_else(|| EngineError::config("entity not found"))?;

        let new_entity = project.scene.clone_object(entity)?;
        project.scene_dirty = true;
        let after = self.scene_snapshot()?;
        self.shell
            .push_undo(UndoCommand::new("Duplicate Object", id_str, before, after));
        self.bump_scene_version();

        let project = self.shell.project().unwrap();
        let obj = project.scene.object(new_entity).unwrap();
        let transform = project
            .scene
            .transforms()
            .world(new_entity)
            .unwrap_or_default();

        Ok(serde_json::json!({
            "id": format!("{:032x}", obj.id.as_u128()),
            "name": obj.name,
            "tag": obj.tag,
            "position": [
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
            ],
        }))
    }

    fn shell_update_transform(&mut self, params: &Value) -> EngineResult<Value> {
        use engine_core::math::{Quat, Transform, Vec3};

        let id_str = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'id'"))?;
        let entity_id = engine_core::EntityId::from_u128(
            u128::from_str_radix(id_str, 16)
                .map_err(|_| EngineError::config("invalid entity id"))?,
        );

        let before = self.scene_snapshot()?;
        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };
        let entity = project
            .scene
            .find_by_id(entity_id)
            .ok_or_else(|| EngineError::config("entity not found"))?;

        // Read current transform as starting point
        let current = project.scene.transforms().local(entity).unwrap_or_default();

        let mut t = Transform {
            translation: current.translation,
            rotation: current.rotation,
            scale: current.scale,
        };

        if let Some(pos) = params.get("position").and_then(|v| v.as_array()) {
            let x = pos
                .get(0)
                .and_then(|v| v.as_f64())
                .unwrap_or(t.translation.x as f64) as f32;
            let y = pos
                .get(1)
                .and_then(|v| v.as_f64())
                .unwrap_or(t.translation.y as f64) as f32;
            let z = pos
                .get(2)
                .and_then(|v| v.as_f64())
                .unwrap_or(t.translation.z as f64) as f32;
            t.translation = Vec3::new(x, y, z);
        }
        if let Some(rot) = params.get("rotation").and_then(|v| v.as_array()) {
            let x = rot
                .get(0)
                .and_then(|v| v.as_f64())
                .unwrap_or(t.rotation.x as f64) as f32;
            let y = rot
                .get(1)
                .and_then(|v| v.as_f64())
                .unwrap_or(t.rotation.y as f64) as f32;
            let z = rot
                .get(2)
                .and_then(|v| v.as_f64())
                .unwrap_or(t.rotation.z as f64) as f32;
            let w = rot
                .get(3)
                .and_then(|v| v.as_f64())
                .unwrap_or(t.rotation.w as f64) as f32;
            t.rotation = Quat { x, y, z, w };
        }
        if let Some(scl) = params.get("scale").and_then(|v| v.as_array()) {
            let x = scl
                .get(0)
                .and_then(|v| v.as_f64())
                .unwrap_or(t.scale.x as f64) as f32;
            let y = scl
                .get(1)
                .and_then(|v| v.as_f64())
                .unwrap_or(t.scale.y as f64) as f32;
            let z = scl
                .get(2)
                .and_then(|v| v.as_f64())
                .unwrap_or(t.scale.z as f64) as f32;
            t.scale = Vec3::new(x, y, z);
        }

        project.scene.transforms_mut().set_local(entity, t);
        project.scene_dirty = true;
        let after = self.scene_snapshot()?;
        if before != after {
            self.shell
                .push_undo(UndoCommand::new("Update Transform", id_str, before, after));
        }
        self.bump_scene_version();
        Ok(serde_json::json!({ "updated": true }))
    }

    fn shell_add_component(&mut self, params: &Value) -> EngineResult<Value> {
        use engine_ecs::ComponentData;

        let id_str = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'id'"))?;
        let comp_type = params
            .get("component_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'component_type'"))?;
        let entity_id = engine_core::EntityId::from_u128(
            u128::from_str_radix(id_str, 16)
                .map_err(|_| EngineError::config("invalid entity id"))?,
        );

        let before = self.scene_snapshot()?;
        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };
        let entity = project
            .scene
            .find_by_id(entity_id)
            .ok_or_else(|| EngineError::config("entity not found"))?;

        let component = match comp_type {
            "Camera" => ComponentData::Camera(Default::default()),
            "Light" => ComponentData::Light(Default::default()),
            "MeshRenderer" => ComponentData::MeshRenderer(Default::default()),
            "Rigidbody" => ComponentData::Rigidbody(Default::default()),
            "Collider" => ComponentData::Collider(Default::default()),
            "AudioSource" => ComponentData::AudioSource(Default::default()),
            "Script" => ComponentData::Script(engine_ecs::ScriptComponentProxy {
                backend: "rhai".to_owned(),
                script: String::new(),
                state_json: None,
                pending_recovery: false,
            }),
            _ => {
                return Err(EngineError::config(format!(
                    "unknown component type: {comp_type}"
                )))
            }
        };

        project.scene.upsert_component(entity, component)?;
        project.scene_dirty = true;
        let after = self.scene_snapshot()?;
        self.shell
            .push_undo(UndoCommand::new("Add Component", id_str, before, after));
        self.bump_scene_version();
        Ok(serde_json::json!({ "added": comp_type }))
    }

    fn shell_remove_component(&mut self, params: &Value) -> EngineResult<Value> {
        let id_str = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'id'"))?;
        let comp_type = params
            .get("component_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'component_type'"))?;
        let entity_id = engine_core::EntityId::from_u128(
            u128::from_str_radix(id_str, 16)
                .map_err(|_| EngineError::config("invalid entity id"))?,
        );

        let before = self.scene_snapshot()?;
        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };
        let entity = project
            .scene
            .find_by_id(entity_id)
            .ok_or_else(|| EngineError::config("entity not found"))?;

        project.scene.remove_component(entity, comp_type)?;
        project.scene_dirty = true;
        let after = self.scene_snapshot()?;
        self.shell
            .push_undo(UndoCommand::new("Remove Component", id_str, before, after));
        self.bump_scene_version();
        Ok(serde_json::json!({ "removed": comp_type }))
    }

    // ── Play handlers ──

    fn play_start(&mut self, _params: &Value) -> EngineResult<Value> {
        self.start_play_runtime()?;
        Ok(serde_json::json!({
            "playing": true,
            "play_version": self.play_version,
        }))
    }

    fn play_stop(&mut self, _params: &Value) -> EngineResult<Value> {
        self.stop_play_runtime();
        Ok(serde_json::json!({ "playing": false }))
    }

    fn play_get_state(&mut self, _params: &Value) -> EngineResult<Value> {
        Ok(serde_json::json!({
            "playing": self.play_runtime.is_some(),
            "play_version": self.play_version,
        }))
    }

    // ── Helpers ──

    fn sync_durable_state(&mut self) {
        self.durable_state = self.hub.durable_state();
        if let Some(project) = self.shell.project() {
            self.durable_state.last_open_project = Some(project.root.clone());
        }
        self.persist_state();
    }

    fn persist_state(&self) {
        self.store.save(&self.durable_state).ok();
    }

    fn reopen_last_project_if_needed(&mut self) {
        if !self.hub.preferences().reopen_last_project {
            return;
        }
        let Some(path) = self.durable_state.last_open_project.clone() else {
            return;
        };
        if self.shell.open_project(&path).is_ok() {
            self.hub.mark_project_open(path);
            self.drain_shell_console();
        }
    }

    fn scene_snapshot(&self) -> EngineResult<String> {
        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };
        project.scene.to_json(project.name())
    }

    fn start_play_runtime(&mut self) -> EngineResult<()> {
        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };
        let config = EngineConfig::new(
            project.name().to_owned(),
            project.root.clone(),
            RuntimeProfile::RuntimeGame,
        );
        let mut runtime =
            headless_services_from_scene(config, project.root.clone(), &project.scene)?;
        runtime.load_project_assets(project.root.join(&project.manifest.asset_root))?;
        self.play_runtime = Some(runtime);
        self.play_last_frame = Some(Instant::now());
        self.play_version = self.play_version.wrapping_add(1);
        Ok(())
    }

    fn stop_play_runtime(&mut self) {
        self.play_runtime = None;
        self.play_last_frame = None;
        self.play_version = self.play_version.wrapping_add(1);
    }

    fn tick_play_runtime(&mut self) -> EngineResult<()> {
        if self.play_runtime.is_none() {
            self.start_play_runtime()?;
        }
        let now = Instant::now();
        let delta = self
            .play_last_frame
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or_else(|| Duration::from_secs_f32(1.0 / 60.0));
        self.play_last_frame = Some(now);
        if let Some(runtime) = self.play_runtime.as_mut() {
            runtime.tick_game_frame(delta.min(Duration::from_millis(100)), false)?;
            self.play_version = self.play_version.wrapping_add(1);
        }
        Ok(())
    }

    /// Forward console entries from the shell's console service to our shared one.
    fn drain_shell_console(&mut self) {
        for entry in self.shell.console().entries().iter() {
            self.console.push(entry.clone());
        }
    }
}

// ─── Thread-safe wrapper ─────────────────────────────────────────────────────

/// Thread-safe wrapper for `EditorHost`.
///
/// `EditorHost` contains non-`Send` closures (`CommandHandler`), but they are only
/// ever accessed while holding the mutex lock, making this safe.
pub struct EditorHostState {
    host: UnsafeCell<EditorHost>,
    lock: Mutex<()>,
}

// SAFETY: The Mutex ensures exclusive access; the non-Send closures are only
// reachable from the thread holding the lock.
unsafe impl Send for EditorHostState {}
unsafe impl Sync for EditorHostState {}

impl EditorHostState {
    pub fn new(host: EditorHost) -> Self {
        Self {
            host: UnsafeCell::new(host),
            lock: Mutex::new(()),
        }
    }

    /// Access the inner `EditorHost` under lock.
    pub fn with_host<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut EditorHost) -> R,
    {
        let _guard = self.lock.lock().expect("poisoned lock");
        // SAFETY: Mutex guarantees exclusive mutable access.
        f(unsafe { &mut *self.host.get() })
    }
}

// ─── Tauri commands ─────────────────────────────────────────────────────────

#[tauri::command]
fn rpc(state: State<'_, EditorHostState>, method: String, params: Value) -> Result<Value, String> {
    state.with_host(|host| host.handle(&method, &params).map_err(|e| e.to_string()))
}

/// Binary viewport readback — returns raw RGBA pixels as ArrayBuffer.
/// Response layout: [width: u32 LE][height: u32 LE][RGBA pixels...]
#[tauri::command]
fn viewport_readback_raw(
    state: State<'_, EditorHostState>,
    width: u32,
    height: u32,
    yaw: f64,
    pitch: f64,
    distance: f64,
    target_x: f64,
    target_y: f64,
    target_z: f64,
    last_version: Option<u64>,
    play_mode: bool,
) -> Result<Vec<u8>, String> {
    state.with_host(|host| {
        let params = serde_json::json!({
            "width": width,
            "height": height,
            "yaw": yaw,
            "pitch": pitch,
            "distance": distance,
            "target_x": target_x,
            "target_y": target_y,
            "target_z": target_z,
            "last_version": last_version,
            "play_mode": play_mode,
        });
        host.viewport_readback_raw(&params)
            .map_err(|e| e.to_string())
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Minimal base64 encoding (no external crate needed for this).
fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}", d.as_secs(), d.subsec_millis())
}

fn dirs_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".config"))
            })
            .map(|p| p.join("aster"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join("Library/Application Support/aster"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|h| PathBuf::from(h).join("aster"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Some(PathBuf::from(".aster-config"))
    }
}

// ─── App entry point ─────────────────────────────────────────────────────────

#[tauri::command]
fn open_game_view(app: tauri::AppHandle) -> Result<(), String> {
    // Check if Game View window already exists
    if let Some(window) = app.get_webview_window("game-view") {
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(
        &app,
        "game-view",
        tauri::WebviewUrl::App("index.html#/game-view".into()),
    )
    .title("Game View")
    .inner_size(1280.0, 720.0)
    .min_inner_size(640.0, 360.0)
    .background_color(Color(24, 24, 24, 255))
    .decorations(true)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn is_cancelled_portal_request(error: &ashpd::Error) -> bool {
    matches!(
        error,
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)
    )
}

#[cfg(target_os = "linux")]
#[tauri::command]
async fn select_project_location() -> Result<Option<String>, String> {
    use ashpd::desktop::file_chooser::SelectedFiles;

    let request = match SelectedFiles::open_file()
        .title("Select Project Location")
        .accept_label("Open")
        .modal(true)
        .multiple(false)
        .directory(true)
        .send()
        .await
    {
        Ok(request) => request,
        Err(error) if is_cancelled_portal_request(&error) => return Ok(None),
        Err(error) => return Err(format!("failed to start portal file chooser: {error}")),
    };

    let files = match request.response() {
        Ok(files) => files,
        Err(error) if is_cancelled_portal_request(&error) => return Ok(None),
        Err(error) => return Err(format!("portal file chooser failed: {error}")),
    };

    let Some(uri) = files.uris().first() else {
        return Ok(None);
    };
    let path = uri
        .to_file_path()
        .map_err(|_| format!("portal returned a non-file URI: {uri}"))?;

    Ok(Some(path.to_string_lossy().into_owned()))
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
async fn select_project_location() -> Result<Option<String>, String> {
    Err("XDG Desktop Portal file chooser is only available on Linux".to_owned())
}

fn apply_desktop_window_adaptations(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let desktop = DesktopIntegration::detect();
    if let Some(window) = app.get_webview_window("main") {
        window.set_background_color(Some(Color(24, 24, 24, 255)))?;
        window.set_decorations(desktop.prefers_native_chrome())?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn apply_pre_gtk_desktop_environment() {
    let has_wayland_display = std::env::var("WAYLAND_DISPLAY").is_ok();
    let backend_already_selected = std::env::var("GDK_BACKEND").is_ok();

    if has_wayland_display && !backend_already_selected {
        // Ask GTK/WebKit/Tao to try native Wayland first, while keeping X11 as a
        // fallback for systems where the Wayland backend is unavailable at runtime.
        std::env::set_var("GDK_BACKEND", "wayland,x11");
    }
}

#[cfg(not(target_os = "linux"))]
fn apply_pre_gtk_desktop_environment() {}

pub fn run() {
    apply_pre_gtk_desktop_environment();

    let config_dir = dirs_config_dir().unwrap_or_else(|| PathBuf::from("."));
    let store_path = config_dir.join("aster-editor-state.toml");
    let store = FileEditorStore::new(&store_path);

    let host = match EditorHost::new(store) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("FATAL: failed to initialize editor host: {e}");
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(EditorHostState::new(host))
        .invoke_handler(tauri::generate_handler![
            rpc,
            open_game_view,
            select_project_location,
            viewport_readback_raw
        ])
        .setup(|app| {
            apply_desktop_window_adaptations(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::DesktopEnvironment;

    #[test]
    fn kde_uses_native_chrome_when_native_wayland_is_preferred() {
        assert!(DesktopEnvironment::Kde.prefers_native_chrome_for_backend(true));
    }

    #[test]
    fn kde_keeps_native_chrome_when_using_x11_backend() {
        assert!(DesktopEnvironment::Kde.prefers_native_chrome_for_backend(false));
    }
}
