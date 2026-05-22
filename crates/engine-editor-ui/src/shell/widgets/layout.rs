//! Layout widgets for the editor shell UI.

use egui::{Align2, CornerRadius, FontId, Frame, Margin, Rect, Sense, Stroke, Vec2};

use super::super::types::InfernuxPalette;
use super::text::{paint_text_in_rect, paint_wrapped_text_in_rect};

/// Creates a panel frame with background and border.
pub fn panel_frame(pal: &InfernuxPalette) -> Frame {
    Frame::NONE
        .fill(pal.panel_bg)
        .stroke(Stroke::new(1.0, pal.border))
        .inner_margin(Margin::same(0))
}

/// Renders a panel title bar with text.
pub fn panel_title(ui: &mut egui::Ui, title: &str, pal: &InfernuxPalette) {
    let rect = ui
        .allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click())
        .0;
    ui.painter()
        .rect_filled(rect, CornerRadius::same(0), pal.header);
    paint_text_in_rect(
        ui,
        rect.shrink2(Vec2::new(8.0, 0.0)),
        title,
        FontId::proportional(13.0),
        pal.text,
        Align2::LEFT_CENTER,
    );
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, pal.border),
    );
}

/// Renders a toolbar row with custom content.
pub fn toolbar_row(ui: &mut egui::Ui, pal: &InfernuxPalette, add: impl FnOnce(&mut egui::Ui)) {
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

/// Renders an empty view with a hint message.
pub fn empty_view(ui: &mut egui::Ui, hint: &str, pal: &InfernuxPalette) {
    let rect = ui.available_rect_before_wrap().shrink(18.0);
    let w = rect.width().clamp(220.0, 460.0);
    let h = rect.height().clamp(120.0, 220.0);
    let box_rect = Rect::from_center_size(rect.center(), Vec2::new(w, h));
    ui.painter().rect_stroke(
        box_rect,
        CornerRadius::same(8),
        Stroke::new(1.0, pal.text_disabled),
        egui::StrokeKind::Inside,
    );
    paint_wrapped_text_in_rect(
        ui,
        box_rect.shrink(16.0),
        hint,
        FontId::proportional(13.0),
        pal.text_dim,
        Align2::CENTER_CENTER,
    );
    ui.allocate_rect(ui.available_rect_before_wrap(), Sense::hover());
}

/// Renders a search text field with hint text.
pub fn search_field(ui: &mut egui::Ui, hint: &str, value: &mut String, pal: &InfernuxPalette) {
    ui.add_sized(
        Vec2::new((ui.available_width() - 4.0).max(80.0), 20.0),
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .font(FontId::proportional(11.0))
            .text_color(pal.text),
    );
}
