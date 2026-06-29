#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Retained-mode UI system with control tree, layout engine, theme, and rendering.

use std::any::Any;
use std::collections::HashMap;

use engine_core::AssetId;
use engine_render::{GuiDrawCmd, GuiDrawList, GuiTextureId, GuiVertex};
use serde::{Deserialize, Serialize};

const DEFAULT_GUI_TEXTURE: GuiTextureId = GuiTextureId(0);

/// 2D vector for UI layout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
}

impl Vec2 {
    /// Creates a new Vec2.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Rectangle for UI positioning.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// X position.
    pub x: f32,
    /// Y position.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Rect {
    /// Returns true when a point is inside this rectangle.
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

/// Margin for UI elements.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Margin {
    /// Left margin.
    pub left: f32,
    /// Right margin.
    pub right: f32,
    /// Top margin.
    pub top: f32,
    /// Bottom margin.
    pub bottom: f32,
}

/// Style box types for rendering UI element backgrounds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StyleBox {
    /// No background.
    Empty,
    /// Flat color fill.
    Flat {
        /// Background color.
        color: [f32; 4],
        /// Corner radius.
        border_radius: f32,
    },
    /// Textured background.
    Texture {
        /// Texture asset GUID.
        texture: AssetId,
        /// Nine-patch border.
        border: [f32; 4],
    },
}

/// UI theme configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    /// Default font asset GUID.
    pub default_font: Option<AssetId>,
    /// Base background color.
    pub base_color: [f32; 4],
    /// Accent/highlight color.
    pub accent_color: [f32; 4],
    /// Text color.
    pub text_color: [f32; 4],
    /// Color for disabled elements.
    pub disabled_color: [f32; 4],
    /// Default font size.
    pub font_size: f32,
    /// Default spacing.
    pub spacing: f32,
    /// Named style boxes.
    #[serde(default)]
    pub styles: HashMap<String, StyleBox>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            default_font: None,
            base_color: [0.15, 0.15, 0.15, 1.0],
            accent_color: [0.3, 0.6, 1.0, 1.0],
            text_color: [1.0, 1.0, 1.0, 1.0],
            disabled_color: [0.5, 0.5, 0.5, 0.5],
            font_size: 14.0,
            spacing: 8.0,
            styles: HashMap::new(),
        }
    }
}

/// UI event types.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    /// Mouse moved.
    MouseMove {
        /// X position.
        x: f32,
        /// Y position.
        y: f32,
    },
    /// Mouse button pressed.
    MouseDown {
        /// Button index.
        button: u8,
        /// X position.
        x: f32,
        /// Y position.
        y: f32,
    },
    /// Mouse button released.
    MouseUp {
        /// Button index.
        button: u8,
        /// X position.
        x: f32,
        /// Y position.
        y: f32,
    },
    /// Key pressed.
    KeyDown {
        /// Key name.
        key: String,
    },
    /// Text input.
    TextInput(String),
    /// Scroll.
    Scroll {
        /// Horizontal scroll.
        x: f32,
        /// Vertical scroll.
        y: f32,
    },
}

/// Result of handling a UI event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventResult {
    /// Event was consumed.
    Consumed,
    /// Event was ignored.
    Ignored,
}

impl Default for EventResult {
    fn default() -> Self {
        Self::Ignored
    }
}

/// Side effects requested by UI event routing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UiReply {
    /// Whether the event was consumed.
    pub result: EventResult,
    /// Widget that should receive keyboard focus.
    pub focus: Option<String>,
    /// Widget that should capture pointer events.
    pub capture_pointer: Option<String>,
    /// Whether pointer capture should be released.
    pub release_pointer: bool,
}

impl UiReply {
    /// Creates a reply for an ignored event.
    pub fn ignored() -> Self {
        Self {
            result: EventResult::Ignored,
            ..Self::default()
        }
    }

    /// Creates a reply for a consumed event.
    pub fn consumed() -> Self {
        Self {
            result: EventResult::Consumed,
            ..Self::default()
        }
    }

    /// Returns whether this reply consumed the event.
    pub fn is_consumed(&self) -> bool {
        self.result == EventResult::Consumed
    }
}

/// Layout data for a control node.
#[derive(Clone, Debug, Default)]
struct LayoutData {
    min_size: Vec2,
    position: Vec2,
    rect: Rect,
    margin: Margin,
}

/// Base control node in the UI tree.
pub struct ControlNode {
    /// Control name.
    pub name: String,
    /// Layout data.
    layout: LayoutData,
    /// Child controls.
    pub children: Vec<ControlNode>,
    /// Whether this control is visible.
    pub visible: bool,
    /// Whether this control is enabled.
    pub enabled: bool,
    /// Widget-specific data.
    widget: Box<dyn Widget>,
}

/// Widget trait for UI controls.
pub trait Widget: Any {
    /// Returns the widget type name.
    fn type_name(&self) -> &'static str;
    /// Measures the minimum size.
    fn measure(&self, theme: &Theme) -> Vec2;
    /// Returns the style box for rendering.
    fn style(&self, theme: &Theme) -> StyleBox;
    /// Handles an event.
    fn handle_event(&mut self, event: &UiEvent, theme: &Theme) -> EventResult;
    /// Updates hover state for widgets that track it.
    fn set_hovered(&mut self, _hovered: bool) {}
    /// Returns immutable any reference.
    fn as_any(&self) -> &dyn Any;
    /// Returns mutable any reference.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// A label widget.
pub struct LabelWidget {
    /// Label text.
    pub text: String,
}

impl Widget for LabelWidget {
    fn type_name(&self) -> &'static str {
        "Label"
    }

    fn measure(&self, theme: &Theme) -> Vec2 {
        Vec2::new(
            self.text.len() as f32 * theme.font_size * 0.6,
            theme.font_size * 1.2,
        )
    }

    fn style(&self, _theme: &Theme) -> StyleBox {
        StyleBox::Empty
    }

    fn handle_event(&mut self, _event: &UiEvent, _theme: &Theme) -> EventResult {
        EventResult::Ignored
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A button widget.
pub struct ButtonWidget {
    /// Button text.
    pub text: String,
    /// Whether the button was clicked this frame.
    pub clicked: bool,
    /// Whether the button is currently pressed.
    pub pressed: bool,
    /// Whether the button is hovered.
    pub hovered: bool,
}

impl Widget for ButtonWidget {
    fn type_name(&self) -> &'static str {
        "Button"
    }

    fn measure(&self, theme: &Theme) -> Vec2 {
        Vec2::new(
            self.text.len() as f32 * theme.font_size * 0.6 + theme.spacing * 2.0,
            theme.font_size * 2.0,
        )
    }

    fn style(&self, theme: &Theme) -> StyleBox {
        if self.pressed {
            StyleBox::Flat {
                color: theme.accent_color,
                border_radius: 4.0,
            }
        } else if self.hovered {
            let mut color = theme.accent_color;
            color[3] *= 0.8;
            StyleBox::Flat {
                color,
                border_radius: 4.0,
            }
        } else {
            StyleBox::Flat {
                color: theme.base_color,
                border_radius: 4.0,
            }
        }
    }

    fn handle_event(&mut self, event: &UiEvent, _theme: &Theme) -> EventResult {
        match event {
            UiEvent::MouseMove { .. } => EventResult::Ignored,
            UiEvent::MouseDown { button: 0, .. } => {
                self.pressed = true;
                EventResult::Consumed
            }
            UiEvent::MouseUp { button: 0, .. } => {
                if self.pressed {
                    self.clicked = true;
                    self.pressed = false;
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            _ => EventResult::Ignored,
        }
    }

    fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
        if !hovered {
            self.pressed = false;
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Root control tree for the UI system.
pub struct ControlTree {
    root: ControlNode,
    theme: Theme,
    focused: Option<String>,
    pointer_capture: Option<String>,
    hovered: Option<String>,
}

impl ControlTree {
    /// Creates a new control tree with default theme.
    pub fn new() -> Self {
        Self {
            root: ControlNode {
                name: "Root".to_string(),
                layout: LayoutData::default(),
                children: Vec::new(),
                visible: true,
                enabled: true,
                widget: Box::new(LabelWidget {
                    text: String::new(),
                }),
            },
            theme: Theme::default(),
            focused: None,
            pointer_capture: None,
            hovered: None,
        }
    }

    /// Returns the theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Returns a mutable reference to the theme.
    pub fn theme_mut(&mut self) -> &mut Theme {
        &mut self.theme
    }

    /// Adds a button to the control tree.
    pub fn add_button(&mut self, name: impl Into<String>, text: impl Into<String>) {
        self.root.children.push(ControlNode {
            name: name.into(),
            layout: LayoutData::default(),
            children: Vec::new(),
            visible: true,
            enabled: true,
            widget: Box::new(ButtonWidget {
                text: text.into(),
                clicked: false,
                pressed: false,
                hovered: false,
            }),
        });
    }

    /// Adds a label to the control tree.
    pub fn add_label(&mut self, name: impl Into<String>, text: impl Into<String>) {
        self.root.children.push(ControlNode {
            name: name.into(),
            layout: LayoutData::default(),
            children: Vec::new(),
            visible: true,
            enabled: true,
            widget: Box::new(LabelWidget { text: text.into() }),
        });
    }

    /// Performs layout for all controls.
    pub fn layout(&mut self, _available: Vec2) {
        let theme = self.theme.clone();
        layout_node(&mut self.root, &theme, Vec2::default());
    }

    /// Routes an event through the control tree.
    pub fn handle_event(&mut self, event: &UiEvent) -> EventResult {
        self.handle_event_reply(event).result
    }

    /// Routes an event through the control tree and returns routing side effects.
    pub fn handle_event_reply(&mut self, event: &UiEvent) -> UiReply {
        let theme = self.theme.clone();
        let mut reply = match event {
            UiEvent::MouseMove { x, y } => {
                let hit = hit_test_node(&self.root, *x, *y);
                self.set_hovered(hit.clone());
                let result = if let Some(captor) = self.pointer_capture.clone().or(hit) {
                    route_event_to_node(&mut self.root, &captor, event, &theme)
                } else {
                    EventResult::Ignored
                };
                UiReply {
                    result,
                    focus: None,
                    capture_pointer: None,
                    release_pointer: false,
                }
            }
            UiEvent::MouseDown { x, y, .. } => {
                if let Some(target) = hit_test_node(&self.root, *x, *y) {
                    let result = route_event_to_node(&mut self.root, &target, event, &theme);
                    if result == EventResult::Consumed {
                        UiReply {
                            result,
                            focus: Some(target.clone()),
                            capture_pointer: Some(target),
                            release_pointer: false,
                        }
                    } else {
                        UiReply::ignored()
                    }
                } else {
                    self.focused = None;
                    UiReply::ignored()
                }
            }
            UiEvent::MouseUp { x, y, .. } => {
                let target = self
                    .pointer_capture
                    .clone()
                    .or_else(|| hit_test_node(&self.root, *x, *y));
                let result = target
                    .as_deref()
                    .map(|name| route_event_to_node(&mut self.root, name, event, &theme))
                    .unwrap_or(EventResult::Ignored);
                UiReply {
                    result,
                    focus: None,
                    capture_pointer: None,
                    release_pointer: true,
                }
            }
            UiEvent::KeyDown { .. } | UiEvent::TextInput(_) => self
                .focused
                .clone()
                .map(|name| route_event_to_node(&mut self.root, &name, event, &theme))
                .map(|result| UiReply {
                    result,
                    focus: None,
                    capture_pointer: None,
                    release_pointer: false,
                })
                .unwrap_or_else(UiReply::ignored),
            UiEvent::Scroll { .. } => self
                .hovered
                .clone()
                .map(|name| route_event_to_node(&mut self.root, &name, event, &theme))
                .map(|result| UiReply {
                    result,
                    focus: None,
                    capture_pointer: None,
                    release_pointer: false,
                })
                .unwrap_or_else(UiReply::ignored),
        };
        self.apply_reply(&reply);
        if reply.release_pointer {
            self.set_hovered(
                pointer_position(event).and_then(|(x, y)| hit_test_node(&self.root, x, y)),
            );
        }
        if reply.result == EventResult::Consumed {
            reply.result = EventResult::Consumed;
        }
        reply
    }

    /// Returns the focused control name, if any.
    pub fn focused_control(&self) -> Option<&str> {
        self.focused.as_deref()
    }

    /// Returns the control currently capturing pointer events, if any.
    pub fn pointer_capture(&self) -> Option<&str> {
        self.pointer_capture.as_deref()
    }

    /// Returns the hovered control name, if any.
    pub fn hovered_control(&self) -> Option<&str> {
        self.hovered.as_deref()
    }

    /// Collects draw data for all visible controls.
    pub fn collect_draw_data(&self) -> Vec<DrawCommand> {
        let mut commands = Vec::new();
        for child in &self.root.children {
            self.collect_node_draw(child, &mut commands);
        }
        commands
    }

    /// Builds a GPU-ready GUI draw list for all visible controls.
    ///
    /// This is the retained UI renderer's low-level output. It currently draws
    /// flat/textured boxes and lightweight text placeholders into the same
    /// [`GuiDrawList`] format used by the WGPU GUI pass.
    pub fn build_gui_draw_list(&self, screen_size: Vec2) -> GuiDrawList {
        let mut builder = UiDrawListBuilder::new(screen_size);
        for child in &self.root.children {
            render_node_to_gui(child, &self.theme, &mut builder);
        }
        builder.finish()
    }

    fn collect_node_draw(&self, node: &ControlNode, commands: &mut Vec<DrawCommand>) {
        if !node.visible {
            return;
        }
        commands.push(DrawCommand {
            position: Vec2::new(node.layout.rect.x, node.layout.rect.y),
            size: node.layout.min_size,
            style: node.widget.style(&self.theme),
        });
        for child in &node.children {
            self.collect_node_draw(child, commands);
        }
    }

    fn apply_reply(&mut self, reply: &UiReply) {
        if let Some(focus) = &reply.focus {
            self.focused = Some(focus.clone());
        }
        if let Some(capture) = &reply.capture_pointer {
            self.pointer_capture = Some(capture.clone());
        }
        if reply.release_pointer {
            self.pointer_capture = None;
        }
    }

    fn set_hovered(&mut self, next: Option<String>) {
        if self.hovered == next {
            return;
        }
        if let Some(previous) = self.hovered.take() {
            set_node_hovered(&mut self.root, &previous, false);
        }
        if let Some(next_name) = next {
            set_node_hovered(&mut self.root, &next_name, true);
            self.hovered = Some(next_name);
        }
    }
}

impl Default for ControlTree {
    fn default() -> Self {
        Self::new()
    }
}

fn layout_node(node: &mut ControlNode, theme: &Theme, origin: Vec2) {
    node.layout.min_size = node.widget.measure(theme);
    node.layout.rect = Rect {
        x: origin.x + node.layout.position.x,
        y: origin.y + node.layout.position.y,
        width: node.layout.min_size.x,
        height: node.layout.min_size.y,
    };
    let mut y_offset = node.layout.margin.top;
    for child in &mut node.children {
        let child_min_size = child.widget.measure(theme);
        child.layout.position = Vec2::new(node.layout.margin.left, y_offset);
        y_offset += child.layout.margin.top + child.layout.margin.bottom + child_min_size.y;
        layout_node(
            child,
            theme,
            Vec2::new(node.layout.rect.x, node.layout.rect.y),
        );
    }
}

fn hit_test_node(node: &ControlNode, x: f32, y: f32) -> Option<String> {
    if !node.visible || !node.enabled {
        return None;
    }
    for child in node.children.iter().rev() {
        if let Some(hit) = hit_test_node(child, x, y) {
            return Some(hit);
        }
    }
    if node.layout.rect.contains(x, y) {
        Some(node.name.clone())
    } else {
        None
    }
}

fn route_event_to_node(
    node: &mut ControlNode,
    target: &str,
    event: &UiEvent,
    theme: &Theme,
) -> EventResult {
    if !node.visible || !node.enabled {
        return EventResult::Ignored;
    }
    if node.name == target {
        return node.widget.handle_event(event, theme);
    }
    for child in &mut node.children {
        let result = route_event_to_node(child, target, event, theme);
        if result == EventResult::Consumed {
            return result;
        }
    }
    EventResult::Ignored
}

fn set_node_hovered(node: &mut ControlNode, target: &str, hovered: bool) -> bool {
    if node.name == target {
        node.widget.set_hovered(hovered);
        return true;
    }
    for child in &mut node.children {
        if set_node_hovered(child, target, hovered) {
            return true;
        }
    }
    false
}

fn pointer_position(event: &UiEvent) -> Option<(f32, f32)> {
    match event {
        UiEvent::MouseMove { x, y }
        | UiEvent::MouseDown { x, y, .. }
        | UiEvent::MouseUp { x, y, .. } => Some((*x, *y)),
        UiEvent::KeyDown { .. } | UiEvent::TextInput(_) | UiEvent::Scroll { .. } => None,
    }
}

/// A draw command for batched UI rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawCommand {
    /// Screen position.
    pub position: Vec2,
    /// Element size.
    pub size: Vec2,
    /// Style box.
    pub style: StyleBox,
}

struct UiDrawListBuilder {
    screen_size: Vec2,
    vertices: Vec<GuiVertex>,
    indices: Vec<u32>,
}

impl UiDrawListBuilder {
    fn new(screen_size: Vec2) -> Self {
        Self {
            screen_size,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn finish(self) -> GuiDrawList {
        let index_count = self.indices.len() as u32;
        let scissor = [
            0,
            0,
            self.screen_size.x.max(0.0).round() as u32,
            self.screen_size.y.max(0.0).round() as u32,
        ];
        let commands = if index_count == 0 {
            Vec::new()
        } else {
            vec![GuiDrawCmd {
                texture: DEFAULT_GUI_TEXTURE,
                scissor,
                index_offset: 0,
                index_count,
            }]
        };
        GuiDrawList {
            vertices: self.vertices,
            indices: self.indices,
            commands,
        }
    }

    fn rect(&mut self, rect: Rect, color: [f32; 4]) {
        if rect.width <= 0.0 || rect.height <= 0.0 || color[3] <= 0.0 {
            return;
        }
        let base = self.vertices.len() as u32;
        let packed = pack_linear_rgba(color);
        let min = [rect.x, rect.y];
        let max = [rect.x + rect.width, rect.y + rect.height];
        for pos in [min, [max[0], min[1]], max, [min[0], max[1]]] {
            self.vertices.push(GuiVertex {
                pos,
                uv: [0.5, 0.5],
                color: packed,
            });
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    }

    fn text_placeholder(&mut self, rect: Rect, text: &str, theme: &Theme) {
        if text.is_empty() {
            return;
        }
        let glyph_width = (theme.font_size * 0.36).max(2.0);
        let glyph_height = (theme.font_size * 0.72).max(4.0);
        let advance = (theme.font_size * 0.58).max(glyph_width + 1.0);
        let y = rect.y + ((rect.height - glyph_height) * 0.5).max(0.0);
        let mut x = rect.x + theme.spacing.min(rect.width * 0.25);
        let max_x = rect.x + rect.width - glyph_width;
        for ch in text.chars() {
            if ch.is_whitespace() {
                x += advance;
                continue;
            }
            if x > max_x {
                break;
            }
            let height_scale = if ch.is_ascii_uppercase() { 1.0 } else { 0.78 };
            self.rect(
                Rect {
                    x,
                    y: y + glyph_height * (1.0 - height_scale),
                    width: glyph_width,
                    height: glyph_height * height_scale,
                },
                theme.text_color,
            );
            x += advance;
        }
    }
}

fn render_node_to_gui(node: &ControlNode, theme: &Theme, builder: &mut UiDrawListBuilder) {
    if !node.visible {
        return;
    }
    match node.widget.style(theme) {
        StyleBox::Empty => {}
        StyleBox::Flat { color, .. } => builder.rect(node.layout.rect, color),
        StyleBox::Texture { .. } => builder.rect(node.layout.rect, [1.0, 1.0, 1.0, 1.0]),
    }
    if let Some(text) = widget_text(node.widget.as_ref()) {
        builder.text_placeholder(node.layout.rect, text, theme);
    }
    for child in &node.children {
        render_node_to_gui(child, theme, builder);
    }
}

fn widget_text(widget: &dyn Widget) -> Option<&str> {
    if let Some(label) = widget.as_any().downcast_ref::<LabelWidget>() {
        return Some(&label.text);
    }
    if let Some(button) = widget.as_any().downcast_ref::<ButtonWidget>() {
        return Some(&button.text);
    }
    None
}

fn pack_linear_rgba(color: [f32; 4]) -> u32 {
    let r = channel_to_u8(color[0]);
    let g = channel_to_u8(color[1]);
    let b = channel_to_u8(color[2]);
    let a = channel_to_u8(color[3]);
    u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16) | (u32::from(a) << 24)
}

fn channel_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_control_tree_builds_empty_gui_draw_list() {
        let mut tree = ControlTree::new();
        tree.layout(Vec2::new(320.0, 180.0));

        let draw_list = tree.build_gui_draw_list(Vec2::new(320.0, 180.0));

        assert!(draw_list.vertices.is_empty());
        assert!(draw_list.indices.is_empty());
        assert!(draw_list.commands.is_empty());
    }

    #[test]
    fn button_and_label_build_gpu_ready_gui_draw_list() {
        let mut tree = ControlTree::new();
        tree.add_button("play", "Play");
        tree.add_label("status", "Ready");
        tree.layout(Vec2::new(640.0, 360.0));

        let draw_list = tree.build_gui_draw_list(Vec2::new(640.0, 360.0));

        assert!(!draw_list.vertices.is_empty());
        assert!(!draw_list.indices.is_empty());
        assert_eq!(draw_list.commands.len(), 1);
        assert_eq!(draw_list.commands[0].texture, DEFAULT_GUI_TEXTURE);
        assert_eq!(draw_list.commands[0].scissor, [0, 0, 640, 360]);
        assert_eq!(
            draw_list.commands[0].index_count as usize,
            draw_list.indices.len()
        );
    }

    #[test]
    fn layout_stacks_multiple_controls_vertically() {
        let mut tree = ControlTree::new();
        tree.add_button("play", "Play");
        tree.add_label("status", "Ready");
        tree.layout(Vec2::new(640.0, 360.0));

        let commands = tree.collect_draw_data();

        assert_eq!(commands.len(), 2);
        assert!(commands[1].position.y > commands[0].position.y);
    }

    #[test]
    fn gui_draw_list_reflects_hovered_button_style() {
        let mut tree = ControlTree::new();
        tree.add_button("play", "Play");
        tree.layout(Vec2::new(320.0, 180.0));
        let normal = tree.build_gui_draw_list(Vec2::new(320.0, 180.0));

        tree.handle_event(&UiEvent::MouseMove { x: 4.0, y: 4.0 });
        let hovered = tree.build_gui_draw_list(Vec2::new(320.0, 180.0));

        assert_ne!(normal.vertices[0].color, hovered.vertices[0].color);
    }
}
