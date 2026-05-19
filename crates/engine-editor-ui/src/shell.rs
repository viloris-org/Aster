//! egui rendering for [`EditorShell`].
//!
//! Call [`draw_shell`] once per frame inside an egui context.

#![allow(deprecated)] // egui 0.34 keeps Panel::show(ctx) available.

use egui::{
    Align2, Color32, CornerRadius, DragValue, FontId, Frame, Margin, Pos2, Rect, RichText, Sense,
    Stroke, StrokeKind, Vec2,
};

use crate::{asset_guid_label, resource_kind_label, EditorShell};
use engine_ecs::ComponentData;
use engine_editor::ConsoleLevel;

fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

#[derive(Clone, Copy)]
struct InfernuxPalette {
    text: Color32,
    text_dim: Color32,
    text_disabled: Color32,
    window_bg: Color32,
    panel_bg: Color32,
    menu_bar: Color32,
    status_bar: Color32,
    viewport_bg: Color32,
    frame_bg: Color32,
    frame_hover: Color32,
    header: Color32,
    header_hover: Color32,
    border: Color32,
    row_alt: Color32,
    selection: Color32,
    accent: Color32,
    play: Color32,
    pause: Color32,
    warning: Color32,
    error: Color32,
}

impl InfernuxPalette {
    const fn dark() -> Self {
        Self {
            text: Color32::from_rgb(214, 214, 214),
            text_dim: Color32::from_rgb(140, 140, 140),
            text_disabled: Color32::from_rgb(102, 102, 102),
            window_bg: Color32::from_rgb(56, 56, 56),
            panel_bg: Color32::from_rgb(54, 54, 54),
            menu_bar: Color32::from_rgb(41, 41, 41),
            status_bar: Color32::from_rgb(33, 33, 33),
            viewport_bg: Color32::from_rgb(31, 31, 31),
            frame_bg: Color32::from_rgb(42, 42, 42),
            frame_hover: Color32::from_rgb(51, 45, 45),
            header: Color32::from_rgb(60, 60, 60),
            header_hover: Color32::from_rgb(71, 61, 61),
            border: Color32::from_rgb(26, 26, 26),
            row_alt: Color32::from_rgba_premultiplied(0, 0, 0, 22),
            selection: Color32::from_rgb(44, 93, 135),
            accent: Color32::from_rgb(235, 87, 87),
            play: Color32::from_rgb(51, 115, 77),
            pause: Color32::from_rgb(128, 102, 38),
            warning: Color32::from_rgb(227, 181, 77),
            error: Color32::from_rgb(235, 87, 87),
        }
    }
}

/// Transient UI state for the editor shell.
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
}

impl ShellUiState {
    /// Creates a default state with the Infernux editor panels open.
    pub fn all_open() -> Self {
        Self {
            show_hierarchy: true,
            show_inspector: true,
            show_project: true,
            show_console: true,
            show_scene_view: true,
            show_game_view: true,
            playing: false,
            paused: false,
            hierarchy_filter: String::new(),
            project_filter: String::new(),
            console_filter: String::new(),
            console_collapse: false,
        }
    }
}

/// Draw the full editor shell into `ctx`.
///
/// Returns `true` when the user requests the window to close.
pub fn draw_shell(
    ctx: &egui::Context,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
) -> bool {
    let pal = InfernuxPalette::dark();
    apply_visuals(ctx, &pal);

    let close = false;

    egui::TopBottomPanel::top("infernux_menu_bar")
        .exact_size(26.0)
        .frame(
            Frame::NONE
                .fill(pal.menu_bar)
                .inner_margin(Margin::symmetric(8, 0)),
        )
        .show(ctx, |ui| draw_menu_bar(ui, shell, ui_state, &pal));

    egui::TopBottomPanel::top("infernux_toolbar")
        .exact_size(36.0)
        .frame(
            Frame::NONE
                .fill(pal.panel_bg)
                .inner_margin(Margin::symmetric(6, 3)),
        )
        .show(ctx, |ui| draw_toolbar(ui, shell, ui_state, &pal));

    egui::TopBottomPanel::bottom("infernux_status_bar")
        .exact_size(24.0)
        .frame(
            Frame::NONE
                .fill(pal.status_bar)
                .inner_margin(Margin::symmetric(8, 0)),
        )
        .show(ctx, |ui| draw_status_bar(ui, shell, ui_state, &pal));

    if ui_state.show_hierarchy {
        egui::SidePanel::left("infernux_hierarchy")
            .default_size(260.0)
            .min_width(180.0)
            .frame(panel_frame(&pal))
            .show(ctx, |ui| {
                panel_title(ui, "Hierarchy", &pal);
                draw_hierarchy(ui, shell, ui_state, &pal);
            });
    }

    if ui_state.show_inspector {
        egui::SidePanel::right("infernux_inspector")
            .default_size(330.0)
            .min_width(240.0)
            .frame(panel_frame(&pal))
            .show(ctx, |ui| {
                panel_title(ui, "Inspector", &pal);
                draw_inspector(ui, shell, &pal);
            });
    }

    if ui_state.show_project || ui_state.show_console {
        egui::TopBottomPanel::bottom("infernux_bottom_dock")
            .default_height(230.0)
            .min_height(120.0)
            .frame(panel_frame(&pal))
            .show(ctx, |ui| draw_bottom_dock(ui, shell, ui_state, &pal));
    }

    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(pal.window_bg))
        .show(ctx, |ui| draw_center_dock(ui, shell, ui_state, &pal));

    close
}

fn draw_menu_bar(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
) {
    ui.horizontal_centered(|ui| {
        for label in [
            "File",
            "Edit",
            "Assets",
            "GameObject",
            "Component",
            "Window",
            "Help",
        ] {
            ghost_button(ui, label, 54.0, pal);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new("Infernux-style Editor")
                    .size(12.0)
                    .color(pal.text_dim),
            );
            let title = shell
                .project()
                .map(|project| project.name().to_owned())
                .unwrap_or_else(|| "Untitled".to_owned());
            ui.label(RichText::new(title).size(12.0).color(pal.text));
            if ui_state.playing {
                ui.label(RichText::new("PLAY").size(11.0).strong().color(pal.play));
            }
        });
    });
}

fn draw_toolbar(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
) {
    ui.horizontal_centered(|ui| {
        tool_button(ui, "Q", "View Tool", false, pal);
        tool_button(ui, "W", "Move Tool", true, pal);
        tool_button(ui, "E", "Rotate Tool", false, pal);
        tool_button(ui, "R", "Scale Tool", false, pal);
        ui.add_space(10.0);
        dropdown_pill(ui, "Global", 76.0, pal);
        dropdown_pill(ui, "Pivot", 68.0, pal);

        ui.with_layout(
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                if transport_button(ui, "▶", ui_state.playing, pal.play, pal).clicked() {
                    ui_state.playing = !ui_state.playing;
                    ui_state.paused = false;
                }
                if transport_button(ui, "⏸", ui_state.paused, pal.pause, pal).clicked() {
                    ui_state.paused = !ui_state.paused;
                }
                if transport_button(ui, "■", false, pal.accent, pal).clicked() {
                    ui_state.playing = false;
                    ui_state.paused = false;
                }
            },
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if small_text_button(ui, "Save", pal).clicked() {
                save_scene(shell);
            }
            panel_toggle(ui, "Game", &mut ui_state.show_game_view, pal);
            panel_toggle(ui, "Scene", &mut ui_state.show_scene_view, pal);
            panel_toggle(ui, "Console", &mut ui_state.show_console, pal);
            panel_toggle(ui, "Project", &mut ui_state.show_project, pal);
            panel_toggle(ui, "Inspector", &mut ui_state.show_inspector, pal);
            panel_toggle(ui, "Hierarchy", &mut ui_state.show_hierarchy, pal);
        });
    });
}

fn draw_status_bar(
    ui: &mut egui::Ui,
    shell: &EditorShell,
    ui_state: &ShellUiState,
    pal: &InfernuxPalette,
) {
    ui.horizontal_centered(|ui| {
        let status = if ui_state.playing {
            "Play Mode"
        } else if shell.project().is_some() {
            "Ready"
        } else {
            "No project loaded"
        };
        ui.label(RichText::new(status).size(11.0).color(pal.text_dim));
        ui.separator();
        ui.label(
            RichText::new(format!("Console: {}", shell.console().entries().len()))
                .size(11.0)
                .color(pal.text_dim),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let rect = ui
                .allocate_exact_size(Vec2::new(180.0, 5.0), Sense::hover())
                .0;
            ui.painter()
                .rect_filled(rect, CornerRadius::same(0), pal.frame_bg);
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(rect.width() * 0.35, rect.height())),
                CornerRadius::same(0),
                pal.accent,
            );
            ui.label(
                RichText::new("Asset indexing")
                    .size(11.0)
                    .color(pal.text_dim),
            );
        });
    });
}

fn draw_center_dock(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
) {
    if ui_state.show_scene_view && ui_state.show_game_view {
        ui.columns(2, |columns| {
            viewport_panel(&mut columns[0], shell, "Scene", true, ui_state, pal);
            viewport_panel(&mut columns[1], shell, "Game", false, ui_state, pal);
        });
    } else if ui_state.show_scene_view {
        viewport_panel(ui, shell, "Scene", true, ui_state, pal);
    } else if ui_state.show_game_view {
        viewport_panel(ui, shell, "Game", false, ui_state, pal);
    } else {
        empty_view(ui, "No viewport open", pal);
    }
}

fn draw_bottom_dock(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
) {
    ui.horizontal(|ui| {
        if ui_state.show_project {
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() * if ui_state.show_console { 0.5 } else { 1.0 });
                panel_title(ui, "Project", pal);
                draw_project_panel(ui, shell, ui_state, pal);
            });
        }
        if ui_state.show_console {
            ui.vertical(|ui| {
                panel_title(ui, "Console", pal);
                draw_console(ui, shell, ui_state, pal);
            });
        }
    });
}

fn viewport_panel(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    label: &str,
    scene_tools: bool,
    ui_state: &ShellUiState,
    pal: &InfernuxPalette,
) {
    let rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(rect, Sense::click());
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
    ui.painter().text(
        tab.min + Vec2::new(10.0, 13.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        pal.text,
    );

    let content_rect = rect.shrink2(Vec2::new(0.0, 26.0));
    draw_grid(ui, content_rect, pal);
    draw_scene_preview(ui, shell, content_rect, scene_tools, pal);

    if scene_tools {
        draw_scene_overlay(ui, rect, pal);
        draw_orientation_gizmo(ui, rect, pal);
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

    if response.clicked() {
        select_first_scene_object(shell);
    }
}

fn draw_scene_preview(
    ui: &mut egui::Ui,
    shell: &EditorShell,
    rect: Rect,
    scene_tools: bool,
    pal: &InfernuxPalette,
) {
    let Some(project) = shell.project() else {
        draw_viewport_hint(ui, rect, "Open a project to preview the scene", pal);
        return;
    };

    let objects = project.scene.objects();
    if objects.is_empty() {
        draw_viewport_hint(ui, rect, "Scene is empty", pal);
        return;
    }

    let selected = shell.selected_entity_id();
    let center = rect.center();
    let scale = (rect.width().min(rect.height()) / 12.0).clamp(18.0, 64.0);
    let mut camera_count = 0usize;
    let mut renderer_count = 0usize;
    let mut physics_count = 0usize;
    let mut light_count = 0usize;

    for (entity, object) in objects {
        let transform = project
            .scene
            .transforms()
            .local(entity)
            .unwrap_or(engine_core::math::Transform::IDENTITY);
        let pos = Pos2::new(
            center.x + transform.translation.x * scale,
            center.y - transform.translation.z * scale - transform.translation.y * scale * 0.35,
        );
        let components = project.scene.components(entity).unwrap_or(&[]);
        let has_camera = components
            .iter()
            .any(|component| matches!(component, ComponentData::Camera(_)));
        let has_renderer = components
            .iter()
            .any(|component| matches!(component, ComponentData::MeshRenderer(_)));
        let has_light = components
            .iter()
            .any(|component| matches!(component, ComponentData::Light(_)));
        let has_physics = components.iter().any(|component| {
            matches!(
                component,
                ComponentData::Rigidbody(_) | ComponentData::Collider(_)
            )
        });

        camera_count += usize::from(has_camera);
        renderer_count += usize::from(has_renderer);
        light_count += usize::from(has_light);
        physics_count += usize::from(has_physics);

        let selected_object = selected == Some(object.id);
        let radius = if selected_object { 13.0 } else { 10.0 };
        let color = if has_camera {
            rgb(91, 157, 245)
        } else if has_light {
            pal.warning
        } else if has_renderer {
            pal.accent
        } else if has_physics {
            rgb(113, 183, 139)
        } else {
            pal.text_dim
        };

        if selected_object {
            ui.painter()
                .circle_stroke(pos, radius + 6.0, Stroke::new(2.0, pal.selection));
        }
        ui.painter().circle_filled(pos, radius, color);
        ui.painter()
            .circle_stroke(pos, radius, Stroke::new(1.0, pal.border));
        ui.painter().text(
            pos + Vec2::new(0.0, radius + 13.0),
            Align2::CENTER_CENTER,
            truncate(&object.name, 18),
            FontId::proportional(11.0),
            if object.active {
                pal.text
            } else {
                pal.text_disabled
            },
        );

        if scene_tools && has_camera {
            let frustum = [
                pos + Vec2::new(-22.0, -16.0),
                pos + Vec2::new(22.0, -16.0),
                pos + Vec2::new(34.0, 20.0),
                pos + Vec2::new(-34.0, 20.0),
            ];
            for pair in frustum.windows(2) {
                ui.painter()
                    .line_segment([pair[0], pair[1]], Stroke::new(1.0, rgb(91, 157, 245)));
            }
            ui.painter().line_segment(
                [frustum[3], frustum[0]],
                Stroke::new(1.0, rgb(91, 157, 245)),
            );
        }
    }

    let stats = format!(
        "{} objects  |  {} mesh  {} camera  {} light  {} physics",
        project.scene.objects().len(),
        renderer_count,
        camera_count,
        light_count,
        physics_count
    );
    ui.painter().text(
        rect.left_bottom() + Vec2::new(10.0, -12.0),
        Align2::LEFT_CENTER,
        stats,
        FontId::proportional(11.0),
        pal.text_dim,
    );
}

fn draw_viewport_hint(ui: &mut egui::Ui, rect: Rect, hint: &str, pal: &InfernuxPalette) {
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        hint,
        FontId::proportional(14.0),
        pal.text_disabled,
    );
}

fn draw_scene_overlay(ui: &mut egui::Ui, rect: Rect, pal: &InfernuxPalette) {
    let mut cursor = rect.min + Vec2::new(8.0, 34.0);
    for (label, active) in [("Q", false), ("W", true), ("E", false), ("R", false)] {
        let button = Rect::from_min_size(cursor, Vec2::splat(22.0));
        let fill = if active {
            pal.selection
        } else {
            Color32::from_rgba_premultiplied(20, 20, 20, 210)
        };
        ui.painter()
            .rect_filled(button, CornerRadius::same(0), fill);
        ui.painter().rect_stroke(
            button,
            CornerRadius::same(0),
            Stroke::new(1.0, pal.border),
            StrokeKind::Inside,
        );
        ui.painter().text(
            button.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(12.0),
            pal.text,
        );
        cursor.x += 23.0;
    }

    let pill = Rect::from_min_size(cursor + Vec2::new(8.0, 0.0), Vec2::new(86.0, 22.0));
    ui.painter().rect_filled(
        pill,
        CornerRadius::same(4),
        Color32::from_rgba_premultiplied(35, 35, 35, 220),
    );
    ui.painter().text(
        pill.center(),
        Align2::CENTER_CENTER,
        "Global",
        FontId::proportional(12.0),
        pal.text,
    );
}

fn draw_orientation_gizmo(ui: &mut egui::Ui, rect: Rect, pal: &InfernuxPalette) {
    let center = rect.right_top() + Vec2::new(-54.0, 62.0);
    ui.painter().circle_filled(
        center,
        40.0,
        Color32::from_rgba_premultiplied(20, 20, 20, 150),
    );
    for (offset, color, label) in [
        (Vec2::new(26.0, 8.0), rgb(220, 70, 70), "X"),
        (Vec2::new(-8.0, -26.0), rgb(95, 190, 95), "Y"),
        (Vec2::new(-20.0, 18.0), rgb(80, 130, 220), "Z"),
    ] {
        ui.painter()
            .line_segment([center, center + offset], Stroke::new(2.0, color));
        ui.painter().circle_filled(center + offset, 7.0, color);
        ui.painter().text(
            center + offset,
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(10.0),
            Color32::WHITE,
        );
    }
    ui.painter()
        .circle_stroke(center, 40.0, Stroke::new(1.0, pal.border));
}

fn draw_hierarchy(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
) {
    let mut create_error = None;
    toolbar_row(ui, pal, |ui| {
        if small_chip(ui, "+", 24.0, pal)
            .on_hover_text("Create empty GameObject")
            .clicked()
        {
            if let Some(project) = shell.project_mut() {
                let name = format!("GameObject {}", project.scene.objects().len() + 1);
                match project.scene.create_object(name) {
                    Ok(entity) => {
                        project.scene_dirty = true;
                        if let Some(id) = project.scene.object(entity).map(|object| object.id) {
                            shell.select_entity_id(id);
                        }
                    }
                    Err(error) => create_error = Some(error.to_string()),
                }
            }
        }
        ui.add_space(4.0);
        search_field(ui, "Search", &mut ui_state.hierarchy_filter, pal);
    });
    if let Some(error) = create_error {
        shell.console_mut().push(engine_editor::ConsoleEntry {
            timestamp: "now".to_string(),
            level: engine_editor::ConsoleLevel::Error,
            source: engine_editor::ConsoleSource {
                subsystem: "editor".to_string(),
                file: None,
                line: None,
            },
            message: error,
        });
    }

    let Some(project) = shell.project() else {
        empty_view(ui, "No scene loaded", pal);
        return;
    };

    let query = ui_state.hierarchy_filter.trim().to_lowercase();
    let rows = project
        .scene
        .objects()
        .into_iter()
        .filter(|(_, object)| query.is_empty() || object.name.to_lowercase().contains(&query))
        .map(|(_, object)| (object.id, object.name.clone(), object.active))
        .collect::<Vec<_>>();
    let selected = shell.selected_entity_id();

    egui::ScrollArea::vertical()
        .id_salt("infernux_hierarchy_scroll")
        .show(ui, |ui| {
            row_label(ui, "v  Sample Scene", false, 0, pal, || {});
            for (idx, (id, name, active)) in rows.into_iter().enumerate() {
                let mut clicked = false;
                row_label(
                    ui,
                    &format!("  {} {}", if active { "□" } else { "◇" }, name),
                    selected == Some(id),
                    idx,
                    pal,
                    || clicked = true,
                );
                if clicked {
                    shell.select_entity_id(id);
                }
            }
        });
}

fn draw_inspector(ui: &mut egui::Ui, shell: &mut EditorShell, pal: &InfernuxPalette) {
    let Some(selected_id) = shell.selected_entity_id() else {
        empty_view(ui, "Select a GameObject to inspect", pal);
        return;
    };
    let Some(project) = shell.project_mut() else {
        empty_view(ui, "No project open", pal);
        return;
    };
    let Some(entity) = project.scene.find_by_id(selected_id) else {
        empty_view(ui, "Selection no longer exists", pal);
        return;
    };

    egui::ScrollArea::vertical()
        .id_salt("infernux_inspector_scroll")
        .show(ui, |ui| {
            let mut dirty = false;
            if let Some(object) = project.scene.object_mut(entity) {
                ui.horizontal(|ui| {
                    ui.add(egui::Checkbox::without_text(&mut object.active));
                    ui.add_sized(
                        Vec2::new(ui.available_width(), 22.0),
                        egui::TextEdit::singleline(&mut object.name),
                    );
                    dirty = true;
                });
                ui.add_space(6.0);
                property_row_text(ui, "Tag", "Untagged", pal);
                property_row_text(ui, "Layer", "Default", pal);
                ui.add_space(8.0);
            }

            if let Some(mut transform) = project.scene.transforms().local(entity) {
                component_header(ui, "Transform", true, pal);
                dirty |= vec3_editor(ui, "Position", &mut transform.translation, pal);
                let mut rotation = engine_core::math::Vec3::ZERO;
                let _ = vec3_editor(ui, "Rotation", &mut rotation, pal);
                dirty |= vec3_editor(ui, "Scale", &mut transform.scale, pal);
                if dirty {
                    project.scene.transforms_mut().set_local(entity, transform);
                }
            }

            if let Some(components) = project.scene.components(entity) {
                for component in components {
                    draw_component(ui, component, pal);
                }
            }

            ui.add_space(8.0);
            let rect = ui
                .allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click())
                .0;
            ui.painter()
                .rect_filled(rect, CornerRadius::same(0), pal.frame_bg);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Add Component",
                FontId::proportional(13.0),
                pal.text,
            );

            if dirty {
                project.scene_dirty = true;
            }
        });
}

fn draw_component(ui: &mut egui::Ui, component: &ComponentData, pal: &InfernuxPalette) {
    match component {
        ComponentData::Camera(camera) => {
            component_header(ui, "Camera", true, pal);
            property_row_text(ui, "Projection", "Perspective", pal);
            property_row_text(
                ui,
                "FOV",
                &format!("{:.0}", camera.vertical_fov_degrees),
                pal,
            );
            property_row_text(ui, "Near", &format!("{:.2}", camera.near), pal);
        }
        ComponentData::MeshRenderer(renderer) => {
            component_header(ui, "Mesh Renderer", true, pal);
            property_row_text(
                ui,
                "Mesh",
                renderer.builtin_mesh.as_deref().unwrap_or("Asset Mesh"),
                pal,
            );
            property_row_text(
                ui,
                "Material",
                renderer
                    .material
                    .builtin
                    .as_deref()
                    .unwrap_or("Asset Material"),
                pal,
            );
        }
        ComponentData::Light(light) => {
            component_header(ui, "Light", true, pal);
            property_row_text(ui, "Type", &light.kind.to_string(), pal);
            property_row_text(ui, "Intensity", &format!("{:.1}", light.intensity), pal);
        }
        ComponentData::Rigidbody(body) => {
            component_header(ui, "Rigidbody", true, pal);
            property_row_text(ui, "Body Type", &body.body_type.to_string(), pal);
            property_row_text(ui, "Mass", &format!("{:.1}", body.mass), pal);
        }
        ComponentData::Collider(collider) => {
            component_header(ui, "Collider", true, pal);
            property_row_text(ui, "Shape", &collider.shape.to_string(), pal);
        }
        ComponentData::AudioSource(source) => {
            component_header(ui, "Audio Source", true, pal);
            property_row_text(ui, "Volume", &format!("{:.1}", source.volume), pal);
        }
        ComponentData::Script(script) => {
            component_header(ui, "Script", true, pal);
            property_row_text(ui, "Backend", &script.backend, pal);
            property_row_text(ui, "Script", &script.script, pal);
        }
    }
}

fn draw_project_panel(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
) {
    toolbar_row(ui, pal, |ui| {
        small_chip(ui, "Create", 64.0, pal).on_hover_text("Asset creation is not wired yet");
        small_chip(ui, "Import", 64.0, pal).on_hover_text("Import pipeline is not wired yet");
        ui.add_space(6.0);
        search_field(ui, "Search Assets", &mut ui_state.project_filter, pal);
    });

    let Some(project) = shell.project() else {
        empty_view(ui, "No project open", pal);
        return;
    };

    ui.label(
        RichText::new(project.root.display().to_string())
            .size(11.0)
            .color(pal.text_dim),
    );
    ui.add_space(6.0);
    egui::ScrollArea::vertical()
        .id_salt("infernux_project_assets_scroll")
        .show(ui, |ui| {
            let query = ui_state.project_filter.trim().to_lowercase();
            let assets = project
                .sorted_assets()
                .into_iter()
                .filter(|asset| {
                    query.is_empty()
                        || asset
                            .source_path
                            .to_string_lossy()
                            .to_lowercase()
                            .contains(&query)
                        || resource_kind_label(asset.kind)
                            .to_lowercase()
                            .contains(&query)
                        || asset_guid_label(asset.guid).contains(&query)
                })
                .collect::<Vec<_>>();

            if project.assets.is_empty() {
                empty_view(ui, "Assets folder is empty", pal);
                return;
            }
            if assets.is_empty() {
                empty_view(ui, "No assets match the current filter", pal);
                return;
            }

            let tile_size = Vec2::new(92.0, 74.0);
            ui.horizontal_wrapped(|ui| {
                for asset in assets {
                    let (rect, _) = ui.allocate_exact_size(tile_size, Sense::click());
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(0), pal.frame_bg);
                    ui.painter().rect_stroke(
                        rect,
                        CornerRadius::same(0),
                        Stroke::new(1.0, pal.border),
                        StrokeKind::Inside,
                    );
                    ui.painter().text(
                        rect.center_top() + Vec2::new(0.0, 18.0),
                        Align2::CENTER_CENTER,
                        resource_kind_label(asset.kind),
                        FontId::proportional(11.0),
                        pal.text_dim,
                    );
                    let name = asset
                        .source_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("asset");
                    ui.painter().text(
                        rect.center_bottom() - Vec2::new(0.0, 20.0),
                        Align2::CENTER_CENTER,
                        truncate(name, 13),
                        FontId::proportional(11.0),
                        pal.text,
                    );
                    ui.painter().text(
                        rect.center_bottom() - Vec2::new(0.0, 7.0),
                        Align2::CENTER_CENTER,
                        truncate(&asset_guid_label(asset.guid), 11),
                        FontId::proportional(9.0),
                        pal.text_disabled,
                    );
                }
            });
        });
}

fn draw_console(
    ui: &mut egui::Ui,
    shell: &mut EditorShell,
    ui_state: &mut ShellUiState,
    pal: &InfernuxPalette,
) {
    toolbar_row(ui, pal, |ui| {
        if small_chip(ui, "Clear", 54.0, pal).clicked() {
            shell.console_mut().clear();
        }
        if small_chip(
            ui,
            if ui_state.console_collapse {
                "Expanded"
            } else {
                "Collapse"
            },
            74.0,
            pal,
        )
        .clicked()
        {
            ui_state.console_collapse = !ui_state.console_collapse;
        }
        ui.add_space(6.0);
        search_field(ui, "Filter", &mut ui_state.console_filter, pal);
    });

    let query = ui_state.console_filter.trim().to_lowercase();
    let mut last_message = String::new();
    egui::ScrollArea::vertical()
        .id_salt("infernux_console_scroll")
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for (idx, entry) in shell.console().entries().iter().enumerate() {
                let row_text = format!("[{:?}] {}", entry.level, entry.message);
                if !query.is_empty() && !row_text.to_lowercase().contains(&query) {
                    continue;
                }
                if ui_state.console_collapse && row_text == last_message {
                    continue;
                }
                last_message = row_text.clone();
                let rect = ui
                    .allocate_exact_size(Vec2::new(ui.available_width(), 23.0), Sense::click())
                    .0;
                if idx % 2 == 0 {
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(0), pal.row_alt);
                }
                let color = match entry.level {
                    ConsoleLevel::Trace | ConsoleLevel::Debug => pal.text_dim,
                    ConsoleLevel::Info => pal.text,
                    ConsoleLevel::Warn => pal.warning,
                    ConsoleLevel::Error => pal.error,
                };
                ui.painter().text(
                    rect.min + Vec2::new(8.0, 11.5),
                    Align2::LEFT_CENTER,
                    row_text,
                    FontId::proportional(12.0),
                    color,
                );
            }
        });
}

fn vec3_editor(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut engine_core::math::Vec3,
    pal: &InfernuxPalette,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(82.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)),
        );
        changed |= axis_drag(ui, "X", &mut value.x, rgb(190, 75, 75));
        changed |= axis_drag(ui, "Y", &mut value.y, rgb(90, 170, 90));
        changed |= axis_drag(ui, "Z", &mut value.z, rgb(80, 120, 190));
    });
    changed
}

fn axis_drag(ui: &mut egui::Ui, label: &str, value: &mut f32, color: Color32) -> bool {
    ui.label(RichText::new(label).size(11.0).strong().color(color));
    ui.add_sized(Vec2::new(58.0, 20.0), DragValue::new(value).speed(0.05))
        .changed()
}

fn property_row_text(ui: &mut egui::Ui, label: &str, value: &str, pal: &InfernuxPalette) {
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(86.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)),
        );
        let rect = ui
            .allocate_exact_size(
                Vec2::new((ui.available_width() - 2.0).max(80.0), 20.0),
                Sense::hover(),
            )
            .0;
        ui.painter()
            .rect_filled(rect, CornerRadius::same(0), pal.frame_bg);
        ui.painter().text(
            rect.min + Vec2::new(6.0, 10.0),
            Align2::LEFT_CENTER,
            value,
            FontId::proportional(12.0),
            pal.text,
        );
    });
}

fn component_header(ui: &mut egui::Ui, title: &str, enabled: bool, pal: &InfernuxPalette) {
    ui.add_space(6.0);
    let rect = ui
        .allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click())
        .0;
    ui.painter()
        .rect_filled(rect, CornerRadius::same(0), pal.header);
    ui.painter().text(
        rect.min + Vec2::new(8.0, 12.0),
        Align2::LEFT_CENTER,
        "v",
        FontId::proportional(11.0),
        pal.text_dim,
    );
    ui.painter().text(
        rect.min + Vec2::new(26.0, 12.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(12.0),
        pal.text,
    );
    let check = Rect::from_min_size(rect.right_top() + Vec2::new(-48.0, 5.0), Vec2::splat(14.0));
    ui.painter().rect_stroke(
        check,
        CornerRadius::same(0),
        Stroke::new(1.0, pal.border),
        StrokeKind::Inside,
    );
    if enabled {
        ui.painter().line_segment(
            [check.left_center(), check.center_bottom()],
            Stroke::new(1.5, pal.accent),
        );
        ui.painter().line_segment(
            [check.center_bottom(), check.right_top()],
            Stroke::new(1.5, pal.accent),
        );
    }
    ui.painter().text(
        rect.right_center() - Vec2::new(14.0, 0.0),
        Align2::CENTER_CENTER,
        "...",
        FontId::proportional(12.0),
        pal.text_dim,
    );
}

fn panel_title(ui: &mut egui::Ui, title: &str, pal: &InfernuxPalette) {
    let rect = ui
        .allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click())
        .0;
    ui.painter()
        .rect_filled(rect, CornerRadius::same(0), pal.header);
    ui.painter().text(
        rect.min + Vec2::new(8.0, 12.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(13.0),
        pal.text,
    );
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, pal.border),
    );
}

fn row_label(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    index: usize,
    pal: &InfernuxPalette,
    on_click: impl FnOnce(),
) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click());
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(0), pal.selection);
    } else if index % 2 == 0 {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(0), pal.row_alt);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(0), pal.header_hover);
    }
    ui.painter().text(
        rect.min + Vec2::new(8.0, 11.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        pal.text,
    );
    if response.clicked() {
        on_click();
    }
}

fn empty_view(ui: &mut egui::Ui, hint: &str, pal: &InfernuxPalette) {
    let rect = ui.available_rect_before_wrap().shrink(18.0);
    let w = rect.width().clamp(220.0, 460.0);
    let h = rect.height().clamp(120.0, 220.0);
    let box_rect = Rect::from_center_size(rect.center(), Vec2::new(w, h));
    ui.painter().rect_stroke(
        box_rect,
        CornerRadius::same(8),
        Stroke::new(1.0, pal.text_disabled),
        StrokeKind::Inside,
    );
    ui.painter().text(
        box_rect.center(),
        Align2::CENTER_CENTER,
        hint,
        FontId::proportional(13.0),
        pal.text_dim,
    );
    ui.allocate_rect(ui.available_rect_before_wrap(), Sense::hover());
}

fn draw_grid(ui: &mut egui::Ui, rect: Rect, pal: &InfernuxPalette) {
    let step = 32.0;
    let line = Color32::from_rgba_premultiplied(255, 255, 255, 13);
    let mut x = rect.left();
    while x < rect.right() {
        ui.painter().line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, line),
        );
        x += step;
    }
    let mut y = rect.top();
    while y < rect.bottom() {
        ui.painter().line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(1.0, line),
        );
        y += step;
    }
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), rect.center().y),
            Pos2::new(rect.right(), rect.center().y),
        ],
        Stroke::new(1.0, pal.border),
    );
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top()),
            Pos2::new(rect.center().x, rect.bottom()),
        ],
        Stroke::new(1.0, pal.border),
    );
}

fn toolbar_row(ui: &mut egui::Ui, pal: &InfernuxPalette, add: impl FnOnce(&mut egui::Ui)) {
    let rect = ui
        .allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover())
        .0;
    ui.painter()
        .rect_filled(rect, CornerRadius::same(0), pal.menu_bar);
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(6.0, 3.0))),
        |ui| {
            ui.horizontal_centered(add);
        },
    );
}

fn panel_toggle(ui: &mut egui::Ui, label: &str, state: &mut bool, pal: &InfernuxPalette) {
    let fill = if *state { pal.header } else { pal.frame_bg };
    let response = ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(if *state {
            pal.text
        } else {
            pal.text_dim
        }))
        .fill(fill)
        .min_size(Vec2::new(58.0, 24.0)),
    );
    if response.clicked() {
        *state = !*state;
    }
}

fn transport_button(
    ui: &mut egui::Ui,
    label: &str,
    active: bool,
    active_color: Color32,
    pal: &InfernuxPalette,
) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(14.0).color(pal.text))
            .fill(if active { active_color } else { pal.frame_bg })
            .min_size(Vec2::new(30.0, 24.0)),
    )
}

fn tool_button(ui: &mut egui::Ui, label: &str, tooltip: &str, active: bool, pal: &InfernuxPalette) {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(pal.text))
            .fill(if active { pal.selection } else { pal.frame_bg })
            .min_size(Vec2::new(26.0, 24.0)),
    )
    .on_hover_text(tooltip);
}

fn dropdown_pill(ui: &mut egui::Ui, label: &str, width: f32, pal: &InfernuxPalette) {
    ui.add(
        egui::Button::new(
            RichText::new(format!("{label} ▾"))
                .size(12.0)
                .color(pal.text),
        )
        .fill(pal.frame_bg)
        .min_size(Vec2::new(width, 24.0)),
    );
}

fn ghost_button(ui: &mut egui::Ui, label: &str, width: f32, pal: &InfernuxPalette) {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(pal.text))
            .fill(Color32::TRANSPARENT)
            .min_size(Vec2::new(width, 22.0)),
    );
}

fn small_text_button(ui: &mut egui::Ui, label: &str, pal: &InfernuxPalette) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(pal.text))
            .fill(pal.frame_bg)
            .min_size(Vec2::new(56.0, 24.0)),
    )
}

fn small_chip(ui: &mut egui::Ui, label: &str, width: f32, pal: &InfernuxPalette) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(pal.text))
            .fill(pal.frame_bg)
            .min_size(Vec2::new(width, 20.0)),
    )
}

fn search_field(ui: &mut egui::Ui, hint: &str, value: &mut String, pal: &InfernuxPalette) {
    ui.add_sized(
        Vec2::new((ui.available_width() - 4.0).max(80.0), 20.0),
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .font(FontId::proportional(11.0))
            .text_color(pal.text),
    );
}

fn panel_frame(pal: &InfernuxPalette) -> Frame {
    Frame::NONE
        .fill(pal.panel_bg)
        .stroke(Stroke::new(1.0, pal.border))
        .inner_margin(Margin::same(0))
}

fn select_first_scene_object(shell: &mut EditorShell) {
    if let Some(id) = shell.project().and_then(|project| {
        project
            .scene
            .find_by_name("Player")
            .and_then(|entity| project.scene.object(entity).map(|object| object.id))
            .or_else(|| {
                project
                    .scene
                    .objects()
                    .into_iter()
                    .next()
                    .map(|(_, object)| object.id)
            })
    }) {
        shell.select_entity_id(id);
    }
}

fn save_scene(shell: &mut EditorShell) {
    if let Err(error) = shell.save_scene() {
        shell.console_mut().push(engine_editor::ConsoleEntry {
            timestamp: "now".to_string(),
            level: engine_editor::ConsoleLevel::Error,
            source: engine_editor::ConsoleSource {
                subsystem: "editor".to_string(),
                file: None,
                line: None,
            },
            message: error.to_string(),
        });
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_owned()
    } else {
        let mut out = value
            .chars()
            .take(max.saturating_sub(1))
            .collect::<String>();
        out.push('…');
        out
    }
}

fn apply_visuals(ctx: &egui::Context, pal: &InfernuxPalette) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = pal.window_bg;
    visuals.window_fill = pal.panel_bg;
    visuals.extreme_bg_color = pal.frame_bg;
    visuals.faint_bg_color = pal.frame_bg;
    visuals.window_stroke = Stroke::new(1.0, pal.border);
    visuals.window_corner_radius = CornerRadius::same(0);
    visuals.widgets.noninteractive.bg_fill = pal.frame_bg;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, pal.text);
    visuals.widgets.inactive.bg_fill = pal.frame_bg;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, pal.text_dim);
    visuals.widgets.hovered.bg_fill = pal.frame_hover;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, pal.text);
    visuals.widgets.active.bg_fill = pal.header_hover;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, pal.text);
    visuals.selection.bg_fill = pal.selection;
    visuals.selection.stroke = Stroke::new(1.0, pal.text);
    ctx.set_visuals(visuals);
}
