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
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
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

    /// Returns this vector with each component at least `min`.
    pub fn max(self, min: Self) -> Self {
        Self::new(self.x.max(min.x), self.y.max(min.y))
    }

    /// Returns this vector with each component at most `max`.
    pub fn min(self, max: Self) -> Self {
        Self::new(self.x.min(max.x), self.y.min(max.y))
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
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

impl Margin {
    /// Creates a margin where every side uses the same value.
    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    /// Creates a margin from explicit side values.
    pub const fn new(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self {
            left,
            right,
            top,
            bottom,
        }
    }
}

/// Visibility and hit-test policy for a control.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Visibility {
    /// The control participates in layout, renders, and can receive input.
    #[default]
    Visible,
    /// The control keeps its layout space but does not render or receive input.
    Hidden,
    /// The control is removed from layout, rendering, and input routing.
    Collapsed,
    /// The control renders but neither it nor its children receive pointer input.
    HitTestInvisible,
    /// The control renders and its children can receive pointer input, but the control itself cannot.
    SelfHitTestInvisible,
}

impl Visibility {
    /// Returns whether this visibility participates in layout.
    pub fn participates_in_layout(self) -> bool {
        !matches!(self, Self::Collapsed)
    }

    /// Returns whether this visibility should render.
    pub fn renders(self) -> bool {
        matches!(
            self,
            Self::Visible | Self::HitTestInvisible | Self::SelfHitTestInvisible
        )
    }

    /// Returns whether this visibility accepts a hit on the control itself.
    pub fn accepts_self_hit_test(self) -> bool {
        matches!(self, Self::Visible)
    }

    /// Returns whether this visibility allows descendants to receive pointer input.
    pub fn allows_child_hit_test(self) -> bool {
        matches!(self, Self::Visible | Self::SelfHitTestInvisible)
    }
}

/// Direction used by stack-style panel layout.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum StackDirection {
    /// Children are arranged top to bottom.
    #[default]
    Vertical,
    /// Children are arranged left to right.
    Horizontal,
}

/// Alignment inside a layout slot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Alignment {
    /// Align to the leading edge.
    #[default]
    Start,
    /// Align to the center.
    Center,
    /// Align to the trailing edge.
    End,
    /// Stretch to fill the available slot span.
    Fill,
}

/// Per-axis size rule for a layout slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum SizeRule {
    /// Use the widget's measured desired size.
    #[default]
    Auto,
    /// Use an explicit size in pixels.
    Fixed(f32),
    /// Share remaining parent space with other fill slots.
    Fill {
        /// Relative fill weight.
        weight: f32,
    },
}

impl SizeRule {
    fn fill_weight(self) -> f32 {
        match self {
            Self::Fill { weight } => weight.max(0.0),
            Self::Auto | Self::Fixed(_) => 0.0,
        }
    }
}

/// Parent-owned layout slot data for a control.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlotLayout {
    /// Outer margin around the control.
    pub margin: Margin,
    /// Width rule.
    pub width: SizeRule,
    /// Height rule.
    pub height: SizeRule,
    /// Minimum final size.
    pub min_size: Vec2,
    /// Maximum final size.
    pub max_size: Vec2,
    /// Horizontal alignment inside the allocated slot.
    pub horizontal_alignment: Alignment,
    /// Vertical alignment inside the allocated slot.
    pub vertical_alignment: Alignment,
}

impl Default for SlotLayout {
    fn default() -> Self {
        Self {
            margin: Margin::default(),
            width: SizeRule::Auto,
            height: SizeRule::Auto,
            min_size: Vec2::default(),
            max_size: Vec2::new(f32::INFINITY, f32::INFINITY),
            horizontal_alignment: Alignment::Start,
            vertical_alignment: Alignment::Start,
        }
    }
}

impl SlotLayout {
    /// Creates a default slot layout.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a slot with the supplied margin.
    pub fn with_margin(mut self, margin: Margin) -> Self {
        self.margin = margin;
        self
    }

    /// Returns a slot with fixed width and height.
    pub fn fixed(mut self, width: f32, height: f32) -> Self {
        self.width = SizeRule::Fixed(width);
        self.height = SizeRule::Fixed(height);
        self
    }

    /// Returns a slot that fills both axes.
    pub fn fill(mut self, weight: f32) -> Self {
        self.width = SizeRule::Fill { weight };
        self.height = SizeRule::Fill { weight };
        self.horizontal_alignment = Alignment::Fill;
        self.vertical_alignment = Alignment::Fill;
        self
    }
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
    desired_size: Vec2,
    min_size: Vec2,
    position: Vec2,
    rect: Rect,
    slot: SlotLayout,
    needs_layout: bool,
    needs_paint: bool,
}

/// Arranged control geometry after a layout pass.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrangedControl {
    /// Stable control name.
    pub name: String,
    /// Widget type name.
    pub widget_type: &'static str,
    /// Final screen-space rectangle.
    pub rect: Rect,
    /// Visibility policy used for this control.
    pub visibility: Visibility,
    /// Tree depth, with root children at depth 0.
    pub depth: usize,
}

/// Counts controls marked dirty for layout or paint.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiInvalidationStats {
    /// Number of controls that need a layout pass.
    pub layout_dirty: usize,
    /// Number of controls that need paint data regeneration.
    pub paint_dirty: usize,
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
    /// Visibility and hit-test policy.
    pub visibility: Visibility,
    /// Whether this control is enabled.
    pub enabled: bool,
    /// Widget-specific data.
    widget: Box<dyn Widget>,
}

impl ControlNode {
    /// Sets the local layout position.
    pub fn set_position(&mut self, position: Vec2) {
        self.layout.position = position;
        self.invalidate_layout();
    }

    /// Sets the margin used by parent panel layout.
    pub fn set_margin(&mut self, margin: Margin) {
        self.layout.slot.margin = margin;
        self.invalidate_layout();
    }

    /// Sets parent-owned slot layout data.
    pub fn set_slot(&mut self, slot: SlotLayout) {
        self.layout.slot = slot;
        self.invalidate_layout();
    }

    /// Sets the visibility policy.
    pub fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
        self.visible = visibility.renders();
        self.invalidate_layout();
    }

    /// Marks this node and its descendants as needing layout.
    pub fn invalidate_layout(&mut self) {
        self.layout.needs_layout = true;
        self.layout.needs_paint = true;
        for child in &mut self.children {
            child.invalidate_layout();
        }
    }

    /// Marks this node and its descendants as needing paint.
    pub fn invalidate_paint(&mut self) {
        self.layout.needs_paint = true;
        for child in &mut self.children {
            child.invalidate_paint();
        }
    }

    fn effective_visibility(&self) -> Visibility {
        if self.visible {
            self.visibility
        } else if self.visibility == Visibility::Collapsed {
            Visibility::Collapsed
        } else {
            Visibility::Hidden
        }
    }
}

/// Widget trait for UI controls.
pub trait Widget: Any {
    /// Returns the widget type name.
    fn type_name(&self) -> &'static str;
    /// Measures the minimum size.
    fn measure(&self, theme: &Theme) -> Vec2;
    /// Returns the style box for rendering.
    fn style(&self, theme: &Theme) -> StyleBox;
    /// Returns stack panel layout direction for widgets that arrange children.
    fn stack_direction(&self) -> Option<StackDirection> {
        Some(StackDirection::Vertical)
    }
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

    fn stack_direction(&self) -> Option<StackDirection> {
        None
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

/// A panel widget that lays out child controls in a stack.
pub struct PanelWidget {
    /// Stack direction used for child layout.
    pub direction: StackDirection,
    /// Optional background style.
    pub background: StyleBox,
}

impl Widget for PanelWidget {
    fn type_name(&self) -> &'static str {
        "Panel"
    }

    fn measure(&self, _theme: &Theme) -> Vec2 {
        Vec2::default()
    }

    fn style(&self, _theme: &Theme) -> StyleBox {
        self.background.clone()
    }

    fn stack_direction(&self) -> Option<StackDirection> {
        Some(self.direction)
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

    fn stack_direction(&self) -> Option<StackDirection> {
        None
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
    last_available: Vec2,
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
                visibility: Visibility::Visible,
                enabled: true,
                widget: Box::new(PanelWidget {
                    direction: StackDirection::Vertical,
                    background: StyleBox::Empty,
                }),
            },
            theme: Theme::default(),
            focused: None,
            pointer_capture: None,
            hovered: None,
            last_available: Vec2::default(),
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
            visibility: Visibility::Visible,
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
            visibility: Visibility::Visible,
            enabled: true,
            widget: Box::new(LabelWidget { text: text.into() }),
        });
    }

    /// Adds a stack panel to the root control.
    pub fn add_panel(&mut self, name: impl Into<String>, direction: StackDirection) {
        self.root.children.push(ControlNode {
            name: name.into(),
            layout: LayoutData::default(),
            children: Vec::new(),
            visible: true,
            visibility: Visibility::Visible,
            enabled: true,
            widget: Box::new(PanelWidget {
                direction,
                background: StyleBox::Empty,
            }),
        });
    }

    /// Adds a label as a child of an existing control.
    pub fn add_label_to(
        &mut self,
        parent: &str,
        name: impl Into<String>,
        text: impl Into<String>,
    ) -> bool {
        let Some(parent) = find_node_mut(&mut self.root, parent) else {
            return false;
        };
        parent.children.push(ControlNode {
            name: name.into(),
            layout: LayoutData::default(),
            children: Vec::new(),
            visible: true,
            visibility: Visibility::Visible,
            enabled: true,
            widget: Box::new(LabelWidget { text: text.into() }),
        });
        true
    }

    /// Adds a button as a child of an existing control.
    pub fn add_button_to(
        &mut self,
        parent: &str,
        name: impl Into<String>,
        text: impl Into<String>,
    ) -> bool {
        let Some(parent) = find_node_mut(&mut self.root, parent) else {
            return false;
        };
        parent.children.push(ControlNode {
            name: name.into(),
            layout: LayoutData::default(),
            children: Vec::new(),
            visible: true,
            visibility: Visibility::Visible,
            enabled: true,
            widget: Box::new(ButtonWidget {
                text: text.into(),
                clicked: false,
                pressed: false,
                hovered: false,
            }),
        });
        true
    }

    /// Adds a stack panel as a child of an existing control.
    pub fn add_panel_to(
        &mut self,
        parent: &str,
        name: impl Into<String>,
        direction: StackDirection,
    ) -> bool {
        let Some(parent) = find_node_mut(&mut self.root, parent) else {
            return false;
        };
        parent.children.push(ControlNode {
            name: name.into(),
            layout: LayoutData::default(),
            children: Vec::new(),
            visible: true,
            visibility: Visibility::Visible,
            enabled: true,
            widget: Box::new(PanelWidget {
                direction,
                background: StyleBox::Empty,
            }),
        });
        true
    }

    /// Sets a control's local position.
    pub fn set_position(&mut self, name: &str, position: Vec2) -> bool {
        find_node_mut(&mut self.root, name)
            .map(|node| node.set_position(position))
            .is_some()
    }

    /// Sets a control's margin.
    pub fn set_margin(&mut self, name: &str, margin: Margin) -> bool {
        find_node_mut(&mut self.root, name)
            .map(|node| node.set_margin(margin))
            .is_some()
    }

    /// Sets a control's parent-owned layout slot.
    pub fn set_slot(&mut self, name: &str, slot: SlotLayout) -> bool {
        find_node_mut(&mut self.root, name)
            .map(|node| node.set_slot(slot))
            .is_some()
    }

    /// Sets a control's visibility and hit-test policy.
    pub fn set_visibility(&mut self, name: &str, visibility: Visibility) -> bool {
        find_node_mut(&mut self.root, name)
            .map(|node| node.set_visibility(visibility))
            .is_some()
    }

    /// Performs layout for all controls.
    pub fn layout(&mut self, available: Vec2) {
        let theme = self.theme.clone();
        self.last_available = available;
        self.root.layout.position = Vec2::default();
        self.root.layout.slot.width = SizeRule::Fixed(available.x);
        self.root.layout.slot.height = SizeRule::Fixed(available.y);
        layout_node(&mut self.root, &theme, Vec2::default(), available);
        clear_layout_dirty(&mut self.root);
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
                let hit = hit_test_node(&self.root, *x, *y).map(|hit| hit.name);
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
                if let Some(target) = hit_test_node(&self.root, *x, *y).map(|hit| hit.name) {
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
                    .or_else(|| hit_test_node(&self.root, *x, *y).map(|hit| hit.name));
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
                pointer_position(event)
                    .and_then(|(x, y)| hit_test_node(&self.root, x, y).map(|hit| hit.name)),
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

    /// Returns arranged control geometry from the most recent layout pass.
    pub fn arranged_controls(&self) -> Vec<ArrangedControl> {
        let mut controls = Vec::new();
        for child in &self.root.children {
            collect_arranged_node(child, 0, &mut controls);
        }
        controls
    }

    /// Returns the deepest hit control and its ancestor path at a screen-space point.
    pub fn hit_test_path(&self, x: f32, y: f32) -> Option<HitTestPath> {
        hit_test_node(&self.root, x, y)
    }

    /// Returns current layout and paint invalidation counts.
    pub fn invalidation_stats(&self) -> UiInvalidationStats {
        let mut stats = UiInvalidationStats::default();
        count_invalidation(&self.root, &mut stats);
        stats
    }

    /// Marks all controls as needing layout and paint.
    pub fn invalidate_layout(&mut self) {
        self.root.invalidate_layout();
    }

    /// Marks all controls as needing paint.
    pub fn invalidate_paint(&mut self) {
        self.root.invalidate_paint();
    }

    /// Returns the size supplied to the latest layout pass.
    pub fn last_available_size(&self) -> Vec2 {
        self.last_available
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
        if !node.effective_visibility().renders() {
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

fn layout_node(node: &mut ControlNode, theme: &Theme, origin: Vec2, available: Vec2) {
    if !node.effective_visibility().participates_in_layout() {
        node.layout.desired_size = Vec2::default();
        node.layout.min_size = Vec2::default();
        node.layout.rect = Rect::default();
        return;
    }

    let widget_min = node.widget.measure(theme);
    let direction = node.widget.stack_direction();
    let desired = measure_node(node, theme);
    let size = resolve_size(node.layout.slot, desired.max(widget_min), available);
    node.layout.desired_size = desired;
    node.layout.min_size = size;
    node.layout.rect = Rect {
        x: origin.x + node.layout.position.x,
        y: origin.y + node.layout.position.y,
        width: size.x,
        height: size.y,
    };

    if let Some(direction) = direction {
        arrange_stack_children(node, theme, direction);
    } else {
        for child in &mut node.children {
            layout_node(
                child,
                theme,
                Vec2::new(node.layout.rect.x, node.layout.rect.y),
                node.layout.min_size,
            );
        }
    }
}

fn measure_node(node: &mut ControlNode, theme: &Theme) -> Vec2 {
    if !node.effective_visibility().participates_in_layout() {
        node.layout.desired_size = Vec2::default();
        return Vec2::default();
    }

    let widget_min = node.widget.measure(theme);
    let Some(direction) = node.widget.stack_direction() else {
        node.layout.desired_size = widget_min;
        return widget_min;
    };

    let mut desired = widget_min;
    match direction {
        StackDirection::Vertical => {
            let mut width: f32 = 0.0;
            let mut height: f32 = 0.0;
            for child in &mut node.children {
                let child_size = measure_node(child, theme);
                if !child.effective_visibility().participates_in_layout() {
                    continue;
                }
                let margin = child.layout.slot.margin;
                width = width.max(margin.left + child_size.x + margin.right);
                height += margin.top + child_size.y + margin.bottom;
            }
            desired = desired.max(Vec2::new(width, height));
        }
        StackDirection::Horizontal => {
            let mut width: f32 = 0.0;
            let mut height: f32 = 0.0;
            for child in &mut node.children {
                let child_size = measure_node(child, theme);
                if !child.effective_visibility().participates_in_layout() {
                    continue;
                }
                let margin = child.layout.slot.margin;
                width += margin.left + child_size.x + margin.right;
                height = height.max(margin.top + child_size.y + margin.bottom);
            }
            desired = desired.max(Vec2::new(width, height));
        }
    }
    node.layout.desired_size = desired;
    desired
}

fn arrange_stack_children(node: &mut ControlNode, theme: &Theme, direction: StackDirection) {
    let content_size = node.layout.min_size;
    let fixed_primary = stack_fixed_primary(&node.children, direction);
    let fill_weight = stack_fill_weight(&node.children, direction);
    let remaining_primary = (stack_primary(content_size, direction) - fixed_primary).max(0.0);
    let mut cursor = 0.0;

    for child in &mut node.children {
        if !child.effective_visibility().participates_in_layout() {
            layout_node(
                child,
                theme,
                Vec2::new(node.layout.rect.x, node.layout.rect.y),
                Vec2::default(),
            );
            continue;
        }

        let slot = child.layout.slot;
        let desired = child.layout.desired_size;
        let margin = slot.margin;
        let primary_margin = stack_primary_margin(margin, direction);
        let cross_margin = stack_cross_margin(margin, direction);
        let slot_primary = match stack_size_rule(slot, direction) {
            SizeRule::Fill { weight } if fill_weight > f32::EPSILON => {
                remaining_primary * weight.max(0.0) / fill_weight
            }
            SizeRule::Fixed(value) => value.max(0.0),
            SizeRule::Auto | SizeRule::Fill { .. } => stack_primary(desired, direction),
        };
        let slot_cross = (stack_cross(content_size, direction) - cross_margin).max(0.0);
        let mut child_available = stack_vec(slot_primary, slot_cross, direction);
        child_available = child_available.max(slot.min_size).min(slot.max_size);

        let child_size = resolve_size(slot, desired, child_available);
        let aligned = align_in_slot(slot, child_size, child_available, direction);
        let position = stack_vec(
            cursor + stack_leading_margin(margin, direction) + stack_primary(aligned, direction),
            stack_leading_cross_margin(margin, direction) + stack_cross(aligned, direction),
            direction,
        );

        child.layout.position = position;
        layout_node(
            child,
            theme,
            Vec2::new(node.layout.rect.x, node.layout.rect.y),
            child_available,
        );
        cursor += primary_margin + slot_primary;
    }
}

fn resolve_size(slot: SlotLayout, desired: Vec2, available: Vec2) -> Vec2 {
    let width = resolve_axis(slot.width, desired.x, available.x);
    let height = resolve_axis(slot.height, desired.y, available.y);
    Vec2::new(width, height)
        .max(slot.min_size)
        .min(slot.max_size)
}

fn resolve_axis(rule: SizeRule, desired: f32, available: f32) -> f32 {
    match rule {
        SizeRule::Auto => desired,
        SizeRule::Fixed(value) => value.max(0.0),
        SizeRule::Fill { .. } => available.max(0.0),
    }
}

fn stack_fixed_primary(children: &[ControlNode], direction: StackDirection) -> f32 {
    children
        .iter()
        .filter(|child| child.effective_visibility().participates_in_layout())
        .map(|child| {
            let margin = stack_primary_margin(child.layout.slot.margin, direction);
            let size = match stack_size_rule(child.layout.slot, direction) {
                SizeRule::Fill { .. } => 0.0,
                SizeRule::Fixed(value) => value.max(0.0),
                SizeRule::Auto => stack_primary(child.layout.desired_size, direction),
            };
            margin + size
        })
        .sum()
}

fn stack_fill_weight(children: &[ControlNode], direction: StackDirection) -> f32 {
    children
        .iter()
        .filter(|child| child.effective_visibility().participates_in_layout())
        .map(|child| stack_size_rule(child.layout.slot, direction).fill_weight())
        .sum()
}

fn stack_size_rule(slot: SlotLayout, direction: StackDirection) -> SizeRule {
    match direction {
        StackDirection::Vertical => slot.height,
        StackDirection::Horizontal => slot.width,
    }
}

fn stack_primary(size: Vec2, direction: StackDirection) -> f32 {
    match direction {
        StackDirection::Vertical => size.y,
        StackDirection::Horizontal => size.x,
    }
}

fn stack_cross(size: Vec2, direction: StackDirection) -> f32 {
    match direction {
        StackDirection::Vertical => size.x,
        StackDirection::Horizontal => size.y,
    }
}

fn stack_vec(primary: f32, cross: f32, direction: StackDirection) -> Vec2 {
    match direction {
        StackDirection::Vertical => Vec2::new(cross, primary),
        StackDirection::Horizontal => Vec2::new(primary, cross),
    }
}

fn stack_primary_margin(margin: Margin, direction: StackDirection) -> f32 {
    match direction {
        StackDirection::Vertical => margin.top + margin.bottom,
        StackDirection::Horizontal => margin.left + margin.right,
    }
}

fn stack_cross_margin(margin: Margin, direction: StackDirection) -> f32 {
    match direction {
        StackDirection::Vertical => margin.left + margin.right,
        StackDirection::Horizontal => margin.top + margin.bottom,
    }
}

fn stack_leading_margin(margin: Margin, direction: StackDirection) -> f32 {
    match direction {
        StackDirection::Vertical => margin.top,
        StackDirection::Horizontal => margin.left,
    }
}

fn stack_leading_cross_margin(margin: Margin, direction: StackDirection) -> f32 {
    match direction {
        StackDirection::Vertical => margin.left,
        StackDirection::Horizontal => margin.top,
    }
}

fn align_in_slot(
    slot: SlotLayout,
    child_size: Vec2,
    available: Vec2,
    direction: StackDirection,
) -> Vec2 {
    let horizontal = align_offset(slot.horizontal_alignment, child_size.x, available.x);
    let vertical = align_offset(slot.vertical_alignment, child_size.y, available.y);
    match direction {
        StackDirection::Vertical | StackDirection::Horizontal => Vec2::new(horizontal, vertical),
    }
}

fn align_offset(alignment: Alignment, child: f32, available: f32) -> f32 {
    match alignment {
        Alignment::Start | Alignment::Fill => 0.0,
        Alignment::Center => ((available - child) * 0.5).max(0.0),
        Alignment::End => (available - child).max(0.0),
    }
}

/// Result of a UI hit-test.
#[derive(Clone, Debug, PartialEq)]
pub struct HitTestPath {
    /// Deepest target control name.
    pub name: String,
    /// Ancestor path from root child to target.
    pub path: Vec<String>,
    /// Target rectangle.
    pub rect: Rect,
}

fn hit_test_node(node: &ControlNode, x: f32, y: f32) -> Option<HitTestPath> {
    let visibility = node.effective_visibility();
    if !node.enabled || !visibility.renders() || !visibility.allows_child_hit_test() {
        return None;
    }
    let mut path = Vec::new();
    for child in node.children.iter().rev() {
        if let Some(hit) = hit_test_node_inner(child, x, y, &mut path) {
            return Some(hit);
        }
    }
    None
}

fn hit_test_node_inner(
    node: &ControlNode,
    x: f32,
    y: f32,
    path: &mut Vec<String>,
) -> Option<HitTestPath> {
    let visibility = node.effective_visibility();
    if !node.enabled || !visibility.renders() {
        return None;
    }
    path.push(node.name.clone());
    if visibility.allows_child_hit_test() {
        for child in node.children.iter().rev() {
            if let Some(hit) = hit_test_node_inner(child, x, y, path) {
                path.pop();
                return Some(hit);
            }
        }
    }
    let hit = if visibility.accepts_self_hit_test() && node.layout.rect.contains(x, y) {
        Some(HitTestPath {
            name: node.name.clone(),
            path: path.clone(),
            rect: node.layout.rect,
        })
    } else {
        None
    };
    path.pop();
    hit
}

fn collect_arranged_node(node: &ControlNode, depth: usize, controls: &mut Vec<ArrangedControl>) {
    let visibility = node.effective_visibility();
    if !visibility.participates_in_layout() {
        return;
    }
    controls.push(ArrangedControl {
        name: node.name.clone(),
        widget_type: node.widget.type_name(),
        rect: node.layout.rect,
        visibility,
        depth,
    });
    for child in &node.children {
        collect_arranged_node(child, depth + 1, controls);
    }
}

fn find_node_mut<'a>(node: &'a mut ControlNode, name: &str) -> Option<&'a mut ControlNode> {
    if node.name == name {
        return Some(node);
    }
    for child in &mut node.children {
        if let Some(hit) = find_node_mut(child, name) {
            return Some(hit);
        }
    }
    None
}

fn clear_layout_dirty(node: &mut ControlNode) {
    node.layout.needs_layout = false;
    node.layout.needs_paint = false;
    for child in &mut node.children {
        clear_layout_dirty(child);
    }
}

fn count_invalidation(node: &ControlNode, stats: &mut UiInvalidationStats) {
    if node.layout.needs_layout {
        stats.layout_dirty += 1;
    }
    if node.layout.needs_paint {
        stats.paint_dirty += 1;
    }
    for child in &node.children {
        count_invalidation(child, stats);
    }
}

fn route_event_to_node(
    node: &mut ControlNode,
    target: &str,
    event: &UiEvent,
    theme: &Theme,
) -> EventResult {
    if !node.enabled || !node.effective_visibility().renders() {
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
    if !node.effective_visibility().renders() {
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

    #[test]
    fn panel_arranges_children_horizontally() {
        let mut tree = ControlTree::new();
        tree.add_panel("toolbar", StackDirection::Horizontal);
        assert!(tree.add_button_to("toolbar", "select", "Select"));
        assert!(tree.add_button_to("toolbar", "move", "Move"));
        assert!(tree.set_margin("move", Margin::new(6.0, 0.0, 0.0, 0.0)));
        tree.layout(Vec2::new(640.0, 360.0));

        let arranged = tree.arranged_controls();
        let select = arranged.iter().find(|item| item.name == "select").unwrap();
        let move_button = arranged.iter().find(|item| item.name == "move").unwrap();

        assert!(move_button.rect.x > select.rect.x + select.rect.width);
        assert_eq!(select.depth, 1);
        assert_eq!(move_button.widget_type, "Button");
    }

    #[test]
    fn hidden_controls_keep_layout_but_do_not_render_or_hit_test() {
        let mut tree = ControlTree::new();
        tree.add_button("hidden", "Hidden");
        tree.add_button("visible", "Visible");
        assert!(tree.set_visibility("hidden", Visibility::Hidden));
        tree.layout(Vec2::new(320.0, 180.0));

        let arranged = tree.arranged_controls();
        assert!(arranged.iter().any(|item| item.name == "hidden"));
        assert_eq!(tree.hit_test_path(4.0, 4.0).map(|hit| hit.name), None);

        let draw = tree.collect_draw_data();
        assert_eq!(draw.len(), 1);
    }

    #[test]
    fn collapsed_controls_do_not_take_layout_space() {
        let mut tree = ControlTree::new();
        tree.add_button("collapsed", "Collapsed");
        tree.add_button("next", "Next");
        assert!(tree.set_visibility("collapsed", Visibility::Collapsed));
        tree.layout(Vec2::new(320.0, 180.0));

        let arranged = tree.arranged_controls();
        assert!(!arranged.iter().any(|item| item.name == "collapsed"));
        let next = arranged.iter().find(|item| item.name == "next").unwrap();

        assert_eq!(next.rect.y, 0.0);
    }

    #[test]
    fn self_hit_test_invisible_panel_routes_to_child() {
        let mut tree = ControlTree::new();
        tree.add_panel("panel", StackDirection::Vertical);
        assert!(tree.add_button_to("panel", "child", "Child"));
        assert!(tree.set_visibility("panel", Visibility::SelfHitTestInvisible));
        tree.layout(Vec2::new(320.0, 180.0));

        let hit = tree.hit_test_path(4.0, 4.0).unwrap();

        assert_eq!(hit.name, "child");
        assert_eq!(hit.path, vec!["panel".to_string(), "child".to_string()]);
    }

    #[test]
    fn hit_test_invisible_panel_blocks_descendant_hits() {
        let mut tree = ControlTree::new();
        tree.add_panel("panel", StackDirection::Vertical);
        assert!(tree.add_button_to("panel", "child", "Child"));
        assert!(tree.set_visibility("panel", Visibility::HitTestInvisible));
        tree.layout(Vec2::new(320.0, 180.0));

        assert_eq!(tree.hit_test_path(4.0, 4.0), None);
    }

    #[test]
    fn fixed_slot_overrides_desired_size() {
        let mut tree = ControlTree::new();
        tree.add_button("wide", "Wide");
        assert!(tree.set_slot("wide", SlotLayout::new().fixed(160.0, 40.0)));
        tree.layout(Vec2::new(320.0, 180.0));

        let arranged = tree.arranged_controls();
        let wide = arranged.iter().find(|item| item.name == "wide").unwrap();

        assert_eq!(wide.rect.width, 160.0);
        assert_eq!(wide.rect.height, 40.0);
    }

    #[test]
    fn fill_slot_consumes_remaining_stack_space() {
        let mut tree = ControlTree::new();
        tree.add_panel("column", StackDirection::Vertical);
        assert!(tree.add_button_to("column", "header", "Header"));
        assert!(tree.add_button_to("column", "body", "Body"));
        assert!(tree.set_slot("column", SlotLayout::new().fixed(200.0, 120.0)));
        assert!(tree.set_slot("header", SlotLayout::new().fixed(200.0, 20.0)));
        assert!(tree.set_slot("body", SlotLayout::new().fill(1.0)));
        tree.layout(Vec2::new(320.0, 180.0));

        let arranged = tree.arranged_controls();
        let body = arranged.iter().find(|item| item.name == "body").unwrap();

        assert_eq!(body.rect.height, 100.0);
        assert_eq!(body.rect.width, 200.0);
    }

    #[test]
    fn slot_alignment_positions_child_inside_cross_axis() {
        let mut tree = ControlTree::new();
        tree.add_panel("row", StackDirection::Horizontal);
        assert!(tree.add_button_to("row", "small", "S"));
        assert!(tree.set_slot("row", SlotLayout::new().fixed(120.0, 80.0)));
        assert!(tree.set_slot(
            "small",
            SlotLayout {
                height: SizeRule::Fixed(20.0),
                vertical_alignment: Alignment::Center,
                ..SlotLayout::new()
            },
        ));
        tree.layout(Vec2::new(320.0, 180.0));

        let arranged = tree.arranged_controls();
        let small = arranged.iter().find(|item| item.name == "small").unwrap();

        assert_eq!(small.rect.y, 30.0);
        assert_eq!(small.rect.height, 20.0);
    }

    #[test]
    fn layout_clears_and_mutation_sets_invalidation() {
        let mut tree = ControlTree::new();
        tree.add_button("play", "Play");
        tree.layout(Vec2::new(320.0, 180.0));
        assert_eq!(tree.invalidation_stats(), UiInvalidationStats::default());

        assert!(tree.set_margin("play", Margin::all(4.0)));
        let stats = tree.invalidation_stats();

        assert!(stats.layout_dirty > 0);
        assert!(stats.paint_dirty > 0);
    }
}
