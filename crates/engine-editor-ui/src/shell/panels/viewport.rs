//! Viewport panel for the editor shell.

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::super::operations::scene_ops::{
    create_object_from_asset, push_scene_undo, scene_snapshot, select_first_scene_object,
};
use super::super::types::{
    rgb, EditorSceneViewOrientation, EditorSceneViewProjection, EditorTransformSpace,
    InfernuxPalette, ShellUiState, ViewportTargetState,
};
use super::super::widgets::layout::{empty_view, panel_title};
use super::super::widgets::text::{paint_text_in_rect, paint_wrapped_text_in_rect};
use super::console::draw_console;
use crate::EditorShell;
use engine_core::{
    math::{Quat, Transform, Vec3 as EngineVec3},
    EntityId,
};
use engine_ecs::ComponentData;
use engine_i18n::Translations;
use engine_render::{
    RenderCamera, RenderLight, RenderObject, RenderParticle, RenderProjection, RenderTargetDesc,
    RenderWorld, ViewKind,
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
    ui.vertical(|ui| {
        panel_title(ui, tr.tr("panel_console"), pal);
        draw_console(ui, shell, ui_state, pal, tr);
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
        tab.shrink2(Vec2::new(if scene_tools { 130.0 } else { 10.0 }, 0.0)),
        label,
        FontId::proportional(13.0),
        pal.text,
        Align2::LEFT_CENTER,
    );
    if scene_tools {
        let menu_rect = Rect::from_min_size(tab.min + Vec2::new(8.0, 3.0), Vec2::new(112.0, 20.0));
        draw_scene_view_menu(ui, menu_rect, ui_state, tr);
    }

    let content_rect = rect.shrink2(Vec2::new(0.0, 26.0));
    draw_render_viewport(ui, shell, ui_state, content_rect, scene_tools, pal, tr);

    let mut viewport_interaction_consumed = false;
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

        viewport_interaction_consumed |=
            draw_scene_overlay(ui, content_rect, shell, ui_state, pal, tr);
        viewport_interaction_consumed |= draw_orientation_gizmo(ui, rect, ui_state, pal);
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

    if response.clicked() && !viewport_interaction_consumed {
        select_first_scene_object(shell);
    }
}

fn draw_scene_view_menu(
    ui: &mut egui::Ui,
    rect: Rect,
    ui_state: &mut ShellUiState,
    tr: &Translations,
) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.set_width(rect.width());
        ui.menu_button(scene_view_label(ui_state, tr), |ui| {
            if ui.button(tr.tr("viewport_view_2d")).clicked() {
                set_editor_camera_2d_view(ui_state);
                ui.close();
            }
            if ui.button(tr.tr("viewport_view_3d")).clicked() {
                set_editor_camera_3d_view(ui_state);
                ui.close();
            }
            ui.separator();
            for (orientation, key) in [
                (EditorSceneViewOrientation::Top, "viewport_view_top"),
                (EditorSceneViewOrientation::Bottom, "viewport_view_bottom"),
                (EditorSceneViewOrientation::Left, "viewport_view_left"),
                (EditorSceneViewOrientation::Right, "viewport_view_right"),
                (EditorSceneViewOrientation::Front, "viewport_view_front"),
                (EditorSceneViewOrientation::Rear, "viewport_view_rear"),
            ] {
                if ui.button(tr.tr(key)).clicked() {
                    set_editor_camera_orientation(ui_state, orientation);
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .button(match ui_state.editor_scene_view_projection {
                    EditorSceneViewProjection::Perspective => {
                        tr.tr("viewport_projection_orthographic")
                    }
                    EditorSceneViewProjection::Orthographic => {
                        tr.tr("viewport_projection_perspective")
                    }
                })
                .clicked()
            {
                toggle_editor_camera_projection(ui_state);
                ui.close();
            }
            ui.checkbox(
                &mut ui_state.editor_scene_view_auto_orthographic,
                tr.tr("viewport_auto_orthographic"),
            );
        });
    });
}

fn scene_view_label(ui_state: &ShellUiState, tr: &Translations) -> String {
    let projection = match ui_state.editor_scene_view_projection {
        EditorSceneViewProjection::Perspective => tr.tr("viewport_projection_perspective"),
        EditorSceneViewProjection::Orthographic => tr.tr("viewport_projection_orthographic"),
    };
    match ui_state.editor_scene_view_orientation {
        EditorSceneViewOrientation::Free => projection,
        EditorSceneViewOrientation::Top => tr.tr("viewport_view_top"),
        EditorSceneViewOrientation::Bottom => tr.tr("viewport_view_bottom"),
        EditorSceneViewOrientation::Left => tr.tr("viewport_view_left"),
        EditorSceneViewOrientation::Right => tr.tr("viewport_view_right"),
        EditorSceneViewOrientation::Front => tr.tr("viewport_view_front"),
        EditorSceneViewOrientation::Rear => tr.tr("viewport_view_rear"),
    }
    .to_owned()
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
        build_editor_render_world(shell, ui_state)
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
    match state.world.camera.as_ref().map(|camera| camera.projection) {
        Some(RenderProjection::Orthographic { .. }) => {
            paint_orthographic_reference_grid(ui, rect, horizon);
        }
        _ => {
            paint_perspective_reference_grid(ui, rect, horizon);
        }
    }
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

fn paint_perspective_reference_grid(ui: &mut egui::Ui, rect: Rect, horizon: f32) {
    let minor = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 24));
    let major = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 42));
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), horizon),
            Pos2::new(rect.right(), horizon),
        ],
        major,
    );

    let vanishing = Pos2::new(rect.center().x, horizon);
    for i in -6..=6 {
        let t = i as f32 / 6.0;
        let x = rect.center().x + t * rect.width() * 0.75;
        ui.painter()
            .line_segment([vanishing, Pos2::new(x, rect.bottom())], minor);
    }

    for i in 1..=8 {
        let depth = i as f32 / 8.0;
        let y = horizon + (rect.bottom() - horizon) * depth.powf(1.75);
        let inset = rect.width() * 0.5 * (1.0 - depth);
        ui.painter().line_segment(
            [
                Pos2::new(rect.left() + inset, y),
                Pos2::new(rect.right() - inset, y),
            ],
            minor,
        );
    }
}

fn paint_orthographic_reference_grid(ui: &mut egui::Ui, rect: Rect, horizon: f32) {
    let minor = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 22));
    let major = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 42));
    let spacing = 28.0;
    let mut x = rect.center().x;
    while x <= rect.right() {
        let stroke = if (x - rect.center().x).abs() < f32::EPSILON {
            major
        } else {
            minor
        };
        ui.painter()
            .line_segment([Pos2::new(x, horizon), Pos2::new(x, rect.bottom())], stroke);
        x += spacing;
    }
    let mut x = rect.center().x - spacing;
    while x >= rect.left() {
        ui.painter()
            .line_segment([Pos2::new(x, horizon), Pos2::new(x, rect.bottom())], minor);
        x -= spacing;
    }

    let mut y = horizon;
    while y <= rect.bottom() {
        let stroke = if (y - horizon).abs() < f32::EPSILON {
            major
        } else {
            minor
        };
        ui.painter().line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            stroke,
        );
        y += spacing;
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

fn draw_scene_overlay(
    ui: &mut egui::Ui,
    rect: Rect,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
    tr: &Translations,
) -> bool {
    let guides = collect_scene_guides(shell);
    let consumed = paint_scene_guides(ui, rect, shell, ui_state, &guides, pal);
    finish_scene_guide_drag_if_released(ui, shell, ui_state);

    let cursor = rect.min + Vec2::new(8.0, 8.0);

    let pill = Rect::from_min_size(cursor, Vec2::new(86.0, 22.0));
    ui.painter().rect_filled(
        pill,
        CornerRadius::same(4),
        Color32::from_rgba_premultiplied(35, 35, 35, 220),
    );
    paint_text_in_rect(
        ui,
        pill.shrink2(Vec2::new(6.0, 0.0)),
        transform_space_label(ui_state.editor_transform_space, tr),
        FontId::proportional(12.0),
        pal.text,
        Align2::CENTER_CENTER,
    );
    consumed
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SceneGuideKind {
    Camera,
    Light,
}

#[derive(Clone, Debug, PartialEq)]
struct SceneGuide {
    id: EntityId,
    name: String,
    kind: SceneGuideKind,
    position: EngineVec3,
    direction: EngineVec3,
}

fn collect_scene_guides(shell: &EditorShell) -> Vec<SceneGuide> {
    let Some(project) = shell.project() else {
        return Vec::new();
    };

    let mut guides = Vec::new();
    for (entity, object) in project.scene.objects() {
        if !object.active {
            continue;
        }
        let transform = project
            .scene
            .transforms()
            .local(entity)
            .unwrap_or(Transform::IDENTITY);
        let direction = transform
            .rotation
            .rotate(EngineVec3::new(0.0, 0.0, -1.0))
            .normalized();

        for component in project.scene.components(entity).unwrap_or(&[]) {
            let kind = match component {
                ComponentData::Camera(_) => Some(SceneGuideKind::Camera),
                ComponentData::Light(_) => Some(SceneGuideKind::Light),
                _ => None,
            };
            if let Some(kind) = kind {
                guides.push(SceneGuide {
                    id: object.id,
                    name: object.name.clone(),
                    kind,
                    position: transform.translation,
                    direction,
                });
            }
        }
    }
    guides
}

fn paint_scene_guides(
    ui: &mut egui::Ui,
    rect: Rect,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    guides: &[SceneGuide],
    pal: &InfernuxPalette,
) -> bool {
    let camera = scene_overlay_camera(ui_state);
    let mut consumed = false;
    for guide in guides {
        let Some(origin) = project_world_to_viewport(guide.position, rect, &camera) else {
            continue;
        };
        let end_world = guide.position + guide.direction.normalized() * guide_length(guide.kind);
        let end = project_world_to_viewport(end_world, rect, &camera).unwrap_or(origin);
        let color = match guide.kind {
            SceneGuideKind::Camera => rgb(95, 155, 235),
            SceneGuideKind::Light => pal.warning,
        };
        let label = match guide.kind {
            SceneGuideKind::Camera => format!("CAM {}", guide.name),
            SceneGuideKind::Light => format!("LIGHT {}", guide.name),
        };

        ui.painter()
            .line_segment([origin, end], Stroke::new(1.5, color));
        draw_arrow_head(ui, origin, end, color);
        let icon_rect = match guide.kind {
            SceneGuideKind::Camera => {
                let icon = Rect::from_center_size(origin, Vec2::new(18.0, 12.0));
                ui.painter().rect_filled(
                    icon,
                    CornerRadius::same(2),
                    Color32::from_rgba_premultiplied(20, 35, 55, 220),
                );
                ui.painter().rect_stroke(
                    icon,
                    CornerRadius::same(2),
                    Stroke::new(1.0, color),
                    StrokeKind::Inside,
                );
                let lens = [
                    Pos2::new(icon.right(), icon.center().y - 4.0),
                    Pos2::new(icon.right() + 7.0, icon.center().y),
                    Pos2::new(icon.right(), icon.center().y + 4.0),
                ];
                ui.painter().add(egui::Shape::convex_polygon(
                    lens.to_vec(),
                    Color32::from_rgba_premultiplied(20, 35, 55, 220),
                    Stroke::new(1.0, color),
                ));
                Rect::from_min_max(icon.min, Pos2::new(icon.right() + 7.0, icon.bottom()))
            }
            SceneGuideKind::Light => {
                ui.painter().circle_filled(
                    origin,
                    7.0,
                    Color32::from_rgba_premultiplied(70, 50, 20, 220),
                );
                ui.painter()
                    .circle_stroke(origin, 7.0, Stroke::new(1.5, color));
                ui.painter().circle_stroke(
                    origin,
                    11.0,
                    Stroke::new(
                        1.0,
                        Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), 90),
                    ),
                );
                Rect::from_center_size(origin, Vec2::splat(24.0))
            }
        };

        let move_response = ui
            .interact(
                icon_rect.expand(4.0),
                ui.make_persistent_id(("scene_guide_move", guide.id.as_u128(), guide.kind)),
                Sense::click_and_drag(),
            )
            .on_hover_text("Drag to move");
        if move_response.clicked() {
            shell.select_entity_id(guide.id);
        }
        consumed |= move_response.clicked() || move_response.dragged();
        if move_response.drag_started() {
            begin_scene_guide_drag(shell, ui_state, guide.id);
        }
        if move_response.dragged() {
            let delta = ui.ctx().input(|input| input.pointer.delta());
            translate_scene_guide(shell, &camera, rect, guide, delta);
            ui.ctx().request_repaint();
        }

        let direction_response = ui
            .interact(
                Rect::from_center_size(end, Vec2::splat(22.0)),
                ui.make_persistent_id(("scene_guide_direction", guide.id.as_u128(), guide.kind)),
                Sense::click_and_drag(),
            )
            .on_hover_text("Drag to aim");
        if direction_response.clicked() {
            shell.select_entity_id(guide.id);
        }
        consumed |= direction_response.clicked() || direction_response.dragged();
        if direction_response.drag_started() {
            begin_scene_guide_drag(shell, ui_state, guide.id);
        }
        if direction_response.dragged() {
            if let Some(pointer) = ui.ctx().input(|input| input.pointer.interact_pos()) {
                aim_scene_guide(shell, &camera, rect, guide, pointer);
                ui.ctx().request_repaint();
            }
        }

        let label_rect =
            Rect::from_min_size(origin + Vec2::new(12.0, -18.0), Vec2::new(150.0, 18.0));
        paint_text_in_rect(
            ui,
            label_rect,
            &label,
            FontId::proportional(11.0),
            pal.text,
            Align2::LEFT_CENTER,
        );
    }
    consumed
}

fn begin_scene_guide_drag(shell: &EditorShell, ui_state: &mut ShellUiState, guide_id: EntityId) {
    if ui_state.scene_guide_drag_before.is_none() {
        if let Some(before) = scene_snapshot(shell) {
            ui_state.scene_guide_drag_before = Some((guide_id, before));
        }
    }
}

fn finish_scene_guide_drag_if_released(
    ui: &egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
) {
    if ui_state.scene_guide_drag_before.is_none() {
        return;
    }
    if !ui.input(|input| input.pointer.any_released()) {
        return;
    }
    if let Some((guide_id, before)) = ui_state.scene_guide_drag_before.take() {
        push_scene_undo(
            shell,
            "Adjust Scene Guide",
            format!("{:032x}", guide_id.as_u128()),
            Some(before),
        );
    }
}

fn translate_scene_guide(
    shell: &mut EditorShell,
    camera: &SceneOverlayCamera,
    rect: Rect,
    guide: &SceneGuide,
    delta: Vec2,
) {
    let depth = (guide.position - camera.eye).dot(camera.forward).max(0.01);
    let world_delta = screen_delta_to_world_delta(delta, rect, camera, depth);
    if world_delta.length_squared() <= f32::EPSILON {
        return;
    }
    update_scene_guide_transform(shell, guide.id, |transform| {
        transform.translation += world_delta;
    });
}

fn aim_scene_guide(
    shell: &mut EditorShell,
    camera: &SceneOverlayCamera,
    rect: Rect,
    guide: &SceneGuide,
    pointer: Pos2,
) {
    let end_world = guide.position + guide.direction.normalized() * guide_length(guide.kind);
    let depth = (end_world - camera.eye).dot(camera.forward).max(0.01);
    let Some(world_point) = screen_to_world_at_depth(pointer, rect, camera, depth) else {
        return;
    };
    let direction = (world_point - guide.position).normalized();
    if direction.length_squared() <= f32::EPSILON {
        return;
    }
    update_scene_guide_transform(shell, guide.id, |transform| {
        transform.rotation = quat_look_at(direction, EngineVec3::new(0.0, 1.0, 0.0));
    });
}

fn update_scene_guide_transform(
    shell: &mut EditorShell,
    guide_id: EntityId,
    update: impl FnOnce(&mut Transform),
) {
    if let Some(project) = shell.project_mut() {
        if let Some(entity) = project.scene.find_by_id(guide_id) {
            if let Some(mut transform) = project.scene.transforms().local(entity) {
                update(&mut transform);
                project.scene.transforms_mut().set_local(entity, transform);
                project.scene_dirty = true;
            }
        }
    }
}

fn draw_arrow_head(ui: &mut egui::Ui, origin: Pos2, end: Pos2, color: Color32) {
    let delta = end - origin;
    let len = delta.length();
    if len <= 8.0 {
        return;
    }
    let dir = delta / len;
    let side = Vec2::new(-dir.y, dir.x);
    let tip = end;
    let left = tip - dir * 8.0 + side * 4.0;
    let right = tip - dir * 8.0 - side * 4.0;
    ui.painter()
        .line_segment([left, tip], Stroke::new(1.5, color));
    ui.painter()
        .line_segment([right, tip], Stroke::new(1.5, color));
}

fn guide_length(kind: SceneGuideKind) -> f32 {
    match kind {
        SceneGuideKind::Camera => 1.4,
        SceneGuideKind::Light => 1.0,
    }
}

#[derive(Clone, Copy, Debug)]
struct SceneOverlayCamera {
    eye: EngineVec3,
    forward: EngineVec3,
    right: EngineVec3,
    up: EngineVec3,
    projection: EditorSceneViewProjection,
    vertical_fov_degrees: f32,
    orthographic_vertical_size: f32,
}

fn scene_overlay_camera(ui_state: &ShellUiState) -> SceneOverlayCamera {
    let yaw = ui_state.editor_camera_yaw;
    let pitch = ui_state.editor_camera_pitch;
    let dist = ui_state.editor_camera_distance;
    let target = EngineVec3::new(
        ui_state.editor_camera_target[0],
        ui_state.editor_camera_target[1],
        ui_state.editor_camera_target[2],
    );

    let eye = EngineVec3::new(
        target.x + dist * pitch.cos() * yaw.sin(),
        target.y + dist * pitch.sin(),
        target.z + dist * pitch.cos() * yaw.cos(),
    );
    let forward = (target - eye).normalized();
    let world_up = EngineVec3::new(0.0, 1.0, 0.0);
    let mut right = forward.cross(world_up).normalized();
    if right.length_squared() <= f32::EPSILON {
        right = EngineVec3::new(yaw.cos(), 0.0, -yaw.sin()).normalized();
    }
    let up = right.cross(forward).normalized();

    SceneOverlayCamera {
        eye,
        forward,
        right,
        up,
        projection: ui_state.editor_scene_view_projection,
        vertical_fov_degrees: 60.0,
        orthographic_vertical_size: ui_state.editor_camera_distance * 2.0,
    }
}

fn project_world_to_viewport(
    point: EngineVec3,
    rect: Rect,
    camera: &SceneOverlayCamera,
) -> Option<Pos2> {
    let local = point - camera.eye;
    let x = local.dot(camera.right);
    let y = local.dot(camera.up);
    let z = local.dot(camera.forward);
    let aspect = (rect.width() / rect.height().max(1.0)).max(0.001);

    let (ndc_x, ndc_y) = match camera.projection {
        EditorSceneViewProjection::Perspective => {
            if z <= 0.01 {
                return None;
            }
            let f = 1.0 / (camera.vertical_fov_degrees.to_radians() * 0.5).tan();
            ((x * f / aspect) / z, (y * f) / z)
        }
        EditorSceneViewProjection::Orthographic => {
            let half_h = (camera.orthographic_vertical_size * 0.5).max(0.001);
            let half_w = half_h * aspect;
            (x / half_w, y / half_h)
        }
    };

    if ndc_x.abs() > 1.15 || ndc_y.abs() > 1.15 {
        return None;
    }

    Some(Pos2::new(
        rect.center().x + ndc_x * rect.width() * 0.5,
        rect.center().y - ndc_y * rect.height() * 0.5,
    ))
}

fn screen_delta_to_world_delta(
    delta: Vec2,
    rect: Rect,
    camera: &SceneOverlayCamera,
    depth: f32,
) -> EngineVec3 {
    let units_per_pixel = match camera.projection {
        EditorSceneViewProjection::Perspective => {
            let visible_height =
                2.0 * depth * (camera.vertical_fov_degrees.to_radians() * 0.5).tan();
            visible_height / rect.height().max(1.0)
        }
        EditorSceneViewProjection::Orthographic => {
            camera.orthographic_vertical_size / rect.height().max(1.0)
        }
    };
    camera.right * (delta.x * units_per_pixel) - camera.up * (delta.y * units_per_pixel)
}

fn screen_to_world_at_depth(
    position: Pos2,
    rect: Rect,
    camera: &SceneOverlayCamera,
    depth: f32,
) -> Option<EngineVec3> {
    if !rect.is_positive() {
        return None;
    }
    let ndc_x = ((position.x - rect.center().x) / (rect.width() * 0.5)).clamp(-4.0, 4.0);
    let ndc_y = ((rect.center().y - position.y) / (rect.height() * 0.5)).clamp(-4.0, 4.0);
    let aspect = (rect.width() / rect.height().max(1.0)).max(0.001);
    let (x, y) = match camera.projection {
        EditorSceneViewProjection::Perspective => {
            let f = 1.0 / (camera.vertical_fov_degrees.to_radians() * 0.5).tan();
            (ndc_x * depth * aspect / f, ndc_y * depth / f)
        }
        EditorSceneViewProjection::Orthographic => {
            let half_h = (camera.orthographic_vertical_size * 0.5).max(0.001);
            let half_w = half_h * aspect;
            (ndc_x * half_w, ndc_y * half_h)
        }
    };
    Some(camera.eye + camera.forward * depth + camera.right * x + camera.up * y)
}

fn transform_space_label(space: EditorTransformSpace, tr: &Translations) -> &str {
    match space {
        EditorTransformSpace::Global => tr.tr("tool_global"),
        EditorTransformSpace::Local => tr.tr("tool_local"),
    }
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
    ui_state.editor_scene_view_orientation = EditorSceneViewOrientation::Free;
    if ui_state.editor_scene_view_auto_orthographic {
        ui_state.editor_scene_view_projection = EditorSceneViewProjection::Perspective;
    }
}

fn set_editor_camera_axis_view(ui_state: &mut ShellUiState, axis: &str) {
    let orientation = match axis {
        "+X" => Some(EditorSceneViewOrientation::Right),
        "-X" => Some(EditorSceneViewOrientation::Left),
        "+Y" => Some(EditorSceneViewOrientation::Top),
        "-Y" => Some(EditorSceneViewOrientation::Bottom),
        "+Z" => Some(EditorSceneViewOrientation::Front),
        "-Z" => Some(EditorSceneViewOrientation::Rear),
        _ => None,
    };
    if let Some(orientation) = orientation {
        set_editor_camera_orientation(ui_state, orientation);
    }
}

fn set_editor_camera_orientation(
    ui_state: &mut ShellUiState,
    orientation: EditorSceneViewOrientation,
) {
    match orientation {
        EditorSceneViewOrientation::Right => {
            ui_state.editor_camera_yaw = std::f32::consts::FRAC_PI_2;
            ui_state.editor_camera_pitch = 0.0;
        }
        EditorSceneViewOrientation::Left => {
            ui_state.editor_camera_yaw = -std::f32::consts::FRAC_PI_2;
            ui_state.editor_camera_pitch = 0.0;
        }
        EditorSceneViewOrientation::Top => {
            ui_state.editor_camera_yaw = 0.0;
            ui_state.editor_camera_pitch = EDITOR_CAMERA_TOP_PITCH;
        }
        EditorSceneViewOrientation::Bottom => {
            ui_state.editor_camera_yaw = 0.0;
            ui_state.editor_camera_pitch = -EDITOR_CAMERA_TOP_PITCH;
        }
        EditorSceneViewOrientation::Front => {
            ui_state.editor_camera_yaw = 0.0;
            ui_state.editor_camera_pitch = 0.0;
        }
        EditorSceneViewOrientation::Rear => {
            ui_state.editor_camera_yaw = std::f32::consts::PI;
            ui_state.editor_camera_pitch = 0.0;
        }
        EditorSceneViewOrientation::Free => {}
    }
    ui_state.editor_scene_view_orientation = orientation;
    if orientation != EditorSceneViewOrientation::Free
        && ui_state.editor_scene_view_auto_orthographic
    {
        ui_state.editor_scene_view_projection = EditorSceneViewProjection::Orthographic;
    }
}

fn set_editor_camera_2d_view(ui_state: &mut ShellUiState) {
    set_editor_camera_orientation(ui_state, EditorSceneViewOrientation::Top);
    ui_state.editor_scene_view_projection = EditorSceneViewProjection::Orthographic;
    ui_state.editor_camera_target[1] = 0.0;
}

fn set_editor_camera_3d_view(ui_state: &mut ShellUiState) {
    ui_state.editor_scene_view_orientation = EditorSceneViewOrientation::Free;
    ui_state.editor_scene_view_projection = EditorSceneViewProjection::Perspective;
    ui_state.editor_camera_pitch = 0.3;
}

fn toggle_editor_camera_projection(ui_state: &mut ShellUiState) {
    ui_state.editor_scene_view_projection = match ui_state.editor_scene_view_projection {
        EditorSceneViewProjection::Perspective => EditorSceneViewProjection::Orthographic,
        EditorSceneViewProjection::Orthographic => EditorSceneViewProjection::Perspective,
    };
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
            projection: RenderProjection::Perspective,
            vertical_fov_degrees: camera.vertical_fov_degrees,
            near: camera.near,
            far: camera.far,
        })
    });
    let mut world = RenderWorld {
        camera,
        objects: Vec::new(),
        lights: Vec::new(),
        particles: Vec::new(),
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
                ComponentData::ParticleEmitter(emitter) => world.particles.extend(
                    engine_ecs::ParticleSystem::sample(emitter, transform)
                        .into_iter()
                        .map(|particle| {
                            let mut particle_transform = Transform::IDENTITY;
                            particle_transform.translation = particle.position;
                            particle_transform.scale =
                                EngineVec3::new(particle.size, particle.size, particle.size);
                            RenderParticle {
                                object: object.id,
                                transform: particle_transform,
                                color: particle.color,
                                age_fraction: particle.age_fraction,
                            }
                        }),
                ),
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
        camera.projection = match ui_state.editor_scene_view_projection {
            EditorSceneViewProjection::Perspective => RenderProjection::Perspective,
            EditorSceneViewProjection::Orthographic => RenderProjection::Orthographic {
                vertical_size: ui_state.editor_camera_distance * 2.0,
            },
        };
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
        assert_eq!(
            ui_state.editor_scene_view_orientation,
            EditorSceneViewOrientation::Right
        );
        assert_eq!(
            ui_state.editor_scene_view_projection,
            EditorSceneViewProjection::Orthographic
        );

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
    fn scene_view_modes_switch_projection_and_orientation() {
        let mut ui_state = ShellUiState::all_open();
        assert_eq!(
            ui_state.editor_transform_space,
            EditorTransformSpace::Global
        );

        set_editor_camera_2d_view(&mut ui_state);
        assert_eq!(
            ui_state.editor_scene_view_orientation,
            EditorSceneViewOrientation::Top
        );
        assert_eq!(
            ui_state.editor_scene_view_projection,
            EditorSceneViewProjection::Orthographic
        );
        assert_eq!(ui_state.editor_camera_target[1], 0.0);

        set_editor_camera_3d_view(&mut ui_state);
        assert_eq!(
            ui_state.editor_scene_view_orientation,
            EditorSceneViewOrientation::Free
        );
        assert_eq!(
            ui_state.editor_scene_view_projection,
            EditorSceneViewProjection::Perspective
        );

        toggle_editor_camera_projection(&mut ui_state);
        assert_eq!(
            ui_state.editor_scene_view_projection,
            EditorSceneViewProjection::Orthographic
        );
    }

    #[test]
    fn transform_space_is_independent_from_scene_view_projection() {
        let mut ui_state = ShellUiState::all_open();

        ui_state.editor_transform_space = EditorTransformSpace::Local;
        toggle_editor_camera_projection(&mut ui_state);
        set_editor_camera_orientation(&mut ui_state, EditorSceneViewOrientation::Front);

        assert_eq!(ui_state.editor_transform_space, EditorTransformSpace::Local);
        assert_eq!(
            ui_state.editor_scene_view_projection,
            EditorSceneViewProjection::Orthographic
        );
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
            ui_state.editor_scene_view_orientation,
            EditorSceneViewOrientation::Free
        );
        assert_eq!(
            ui_state.editor_scene_view_projection,
            EditorSceneViewProjection::Perspective
        );
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

    #[test]
    fn scene_overlay_camera_projects_target_to_view_center() {
        let ui_state = ShellUiState::all_open();
        let camera = scene_overlay_camera(&ui_state);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let target = EngineVec3::new(
            ui_state.editor_camera_target[0],
            ui_state.editor_camera_target[1],
            ui_state.editor_camera_target[2],
        );

        let projected = project_world_to_viewport(target, rect, &camera).unwrap();

        assert!((projected.x - rect.center().x).abs() < 0.001);
        assert!((projected.y - rect.center().y).abs() < 0.001);
    }

    #[test]
    fn guide_lengths_make_camera_handles_more_visible_than_lights() {
        assert!(guide_length(SceneGuideKind::Camera) > guide_length(SceneGuideKind::Light));
    }
}
