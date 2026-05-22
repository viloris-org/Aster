//! Component UI widgets for the editor shell UI.

use egui::{Align2, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::super::types::InfernuxPalette;
use super::icons::ui as icon;
use super::text::paint_text_in_rect;

/// Renders a component header with collapse/expand functionality.
pub fn component_header(
    ui: &mut egui::Ui,
    title: &str,
    enabled: bool,
    collapsed: bool,
    pal: &InfernuxPalette,
) -> egui::Response {
    ui.add_space(6.0);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(0), pal.header);
    let arrow = if collapsed {
        icon::CHEVRON_RIGHT
    } else {
        icon::CHEVRON_DOWN
    };
    paint_text_in_rect(
        ui,
        Rect::from_min_max(
            rect.min + Vec2::new(8.0, 0.0),
            rect.min + Vec2::new(22.0, 24.0),
        ),
        arrow,
        FontId::proportional(11.0),
        pal.text_dim,
        Align2::LEFT_CENTER,
    );
    paint_text_in_rect(
        ui,
        Rect::from_min_max(
            rect.min + Vec2::new(26.0, 0.0),
            rect.right_top() + Vec2::new(-54.0, 24.0),
        ),
        title,
        FontId::proportional(12.0),
        pal.text,
        Align2::LEFT_CENTER,
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
    paint_text_in_rect(
        ui,
        Rect::from_center_size(
            rect.right_center() - Vec2::new(14.0, 0.0),
            Vec2::new(28.0, 20.0),
        ),
        "...",
        FontId::proportional(12.0),
        pal.text_dim,
        Align2::CENTER_CENTER,
    );
    response
}
