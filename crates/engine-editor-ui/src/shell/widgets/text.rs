//! Text rendering utilities for the editor shell UI.

use egui::{Align2, Color32, FontId, Rect};

/// Paints single-line text in a rectangle with alignment and automatic elision.
pub fn paint_text_in_rect(
    ui: &egui::Ui,
    rect: Rect,
    text: &str,
    font: FontId,
    color: Color32,
    align: Align2,
) {
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return;
    }
    let text = elide_to_width(ui, text, font.clone(), color, rect.width());
    let galley = ui.painter().layout_no_wrap(text, font, color);
    let text_rect = align.align_size_within_rect(galley.size(), rect);
    ui.painter()
        .with_clip_rect(rect)
        .galley(text_rect.min, galley, color);
}

/// Paints multi-line wrapped text in a rectangle with alignment.
pub fn paint_wrapped_text_in_rect(
    ui: &egui::Ui,
    rect: Rect,
    text: &str,
    font: FontId,
    color: Color32,
    align: Align2,
) {
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return;
    }
    let galley = ui
        .painter()
        .layout(text.to_owned(), font, color, rect.width());
    let text_rect = align.align_size_within_rect(galley.size(), rect);
    ui.painter()
        .with_clip_rect(rect)
        .galley(text_rect.min, galley, color);
}

/// Truncates text with ellipsis to fit within a maximum width using binary search.
pub fn elide_to_width(
    ui: &egui::Ui,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> String {
    if ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), color)
        .size()
        .x
        <= max_width
    {
        return text.to_owned();
    }

    let ellipsis = "...";
    if ui
        .painter()
        .layout_no_wrap(ellipsis.to_owned(), font.clone(), color)
        .size()
        .x
        > max_width
    {
        return String::new();
    }

    let chars = text.chars().collect::<Vec<_>>();
    let mut low = 0;
    let mut high = chars.len();
    while low < high {
        let mid = (low + high).div_ceil(2);
        let candidate = chars
            .iter()
            .take(mid)
            .chain(ellipsis.chars().collect::<Vec<_>>().iter())
            .collect::<String>();
        let width = ui
            .painter()
            .layout_no_wrap(candidate, font.clone(), color)
            .size()
            .x;
        if width <= max_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    chars
        .into_iter()
        .take(low)
        .chain(ellipsis.chars())
        .collect()
}
