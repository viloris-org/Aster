use std::{
    cell::RefCell,
    env,
    path::{Component, Path, PathBuf},
    rc::Rc,
};

use engine_core::{
    EngineError, EngineResult,
    math::{Transform, Vec3},
};
use engine_ecs::Entity;
use engine_editor::{ConsoleEntry, ConsoleLevel, ConsoleSource, EditorPreferences, EditorShell};
use engine_render::{ImageFormat, RenderCamera, RenderProjection, RenderWorld};
use engine_render_wgpu::{WgpuOffscreenConfig, WgpuRenderDevice};
use slint::{
    ComponentHandle, Image, ModelRc, PhysicalSize, Rgba8Pixel, SharedPixelBuffer, SharedString,
    VecModel, Weak,
};

slint::include_modules!();

#[path = "../../editor/src-tauri/src/quest.rs"]
mod quest;

#[derive(Clone, Debug)]
struct UiRow {
    label: String,
    detail: String,
    selected: bool,
}

#[derive(Clone, Debug)]
struct UiQuestSummary {
    title: String,
    status: String,
    group: String,
    badge: String,
    detail: String,
    selected: bool,
}

#[derive(Clone, Debug)]
struct UiMetric {
    label: String,
    value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Selection {
    None,
    Entity(Entity),
    Asset(usize),
    Console(usize),
}

impl Default for Selection {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Copy, Debug)]
struct EditorCamera {
    target: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
}

impl Default for EditorCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 6.0,
            yaw: -0.5,
            pitch: 0.3,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SceneDrag {
    mode: SceneDragMode,
    last_x: f32,
    last_y: f32,
}

#[derive(Clone, Copy, Debug)]
enum SceneDragMode {
    Orbit,
    Pan,
}

#[derive(Debug)]
struct SlintEditorState {
    shell: EditorShell,
    renderer: Option<WgpuRenderDevice>,
    scene_image: Image,
    scene_status: String,
    editor_camera: EditorCamera,
    scene_drag: Option<SceneDrag>,
    selection: Selection,
    active_screen: String,
    active_tool: String,
    active_rail: String,
    console_tab: String,
    quest_store: quest::QuestStore,
    quest_records: Vec<quest::QuestRecord>,
    selected_quest_id: Option<String>,
    selected_quest: Option<quest::QuestDetail>,
    quest_tab: String,
    play_mode: bool,
}

impl SlintEditorState {
    fn new(shell: EditorShell, quest_store: quest::QuestStore) -> Self {
        let active_screen = if shell.project().is_some() {
            "editor".to_owned()
        } else {
            "hub".to_owned()
        };
        let mut state = Self {
            shell,
            renderer: None,
            scene_image: placeholder_scene_image(960, 540),
            scene_status: "Scene View waiting for project".to_owned(),
            editor_camera: EditorCamera::default(),
            scene_drag: None,
            selection: Selection::None,
            active_screen,
            active_tool: "move".to_owned(),
            active_rail: "scene".to_owned(),
            console_tab: "problems".to_owned(),
            quest_store,
            quest_records: Vec::new(),
            selected_quest_id: None,
            selected_quest: None,
            quest_tab: "overview".to_owned(),
            play_mode: false,
        };
        state.refresh_quests();
        state
    }

    fn push_info(&mut self, message: impl Into<String>) {
        self.push_console(ConsoleLevel::Info, message);
    }

    fn push_warn(&mut self, message: impl Into<String>) {
        self.push_console(ConsoleLevel::Warn, message);
    }

    fn push_error(&mut self, message: impl Into<String>) {
        self.push_console(ConsoleLevel::Error, message);
    }

    fn push_console(&mut self, level: ConsoleLevel, message: impl Into<String>) {
        self.shell.console_mut().push(ConsoleEntry {
            timestamp: "now".to_owned(),
            level,
            source: ConsoleSource {
                subsystem: "slint-editor".to_owned(),
                file: None,
                line: None,
            },
            message: message.into(),
        });
    }

    fn refresh_scene_view(&mut self) {
        match self.render_scene_view(960, 540) {
            Ok((width, height)) => {
                self.scene_status = format!("WGPU readback {width}x{height}");
            }
            Err(error) => {
                self.scene_status = format!("Scene View unavailable: {error}");
                self.push_warn(self.scene_status.clone());
            }
        }
    }

    fn render_scene_view(&mut self, width: u32, height: u32) -> EngineResult<(u32, u32)> {
        let Some(project) = self.shell.project() else {
            self.scene_image = placeholder_scene_image(width, height);
            return Ok((width, height));
        };

        let mut world = RenderWorld::extract(&project.scene);
        install_editor_camera(&mut world, self.editor_camera);

        if self.renderer.is_none() {
            self.renderer = Some(WgpuRenderDevice::new_offscreen(WgpuOffscreenConfig {
                width: width.max(1),
                height: height.max(1),
                format: ImageFormat::Rgba8Srgb,
            })?);
        }

        let renderer = self.renderer.as_mut().unwrap();
        let (current_width, current_height) = renderer.default_target_size();
        if current_width != width || current_height != height {
            renderer.resize_default_target(width.max(1), height.max(1))?;
        }

        renderer.render_world_offscreen(&world)?;
        let (readback_width, readback_height, rgba) = renderer.readback_default_target()?;
        self.scene_image = rgba_image(readback_width, readback_height, &rgba);
        Ok((readback_width, readback_height))
    }

    fn save_scene(&mut self) {
        match self.shell.save_scene() {
            Ok(path) => self.push_info(format!("saved scene {path}")),
            Err(error) => self.push_error(format!("save failed: {error}")),
        }
    }

    fn undo(&mut self) {
        match self.shell.undo_scene_command() {
            Ok(true) => self.push_info("undid latest scene command"),
            Ok(false) => self.push_warn("nothing to undo"),
            Err(error) => self.push_error(format!("undo failed: {error}")),
        }
    }

    fn redo(&mut self) {
        match self.shell.redo_scene_command() {
            Ok(true) => self.push_info("redid latest scene command"),
            Ok(false) => self.push_warn("nothing to redo"),
            Err(error) => self.push_error(format!("redo failed: {error}")),
        }
    }

    fn toggle_play(&mut self) {
        self.play_mode = !self.play_mode;
        if self.play_mode {
            self.push_info("opened Game View window");
        } else {
            self.push_info("closed Game View window");
        }
    }

    fn scene_pointer_event(&mut self, kind: &str, button: &str, x: f32, y: f32) {
        match kind {
            "down" => {
                self.scene_drag = match button {
                    "left" | "right" => Some(SceneDrag {
                        mode: SceneDragMode::Orbit,
                        last_x: x,
                        last_y: y,
                    }),
                    "middle" => Some(SceneDrag {
                        mode: SceneDragMode::Pan,
                        last_x: x,
                        last_y: y,
                    }),
                    _ => None,
                };
            }
            "up" | "cancel" => {
                self.scene_drag = None;
            }
            "move" => {
                let Some(mut drag) = self.scene_drag else {
                    return;
                };
                let dx = x - drag.last_x;
                let dy = y - drag.last_y;
                match drag.mode {
                    SceneDragMode::Orbit => {
                        self.editor_camera.yaw -= dx * 0.005;
                        self.editor_camera.pitch =
                            (self.editor_camera.pitch + dy * 0.005).clamp(-1.5, 1.5);
                    }
                    SceneDragMode::Pan => {
                        let d = self.editor_camera.distance * 0.002;
                        let yaw = self.editor_camera.yaw;
                        self.editor_camera.target.x += (-dx * yaw.cos() - dy * yaw.sin() * 0.5) * d;
                        self.editor_camera.target.y += dy * d * 0.5;
                        self.editor_camera.target.z += (dx * yaw.sin() - dy * yaw.cos() * 0.5) * d;
                    }
                }
                drag.last_x = x;
                drag.last_y = y;
                self.scene_drag = Some(drag);
            }
            _ => {}
        }
    }

    fn scene_scrolled(&mut self, delta_y: f32) {
        self.editor_camera.distance =
            (self.editor_camera.distance - delta_y * 0.01).clamp(0.5, 100.0);
    }

    fn select_tool(&mut self, tool: &str) {
        self.active_tool = tool.to_owned();
        self.push_info(format!("active tool: {tool}"));
    }

    fn create_object(&mut self) {
        match self.try_create_object() {
            Ok(name) => self.push_info(format!("created object {name}")),
            Err(error) => self.push_error(format!("create object failed: {error}")),
        }
    }

    fn delete_selected(&mut self) {
        match self.try_delete_selected() {
            Ok(name) => self.push_info(format!("deleted object {name}")),
            Err(error) => self.push_error(format!("delete failed: {error}")),
        }
    }

    fn rename_selected(&mut self) {
        match self.try_rename_selected() {
            Ok(name) => self.push_info(format!("renamed object to {name}")),
            Err(error) => self.push_error(format!("rename failed: {error}")),
        }
    }

    fn nudge_selected(&mut self, direction: &str) {
        match self.try_nudge_selected(direction) {
            Ok(label) => self.push_info(format!("moved selection {label}")),
            Err(error) => self.push_error(format!("move failed: {error}")),
        }
    }

    fn add_component(&mut self, component_type: &str) {
        match self.try_add_component(component_type) {
            Ok(component) => self.push_info(format!("added {component} component")),
            Err(error) => self.push_error(format!("add component failed: {error}")),
        }
    }

    fn remove_component(&mut self, component_type: &str) {
        match self.try_remove_component(component_type) {
            Ok(true) => self.push_info(format!("removed {component_type} component")),
            Ok(false) => self.push_warn(format!("selection has no {component_type} component")),
            Err(error) => self.push_error(format!("remove component failed: {error}")),
        }
    }

    fn create_asset(&mut self, kind: &str) {
        match self.try_create_asset(kind) {
            Ok(path) => self.push_info(format!("created {kind} asset {path}")),
            Err(error) => self.push_error(format!("create asset failed: {error}")),
        }
    }

    fn rename_selected_asset(&mut self) {
        match self.try_rename_selected_asset() {
            Ok(path) => self.push_info(format!("renamed asset to {path}")),
            Err(error) => self.push_error(format!("asset rename failed: {error}")),
        }
    }

    fn delete_selected_asset(&mut self) {
        match self.try_delete_selected_asset() {
            Ok(path) => self.push_info(format!("deleted asset {path}")),
            Err(error) => self.push_error(format!("asset delete failed: {error}")),
        }
    }

    fn reimport_selected_asset(&mut self) {
        match self.try_reimport_selected_asset() {
            Ok(path) => self.push_info(format!("reimported asset {path}")),
            Err(error) => self.push_error(format!("asset reimport failed: {error}")),
        }
    }

    fn reimport_all_assets(&mut self) {
        match self.try_reimport_all_assets() {
            Ok(count) => self.push_info(format!("reimported all assets ({count} found)")),
            Err(error) => self.push_error(format!("asset reimport failed: {error}")),
        }
    }

    fn select_screen(&mut self, screen: &str) {
        self.active_screen = match screen {
            "hub" | "editor" | "quest" => screen.to_owned(),
            _ => {
                self.push_warn(format!("unknown screen: {screen}"));
                return;
            }
        };
        if screen == "quest" {
            self.refresh_quests();
        }
        self.push_info(format!("opened {screen} screen"));
    }

    fn select_rail(&mut self, rail: &str) {
        self.active_rail = rail.to_owned();
        self.console_tab = match rail {
            "assets" => "assets".to_owned(),
            "console" => "console".to_owned(),
            _ => self.console_tab.clone(),
        };
        self.push_info(format!("opened {rail} rail"));
    }

    fn refresh_quests(&mut self) {
        match self.quest_store.list() {
            Ok(records) => {
                self.quest_records = records;
                if self.selected_quest_id.is_none() {
                    self.selected_quest_id =
                        self.quest_records.first().map(|record| record.id.clone());
                }
                self.refresh_selected_quest();
            }
            Err(error) => {
                self.quest_records.clear();
                self.selected_quest = None;
                self.push_error(format!("quest list failed: {error}"));
            }
        }
    }

    fn refresh_selected_quest(&mut self) {
        let Some(id) = self.selected_quest_id.clone() else {
            self.selected_quest = None;
            return;
        };
        match self.quest_store.get(&id) {
            Ok(detail) => self.selected_quest = Some(detail),
            Err(error) => {
                self.selected_quest_id = None;
                self.selected_quest = None;
                self.push_warn(format!("quest unavailable: {error}"));
            }
        }
    }

    fn select_quest(&mut self, index: usize) {
        let Some(record) = self.quest_records.get(index) else {
            self.push_warn("quest row no longer exists");
            return;
        };
        self.selected_quest_id = Some(record.id.clone());
        self.refresh_selected_quest();
        if let Some(detail) = &self.selected_quest {
            self.push_info(format!("selected quest {}", detail.record.title));
        }
    }

    fn select_quest_tab(&mut self, tab: &str) {
        self.quest_tab = tab.to_owned();
    }

    fn run_quest_action(&mut self, action: &str) {
        if action == "refresh" {
            self.refresh_quests();
            return;
        }

        let Some(id) = self.selected_quest_id.clone() else {
            self.push_warn("select a quest first");
            return;
        };

        let result = match action {
            "approve" => self
                .quest_store
                .transition(&id, quest::QuestStatus::Specified),
            "execute" => self.quest_store.mock_execute(&id),
            "cancel" => self.quest_store.cancel(&id, "Canceled from Slint editor"),
            "reopen" => self.quest_store.reopen(&id, "Reopened from Slint editor"),
            "branch" => self.quest_store.branch(&id, None),
            "continue" => self
                .quest_store
                .continue_quest(&id, "Continue Quest from Slint editor"),
            "request_revision" => {
                let decision = self.quest_store.record_decision(
                    &id,
                    "revise",
                    "Requested Quest revision from Slint editor",
                    Vec::new(),
                );
                match decision {
                    Ok(_) => self
                        .quest_store
                        .transition(&id, quest::QuestStatus::Specified),
                    Err(error) => Err(error),
                }
            }
            "export" => self.export_selected_quest(&id),
            "delete" => match self.quest_store.delete(&id) {
                Ok(()) => {
                    self.selected_quest_id = None;
                    self.selected_quest = None;
                    self.refresh_quests();
                    self.push_info("quest deleted");
                    return;
                }
                Err(error) => Err(error),
            },
            "reject" => {
                let decision = self.quest_store.record_decision(
                    &id,
                    "reject",
                    "Rejected reviewed Quest result from Slint editor",
                    Vec::new(),
                );
                match decision {
                    Ok(_) => self
                        .quest_store
                        .transition(&id, quest::QuestStatus::Archived),
                    Err(error) => Err(error),
                }
            }
            "apply_all" | "discard_all" | "open_editor" | "new" => {
                self.push_warn(format!(
                    "Quest action `{action}` requires the shared Tauri QuestHost before Slint can be 1:1"
                ));
                return;
            }
            other => {
                self.push_warn(format!("unknown quest action: {other}"));
                return;
            }
        };

        match result {
            Ok(detail) => {
                self.selected_quest_id = Some(detail.record.id.clone());
                self.selected_quest = Some(detail);
                self.refresh_quests();
                self.push_info(format!("quest action completed: {action}"));
            }
            Err(error) => self.push_error(format!("quest action failed: {error}")),
        }
    }

    fn export_selected_quest(&mut self, id: &str) -> EngineResult<quest::QuestDetail> {
        let detail = self.quest_store.get(id)?;
        let quest_dir = self.quest_store.quest_path(id)?;
        let export_root = detail
            .record
            .project
            .path
            .join(".aster")
            .join("quests")
            .join(id);
        std::fs::create_dir_all(&export_root).map_err(|source| EngineError::Filesystem {
            path: export_root.clone(),
            source,
        })?;
        for file_name in ["quest.json", "intent.md", "spec.md", "events.jsonl"] {
            let source = quest_dir.join(file_name);
            if source.is_file() {
                std::fs::copy(&source, export_root.join(file_name)).map_err(|source| {
                    EngineError::Filesystem {
                        path: export_root.join(file_name),
                        source,
                    }
                })?;
            }
        }
        let relative_export = format!(".aster/quests/{id}");
        self.quest_store.record_decision(
            id,
            "export",
            &format!("Exported Quest artifacts to {relative_export}"),
            vec![relative_export],
        )
    }

    fn select_hierarchy(&mut self, index: usize) {
        let Some(project) = self.shell.project() else {
            self.push_warn("no project is open");
            return;
        };
        let Some((entity, object_id, object_name)) = project
            .scene
            .objects()
            .get(index)
            .map(|(entity, object)| (*entity, object.id, object.name.clone()))
        else {
            self.push_warn("scene row no longer exists");
            return;
        };
        self.shell.select_entity_id(object_id);
        self.selection = Selection::Entity(entity);
        self.active_rail = "scene".to_owned();
        self.push_info(format!("selected entity {object_name}"));
    }

    fn select_asset(&mut self, index: usize) {
        let Some(project) = self.shell.project() else {
            self.push_warn("no project is open");
            return;
        };
        let Some(asset) = project.sorted_assets().get(index).copied() else {
            self.push_warn("asset row no longer exists");
            return;
        };
        self.selection = Selection::Asset(index);
        self.active_rail = "assets".to_owned();
        self.console_tab = "assets".to_owned();
        self.push_info(format!("selected asset {}", asset.source_path.display()));
    }

    fn select_script(&mut self, index: usize) {
        let Some(row) = self.script_rows().get(index).cloned() else {
            self.push_warn("script row no longer exists");
            return;
        };

        if let Some(project) = self.shell.project()
            && let Some(asset_index) = project
                .sorted_assets()
                .iter()
                .position(|asset| asset.source_path.to_string_lossy() == row.label)
        {
            self.selection = Selection::Asset(asset_index);
        }
        self.active_rail = "scripts".to_owned();
        self.console_tab = "assets".to_owned();
        self.push_info(format!("selected script {}", row.label));
    }

    fn select_console_tab(&mut self, tab: &str) {
        self.console_tab = tab.to_owned();
        self.active_rail = "console".to_owned();
    }

    fn open_console_row(&mut self, index: usize) {
        self.selection = Selection::Console(index);
        let label = self.console_rows().get(index).map(|row| row.label.clone());
        if let Some(label) = label {
            self.push_info(format!("opened console row: {label}"));
        }
    }

    fn selected_name_and_kind(&self) -> (String, String) {
        match self.selection {
            Selection::Entity(entity) => self
                .shell
                .project()
                .and_then(|project| project.scene.object(entity))
                .map(|object| (object.name.clone(), "Entity".to_owned()))
                .unwrap_or_else(|| (String::new(), "Entity".to_owned())),
            Selection::Asset(index) => self
                .shell
                .project()
                .and_then(|project| {
                    project.sorted_assets().get(index).map(|asset| {
                        (
                            asset
                                .source_path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("Asset")
                                .to_owned(),
                            "Asset".to_owned(),
                        )
                    })
                })
                .unwrap_or_else(|| (String::new(), "Asset".to_owned())),
            Selection::Console(_) => ("Console row".to_owned(), "Diagnostic".to_owned()),
            Selection::None => (String::new(), "Entity".to_owned()),
        }
    }

    fn selected_entity(&self) -> EngineResult<Entity> {
        self.shell
            .selected_entity_id()
            .and_then(|id| {
                self.shell
                    .project()
                    .and_then(|project| project.scene.find_by_id(id))
            })
            .ok_or_else(|| engine_core::EngineError::config("select an entity first"))
    }

    fn try_create_object(&mut self) -> EngineResult<String> {
        let name = {
            let Some(project) = self.shell.project() else {
                return Err(engine_core::EngineError::config("no project is open"));
            };
            let name = format!("New Object {}", project.scene.object_count() + 1);
            name
        };
        let object_id = self.shell.create_scene_object(name)?;
        if let Some(entity) = self
            .shell
            .project()
            .and_then(|project| project.scene.find_by_id(object_id))
        {
            self.selection = Selection::Entity(entity);
        }
        self.active_rail = "scene".to_owned();
        Ok(self
            .shell
            .project()
            .and_then(|project| {
                project
                    .scene
                    .find_by_id(object_id)
                    .and_then(|entity| project.scene.object(entity))
            })
            .map(|object| object.name.clone())
            .unwrap_or_else(|| "New Object".to_owned()))
    }

    fn try_delete_selected(&mut self) -> EngineResult<String> {
        let name = self.shell.delete_selected_scene_object()?;
        self.selection = Selection::None;
        Ok(name)
    }

    fn try_rename_selected(&mut self) -> EngineResult<String> {
        let entity = self.selected_entity()?;
        let name = {
            let Some(project) = self.shell.project() else {
                return Err(engine_core::EngineError::config("no project is open"));
            };
            let count = project.scene.object_count();
            let object = project.scene.object(entity).ok_or_else(|| {
                engine_core::EngineError::config("selected entity no longer exists")
            })?;
            format!("{} {}", object.name.trim_end_matches(" *"), count)
        };
        self.shell.rename_selected_scene_object(&name)?;
        Ok(name)
    }

    fn try_nudge_selected(&mut self, direction: &str) -> EngineResult<&'static str> {
        use engine_core::math::Vec3;

        let (delta, label) = match direction {
            "x-" => (Vec3::new(-0.25, 0.0, 0.0), "-X"),
            "x+" => (Vec3::new(0.25, 0.0, 0.0), "+X"),
            "y-" => (Vec3::new(0.0, -0.25, 0.0), "-Y"),
            "y+" => (Vec3::new(0.0, 0.25, 0.0), "+Y"),
            "z-" => (Vec3::new(0.0, 0.0, -0.25), "-Z"),
            "z+" => (Vec3::new(0.0, 0.0, 0.25), "+Z"),
            _ => return Err(engine_core::EngineError::config("unknown nudge direction")),
        };
        self.shell.nudge_selected_scene_object(delta, label)?;
        Ok(label)
    }

    fn try_add_component(&mut self, component_type: &str) -> EngineResult<String> {
        let component = match component_type {
            "Camera" => engine_ecs::ComponentData::Camera(Default::default()),
            "Light" => engine_ecs::ComponentData::Light(Default::default()),
            "MeshRenderer" => engine_ecs::ComponentData::MeshRenderer(Default::default()),
            other => {
                return Err(engine_core::EngineError::config(format!(
                    "unsupported component: {other}"
                )));
            }
        };
        self.shell
            .add_component_to_selected_scene_object(component)
            .map(str::to_owned)
    }

    fn try_remove_component(&mut self, component_type: &str) -> EngineResult<bool> {
        self.shell
            .remove_component_from_selected_scene_object(component_type)
    }

    fn selected_asset_path(&self) -> EngineResult<String> {
        let Selection::Asset(index) = self.selection else {
            return Err(EngineError::config("select an asset first"));
        };
        self.shell
            .project()
            .and_then(|project| {
                project
                    .sorted_assets()
                    .get(index)
                    .map(|asset| asset.source_path.to_string_lossy().to_string())
            })
            .ok_or_else(|| EngineError::config("selected asset no longer exists"))
    }

    fn try_create_asset(&mut self, kind: &str) -> EngineResult<String> {
        let (directory, stem, extension, content) = {
            let Some(project) = self.shell.project() else {
                return Err(EngineError::config("no project is open"));
            };
            let count = project.assets.len() + 1;
            match kind {
                "script" => {
                    let name = format!("new_script_{count}");
                    ("scripts", name.clone(), "varg", varg_script_template(&name))
                }
                "material" => {
                    let name = format!("new_material_{count}");
                    (
                        "materials",
                        name.clone(),
                        "vasset",
                        varg_material_template(&name),
                    )
                }
                "prefab" => {
                    let name = format!("new_prefab_{count}");
                    (
                        "prefabs",
                        name.clone(),
                        "vscene",
                        varg_prefab_template(&name),
                    )
                }
                "scene" => {
                    let name = format!("new_scene_{count}");
                    ("scenes", name.clone(), "vscene", varg_scene_template(&name))
                }
                other => {
                    return Err(EngineError::config(format!(
                        "unsupported asset kind: {other}"
                    )));
                }
            }
        };
        validate_file_name(&stem)?;
        let asset_path = format!("{directory}/{stem}.{extension}");
        let project = self
            .shell
            .project_mut()
            .ok_or_else(|| EngineError::config("no project is open"))?;
        let (_asset_path, full_path) = write_project_asset(project, &asset_path, &content)?;
        project.rescan_assets()?;
        self.active_rail = "assets".to_owned();
        self.console_tab = "assets".to_owned();
        select_asset_by_path(project, &asset_path)
            .map(|index| self.selection = Selection::Asset(index));
        Ok(full_path
            .strip_prefix(project.root.join(&project.manifest.asset_root))
            .unwrap_or(&full_path)
            .to_string_lossy()
            .to_string())
    }

    fn try_rename_selected_asset(&mut self) -> EngineResult<String> {
        let old_path_str = self.selected_asset_path()?;
        let project = self
            .shell
            .project_mut()
            .ok_or_else(|| EngineError::config("no project is open"))?;
        let asset_root = project.root.join(&project.manifest.asset_root);
        let old_path = resolve_existing_relative_path(&asset_root, &old_path_str)?;
        let parent = old_path
            .parent()
            .ok_or_else(|| EngineError::config("cannot rename root directory"))?;
        let stem = old_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("asset");
        let extension = old_path
            .extension()
            .map(|extension| format!(".{}", extension.to_string_lossy()))
            .unwrap_or_default();
        let new_stem = unique_renamed_stem(parent, stem, &extension);
        validate_file_name(&new_stem)?;
        let new_path = parent.join(format!("{new_stem}{extension}"));
        let canonical_asset_root =
            asset_root
                .canonicalize()
                .map_err(|source| EngineError::Filesystem {
                    path: asset_root.clone(),
                    source,
                })?;
        if !new_path.starts_with(&canonical_asset_root) {
            return Err(EngineError::config("path is outside the project"));
        }

        std::fs::rename(&old_path, &new_path).map_err(|source| EngineError::Filesystem {
            path: old_path.clone(),
            source,
        })?;
        let old_meta = asset_meta_path_for_source(&old_path);
        if old_meta.exists() {
            let new_meta = asset_meta_path_for_source(&new_path);
            let _ = std::fs::rename(old_meta, new_meta);
        }
        project.rescan_assets()?;
        let relative = new_path
            .strip_prefix(&canonical_asset_root)
            .unwrap_or(&new_path)
            .to_string_lossy()
            .to_string();
        select_asset_by_path(project, &relative)
            .map(|index| self.selection = Selection::Asset(index));
        Ok(relative)
    }

    fn try_delete_selected_asset(&mut self) -> EngineResult<String> {
        let path_str = self.selected_asset_path()?;
        let project = self
            .shell
            .project_mut()
            .ok_or_else(|| EngineError::config("no project is open"))?;
        let asset_root = project.root.join(&project.manifest.asset_root);
        let path = resolve_existing_relative_path(&asset_root, &path_str)?;
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|source| EngineError::Filesystem {
                path: path.clone(),
                source,
            })?;
        } else {
            std::fs::remove_file(&path).map_err(|source| EngineError::Filesystem {
                path: path.clone(),
                source,
            })?;
            let meta_path = asset_meta_path_for_source(&path);
            if meta_path.exists() {
                let _ = std::fs::remove_file(meta_path);
            }
        }
        project.rescan_assets()?;
        self.selection = Selection::None;
        Ok(path_str)
    }

    fn try_reimport_selected_asset(&mut self) -> EngineResult<String> {
        let path_str = self.selected_asset_path()?;
        let project = self
            .shell
            .project_mut()
            .ok_or_else(|| EngineError::config("no project is open"))?;
        let asset_root = project.root.join(&project.manifest.asset_root);
        let path = resolve_existing_relative_path(&asset_root, &path_str)?;
        let meta_path = asset_meta_path_for_source(&path);
        if meta_path.exists() {
            let _ = std::fs::remove_file(meta_path);
        }
        project.rescan_assets()?;
        select_asset_by_path(project, &path_str)
            .map(|index| self.selection = Selection::Asset(index));
        Ok(path_str)
    }

    fn try_reimport_all_assets(&mut self) -> EngineResult<usize> {
        let project = self
            .shell
            .project_mut()
            .ok_or_else(|| EngineError::config("no project is open"))?;
        let asset_root = project.root.join(&project.manifest.asset_root);
        let mut stack = vec![asset_root.clone()];
        while let Some(path) = stack.pop() {
            let entries = std::fs::read_dir(&path).map_err(|source| EngineError::Filesystem {
                path: path.clone(),
                source,
            })?;
            for entry in entries {
                let entry = entry.map_err(|source| EngineError::Filesystem {
                    path: path.clone(),
                    source,
                })?;
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    stack.push(entry_path);
                } else if entry_path.extension().is_some_and(|ext| ext == "meta") {
                    let _ = std::fs::remove_file(entry_path);
                }
            }
        }
        project.rescan_assets()?;
        Ok(project.assets.len())
    }

    fn hierarchy_rows(&self) -> Vec<UiRow> {
        let Some(project) = self.shell.project() else {
            return vec![UiRow::plain("No scene loaded", "")];
        };

        non_empty_rows(
            project
                .scene
                .objects()
                .into_iter()
                .map(|(entity, object)| UiRow {
                    label: object.name.clone(),
                    detail: object.tag.clone(),
                    selected: self.selection == Selection::Entity(entity),
                })
                .collect(),
            "Scene is empty",
        )
    }

    fn asset_rows(&self) -> Vec<UiRow> {
        let Some(project) = self.shell.project() else {
            return vec![UiRow::plain("No asset root loaded", "")];
        };

        non_empty_rows(
            project
                .sorted_assets()
                .into_iter()
                .enumerate()
                .map(|(index, asset)| UiRow {
                    label: asset.source_path.display().to_string(),
                    detail: format!("{:?}", asset.kind),
                    selected: self.selection == Selection::Asset(index),
                })
                .collect(),
            "No imported assets",
        )
    }

    fn script_rows(&self) -> Vec<UiRow> {
        let Some(project) = self.shell.project() else {
            return vec![UiRow::plain("No scripts loaded", "")];
        };

        non_empty_rows(
            project
                .sorted_assets()
                .into_iter()
                .enumerate()
                .filter_map(|(index, asset)| {
                    let path = asset.source_path.to_string_lossy().to_string();
                    let extension = Path::new(path.as_str())
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .unwrap_or_default();
                    let language = match extension {
                        "varg" | "vscene" | "vasset" => "Varg",
                        "rhai" => "Rhai",
                        "py" => "Python",
                        _ => return None,
                    };
                    Some(UiRow {
                        label: path,
                        detail: format!("{language} · {:?}", asset.kind),
                        selected: self.selection == Selection::Asset(index),
                    })
                })
                .collect(),
            "No script or text behavior assets",
        )
    }

    fn build_rows(&self) -> Vec<UiRow> {
        let Some(project) = self.shell.project() else {
            return vec![UiRow::plain("No project open", "open with --project <path>")];
        };

        vec![
            UiRow::plain("Validate project", "ready"),
            UiRow::plain("Export runtime", "planned · runtime-min"),
            UiRow::plain("Bundle assets", &format!("{} assets", project.assets.len())),
            UiRow::plain("Create installer", "planned · linux/windows/macos"),
            UiRow::plain(
                "Output",
                &format!("exports/{}/linux-x64/debug", project.name()),
            ),
        ]
    }

    fn diagnostic_rows(&self) -> Vec<UiRow> {
        non_empty_rows(
            self.shell
                .console()
                .entries()
                .iter()
                .rev()
                .take(24)
                .enumerate()
                .map(|(index, entry)| UiRow {
                    label: format!("{:?}: {}", entry.level, entry.message),
                    detail: entry
                        .source
                        .file
                        .as_ref()
                        .map(|file| {
                            let file = file.to_string_lossy();
                            entry
                                .source
                                .line
                                .map(|line| format!("{file}:{line}"))
                                .unwrap_or_else(|| file.to_string())
                        })
                        .unwrap_or_else(|| format!("[{}] {}", entry.timestamp, entry.source.subsystem)),
                    selected: self.selection == Selection::Console(index),
                })
                .collect(),
            "No diagnostics or tool output yet",
        )
    }

    fn inspector_rows(&self) -> Vec<UiRow> {
        let Some(project) = self.shell.project() else {
            return vec![
                UiRow::plain("Project", "No Project"),
                UiRow::plain("Hint", "Open with --project <path>"),
            ];
        };

        match self.selection {
            Selection::Entity(entity) => {
                let Some(object) = project.scene.object(entity) else {
                    return vec![UiRow::plain("Selection", "Missing entity")];
                };
                let transform = project.scene.transforms().local(entity).unwrap_or_default();
                let components = project
                    .scene
                    .components(entity)
                    .unwrap_or_default()
                    .iter()
                    .map(|component| component.type_id())
                    .collect::<Vec<_>>()
                    .join(", ");
                vec![
                    UiRow::plain("Name", &object.name),
                    UiRow::plain("Tag", &object.tag),
                    UiRow::plain("Active", if object.active { "yes" } else { "no" }),
                    UiRow::plain(
                        "Position",
                        &format!(
                            "{:.2}, {:.2}, {:.2}",
                            transform.translation.x,
                            transform.translation.y,
                            transform.translation.z
                        ),
                    ),
                    UiRow::plain(
                        "Scale",
                        &format!(
                            "{:.2}, {:.2}, {:.2}",
                            transform.scale.x, transform.scale.y, transform.scale.z
                        ),
                    ),
                    UiRow::plain(
                        "Components",
                        if components.is_empty() {
                            "none"
                        } else {
                            &components
                        },
                    ),
                ]
            }
            Selection::Asset(index) => project
                .sorted_assets()
                .get(index)
                .map(|asset| {
                    vec![
                        UiRow::plain("Path", &asset.source_path.display().to_string()),
                        UiRow::plain("Kind", &format!("{:?}", asset.kind)),
                        UiRow::plain("Importer", &asset.importer),
                        UiRow::plain("Guid", &asset.guid.to_string()),
                        UiRow::plain("Dependencies", &asset.dependencies.len().to_string()),
                    ]
                })
                .unwrap_or_else(|| vec![UiRow::plain("Selection", "Missing asset")]),
            Selection::Console(index) => self
                .console_rows()
                .get(index)
                .map(|row| {
                    vec![
                        UiRow::plain("Diagnostic", &row.label),
                        UiRow::plain("Source", &row.detail),
                    ]
                })
                .unwrap_or_else(|| vec![UiRow::plain("Selection", "Missing console row")]),
            Selection::None => vec![
                UiRow::plain("Project", project.name()),
                UiRow::plain("Default scene", &project.manifest.default_scene),
                UiRow::plain("Asset root", &project.manifest.asset_root),
                UiRow::plain("Objects", &project.scene.object_count().to_string()),
                UiRow::plain("Assets", &project.assets.len().to_string()),
            ],
        }
    }

    fn console_rows(&self) -> Vec<UiRow> {
        let rows = match self.console_tab.as_str() {
            "assets" => self.asset_rows(),
            "tasks" => vec![
                UiRow::plain("Attach WGPU viewport adapter", "pending"),
                UiRow::plain("Wire component editing", "pending"),
                UiRow::plain("Port command palette", "pending"),
            ],
            "problems" => self.problem_rows(),
            _ => self
                .shell
                .console()
                .entries()
                .iter()
                .rev()
                .take(12)
                .enumerate()
                .map(|(index, entry)| UiRow {
                    label: entry.message.clone(),
                    detail: format!("[{}] {}", entry.timestamp, entry.source.subsystem),
                    selected: self.selection == Selection::Console(index),
                })
                .collect(),
        };
        non_empty_rows(rows, "No diagnostics or tool output yet")
    }

    fn problem_rows(&self) -> Vec<UiRow> {
        let mut rows = self
            .shell
            .console()
            .entries()
            .iter()
            .rev()
            .filter(|entry| entry.level >= ConsoleLevel::Warn)
            .take(12)
            .enumerate()
            .map(|(index, entry)| UiRow {
                label: entry.message.clone(),
                detail: format!("[{}] {}", entry.timestamp, entry.source.subsystem),
                selected: self.selection == Selection::Console(index),
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            rows.push(UiRow::plain("No problems detected", "Slint shell"));
        }
        rows
    }

    fn snapshot(&self) -> ShellSnapshot {
        ShellSnapshot::from_state(self)
    }

    fn quest_summaries(&self) -> Vec<UiQuestSummary> {
        non_empty_quests(
            self.quest_records
                .iter()
                .map(|record| UiQuestSummary {
                    title: record.title.clone(),
                    status: quest_status_slug(record.status).to_owned(),
                    group: quest_queue_group(record.status).to_owned(),
                    badge: quest_status_badge(record.status).to_owned(),
                    detail: format!(
                        "{} · {}",
                        quest_mode_label(record.mode),
                        format_time_ms(record.updated_at_ms)
                    ),
                    selected: self.selected_quest_id.as_deref() == Some(record.id.as_str()),
                })
                .collect(),
        )
    }

    fn quest_task_rows(&self) -> Vec<UiRow> {
        let Some(detail) = &self.selected_quest else {
            return vec![UiRow::plain("No Quest selected", "")];
        };
        non_empty_rows(
            detail
                .record
                .tasks
                .iter()
                .map(|task| UiRow {
                    label: task.title.clone(),
                    detail: if task.done { "done" } else { "pending" }.to_owned(),
                    selected: task.done,
                })
                .collect(),
            "No task breakdown yet",
        )
    }

    fn quest_timeline_rows(&self) -> Vec<UiRow> {
        let Some(detail) = &self.selected_quest else {
            return vec![UiRow::plain("No Quest selected", "")];
        };
        non_empty_rows(
            detail
                .events
                .iter()
                .rev()
                .take(40)
                .map(|event| UiRow::plain(&event.summary, &event.kind))
                .collect(),
            "No timeline events yet",
        )
    }

    fn quest_file_rows(&self) -> Vec<UiRow> {
        let Some(detail) = &self.selected_quest else {
            return vec![UiRow::plain("No Quest selected", "")];
        };
        let Some(review) = &detail.record.review else {
            return vec![UiRow::plain(
                "No review bundle yet",
                "approve and execute first",
            )];
        };
        non_empty_rows(
            review
                .changed_files
                .iter()
                .map(|file| {
                    UiRow::plain(
                        &file.path,
                        &format!("{} +{} -{}", file.status, file.additions, file.deletions),
                    )
                })
                .collect(),
            "No file changes in review",
        )
    }

    fn quest_knowledge_rows(&self) -> Vec<UiRow> {
        let Some(detail) = &self.selected_quest else {
            return vec![UiRow::plain("No Quest selected", "")];
        };
        let mut rows = detail
            .attached_knowledge
            .iter()
            .map(|entry| UiRow::plain(&entry.category, &entry.reference_status))
            .collect::<Vec<_>>();
        if let Ok(entries) = self.quest_store.list_knowledge() {
            rows.extend(
                entries
                    .iter()
                    .filter(|entry| entry.status == "pending")
                    .take(12)
                    .map(|entry| {
                        UiRow::plain(&entry.category, &format!("pending · {}", entry.source))
                    }),
            );
        }
        non_empty_rows(rows, "No knowledge attached")
    }

    fn quest_metrics(&self) -> Vec<UiMetric> {
        let Some(detail) = &self.selected_quest else {
            return Vec::new();
        };
        let review = detail.record.review.as_ref();
        vec![
            UiMetric {
                label: "Status".to_owned(),
                value: quest_status_label(detail.record.status).to_owned(),
            },
            UiMetric {
                label: "Mode".to_owned(),
                value: quest_mode_label(detail.record.mode).to_owned(),
            },
            UiMetric {
                label: "Tasks".to_owned(),
                value: format!(
                    "{}/{}",
                    detail.record.tasks.iter().filter(|task| task.done).count(),
                    detail.record.tasks.len()
                ),
            },
            UiMetric {
                label: "Changed files".to_owned(),
                value: review
                    .map(|review| review.changed_files.len().to_string())
                    .unwrap_or_else(|| "0".to_owned()),
            },
            UiMetric {
                label: "Validations".to_owned(),
                value: review
                    .map(|review| review.validations.len().to_string())
                    .unwrap_or_else(|| "0".to_owned()),
            },
            UiMetric {
                label: "Risk".to_owned(),
                value: review
                    .map(|review| review.risk.clone())
                    .filter(|risk| !risk.trim().is_empty())
                    .unwrap_or_else(|| "pending".to_owned()),
            },
        ]
    }
}

impl UiRow {
    fn plain(label: &str, detail: &str) -> Self {
        Self {
            label: label.to_owned(),
            detail: detail.to_owned(),
            selected: false,
        }
    }
}

#[derive(Clone, Debug)]
struct ShellSnapshot {
    project_name: String,
    project_path: String,
    scene_path: String,
    scene_dirty: bool,
    object_count: usize,
    asset_count: usize,
    can_undo: bool,
    can_redo: bool,
    active_screen: String,
    active_tool: String,
    active_rail: String,
    console_tab: String,
    selected_kind: String,
    selected_name: String,
    play_mode: bool,
    hierarchy: Vec<UiRow>,
    assets: Vec<UiRow>,
    scripts: Vec<UiRow>,
    build_rows: Vec<UiRow>,
    diagnostics: Vec<UiRow>,
    inspector: Vec<UiRow>,
    console: Vec<UiRow>,
    quests: Vec<UiQuestSummary>,
    quest_timeline: Vec<UiRow>,
    quest_tasks: Vec<UiRow>,
    quest_files: Vec<UiRow>,
    quest_knowledge: Vec<UiRow>,
    quest_metrics: Vec<UiMetric>,
    quest_tab: String,
    quest_title: String,
    quest_status: String,
    quest_goal: String,
    quest_next_action: String,
    quest_next_reason: String,
    quest_document: String,
    has_selected_quest: bool,
    scene_image: Image,
    scene_status: String,
}

impl ShellSnapshot {
    fn from_state(state: &SlintEditorState) -> Self {
        let (selected_name, selected_kind) = state.selected_name_and_kind();
        let Some(project) = state.shell.project() else {
            return Self {
                project_name: "No Project".to_owned(),
                project_path: "Open a project with --project <path>".to_owned(),
                scene_path: "Scene View is ready for the Slint native viewport adapter.".to_owned(),
                scene_dirty: false,
                object_count: 0,
                asset_count: 0,
                can_undo: state.shell.undo_stack().can_undo(),
                can_redo: state.shell.undo_stack().can_redo(),
                active_screen: state.active_screen.clone(),
                active_tool: state.active_tool.clone(),
                active_rail: state.active_rail.clone(),
                console_tab: state.console_tab.clone(),
                selected_kind,
                selected_name,
                play_mode: state.play_mode,
                hierarchy: state.hierarchy_rows(),
                assets: state.asset_rows(),
                scripts: state.script_rows(),
                build_rows: state.build_rows(),
                diagnostics: state.diagnostic_rows(),
                inspector: state.inspector_rows(),
                console: state.console_rows(),
                quests: state.quest_summaries(),
                quest_timeline: state.quest_timeline_rows(),
                quest_tasks: state.quest_task_rows(),
                quest_files: state.quest_file_rows(),
                quest_knowledge: state.quest_knowledge_rows(),
                quest_metrics: state.quest_metrics(),
                quest_tab: state.quest_tab.clone(),
                quest_title: "Quests".to_owned(),
                quest_status: String::new(),
                quest_goal: String::new(),
                quest_next_action: String::new(),
                quest_next_reason: String::new(),
                quest_document: String::new(),
                has_selected_quest: false,
                scene_image: state.scene_image.clone(),
                scene_status: state.scene_status.clone(),
            };
        };
        let quest = state.selected_quest.as_ref();

        Self {
            project_name: project.name().to_owned(),
            project_path: project.root.display().to_string(),
            scene_path: project.scene_path.display().to_string(),
            scene_dirty: state.shell.is_scene_dirty(),
            object_count: project.scene.object_count(),
            asset_count: project.assets.len(),
            can_undo: state.shell.undo_stack().can_undo(),
            can_redo: state.shell.undo_stack().can_redo(),
            active_screen: state.active_screen.clone(),
            active_tool: state.active_tool.clone(),
            active_rail: state.active_rail.clone(),
            console_tab: state.console_tab.clone(),
            selected_kind,
            selected_name,
            play_mode: state.play_mode,
            hierarchy: state.hierarchy_rows(),
            assets: state.asset_rows(),
            scripts: state.script_rows(),
            build_rows: state.build_rows(),
            diagnostics: state.diagnostic_rows(),
            inspector: state.inspector_rows(),
            console: state.console_rows(),
            quests: state.quest_summaries(),
            quest_timeline: state.quest_timeline_rows(),
            quest_tasks: state.quest_task_rows(),
            quest_files: state.quest_file_rows(),
            quest_knowledge: state.quest_knowledge_rows(),
            quest_metrics: state.quest_metrics(),
            quest_tab: state.quest_tab.clone(),
            quest_title: quest
                .map(|detail| detail.record.title.clone())
                .unwrap_or_else(|| "Quests".to_owned()),
            quest_status: quest
                .map(|detail| quest_status_slug(detail.record.status).to_owned())
                .unwrap_or_default(),
            quest_goal: quest
                .map(|detail| detail.record.goal.clone())
                .unwrap_or_default(),
            quest_next_action: quest
                .map(|detail| detail.record.next_action.label.clone())
                .unwrap_or_default(),
            quest_next_reason: quest
                .map(|detail| detail.record.next_action.reason.clone())
                .unwrap_or_default(),
            quest_document: quest
                .map(|detail| match state.quest_tab.as_str() {
                    "spec" => detail.spec.clone(),
                    _ => detail.intent.clone(),
                })
                .unwrap_or_default(),
            has_selected_quest: quest.is_some(),
            scene_image: state.scene_image.clone(),
            scene_status: state.scene_status.clone(),
        }
    }
}

fn non_empty_quests(mut rows: Vec<UiQuestSummary>) -> Vec<UiQuestSummary> {
    if rows.is_empty() {
        rows.push(UiQuestSummary {
            title: "No Quests yet".to_owned(),
            status: "empty".to_owned(),
            group: "recent".to_owned(),
            badge: "Empty".to_owned(),
            detail: "Create a Quest from the React/Tauri workspace".to_owned(),
            selected: false,
        });
    }
    rows
}

fn non_empty_rows(mut rows: Vec<UiRow>, fallback: &str) -> Vec<UiRow> {
    if rows.is_empty() {
        rows.push(UiRow::plain(fallback, ""));
    }
    rows
}

fn quest_status_slug(status: quest::QuestStatus) -> &'static str {
    match status {
        quest::QuestStatus::Draft => "draft",
        quest::QuestStatus::Clarifying => "clarifying",
        quest::QuestStatus::Specified => "specified",
        quest::QuestStatus::Planning => "planning",
        quest::QuestStatus::Prepared => "prepared",
        quest::QuestStatus::Running => "running",
        quest::QuestStatus::WaitingForUser => "waiting_for_user",
        quest::QuestStatus::Validating => "validating",
        quest::QuestStatus::Repairing => "repairing",
        quest::QuestStatus::ReadyForReview => "ready_for_review",
        quest::QuestStatus::Applying => "applying",
        quest::QuestStatus::Completed => "completed",
        quest::QuestStatus::Blocked => "blocked",
        quest::QuestStatus::Canceled => "canceled",
        quest::QuestStatus::Archived => "archived",
    }
}

fn quest_status_label(status: quest::QuestStatus) -> &'static str {
    match status {
        quest::QuestStatus::Draft => "Draft",
        quest::QuestStatus::Clarifying => "Clarifying",
        quest::QuestStatus::Specified => "Specified",
        quest::QuestStatus::Planning => "Planning",
        quest::QuestStatus::Prepared => "Prepared",
        quest::QuestStatus::Running => "Running",
        quest::QuestStatus::WaitingForUser => "Waiting",
        quest::QuestStatus::Validating => "Validating",
        quest::QuestStatus::Repairing => "Repairing",
        quest::QuestStatus::ReadyForReview => "Ready for review",
        quest::QuestStatus::Applying => "Applying",
        quest::QuestStatus::Completed => "Completed",
        quest::QuestStatus::Blocked => "Blocked",
        quest::QuestStatus::Canceled => "Canceled",
        quest::QuestStatus::Archived => "Archived",
    }
}

fn quest_queue_group(status: quest::QuestStatus) -> &'static str {
    match status {
        quest::QuestStatus::Clarifying
        | quest::QuestStatus::WaitingForUser
        | quest::QuestStatus::ReadyForReview
        | quest::QuestStatus::Blocked => "needs_action",
        quest::QuestStatus::Prepared
        | quest::QuestStatus::Running
        | quest::QuestStatus::Validating
        | quest::QuestStatus::Repairing
        | quest::QuestStatus::Applying => "running",
        quest::QuestStatus::Archived | quest::QuestStatus::Canceled => "archived",
        quest::QuestStatus::Draft
        | quest::QuestStatus::Specified
        | quest::QuestStatus::Planning
        | quest::QuestStatus::Completed => "recent",
    }
}

fn quest_status_badge(status: quest::QuestStatus) -> &'static str {
    match status {
        quest::QuestStatus::Clarifying | quest::QuestStatus::WaitingForUser => "Action Required",
        quest::QuestStatus::ReadyForReview => "Review",
        quest::QuestStatus::Prepared
        | quest::QuestStatus::Running
        | quest::QuestStatus::Validating
        | quest::QuestStatus::Repairing
        | quest::QuestStatus::Applying => "Running",
        quest::QuestStatus::Blocked => "Blocked",
        quest::QuestStatus::Completed => "Done",
        quest::QuestStatus::Archived => "Archived",
        quest::QuestStatus::Canceled => "Canceled",
        quest::QuestStatus::Draft => "Draft",
        quest::QuestStatus::Specified | quest::QuestStatus::Planning => "Spec",
    }
}

fn format_time_ms(value: u64) -> String {
    if value == 0 {
        "unknown time".to_owned()
    } else {
        value.to_string()
    }
}

fn quest_mode_label(mode: quest::QuestMode) -> &'static str {
    match mode {
        quest::QuestMode::Solo => "Solo",
        quest::QuestMode::Extra => "Extra",
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let project_path = project_path_arg();
    let mut shell = EditorShell::with_core_services(EditorPreferences::default());

    if let Some(path) = project_path {
        if let Err(error) = open_project(&mut shell, &path) {
            shell.console_mut().push(ConsoleEntry {
                timestamp: "now".to_owned(),
                level: ConsoleLevel::Error,
                source: ConsoleSource {
                    subsystem: "slint-editor".to_owned(),
                    file: None,
                    line: None,
                },
                message: format!("failed to open project {}: {error}", path.display()),
            });
        }
    }

    let quest_store = quest::QuestStore::new(default_quest_root());
    let app = AppWindow::new()?;
    app.window().set_size(PhysicalSize::new(1440, 900));
    app.window().set_maximized(true);
    let app_weak = app.as_weak();
    let game_view = Rc::new(GameViewWindow::new()?);
    game_view.window().set_size(PhysicalSize::new(1280, 720));
    let state = Rc::new(RefCell::new(SlintEditorState::new(shell, quest_store)));
    state.borrow_mut().refresh_scene_view();
    apply_state(&app, &state.borrow());
    apply_game_view(&game_view, &state.borrow());
    install_callbacks(&app, app_weak, game_view, state);
    app.run()
}

fn install_callbacks(
    app: &AppWindow,
    app_weak: Weak<AppWindow>,
    game_view: Rc<GameViewWindow>,
    state: Rc<RefCell<SlintEditorState>>,
) {
    app.on_save_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move || {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.save_scene()
            })
        }
    });
    app.on_undo_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move || mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| state.undo())
    });
    app.on_redo_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move || mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| state.redo())
    });
    app.on_play_toggled({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move || {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.toggle_play()
            });
            if state.borrow().play_mode {
                if let Err(error) = game_view.show() {
                    eprintln!("failed to show Game View: {error}");
                }
            } else if let Err(error) = game_view.hide() {
                eprintln!("failed to hide Game View: {error}");
            }
        }
    });
    app.on_tool_selected({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |tool| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.select_tool(tool.as_str())
            })
        }
    });
    app.on_screen_selected({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |screen| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.select_screen(screen.as_str())
            })
        }
    });
    app.on_rail_selected({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |rail| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.select_rail(rail.as_str())
            })
        }
    });
    app.on_hierarchy_selected({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |index| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.select_hierarchy(index.max(0) as usize);
            })
        }
    });
    app.on_asset_selected({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |index| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.select_asset(index.max(0) as usize);
            })
        }
    });
    app.on_script_selected({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |index| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.select_script(index.max(0) as usize);
            })
        }
    });
    app.on_console_tab_selected({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |tab| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.select_console_tab(tab.as_str());
            })
        }
    });
    app.on_console_row_opened({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |index| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.open_console_row(index.max(0) as usize);
            })
        }
    });
    app.on_create_object_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move || {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.create_object()
            })
        }
    });
    app.on_delete_selection_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move || {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.delete_selected()
            })
        }
    });
    app.on_rename_selection_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move || {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.rename_selected()
            })
        }
    });
    app.on_nudge_selection_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |direction| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.nudge_selected(direction.as_str());
            })
        }
    });
    app.on_add_component_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |component_type| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.add_component(component_type.as_str());
            })
        }
    });
    app.on_remove_component_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |component_type| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.remove_component(component_type.as_str());
            })
        }
    });
    app.on_create_asset_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |kind| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.create_asset(kind.as_str());
            })
        }
    });
    app.on_rename_asset_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move || {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.rename_selected_asset()
            })
        }
    });
    app.on_delete_asset_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move || {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.delete_selected_asset()
            })
        }
    });
    app.on_reimport_asset_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move || {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.reimport_selected_asset()
            })
        }
    });
    app.on_reimport_all_assets_requested({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move || {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.reimport_all_assets()
            })
        }
    });
    app.on_quest_selected({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |index| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.select_quest(index.max(0) as usize);
            })
        }
    });
    app.on_quest_tab_selected({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |tab| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.select_quest_tab(tab.as_str());
            })
        }
    });
    app.on_quest_action({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |action| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.run_quest_action(action.as_str());
            })
        }
    });
    app.on_scene_pointer_event({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |kind, button, x, y| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.scene_pointer_event(kind.as_str(), button.as_str(), x, y);
            })
        }
    });
    app.on_scene_scrolled({
        let app_weak = app_weak.clone();
        let game_view = Rc::clone(&game_view);
        let state = Rc::clone(&state);
        move |delta_y| {
            mutate_and_refresh(&app_weak, Some(&game_view), &state, |state| {
                state.scene_scrolled(delta_y);
            })
        }
    });
}

fn mutate_and_refresh(
    app_weak: &Weak<AppWindow>,
    game_view: Option<&GameViewWindow>,
    state: &Rc<RefCell<SlintEditorState>>,
    mutate: impl FnOnce(&mut SlintEditorState),
) {
    {
        let mut state = state.borrow_mut();
        mutate(&mut state);
        state.refresh_scene_view();
    }
    if let Some(app) = app_weak.upgrade() {
        apply_state(&app, &state.borrow());
    }
    if let Some(game_view) = game_view {
        apply_game_view(game_view, &state.borrow());
    }
}

fn project_path_arg() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--project" || arg == "-p" {
            return args.next().map(PathBuf::from);
        }
        let path = PathBuf::from(&arg);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn default_quest_root() -> PathBuf {
    let config_dir = dirs_config_dir().unwrap_or_else(|| PathBuf::from("."));
    dirs_data_dir()
        .unwrap_or_else(|| config_dir.clone())
        .join("quests")
}

fn dirs_config_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("Aster"))
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME").map(PathBuf::from).map(|home| {
            home.join("Library")
                .join("Application Support")
                .join("Aster")
        })
    } else if let Some(path) = env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
        Some(path.join("aster"))
    } else {
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".config").join("aster"))
    }
}

fn dirs_data_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("Aster"))
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME").map(PathBuf::from).map(|home| {
            home.join("Library")
                .join("Application Support")
                .join("Aster")
        })
    } else if let Some(path) = env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        Some(path.join("aster"))
    } else {
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".local").join("share").join("aster"))
    }
}

fn open_project(shell: &mut EditorShell, path: &Path) -> EngineResult<()> {
    shell.open_project(path)
}

fn normalize_relative_path(path: &str) -> EngineResult<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(EngineError::config("path must stay inside the project"));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(EngineError::config("path must not be empty"));
    }

    Ok(normalized)
}

fn validate_file_name(name: &str) -> EngineResult<()> {
    let mut components = Path::new(name).components();
    if matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none() {
        Ok(())
    } else {
        Err(EngineError::config(
            "file name must not contain path separators",
        ))
    }
}

fn resolve_existing_relative_path(root: &Path, path: &str) -> EngineResult<PathBuf> {
    let relative = normalize_relative_path(path)?;
    let canonical_root = root
        .canonicalize()
        .map_err(|source| EngineError::Filesystem {
            path: root.to_path_buf(),
            source,
        })?;
    let full_path = canonical_root.join(relative);
    let canonical = full_path
        .canonicalize()
        .map_err(|source| EngineError::Filesystem {
            path: full_path.clone(),
            source,
        })?;

    if !canonical.starts_with(&canonical_root) {
        return Err(EngineError::config("path is outside the project"));
    }

    Ok(canonical)
}

fn resolve_writable_relative_path(root: &Path, path: &str) -> EngineResult<PathBuf> {
    let relative = normalize_relative_path(path)?;
    std::fs::create_dir_all(root).map_err(|source| EngineError::Filesystem {
        path: root.to_path_buf(),
        source,
    })?;
    let canonical_root = root
        .canonicalize()
        .map_err(|source| EngineError::Filesystem {
            path: root.to_path_buf(),
            source,
        })?;
    let full_path = canonical_root.join(relative);
    if !full_path.starts_with(&canonical_root) {
        return Err(EngineError::config("path is outside the project"));
    }
    Ok(full_path)
}

fn asset_meta_path_for_source(path: &Path) -> PathBuf {
    let mut meta_path = path.to_path_buf();
    if let Some(name) = path.file_name() {
        let mut meta_name = name.to_os_string();
        meta_name.push(".meta");
        meta_path.set_file_name(meta_name);
    } else {
        meta_path.set_extension("meta");
    }
    meta_path
}

fn write_project_asset(
    project: &engine_editor::ProjectContext,
    asset_path: &str,
    content: &str,
) -> EngineResult<(String, PathBuf)> {
    let asset_root = project.root.join(&project.manifest.asset_root);
    let full_path = resolve_writable_relative_path(&asset_root, asset_path)?;
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| EngineError::Filesystem {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(&full_path, content).map_err(|source| EngineError::Filesystem {
        path: full_path.clone(),
        source,
    })?;
    Ok((asset_path.to_owned(), full_path))
}

fn select_asset_by_path(project: &engine_editor::ProjectContext, path: &str) -> Option<usize> {
    project
        .sorted_assets()
        .iter()
        .position(|asset| asset.source_path.to_string_lossy() == path)
}

fn unique_renamed_stem(parent: &Path, stem: &str, extension: &str) -> String {
    let mut candidate = format!("{stem}_renamed");
    let mut counter = 2;
    while parent.join(format!("{candidate}{extension}")).exists() {
        candidate = format!("{stem}_renamed_{counter}");
        counter += 1;
    }
    candidate
}

fn varg_script_template(name: &str) -> String {
    format!(
        r#"script {name} {{
    @export var speed: Float = 6.0

    func start() {{
        log("{name} ready")
    }}

    func update(_dt: Float) {{
    }}
}}
"#
    )
}

fn varg_scene_template(name: &str) -> String {
    format!(
        r##"scene {name} {{
    camera "MainCamera" {{
        transform {{
            position: Vec3(0, 3, 8)
            rotation: Euler(-20, 0, 0)
        }}

        perspective {{
            fov: 60
            near: 0.1
            far: 1000
        }}

        primary: true
    }}

    light "Sun" {{
        kind: directional
        intensity: 2.0
        rotation: Euler(-45, 35, 0)
    }}
}}
"##
    )
}

fn varg_prefab_template(name: &str) -> String {
    format!(
        r##"prefab {name} {{
    entity "{name}" {{
        mesh: Box(size: Vec3(1, 1, 1))

        material {{
            baseColor: Color("#7aa2ff")
            roughness: 0.65
        }}
    }}
}}
"##
    )
}

fn varg_material_template(name: &str) -> String {
    format!(
        r##"material {name} {{
    shader: "pbr"

    baseColor: Color("#7aa2ff")
    roughness: 0.7
    metallic: 0.0
}}
"##
    )
}

fn apply_state(app: &AppWindow, state: &SlintEditorState) {
    apply_snapshot(app, state.snapshot());
}

fn apply_snapshot(app: &AppWindow, snapshot: ShellSnapshot) {
    app.set_project_name(snapshot.project_name.into());
    app.set_project_path(snapshot.project_path.into());
    app.set_scene_path(snapshot.scene_path.into());
    app.set_scene_dirty(snapshot.scene_dirty);
    app.set_object_count(snapshot.object_count as i32);
    app.set_asset_count(snapshot.asset_count as i32);
    app.set_can_undo(snapshot.can_undo);
    app.set_can_redo(snapshot.can_redo);
    app.set_active_screen(snapshot.active_screen.into());
    app.set_active_tool(snapshot.active_tool.into());
    app.set_active_rail(snapshot.active_rail.into());
    app.set_console_tab(snapshot.console_tab.into());
    app.set_selected_kind(snapshot.selected_kind.into());
    app.set_selected_name(snapshot.selected_name.into());
    app.set_play_mode(snapshot.play_mode);
    app.set_hierarchy(rows_model(snapshot.hierarchy));
    app.set_assets(rows_model(snapshot.assets));
    app.set_scripts(rows_model(snapshot.scripts));
    app.set_build_rows(rows_model(snapshot.build_rows));
    app.set_diagnostics(rows_model(snapshot.diagnostics));
    app.set_inspector(rows_model(snapshot.inspector));
    app.set_console(rows_model(snapshot.console));
    app.set_quests(quest_summary_model(snapshot.quests));
    app.set_quest_timeline(rows_model(snapshot.quest_timeline));
    app.set_quest_tasks(rows_model(snapshot.quest_tasks));
    app.set_quest_files(rows_model(snapshot.quest_files));
    app.set_quest_knowledge(rows_model(snapshot.quest_knowledge));
    app.set_quest_metrics(quest_metric_model(snapshot.quest_metrics));
    app.set_quest_tab(snapshot.quest_tab.into());
    app.set_quest_title(snapshot.quest_title.into());
    app.set_quest_status(snapshot.quest_status.into());
    app.set_quest_goal(snapshot.quest_goal.into());
    app.set_quest_next_action(snapshot.quest_next_action.into());
    app.set_quest_next_reason(snapshot.quest_next_reason.into());
    app.set_quest_document(snapshot.quest_document.into());
    app.set_has_selected_quest(snapshot.has_selected_quest);
    app.set_scene_image(snapshot.scene_image);
    app.set_scene_status(snapshot.scene_status.into());
}

fn apply_game_view(game_view: &GameViewWindow, state: &SlintEditorState) {
    let snapshot = state.snapshot();
    game_view.set_project_name(snapshot.project_name.into());
    game_view.set_status(
        format!(
            "{} objects   {} assets   {}",
            snapshot.object_count, snapshot.asset_count, snapshot.scene_status
        )
        .into(),
    );
    game_view.set_game_image(snapshot.scene_image);
}

fn rows_model(rows: Vec<UiRow>) -> ModelRc<ShellRow> {
    ModelRc::new(VecModel::from(
        rows.into_iter()
            .map(|row| ShellRow {
                label: SharedString::from(row.label),
                detail: SharedString::from(row.detail),
                selected: row.selected,
            })
            .collect::<Vec<_>>(),
    ))
}

fn quest_summary_model(rows: Vec<UiQuestSummary>) -> ModelRc<QuestSummary> {
    ModelRc::new(VecModel::from(
        rows.into_iter()
            .map(|row| QuestSummary {
                title: SharedString::from(row.title),
                status: SharedString::from(row.status),
                group: SharedString::from(row.group),
                badge: SharedString::from(row.badge),
                detail: SharedString::from(row.detail),
                selected: row.selected,
            })
            .collect::<Vec<_>>(),
    ))
}

fn quest_metric_model(rows: Vec<UiMetric>) -> ModelRc<QuestMetric> {
    ModelRc::new(VecModel::from(
        rows.into_iter()
            .map(|row| QuestMetric {
                label: SharedString::from(row.label),
                value: SharedString::from(row.value),
            })
            .collect::<Vec<_>>(),
    ))
}

fn install_editor_camera(world: &mut RenderWorld, camera: EditorCamera) {
    let object = world
        .camera
        .as_ref()
        .map(|camera| camera.object)
        .unwrap_or_else(|| engine_core::EntityId::from_u128(0));
    let target = camera.target;
    let distance = camera.distance;
    let yaw = camera.yaw;
    let pitch = camera.pitch;
    world.camera = Some(RenderCamera {
        object,
        transform: Transform {
            translation: Vec3::new(
                distance * pitch.cos() * yaw.sin(),
                distance * pitch.sin(),
                distance * pitch.cos() * yaw.cos(),
            ),
            ..Transform::IDENTITY
        },
        projection: RenderProjection::Perspective,
        vertical_fov_degrees: 60.0,
        near: 0.01,
        far: 1000.0,
        look_at_target: Some(target),
    });
}

fn rgba_image(width: u32, height: u32, rgba: &[u8]) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        rgba, width, height,
    ))
}

fn placeholder_scene_image(width: u32, height: u32) -> Image {
    let width = width.max(1);
    let height = height.max(1);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let grid = (x / 32 + y / 32) % 2 == 0;
            let shade = if grid { 18 } else { 13 };
            rgba.extend_from_slice(&[shade, shade + 2, shade + 4, 255]);
        }
    }
    rgba_image(width, height, &rgba)
}
