//! Converts an egui [`ClippedPrimitive`] list into a [`GuiDrawList`].
//!
//! Only compiled when the `editor` feature is enabled.

use egui::{epaint::Primitive, ClippedPrimitive, TexturesDelta};

use crate::pipeline::{GuiDrawCmd, GuiDrawList, GuiTextureId, GuiVertex};

/// Map an egui `TextureId` to our opaque `GuiTextureId`.
fn map_texture_id(id: egui::TextureId) -> GuiTextureId {
    match id {
        egui::TextureId::Managed(v) => GuiTextureId(v),
        egui::TextureId::User(v) => GuiTextureId(v),
    }
}

/// Convert egui output into a [`GuiDrawList`] ready for [`crate::RenderDevice::draw_gui`].
///
/// `pixels_per_point` is used to convert egui's logical scissor rects to physical pixels.
pub fn egui_to_gui_draw_list(
    primitives: &[ClippedPrimitive],
    pixels_per_point: f32,
) -> GuiDrawList {
    let mut list = GuiDrawList::default();

    for clipped in primitives {
        let Primitive::Mesh(mesh) = &clipped.primitive else {
            continue;
        };

        if mesh.indices.is_empty() {
            continue;
        }

        let index_offset = list.indices.len() as u32;

        for v in &mesh.vertices {
            let [r, g, b, a] = v.color.to_array();
            list.vertices.push(GuiVertex {
                pos: [v.pos.x, v.pos.y],
                uv: [v.uv.x, v.uv.y],
                color: u32::from_le_bytes([r, g, b, a]),
            });
        }
        list.indices.extend_from_slice(&mesh.indices);

        let rect = &clipped.clip_rect;
        let x = (rect.min.x * pixels_per_point).round() as u32;
        let y = (rect.min.y * pixels_per_point).round() as u32;
        let w = ((rect.max.x - rect.min.x) * pixels_per_point).round() as u32;
        let h = ((rect.max.y - rect.min.y) * pixels_per_point).round() as u32;

        list.commands.push(GuiDrawCmd {
            texture: map_texture_id(mesh.texture_id),
            scissor: [x, y, w, h],
            index_offset,
            index_count: mesh.indices.len() as u32,
        });
    }

    list
}

/// Extract texture uploads from egui's [`TexturesDelta`] as raw RGBA bytes.
///
/// Returns `(id, width, height, rgba_bytes)` for each new or updated texture.
pub fn extract_texture_updates(delta: &TexturesDelta) -> Vec<(GuiTextureId, u32, u32, Vec<u8>)> {
    delta
        .set
        .iter()
        .filter_map(|(id, image_delta)| {
            // Only handle full uploads (no sub-rect updates for now).
            if image_delta.pos.is_some() {
                return None;
            }
            let (w, h, rgba) = match &image_delta.image {
                egui::ImageData::Color(img) => {
                    let rgba = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                    (img.width() as u32, img.height() as u32, rgba)
                }
            };
            Some((map_texture_id(*id), w, h, rgba))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_primitives_produce_empty_draw_list() {
        let list = egui_to_gui_draw_list(&[], 1.0);
        assert!(list.vertices.is_empty());
        assert!(list.indices.is_empty());
        assert!(list.commands.is_empty());
    }
}
