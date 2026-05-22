//! Viewport panel for the editor shell.

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::super::operations::scene_ops::{create_object_from_asset, select_first_scene_object};
use super::super::types::{rgb, InfernuxPalette, ShellUiState, ViewportTargetState};
use super::super::widgets::layout::{empty_view, panel_title};
use super::super::widgets::text::{paint_text_in_rect, paint_wrapped_text_in_rect};
use super::console::draw_console;
use super::project::draw_project_panel;
use crate::EditorShell;
use engine_core::math::{Quat, Transform, Vec3 as EngineVec3};
use engine_ecs::ComponentData;
use engine_i18n::Translations;
use engine_render::{
    RenderCamera, RenderLight, RenderObject, RenderTargetDesc, RenderWorld, ViewKind,
};

const EDITOR_CAMERA_MIN_DISTANCE: f32 = 0.5;
const EDITOR_CAMERA_MAX_DISTANCE: f32 = 100.0;
const EDITOR_CAMERA_ORBIT_SENSITIVITY: f32 = 0.005;
const EDITOR_CAMERA_ZOOM_SENSITIVITY: f32 = 0.0035;
const EDITOR_CAMERA_ZOOM_DAMPING: f32 = 22.0;
const EDITOR_CAMERA_TOP_PITCH: f32 = 1.5;
/// Renders the center dock area with viewport tabs.

pub fn draw_center_dock(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    if ui_state.show_scene_view && ui_state.show_game_view {
        ui.columns(2, |columns| {
            viewport_panel(
                &mut columns[0],
                shell,
                tr.tr("viewport_scene"),
                true,
                ui_state,
                pal,
                tr,
            );
            viewport_panel(
                &mut columns[1],
                shell,
                tr.tr("viewport_game"),
                false,
                ui_state,
                pal,
                tr,
            );
        });
    } else if ui_state.show_scene_view {
        viewport_panel(ui, shell, tr.tr("viewport_scene"), true, ui_state, pal, tr);
    } else if ui_state.show_game_view {
        viewport_panel(ui, shell, tr.tr("viewport_game"), false, ui_state, pal, tr);
    } else {
        empty_view(ui, tr.tr("viewport_no_viewport"), pal);
    }
}

/// Renders the bottom dock area with console panel.
pub fn draw_bottom_dock(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    ui.horizontal(|ui| {
        if ui_state.show_project {
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() * if ui_state.show_console { 0.5 } else { 1.0 });
                panel_title(ui, tr.tr("panel_project"), pal);
                draw_project_panel(ui, shell, ui_state, pal, tr);
            });
        }
        if ui_state.show_console {
            ui.vertical(|ui| {
                panel_title(ui, tr.tr("panel_console"), pal);
                draw_console(ui, shell, ui_state, pal, tr);
            });
        }
    });
}

fn update_editor_camera_zoom(ui: &egui::Ui, ui_state: &mut ShellUiState, hovered: bool) {
    if hovered {
        let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
        if scroll.abs() > f32::EPSILON {
            let zoom = (-scroll * EDITOR_CAMERA_ZOOM_SENSITIVITY).exp();
            ui_state.editor_camera_target_distance = (ui_state.editor_camera_target_distance
                * zoom)
                .clamp(EDITOR_CAMERA_MIN_DISTANCE, EDITOR_CAMERA_MAX_DISTANCE);
        }
    }

    let diff = ui_state.editor_camera_target_distance - ui_state.editor_camera_distance;
    if diff.abs() <= 0.001 {
        ui_state.editor_camera_distance = ui_state.editor_camera_target_distance;
        return;
    }

    let dt = ui.ctx().input(|input| input.stable_dt).clamp(0.0, 0.1);
    let blend = 1.0 - (-EDITOR_CAMERA_ZOOM_DAMPING * dt).exp();
    ui_state.editor_camera_distance = (ui_state.editor_camera_distance + diff * blend)
        .clamp(EDITOR_CAMERA_MIN_DISTANCE, EDITOR_CAMERA_MAX_DISTANCE);
    ui.ctx().request_repaint();
}

fn viewport_panel(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    label: &str,
    scene_tools: bool,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    let rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(0), pal.viewport_bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(0),
        Stroke::new(1.0, pal.border),
        StrokeKind::Inside,
    );

    let tab = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 26.0));
    ui.painter()
        .rect_filled(tab, CornerRadius::same(0), pal.header);
    paint_text_in_rect(
        ui,
        tab.shrink2(Vec2::new(10.0, 0.0)),
        label,
        FontId::proportional(13.0),
        pal.text,
        Align2::LEFT_CENTER,
    );

    let content_rect = rect.shrink2(Vec2::new(0.0, 26.0));
    draw_render_viewport(ui, shell, ui_state, content_rect, scene_tools, pal, tr);

    let mut orientation_clicked = false;
    if scene_tools {
        update_editor_camera_zoom(ui, ui_state, response.hovered());

        if response.dragged_by(egui::PointerButton::Secondary) {
            let delta = ui.ctx().input(|input| input.pointer.delta());
            orbit_editor_camera(ui_state, delta);
        }
        if response.dragged_by(egui::PointerButton::Middle) {
            let delta = response.drag_delta();
            let pan_speed = ui_state.editor_camera_distance * 0.002;
            ui_state.editor_camera_target[0] -= delta.x * pan_speed;
            ui_state.editor_camera_target[1] += delta.y * pan_speed;
        }
        if response.hovered()
            && ui.input(|input| input.pointer.any_released())
            && ui_state.dragged_asset.is_some()
        {
            if let Some(guid) = ui_state.dragged_asset.take() {
                if let Some(project) = shell.project() {
                    let kind = project.database.entry_for_guid(guid).map(|e| e.kind);
                    if let Some(kind) = kind {
                        if let Some(id) = create_object_from_asset(shell, guid, kind) {
                            shell.select_entity_id(id);
                        }
                    }
                }
            }
        }

        draw_scene_overlay(ui, rect, pal);
        orientation_clicked = draw_orientation_gizmo(ui, rect, ui_state, pal);
    }

    if ui_state.playing || ui_state.paused {
        let color = if ui_state.paused { pal.pause } else { pal.play };
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            CornerRadius::same(0),
            Stroke::new(2.0, color),
            StrokeKind::Inside,
        );
    }

    if response.clicked() && !orientation_clicked {
        select_first_scene_object(shell);
    }
}

fn draw_render_viewport(
    ui: &mut egui::Ui,
    shell: &EditorShell,
    ui_state: &mut ShellUiState,
    rect: Rect,
    scene_tools: bool,
    pal: &InfernuxPalette,
    tr: &Translations,
) {
    if shell.project().is_none() {
        draw_viewport_hint(ui, rect, tr.tr("viewport_hint_open"), pal);
        return;
    }

    let world = if scene_tools {
        extract_render_world(shell, true)
    } else {
        ui_state
            .runtime_game_world
            .clone()
            .unwrap_or_else(|| extract_render_world(shell, false))
    };
    let width = rect.width().round().max(1.0) as u32;
    let height = rect.height().round().max(1.0) as u32;
    let desc = RenderTargetDesc::view(
        width,
        height,
        if scene_tools {
            ViewKind::SceneView
        } else {
            ViewKind::GameView
        },
    );
    let state = ViewportTargetState {
        desc: desc.clone(),
        world,
    };
    if scene_tools {
        ui_state.scene_view_target = Some(state.clone());
    } else {
        ui_state.game_view_target = Some(state.clone());
    }

    if !state.world.is_visible() {
        draw_viewport_hint(ui, rect, tr.tr("viewport_hint_empty"), pal);
    } else if scene_tools {
        if let Some(texture) = &ui_state.scene_view_texture {
            let texture_id = egui::TextureId::User(texture.id);
            let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
            ui.painter().image(texture_id, rect, uv, Color32::WHITE);
        } else {
            paint_render_target_placeholder(ui, rect, &state, scene_tools, pal);
        }
    } else {
        if ui_state.playing || ui_state.paused {
            if let Some(texture) = &ui_state.game_view_texture {
                let texture_id = egui::TextureId::User(texture.id);
                let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
                ui.painter().image(texture_id, rect, uv, Color32::WHITE);
            } else {
                paint_render_target_placeholder(ui, rect, &state, scene_tools, pal);
            }
        } else {
            draw_viewport_hint(ui, rect, tr.tr("viewport_hint_game_view"), pal);
        }
    }

    let stats = format!(
        "{} target: {}x{} | camera: {} | draws: {} | lights: {}",
        if scene_tools { "Scene" } else { "Game" },
        desc.width,
        desc.height,
        state
            .world
            .camera
            .as_ref()
            .map(|camera| format!("{:032x}", camera.object.as_u128()))
            .unwrap_or_else(|| "none".to_owned()),
        state.world.objects.len(),
        state.world.lights.len()
    );
    paint_text_in_rect(
        ui,
        Rect::from_min_max(
            rect.left_bottom() + Vec2::new(10.0, -24.0),
            rect.right_bottom() + Vec2::new(-10.0, -2.0),
        ),
        &stats,
        FontId::proportional(11.0),
        pal.text_dim,
        Align2::LEFT_CENTER,
    );
}

fn paint_render_target_placeholder(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &ViewportTargetState,
    scene_tools: bool,
    pal: &InfernuxPalette,
) {
    let top = rgb(26, 28, 30);
    let bottom = if scene_tools {
        rgb(35, 39, 42)
    } else {
        rgb(22, 25, 31)
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(0), bottom);
    ui.painter().rect_filled(
        Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.center().y)),
        CornerRadius::same(0),
        top,
    );
    let horizon = rect.center().y + rect.height() * 0.12;
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), horizon),
            Pos2::new(rect.right(), horizon),
        ],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 32)),
    );
    for (idx, object) in state.world.objects.iter().enumerate() {
        let x = rect.left() + rect.width() * (0.2 + (idx as f32 * 0.19) % 0.62);
        let y = horizon - object.transform.translation.y * 7.0;
        let h = (26.0_f32 + object.transform.scale.y.abs() * 24.0_f32).clamp(18.0, 80.0);
        let w = (24.0_f32 + object.transform.scale.x.abs() * 20.0_f32).clamp(18.0, 70.0);
        let mesh_rect = Rect::from_center_size(Pos2::new(x, y - h * 0.5), Vec2::new(w, h));
        ui.painter()
            .rect_filled(mesh_rect, CornerRadius::same(2), pal.accent);
        ui.painter().rect_stroke(
            mesh_rect,
            CornerRadius::same(2),
            Stroke::new(1.0, pal.border),
            StrokeKind::Inside,
        );
    }
    for light in &state.world.lights {
        let x = rect.center().x + light.transform.translation.x * 12.0;
        let y = rect.top() + 52.0 + light.transform.translation.y.abs() * 4.0;
        ui.painter()
            .circle_filled(Pos2::new(x, y), 7.0, pal.warning);
    }
}

fn draw_viewport_hint(ui: &mut egui::Ui, rect: Rect, hint: &str, pal: &InfernuxPalette) {
    paint_wrapped_text_in_rect(
        ui,
        rect.shrink(16.0),
        hint,
        FontId::proportional(14.0),
        pal.text_disabled,
        Align2::CENTER_CENTER,
    );
}

fn draw_scene_overlay(ui: &mut egui::Ui, rect: Rect, pal: &InfernuxPalette) {
    let cursor = rect.min + Vec2::new(8.0, 34.0);

    let pill = Rect::from_min_size(cursor, Vec2::new(86.0, 22.0));
    ui.painter().rect_filled(
        pill,
        CornerRadius::same(4),
        Color32::from_rgba_premultiplied(35, 35, 35, 220),
    );
    paint_text_in_rect(
        ui,
        pill.shrink2(Vec2::new(6.0, 0.0)),
        "Global",
        FontId::proportional(12.0),
        pal.text,
        Align2::CENTER_CENTER,
    );
}

fn draw_orientation_gizmo(
    ui: &mut egui::Ui,
    rect: Rect,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
) -> bool {
    let center = rect.right_top() + Vec2::new(-54.0, 62.0);
    ui.painter().circle_filled(
        center,
        40.0,
        Color32::from_rgba_premultiplied(20, 20, 20, 150),
    );
    let gizmo_response = ui
        .interact(
            Rect::from_center_size(center, Vec2::splat(88.0)),
            ui.make_persistent_id("orientation_gizmo_orbit"),
            Sense::click_and_drag(),
        )
        .on_hover_text("Drag to orbit view");
    let mut clicked = false;
    let mut dragged = false;

    if gizmo_response.dragged_by(egui::PointerButton::Primary) {
        let delta = ui.ctx().input(|input| input.pointer.delta());
        orbit_editor_camera(ui_state, delta);
        ui.ctx().request_repaint();
        dragged = true;
    }

    let mut axes = projected_orientation_axes(ui_state);
    axes.sort_by(|a, b| a.depth.total_cmp(&b.depth));

    for axis in axes {
        let axis_center = center + axis.offset;
        let hit_rect = Rect::from_center_size(axis_center, Vec2::splat(axis.hit_size));
        let response = ui
            .interact(
                hit_rect,
                ui.make_persistent_id(("orientation_gizmo_axis", axis.id)),
                Sense::click(),
            )
            .on_hover_text(format!("View along {} axis", axis.id));
        if response.clicked() {
            set_editor_camera_axis_view(ui_state, axis.id);
            ui.ctx().request_repaint();
            clicked = true;
        }

        if axis.positive {
            ui.painter()
                .line_segment([center, axis_center], Stroke::new(2.0, axis.color));
        }
        let radius = if response.hovered() {
            axis.radius + 1.5
        } else {
            axis.radius
        };
        ui.painter().circle_filled(axis_center, radius, axis.color);
        if response.hovered() {
            ui.painter()
                .circle_stroke(axis_center, radius + 2.0, Stroke::new(1.0, Color32::WHITE));
        }
        paint_text_in_rect(
            ui,
            Rect::from_center_size(axis_center, Vec2::splat(axis.hit_size)),
            axis.label,
            FontId::proportional(if axis.positive { 10.0 } else { 9.0 }),
            Color32::WHITE,
            Align2::CENTER_CENTER,
        );
    }
    ui.painter()
        .circle_stroke(center, 40.0, Stroke::new(1.0, pal.border));
    clicked || dragged
}

fn orbit_editor_camera(ui_state: &mut ShellUiState, delta: Vec2) {
    ui_state.editor_camera_yaw -= delta.x * EDITOR_CAMERA_ORBIT_SENSITIVITY;
    ui_state.editor_camera_pitch = (ui_state.editor_camera_pitch
        + delta.y * EDITOR_CAMERA_ORBIT_SENSITIVITY)
        .clamp(-EDITOR_CAMERA_TOP_PITCH, EDITOR_CAMERA_TOP_PITCH);
}

fn set_editor_camera_axis_view(ui_state: &mut ShellUiState, axis: &str) {
    match axis {
        "+X" => {
            ui_state.editor_camera_yaw = std::f32::consts::FRAC_PI_2;
            ui_state.editor_camera_pitch = 0.0;
        }
        "-X" => {
            ui_state.editor_camera_yaw = -std::f32::consts::FRAC_PI_2;
            ui_state.editor_camera_pitch = 0.0;
        }
        "+Y" => {
            ui_state.editor_camera_yaw = 0.0;
            ui_state.editor_camera_pitch = EDITOR_CAMERA_TOP_PITCH;
        }
        "-Y" => {
            ui_state.editor_camera_yaw = 0.0;
            ui_state.editor_camera_pitch = -EDITOR_CAMERA_TOP_PITCH;
        }
        "+Z" => {
            ui_state.editor_camera_yaw = 0.0;
            ui_state.editor_camera_pitch = 0.0;
        }
        "-Z" => {
            ui_state.editor_camera_yaw = std::f32::consts::PI;
            ui_state.editor_camera_pitch = 0.0;
        }
        _ => {}
    }
}

#[derive(Clone, Copy, Debug)]
struct ProjectedAxis {
    id: &'static str,
    label: &'static str,
    offset: Vec2,
    depth: f32,
    color: Color32,
    radius: f32,
    hit_size: f32,
    positive: bool,
}

fn projected_orientation_axes(ui_state: &ShellUiState) -> Vec<ProjectedAxis> {
    let yaw = ui_state.editor_camera_yaw;
    let pitch = ui_state.editor_camera_pitch;
    let cos_pitch = pitch.cos();
    let eye_dir = [cos_pitch * yaw.sin(), pitch.sin(), cos_pitch * yaw.cos()];
    let forward = [-eye_dir[0], -eye_dir[1], -eye_dir[2]];
    let mut right = cross(forward, [0.0, 1.0, 0.0]);
    if length_squared(right) <= f32::EPSILON {
        right = [yaw.cos(), 0.0, -yaw.sin()];
    }
    let right = normalized(right);
    let up = normalized(cross(right, forward));

    let axis_radius = 29.0;
    [
        ("+X", "X", [1.0, 0.0, 0.0], rgb(220, 70, 70), true),
        ("-X", "-X", [-1.0, 0.0, 0.0], rgb(150, 58, 58), false),
        ("+Y", "Y", [0.0, 1.0, 0.0], rgb(95, 190, 95), true),
        ("-Y", "-Y", [0.0, -1.0, 0.0], rgb(64, 130, 64), false),
        ("+Z", "Z", [0.0, 0.0, 1.0], rgb(80, 130, 220), true),
        ("-Z", "-Z", [0.0, 0.0, -1.0], rgb(58, 86, 150), false),
    ]
    .into_iter()
    .map(|(id, label, dir, color, positive)| ProjectedAxis {
        id,
        label,
        offset: Vec2::new(dot(dir, right), -dot(dir, up)) * axis_radius,
        depth: dot(dir, eye_dir),
        color,
        radius: if positive { 7.0 } else { 5.5 },
        hit_size: if positive { 20.0 } else { 22.0 },
        positive,
    })
    .collect()
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length_squared(v: [f32; 3]) -> f32 {
    dot(v, v)
}

fn normalized(v: [f32; 3]) -> [f32; 3] {
    let len = length_squared(v).sqrt();
    if len <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

fn extract_render_world(shell: &EditorShell, scene_view: bool) -> RenderWorld {
    let Some(project) = shell.project() else {
        return RenderWorld::default();
    };
    let camera_entity = if scene_view {
        project.scene.main_camera().or_else(|| {
            shell
                .selected_entity_id()
                .and_then(|id| project.scene.find_by_id(id))
        })
    } else {
        project.scene.game_camera().or_else(|| {
            project.scene.objects().into_iter().find_map(|(entity, _)| {
                project.scene.components(entity).and_then(|components| {
                    components.iter().find_map(|component| match component {
                        ComponentData::Camera(camera) if camera.primary => Some(entity),
                        _ => None,
                    })
                })
            })
        })
    };
    let camera = camera_entity.and_then(|entity| {
        let object = project.scene.object(entity)?;
        let camera = project
            .scene
            .components(entity)?
            .iter()
            .find_map(|component| {
                if let ComponentData::Camera(camera) = component {
                    Some(camera)
                } else {
                    None
                }
            })?;
        Some(RenderCamera {
            object: object.id,
            transform: project
                .scene
                .transforms()
                .local(entity)
                .unwrap_or(Transform::IDENTITY),
            vertical_fov_degrees: camera.vertical_fov_degrees,
            near: camera.near,
            far: camera.far,
        })
    });
    let mut world = RenderWorld {
        camera,
        objects: Vec::new(),
        lights: Vec::new(),
    };
    for (entity, object) in project.scene.objects() {
        if !object.active {
            continue;
        }
        let transform = project
            .scene
            .transforms()
            .local(entity)
            .unwrap_or(Transform::IDENTITY);
        for component in project.scene.components(entity).unwrap_or(&[]) {
            match component {
                ComponentData::MeshRenderer(renderer) => world.objects.push(RenderObject {
                    object: object.id,
                    transform,
                    mesh: renderer
                        .builtin_mesh
                        .clone()
                        .or_else(|| renderer.mesh.map(|id| format!("{:032x}", id.as_u128())))
                        .unwrap_or_else(|| "missing-mesh".to_owned()),
                    material: renderer
                        .material
                        .builtin
                        .clone()
                        .or_else(|| {
                            renderer
                                .material
                                .asset
                                .map(|id| format!("{:032x}", id.as_u128()))
                        })
                        .unwrap_or_else(|| "missing-material".to_owned()),
                }),
                ComponentData::Light(light) => world.lights.push(RenderLight {
                    object: object.id,
                    transform,
                    kind: light.kind.clone(),
                    color: light.color,
                    intensity: light.intensity,
                    range: light.range,
                    spot_angle: light.spot_angle,
                }),
                _ => {}
            }
        }
    }
    world
}

/// Builds a [`RenderWorld`] for the scene view using the editor orbit camera.
/// Builds the render world for the editor viewport.
pub fn build_editor_render_world(shell: &EditorShell, ui_state: &ShellUiState) -> RenderWorld {
    let mut world = extract_render_world(shell, true);
    if let Some(ref mut camera) = world.camera {
        let yaw = ui_state.editor_camera_yaw;
        let pitch = ui_state.editor_camera_pitch;
        let dist = ui_state.editor_camera_distance;
        let target = &ui_state.editor_camera_target;

        let eye_x = target[0] + dist * pitch.cos() * yaw.sin();
        let eye_y = target[1] + dist * pitch.sin();
        let eye_z = target[2] + dist * pitch.cos() * yaw.cos();

        camera.transform.translation = EngineVec3::new(eye_x, eye_y, eye_z);

        let forward =
            EngineVec3::new(target[0] - eye_x, target[1] - eye_y, target[2] - eye_z).normalized();
        camera.transform.rotation = quat_look_at(forward, EngineVec3::new(0.0, 1.0, 0.0));
    }
    world
}

fn quat_look_at(forward: EngineVec3, up: EngineVec3) -> Quat {
    let forward = forward.normalized();
    if forward.length_squared() < f32::EPSILON {
        return Quat::IDENTITY;
    }
    let r_x = up.y * forward.z - up.z * forward.y;
    let r_y = up.z * forward.x - up.x * forward.z;
    let r_z = up.x * forward.y - up.y * forward.x;
    let right_len = (r_x * r_x + r_y * r_y + r_z * r_z).sqrt();
    let right = if right_len > f32::EPSILON {
        EngineVec3::new(r_x / right_len, r_y / right_len, r_z / right_len)
    } else {
        return Quat::IDENTITY;
    };
    let u_x = forward.y * right.z - forward.z * right.y;
    let u_y = forward.z * right.x - forward.x * right.z;
    let u_z = forward.x * right.y - forward.y * right.x;
    let up = EngineVec3::new(u_x, u_y, u_z);

    let m00 = right.x;
    let m01 = right.y;
    let m02 = right.z;
    let m10 = up.x;
    let m11 = up.y;
    let m12 = up.z;
    let m20 = -forward.x;
    let m21 = -forward.y;
    let m22 = -forward.z;

    let trace = m00 + m11 + m22;
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        Quat {
            x: (m21 - m12) / s,
            y: (m02 - m20) / s,
            z: (m10 - m01) / s,
            w: 0.25 * s,
        }
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        Quat {
            x: 0.25 * s,
            y: (m01 + m10) / s,
            z: (m02 + m20) / s,
            w: (m21 - m12) / s,
        }
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        Quat {
            x: (m01 + m10) / s,
            y: 0.25 * s,
            z: (m12 + m21) / s,
            w: (m02 - m20) / s,
        }
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        Quat {
            x: (m02 + m20) / s,
            y: (m12 + m21) / s,
            z: 0.25 * s,
            w: (m10 - m01) / s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_gizmo_axis_updates_editor_camera_angles() {
        let mut ui_state = ShellUiState::all_open();

        set_editor_camera_axis_view(&mut ui_state, "+X");
        assert_eq!(ui_state.editor_camera_yaw, std::f32::consts::FRAC_PI_2);
        assert_eq!(ui_state.editor_camera_pitch, 0.0);

        set_editor_camera_axis_view(&mut ui_state, "-X");
        assert_eq!(ui_state.editor_camera_yaw, -std::f32::consts::FRAC_PI_2);
        assert_eq!(ui_state.editor_camera_pitch, 0.0);

        set_editor_camera_axis_view(&mut ui_state, "+Y");
        assert_eq!(ui_state.editor_camera_yaw, 0.0);
        assert_eq!(ui_state.editor_camera_pitch, EDITOR_CAMERA_TOP_PITCH);

        set_editor_camera_axis_view(&mut ui_state, "-Y");
        assert_eq!(ui_state.editor_camera_yaw, 0.0);
        assert_eq!(ui_state.editor_camera_pitch, -EDITOR_CAMERA_TOP_PITCH);

        set_editor_camera_axis_view(&mut ui_state, "+Z");
        assert_eq!(ui_state.editor_camera_yaw, 0.0);
        assert_eq!(ui_state.editor_camera_pitch, 0.0);

        set_editor_camera_axis_view(&mut ui_state, "-Z");
        assert_eq!(ui_state.editor_camera_yaw, std::f32::consts::PI);
        assert_eq!(ui_state.editor_camera_pitch, 0.0);
    }

    #[test]
    fn orientation_gizmo_projection_tracks_camera_angles() {
        let mut ui_state = ShellUiState::all_open();
        ui_state.editor_camera_yaw = 0.0;
        ui_state.editor_camera_pitch = 0.0;
        let axes = projected_orientation_axes(&ui_state);
        let x = axes.iter().find(|axis| axis.id == "+X").unwrap();
        let y = axes.iter().find(|axis| axis.id == "+Y").unwrap();
        let z = axes.iter().find(|axis| axis.id == "+Z").unwrap();

        assert!(x.offset.x > 0.0);
        assert!(y.offset.y < 0.0);
        assert!(z.depth > 0.0);
    }

    #[test]
    fn orientation_gizmo_drag_orbits_editor_camera() {
        let mut ui_state = ShellUiState::all_open();
        ui_state.editor_camera_yaw = 0.0;
        ui_state.editor_camera_pitch = 0.0;

        orbit_editor_camera(&mut ui_state, Vec2::new(10.0, 20.0));

        assert_eq!(
            ui_state.editor_camera_yaw,
            -10.0 * EDITOR_CAMERA_ORBIT_SENSITIVITY
        );
        assert_eq!(
            ui_state.editor_camera_pitch,
            20.0 * EDITOR_CAMERA_ORBIT_SENSITIVITY
        );

        orbit_editor_camera(&mut ui_state, Vec2::new(0.0, 10_000.0));

        assert_eq!(ui_state.editor_camera_pitch, EDITOR_CAMERA_TOP_PITCH);
    }
}
