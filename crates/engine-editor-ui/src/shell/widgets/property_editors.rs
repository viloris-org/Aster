//! Property editor widgets for the editor shell UI.

use egui::{Color32, DragValue, FontId, RichText, Vec2};

use engine_assets::{ResourceKind, ResourceMetaFormat};
use engine_core::math::Vec3;
use engine_ecs::MaterialRef;

use super::super::types::{rgb, InfernuxPalette};

/// Renders a Vec3 editor with X, Y, Z drag values.
pub fn vec3_editor(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Vec3,
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

/// Renders a Vec3 editor with custom step size and drag state tracking.
pub fn vec3_editor_with_step(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Vec3,
    step: f32,
    pal: &InfernuxPalette,
) -> (bool, bool) {
    let mut changed = false;
    let mut any_dragging = false;
    ui.add_sized(
        Vec2::new(72.0, 20.0),
        egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)),
    );
    let (c, d) = axis_drag_with_info(ui, "X", &mut value.x, rgb(190, 75, 75), step);
    changed |= c;
    any_dragging |= d;
    let (c, d) = axis_drag_with_info(ui, "Y", &mut value.y, rgb(90, 170, 90), step);
    changed |= c;
    any_dragging |= d;
    let (c, d) = axis_drag_with_info(ui, "Z", &mut value.z, rgb(80, 120, 190), step);
    changed |= c;
    any_dragging |= d;
    (changed, any_dragging)
}

/// Renders a Vec3 editor for color values (clamped 0.0-1.0).
pub fn color_vec3_editor(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Vec3,
    pal: &InfernuxPalette,
) -> bool {
    let before = *value;
    value.x = value.x.clamp(0.0, 1.0);
    value.y = value.y.clamp(0.0, 1.0);
    value.z = value.z.clamp(0.0, 1.0);

    let mut changed = before != *value;
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(82.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)),
        );
        changed |= axis_drag_clamped(ui, "X", &mut value.x, rgb(190, 75, 75), 0.0..=1.0);
        changed |= axis_drag_clamped(ui, "Y", &mut value.y, rgb(90, 170, 90), 0.0..=1.0);
        changed |= axis_drag_clamped(ui, "Z", &mut value.z, rgb(80, 120, 190), 0.0..=1.0);
    });
    changed
}

/// Renders a single axis drag value with colored label.
pub fn axis_drag(ui: &mut egui::Ui, label: &str, value: &mut f32, color: Color32) -> bool {
    ui.label(RichText::new(label).size(11.0).strong().color(color));
    ui.add_sized(Vec2::new(58.0, 20.0), DragValue::new(value).speed(0.05))
        .changed()
}

/// Renders a single axis drag value with range clamping.
pub fn axis_drag_clamped(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    color: Color32,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    ui.label(RichText::new(label).size(11.0).strong().color(color));
    ui.add_sized(
        Vec2::new(58.0, 20.0),
        DragValue::new(value).speed(0.01).range(range),
    )
    .changed()
}

/// Renders a single axis drag value with change and drag state tracking.
pub fn axis_drag_with_info(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    color: Color32,
    step: f32,
) -> (bool, bool) {
    ui.label(RichText::new(label).size(11.0).strong().color(color));
    let response = ui.add_sized(
        Vec2::new(58.0, 20.0),
        DragValue::new(value).speed(step).fixed_decimals(2),
    );
    (response.changed(), response.dragged())
}

/// Renders a single axis boolean checkbox with colored label.
pub fn axis_bool(ui: &mut egui::Ui, label: &str, value: &mut bool, color: Color32) -> bool {
    ui.label(RichText::new(label).size(11.0).strong().color(color));
    ui.checkbox(value, "").changed()
}

/// Renders a lock axes editor with X, Y, Z checkboxes.
pub fn lock_axes_editor(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut [bool; 3],
    pal: &InfernuxPalette,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(86.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)).truncate(),
        );
        changed |= axis_bool(ui, "X", &mut value[0], rgb(190, 75, 75));
        changed |= axis_bool(ui, "Y", &mut value[1], rgb(90, 170, 90));
        changed |= axis_bool(ui, "Z", &mut value[2], rgb(80, 120, 190));
    });
    changed
}

/// Renders a string property editor row.
pub fn string_property_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    pal: &InfernuxPalette,
) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(86.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)).truncate(),
        );
        ui.add_sized(
            Vec2::new((ui.available_width() - 2.0).max(80.0), 20.0),
            egui::TextEdit::singleline(value).font(FontId::proportional(12.0)),
        )
        .changed()
    })
    .inner
}

/// Renders an f32 property editor row.
pub fn f32_property_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    pal: &InfernuxPalette,
) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(86.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)).truncate(),
        );
        ui.add_sized(Vec2::new(96.0, 20.0), DragValue::new(value).speed(0.05))
            .changed()
    })
    .inner
}

/// Renders an f32 property editor row with range clamping.
pub fn f32_property_row_clamped(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    speed: f32,
    pal: &InfernuxPalette,
) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(86.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)).truncate(),
        );
        ui.add_sized(
            Vec2::new(96.0, 20.0),
            DragValue::new(value).speed(speed).range(range),
        )
        .changed()
    })
    .inner
}

/// Renders a u32 property editor row.
pub fn u32_property_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u32,
    pal: &InfernuxPalette,
) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(86.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)).truncate(),
        );
        ui.add_sized(Vec2::new(96.0, 20.0), DragValue::new(value).speed(1.0))
            .changed()
    })
    .inner
}

/// Renders a boolean property editor row.
pub fn bool_property_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut bool,
    pal: &InfernuxPalette,
) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(86.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)).truncate(),
        );
        ui.checkbox(value, "").changed()
    })
    .inner
}

/// Renders an enum property editor row with dropdown selection.
pub fn enum_property_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    options: &[&str],
    pal: &InfernuxPalette,
) -> bool {
    let before = value.clone();
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(86.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)).truncate(),
        );
        egui::ComboBox::from_id_salt(format!("enum_{label}_{before}"))
            .selected_text(value.as_str())
            .show_ui(ui, |ui| {
                for option in options {
                    ui.selectable_value(value, (*option).to_owned(), *option);
                }
            });
    });
    *value != before
}

/// Renders an asset reference property editor row with dropdown selection.
pub fn asset_ref_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<engine_core::AssetId>,
    builtin: &mut Option<String>,
    assets: &[ResourceMetaFormat],
    accepted: &[ResourceKind],
    pal: &InfernuxPalette,
) -> bool {
    let before = *value;
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(86.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)).truncate(),
        );
        let selected = value
            .map(|id| format!("{:032x}", id.as_u128()))
            .or_else(|| builtin.clone())
            .unwrap_or_else(|| "None".to_owned());
        egui::ComboBox::from_id_salt(format!("asset_{label}_{selected}"))
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui.selectable_label(value.is_none(), "None").clicked() {
                    *value = None;
                    *builtin = None;
                }
                for asset in assets.iter().filter(|asset| accepted.contains(&asset.kind)) {
                    let name = asset.source_path.to_string_lossy();
                    if ui
                        .selectable_label(*value == Some(asset.guid.as_asset_id()), name.as_ref())
                        .clicked()
                    {
                        *value = Some(asset.guid.as_asset_id());
                        *builtin = None;
                    }
                }
            });
    });
    *value != before
}

/// Renders a material reference property editor row with dropdown selection.
pub fn material_ref_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut MaterialRef,
    assets: &[ResourceMetaFormat],
    pal: &InfernuxPalette,
) -> bool {
    let before = value.clone();
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(86.0, 20.0),
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text_dim)).truncate(),
        );
        let selected = value
            .asset
            .map(|id| format!("{:032x}", id.as_u128()))
            .or_else(|| value.builtin.clone())
            .unwrap_or_else(|| "None".to_owned());
        egui::ComboBox::from_id_salt(format!("material_{selected}"))
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(value.asset.is_none() && value.builtin.is_none(), "None")
                    .clicked()
                {
                    value.asset = None;
                    value.builtin = None;
                }
                if ui
                    .selectable_label(
                        value.builtin.as_deref() == Some("debug/default"),
                        "debug/default",
                    )
                    .clicked()
                {
                    value.asset = None;
                    value.builtin = Some("debug/default".to_owned());
                }
                for asset in assets
                    .iter()
                    .filter(|asset| asset.kind == ResourceKind::Material)
                {
                    let name = asset.source_path.to_string_lossy();
                    if ui
                        .selectable_label(
                            value.asset == Some(asset.guid.as_asset_id()),
                            name.as_ref(),
                        )
                        .clicked()
                    {
                        value.asset = Some(asset.guid.as_asset_id());
                        value.builtin = None;
                    }
                }
            });
    });
    *value != before
}
