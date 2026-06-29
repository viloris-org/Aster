use super::*;

pub(crate) fn build_script_ui_draw_list(
    commands: &[VargUiCommand],
    gui_textures: &HashMap<String, GuiTextureId>,
) -> GuiDrawList {
    let mut draw_list = GuiDrawList::default();
    for command in commands {
        match command {
            VargUiCommand::Rect {
                x,
                y,
                width,
                height,
                color,
                ..
            } => push_gui_quad(&mut draw_list, *x, *y, *width, *height, *color),
            VargUiCommand::Texture {
                texture,
                x,
                y,
                width,
                height,
                color,
                ..
            } => {
                let texture_id = gui_textures
                    .get(texture)
                    .copied()
                    .unwrap_or(GuiTextureId(0));
                push_gui_textured_quad(&mut draw_list, texture_id, *x, *y, *width, *height, *color);
            }
            VargUiCommand::Label { text, x, y, .. } => {
                let mut cursor_x = *x;
                for ch in text.chars() {
                    if ch.is_whitespace() {
                        cursor_x += 6.0;
                        continue;
                    }
                    push_gui_text_glyph(&mut draw_list, cursor_x, *y, ch, [1.0, 1.0, 1.0, 1.0]);
                    cursor_x += glyph_advance(ch);
                }
            }
        }
    }
    draw_list
}

pub(crate) fn build_pause_menu_draw_list(
    mut draw_list: GuiDrawList,
    preferences: RuntimeUserPreferences,
    output_size: (u32, u32),
) -> GuiDrawList {
    let layout = PauseMenuLayout::for_output(output_size);
    let screen = layout.screen;
    let panel = layout.panel;
    let accent = layout.accent;
    let continue_button = layout.continue_button;
    let exit_button = layout.exit_button;
    let invert_x_button = layout.invert_x_button;
    let invert_y_button = layout.invert_y_button;
    push_gui_quad(
        &mut draw_list,
        screen.x,
        screen.y,
        screen.width,
        screen.height,
        [0.0, 0.0, 0.0, 0.58],
    );
    push_gui_quad(
        &mut draw_list,
        panel.x,
        panel.y,
        panel.width,
        panel.height,
        [0.035, 0.045, 0.06, 0.94],
    );
    push_gui_quad(
        &mut draw_list,
        accent.x,
        accent.y,
        accent.width,
        accent.height,
        [0.34, 0.75, 0.92, 1.0],
    );
    push_gui_text(
        &mut draw_list,
        layout.content_x,
        panel.y + 60.0,
        "PAUSED",
        [1.0, 1.0, 1.0, 1.0],
    );
    push_gui_quad(
        &mut draw_list,
        continue_button.x,
        continue_button.y,
        continue_button.width,
        continue_button.height,
        [0.12, 0.24, 0.32, 0.96],
    );
    push_gui_text(
        &mut draw_list,
        continue_button.x + 28.0,
        continue_button.y + 20.0,
        "CONTINUE",
        [0.88, 0.94, 1.0, 1.0],
    );
    push_gui_quad(
        &mut draw_list,
        exit_button.x,
        exit_button.y,
        exit_button.width,
        exit_button.height,
        [0.25, 0.1, 0.1, 0.96],
    );
    push_gui_text(
        &mut draw_list,
        exit_button.x + 28.0,
        exit_button.y + 20.0,
        "EXIT GAME",
        [0.88, 0.94, 1.0, 1.0],
    );
    push_gui_quad(
        &mut draw_list,
        invert_x_button.x,
        invert_x_button.y,
        invert_x_button.width,
        invert_x_button.height,
        [0.1, 0.13, 0.18, 0.96],
    );
    push_gui_text(
        &mut draw_list,
        invert_x_button.x + 28.0,
        invert_x_button.y + 16.0,
        &format!(
            "INVERT MOUSE X: {}",
            if preferences.invert_mouse_x {
                "ON"
            } else {
                "OFF"
            }
        ),
        [0.88, 0.94, 1.0, 1.0],
    );
    push_gui_quad(
        &mut draw_list,
        invert_y_button.x,
        invert_y_button.y,
        invert_y_button.width,
        invert_y_button.height,
        [0.1, 0.13, 0.18, 0.96],
    );
    push_gui_text(
        &mut draw_list,
        invert_y_button.x + 28.0,
        invert_y_button.y + 16.0,
        &format!(
            "INVERT MOUSE Y: {}",
            if preferences.invert_mouse_y {
                "ON"
            } else {
                "OFF"
            }
        ),
        [0.88, 0.94, 1.0, 1.0],
    );
    push_gui_text(
        &mut draw_list,
        layout.content_x,
        panel.y + panel.height - 36.0,
        "Esc / Enter / Space / E continue    Q exits    X/Y toggles",
        [0.72, 0.78, 0.86, 1.0],
    );
    draw_list
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiRect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl UiRect {
    pub(crate) fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PauseMenuLayout {
    pub(crate) screen: UiRect,
    pub(crate) panel: UiRect,
    pub(crate) accent: UiRect,
    pub(crate) continue_button: UiRect,
    pub(crate) exit_button: UiRect,
    pub(crate) invert_x_button: UiRect,
    pub(crate) invert_y_button: UiRect,
    pub(crate) content_x: f32,
}

impl PauseMenuLayout {
    pub(crate) fn for_output(output_size: (u32, u32)) -> Self {
        let screen_width = output_size.0.max(1) as f32;
        let screen_height = output_size.1.max(1) as f32;
        let panel_width = 600.0_f32.min((screen_width - 32.0).max(320.0));
        let panel_height = 410.0_f32.min((screen_height - 32.0).max(300.0));
        let panel_x = ((screen_width - panel_width) * 0.5).max(16.0);
        let panel_y = ((screen_height - panel_height) * 0.5).max(16.0);
        let content_x = panel_x + 60.0;
        let button_width = (panel_width - 120.0).max(220.0);

        Self {
            screen: UiRect {
                x: 0.0,
                y: 0.0,
                width: screen_width,
                height: screen_height,
            },
            panel: UiRect {
                x: panel_x,
                y: panel_y,
                width: panel_width,
                height: panel_height,
            },
            accent: UiRect {
                x: panel_x,
                y: panel_y,
                width: 6.0,
                height: panel_height,
            },
            continue_button: UiRect {
                x: content_x,
                y: panel_y + 110.0,
                width: button_width,
                height: 52.0,
            },
            exit_button: UiRect {
                x: content_x,
                y: panel_y + 182.0,
                width: button_width,
                height: 52.0,
            },
            invert_x_button: UiRect {
                x: content_x,
                y: panel_y + 254.0,
                width: button_width,
                height: 44.0,
            },
            invert_y_button: UiRect {
                x: content_x,
                y: panel_y + 310.0,
                width: button_width,
                height: 44.0,
            },
            content_x,
        }
    }
}

fn push_gui_text(draw_list: &mut GuiDrawList, x: f32, y: f32, text: &str, color: [f32; 4]) {
    let mut cursor_x = x;
    for ch in text.chars() {
        if ch.is_whitespace() {
            cursor_x += 6.0;
            continue;
        }
        push_gui_text_glyph(draw_list, cursor_x, y, ch, color);
        cursor_x += glyph_advance(ch);
    }
}

fn push_gui_text_glyph(draw_list: &mut GuiDrawList, x: f32, y: f32, ch: char, color: [f32; 4]) {
    let pixel = 1.5;
    let rows = glyph_rows(ch);
    for (row, bits) in rows.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) != 0 {
                push_gui_quad(
                    draw_list,
                    x + col as f32 * pixel,
                    y + row as f32 * pixel,
                    pixel,
                    pixel,
                    color,
                );
            }
        }
    }
}

fn glyph_advance(ch: char) -> f32 {
    match ch {
        '.' | ',' | ':' | ';' | '!' | '|' => 5.0,
        '/' | '-' => 7.0,
        _ => 9.0,
    }
}

fn glyph_rows(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000,
        ],
        '!' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
        '?' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        _ => [
            0b11111, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
        ],
    }
}

fn push_gui_quad(
    draw_list: &mut GuiDrawList,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [f32; 4],
) {
    push_gui_textured_quad(draw_list, GuiTextureId(0), x, y, width, height, color);
}

fn push_gui_textured_quad(
    draw_list: &mut GuiDrawList,
    texture: GuiTextureId,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [f32; 4],
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let base = draw_list.vertices.len() as u32;
    let color = pack_gui_color(color);
    draw_list.vertices.extend([
        GuiVertex {
            pos: [x, y],
            uv: [0.0, 0.0],
            color,
        },
        GuiVertex {
            pos: [x + width, y],
            uv: [1.0, 0.0],
            color,
        },
        GuiVertex {
            pos: [x + width, y + height],
            uv: [1.0, 1.0],
            color,
        },
        GuiVertex {
            pos: [x, y + height],
            uv: [0.0, 1.0],
            color,
        },
    ]);
    let index_offset = draw_list.indices.len() as u32;
    draw_list
        .indices
        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    draw_list.commands.push(GuiDrawCmd {
        texture,
        scissor: gui_scissor_for_rect(x, y, width, height),
        index_offset,
        index_count: 6,
    });
}

pub(crate) fn builtin_gui_texture_sources() -> Vec<(&'static str, u32, u32, Vec<u8>)> {
    vec![
        ("vargcraft:hotbar", 182, 22, hotbar_texture_pixels()),
        ("vargcraft:slot", 20, 20, slot_texture_pixels(false)),
        ("vargcraft:slot_selected", 24, 24, slot_texture_pixels(true)),
        (
            "vargcraft:block_grass",
            16,
            16,
            block_icon_pixels([85, 139, 52, 255], [117, 179, 76, 255], [96, 72, 45, 255]),
        ),
        (
            "vargcraft:block_stone",
            16,
            16,
            block_icon_pixels([92, 93, 91, 255], [132, 132, 126, 255], [58, 59, 57, 255]),
        ),
        (
            "vargcraft:block_wood",
            16,
            16,
            block_icon_pixels([126, 86, 46, 255], [174, 122, 63, 255], [72, 48, 28, 255]),
        ),
        (
            "vargcraft:heart",
            9,
            9,
            mask_icon_pixels(
                9,
                9,
                &[
                    "011011000",
                    "111111100",
                    "111111100",
                    "111111100",
                    "011111000",
                    "001110000",
                    "000100000",
                    "000000000",
                    "000000000",
                ],
                [223, 41, 41, 255],
                [96, 12, 12, 255],
            ),
        ),
        (
            "vargcraft:armor",
            9,
            9,
            mask_icon_pixels(
                9,
                9,
                &[
                    "001110000",
                    "011111000",
                    "111111100",
                    "110101100",
                    "111111100",
                    "011111000",
                    "001010000",
                    "000000000",
                    "000000000",
                ],
                [184, 205, 216, 255],
                [72, 92, 104, 255],
            ),
        ),
    ]
}

fn hotbar_texture_pixels() -> Vec<u8> {
    let mut pixels = vec![0; 182 * 22 * 4];
    for y in 0..22 {
        for x in 0..182 {
            let slot_x = x % 20;
            let border = y <= 1 || y >= 20 || slot_x <= 1 || slot_x >= 18;
            let inner_shadow = y >= 17 || slot_x >= 16;
            let color = if border {
                [44, 44, 44, 224]
            } else if inner_shadow {
                [83, 83, 83, 224]
            } else {
                [139, 139, 139, 216]
            };
            write_pixel(&mut pixels, 182, x, y, color);
        }
    }
    pixels
}

fn slot_texture_pixels(selected: bool) -> Vec<u8> {
    let size = if selected { 24 } else { 20 };
    let mut pixels = vec![0; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let edge = x == 0 || y == 0 || x == size - 1 || y == size - 1;
            let inner_edge = x == 1 || y == 1 || x == size - 2 || y == size - 2;
            let color = if selected && edge {
                [245, 245, 245, 255]
            } else if selected && inner_edge {
                [52, 52, 52, 248]
            } else if edge {
                [38, 38, 38, 230]
            } else {
                [118, 118, 118, 210]
            };
            write_pixel(&mut pixels, size as u32, x as u32, y as u32, color);
        }
    }
    pixels
}

fn block_icon_pixels(base: [u8; 4], light: [u8; 4], dark: [u8; 4]) -> Vec<u8> {
    let mut pixels = vec![0; 16 * 16 * 4];
    for y in 0..16 {
        for x in 0..16 {
            let checker = ((x / 4) + (y / 4)) % 2 == 0;
            let edge = x == 0 || y == 0 || x == 15 || y == 15;
            let top = y < 4;
            let color = if edge {
                dark
            } else if top || checker {
                light
            } else {
                base
            };
            write_pixel(&mut pixels, 16, x, y, color);
        }
    }
    pixels
}

fn mask_icon_pixels(
    width: usize,
    height: usize,
    mask: &[&str],
    fill: [u8; 4],
    shade: [u8; 4],
) -> Vec<u8> {
    let mut pixels = vec![0; width * height * 4];
    for y in 0..height {
        for (x, bit) in mask[y].bytes().enumerate() {
            if bit != b'1' {
                continue;
            }
            let color = if y > height / 2 || x == 0 || x == width - 1 {
                shade
            } else {
                fill
            };
            write_pixel(&mut pixels, width as u32, x as u32, y as u32, color);
        }
    }
    pixels
}

fn write_pixel(pixels: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let offset = ((y * width + x) * 4) as usize;
    pixels[offset..offset + 4].copy_from_slice(&color);
}

fn pack_gui_color(color: [f32; 4]) -> u32 {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    channel(color[0])
        | (channel(color[1]) << 8)
        | (channel(color[2]) << 16)
        | (channel(color[3]) << 24)
}

fn gui_scissor_for_rect(x: f32, y: f32, width: f32, height: f32) -> [u32; 4] {
    let left = x.floor().max(0.0) as u32;
    let top = y.floor().max(0.0) as u32;
    let right = (x + width).ceil().max(left as f32) as u32;
    let bottom = (y + height).ceil().max(top as f32) as u32;
    [
        left,
        top,
        right.saturating_sub(left).max(1),
        bottom.saturating_sub(top).max(1),
    ]
}
