//! Input map with named actions, bindings, dead zones, and chord detection.

use std::collections::{HashMap, HashSet};

use crate::input::{InputState, KeyCode, MouseButton};

/// Gamepad button identifiers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GamepadButton {
    /// A button (south face button).
    A,
    /// B button (east face button).
    B,
    /// X button (west face button).
    X,
    /// Y button (north face button).
    Y,
    /// Left bumper.
    LB,
    /// Right bumper.
    RB,
    /// Left trigger (analog).
    LT,
    /// Right trigger (analog).
    RT,
    /// Start button.
    Start,
    /// Select/Back button.
    Select,
    /// Left stick press.
    LeftStick,
    /// Right stick press.
    RightStick,
    /// D-pad up.
    DPadUp,
    /// D-pad down.
    DPadDown,
    /// D-pad left.
    DPadLeft,
    /// D-pad right.
    DPadRight,
}

/// Gamepad axis identifiers normalized across common controllers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GamepadAxis {
    /// Left stick horizontal axis.
    LeftStickX,
    /// Left stick vertical axis.
    LeftStickY,
    /// Right stick horizontal axis.
    RightStickX,
    /// Right stick vertical axis.
    RightStickY,
    /// Left trigger analog value.
    LeftTrigger,
    /// Right trigger analog value.
    RightTrigger,
}

/// Dead zone configuration for analog input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeadZone {
    /// Input values below this threshold are treated as zero.
    pub inner: f32,
    /// Input values above this threshold are treated as 1.0.
    pub outer: f32,
}

impl Default for DeadZone {
    fn default() -> Self {
        Self {
            inner: 0.2,
            outer: 0.95,
        }
    }
}

/// Axis type for an input action.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AxisType {
    /// Digital on/off (0.0 or 1.0).
    #[default]
    Digital,
    /// One-dimensional axis (-1.0 to 1.0).
    Axis1D,
    /// Two-dimensional axis.
    Axis2D,
}

/// How multiple bindings for the same logical action are accumulated.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AccumulationMode {
    /// Use the binding with the largest absolute value.
    #[default]
    HighestAbsolute,
    /// Add all binding values together and clamp to the action range.
    Cumulative,
}

/// Per-binding value modifier, inspired by Unreal Enhanced Input modifiers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputModifier {
    /// Inverts the value.
    Negate,
    /// Multiplies the value by a scalar.
    Scalar(f32),
    /// Multiplies the value by the frame delta time.
    ScaleByDeltaTime,
    /// Applies an inner/outer dead zone.
    DeadZone(DeadZone),
    /// Applies a signed exponential response curve.
    ResponseCurve {
        /// Signed exponent applied to the absolute value.
        exponent: f32,
    },
}

impl InputModifier {
    fn apply(self, value: f32, delta_time: f32) -> f32 {
        match self {
            Self::Negate => -value,
            Self::Scalar(scale) => value * scale,
            Self::ScaleByDeltaTime => value * delta_time,
            Self::DeadZone(dead_zone) => apply_deadzone(value, dead_zone),
            Self::ResponseCurve { exponent } => {
                let exponent = exponent.max(f32::EPSILON);
                value.signum() * value.abs().powf(exponent)
            }
        }
    }
}

/// Trigger condition for a binding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputTrigger {
    /// Binding is active while actuated.
    Down,
    /// Binding activates only on the frame it crosses the threshold.
    Pressed,
    /// Binding activates only on the frame it falls below the threshold.
    Released,
    /// Binding activates after being actuated for at least `seconds`.
    Hold {
        /// Required actuation duration.
        seconds: f32,
    },
}

/// Runtime state generated for a logical action.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActionState {
    /// Current action value.
    pub value: f32,
    /// Whether the action is currently actuated.
    pub down: bool,
    /// Whether the action began this frame.
    pub pressed: bool,
    /// Whether the action ended this frame.
    pub released: bool,
    /// Whether at least one trigger condition was satisfied this frame.
    pub triggered: bool,
    held_seconds: f32,
}

impl ActionState {
    /// Returns an inactive action state.
    pub fn inactive() -> Self {
        Self::default()
    }
}

/// A single input binding for an action.
#[derive(Clone, Debug, PartialEq)]
pub struct InputBinding {
    /// Keys that produce a positive action value.
    pub positive_keys: Vec<KeyCode>,
    /// Keys that produce a negative action value.
    pub negative_keys: Vec<KeyCode>,
    /// Mouse buttons for positive action.
    pub positive_mouse: Vec<MouseButton>,
    /// Gamepad buttons for positive action.
    pub positive_gamepad: Vec<GamepadButton>,
    /// Gamepad buttons for negative action.
    pub negative_gamepad: Vec<GamepadButton>,
    /// Gamepad axes for positive action values.
    pub positive_gamepad_axes: Vec<GamepadAxis>,
    /// Gamepad axes inverted into negative action values.
    pub negative_gamepad_axes: Vec<GamepadAxis>,
    /// Dead zone for this binding.
    pub dead_zone: Option<DeadZone>,
    /// Number of frames to buffer this input for.
    pub buffer_frames: u32,
    /// Keys that must all be held simultaneously for this binding to activate.
    pub chord_keys: Vec<KeyCode>,
    /// Axis type.
    pub axis_type: AxisType,
    /// Value modifiers applied in declaration order.
    pub modifiers: Vec<InputModifier>,
    /// Trigger rules applied after modifiers.
    pub triggers: Vec<InputTrigger>,
    /// If true, lower-priority contexts cannot contribute this action.
    pub consume_lower_priority: bool,
}

impl Default for InputBinding {
    fn default() -> Self {
        Self {
            positive_keys: Vec::new(),
            negative_keys: Vec::new(),
            positive_mouse: Vec::new(),
            positive_gamepad: Vec::new(),
            negative_gamepad: Vec::new(),
            positive_gamepad_axes: Vec::new(),
            negative_gamepad_axes: Vec::new(),
            dead_zone: None,
            buffer_frames: 0,
            chord_keys: Vec::new(),
            axis_type: AxisType::Digital,
            modifiers: Vec::new(),
            triggers: Vec::new(),
            consume_lower_priority: false,
        }
    }
}

impl InputBinding {
    /// Creates a digital action from keys.
    pub fn digital(keys: impl IntoIterator<Item = KeyCode>) -> Self {
        Self {
            positive_keys: keys.into_iter().collect(),
            ..Default::default()
        }
    }

    /// Creates a 1D axis binding.
    pub fn axis(
        negative: impl IntoIterator<Item = KeyCode>,
        positive: impl IntoIterator<Item = KeyCode>,
    ) -> Self {
        Self {
            positive_keys: positive.into_iter().collect(),
            negative_keys: negative.into_iter().collect(),
            axis_type: AxisType::Axis1D,
            ..Default::default()
        }
    }
}

/// Data-driven input map that binds actions to physical inputs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputMap {
    /// Map name.
    pub name: String,
    /// Named action bindings.
    pub actions: HashMap<String, InputBinding>,
    /// Additional mappings. Use this when multiple physical bindings drive one action.
    pub mappings: Vec<(String, InputBinding)>,
    /// Context priority. Larger values override lower-priority contexts.
    pub priority: i32,
    /// Accumulation behavior per action.
    pub accumulation: HashMap<String, AccumulationMode>,
}

impl InputMap {
    /// Evaluates all actions against the current input state.
    pub fn evaluate(&self, input: &InputState) -> HashMap<String, f32> {
        let mut values = HashMap::<String, f32>::new();
        for (name, binding) in self.iter_mappings() {
            let value = self.evaluate_binding(binding, input, 1.0 / 60.0);
            if value.abs() > f32::EPSILON {
                accumulate_value(&mut values, name, value, self.accumulation_mode(name));
            }
        }
        values.retain(|_, value| value.abs() > f32::EPSILON);
        values
    }

    /// Evaluates this map into full per-action state.
    pub fn evaluate_actions(
        &self,
        input: &InputState,
        previous: &HashMap<String, ActionState>,
        delta_time: f32,
    ) -> HashMap<String, ActionState> {
        let mut values = HashMap::new();
        let mut triggered = HashMap::new();
        for (name, binding) in self.iter_mappings() {
            let value = self.evaluate_binding(binding, input, delta_time);
            let previous_state = previous.get(name).copied().unwrap_or_default();
            let trigger_satisfied = triggers_satisfied(binding, value, previous_state, delta_time);
            if value.abs() > f32::EPSILON {
                accumulate_value(&mut values, name, value, self.accumulation_mode(name));
            }
            if trigger_satisfied {
                triggered.insert(name.clone(), true);
            }
        }
        action_states_from_values(values, previous, delta_time, &triggered)
    }

    fn accumulation_mode(&self, action: &str) -> AccumulationMode {
        self.accumulation.get(action).copied().unwrap_or_default()
    }

    fn iter_mappings(&self) -> impl Iterator<Item = (&String, &InputBinding)> {
        self.actions
            .iter()
            .chain(self.mappings.iter().map(|(name, binding)| (name, binding)))
    }

    fn evaluate_binding(&self, binding: &InputBinding, input: &InputState, delta_time: f32) -> f32 {
        let deadzone = binding.dead_zone.unwrap_or_default();

        let key_value = Self::key_axis_value(binding, input);
        let mouse_value = Self::mouse_value(binding, input);
        let gamepad_value = Self::gamepad_value(binding, input, deadzone);

        let raw = if key_value.abs() > mouse_value.abs() && key_value.abs() > gamepad_value.abs() {
            key_value
        } else if mouse_value.abs() > gamepad_value.abs() {
            mouse_value
        } else {
            gamepad_value
        };

        if !binding.chord_keys.is_empty() && !binding.chord_keys.iter().all(|k| input.key_down(*k))
        {
            return 0.0;
        }

        match binding.axis_type {
            AxisType::Digital => {
                if raw.abs() > deadzone.inner {
                    apply_modifiers(raw.signum(), &binding.modifiers, delta_time)
                } else {
                    0.0
                }
            }
            AxisType::Axis1D => apply_modifiers(
                apply_deadzone(raw, deadzone),
                &binding.modifiers,
                delta_time,
            ),
            AxisType::Axis2D => apply_modifiers(raw, &binding.modifiers, delta_time),
        }
    }

    fn key_axis_value(binding: &InputBinding, input: &InputState) -> f32 {
        let positive = binding.positive_keys.iter().any(|k| input.key_down(*k));
        let negative = binding.negative_keys.iter().any(|k| input.key_down(*k));
        match (negative, positive) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        }
    }

    fn mouse_value(binding: &InputBinding, input: &InputState) -> f32 {
        if binding
            .positive_mouse
            .iter()
            .any(|b| input.mouse_button_down(*b))
        {
            1.0
        } else {
            0.0
        }
    }

    fn gamepad_value(binding: &InputBinding, input: &InputState, _deadzone: DeadZone) -> f32 {
        let axis_value = input
            .gamepad_states()
            .iter()
            .flat_map(|gamepad| {
                binding
                    .positive_gamepad_axes
                    .iter()
                    .map(|axis| gamepad_axis_value(gamepad, *axis))
                    .chain(
                        binding
                            .negative_gamepad_axes
                            .iter()
                            .map(|axis| -gamepad_axis_value(gamepad, *axis)),
                    )
            })
            .max_by(|a, b| a.abs().total_cmp(&b.abs()))
            .unwrap_or(0.0);

        if axis_value.abs() > f32::EPSILON {
            return axis_value;
        }

        if binding
            .positive_gamepad
            .iter()
            .any(|b| input.gamepad_button_down(*b))
        {
            1.0
        } else if binding
            .negative_gamepad
            .iter()
            .any(|b| input.gamepad_button_down(*b))
        {
            -1.0
        } else {
            0.0
        }
    }
}

/// A stack of active input mapping contexts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputMapStack {
    contexts: Vec<InputMap>,
    previous: HashMap<String, ActionState>,
}

impl InputMapStack {
    /// Adds or replaces a mapping context by name.
    pub fn add_context(&mut self, map: InputMap) {
        if let Some(existing) = self
            .contexts
            .iter_mut()
            .find(|existing| existing.name == map.name)
        {
            *existing = map;
        } else {
            self.contexts.push(map);
        }
        self.contexts.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.name.cmp(&b.name))
        });
    }

    /// Removes a mapping context by name.
    pub fn remove_context(&mut self, name: &str) -> Option<InputMap> {
        let index = self
            .contexts
            .iter()
            .position(|context| context.name == name)?;
        Some(self.contexts.remove(index))
    }

    /// Clears all contexts and previous action state.
    pub fn clear(&mut self) {
        self.contexts.clear();
        self.previous.clear();
    }

    /// Returns active contexts in evaluation order.
    pub fn contexts(&self) -> &[InputMap] {
        &self.contexts
    }

    /// Evaluates all contexts and updates previous action state.
    pub fn evaluate(
        &mut self,
        input: &InputState,
        delta_time: f32,
    ) -> HashMap<String, ActionState> {
        let mut values = HashMap::new();
        let mut consumed = HashSet::new();

        for context in &self.contexts {
            for (name, binding) in context.iter_mappings() {
                if consumed.contains(name) {
                    continue;
                }
                let value = context.evaluate_binding(binding, input, delta_time);
                if value.abs() <= f32::EPSILON {
                    continue;
                }
                accumulate_value(&mut values, name, value, context.accumulation_mode(name));
                if binding.consume_lower_priority {
                    consumed.insert(name.clone());
                }
            }
        }
        values.retain(|_, value| value.abs() > f32::EPSILON);

        let mut triggered = HashMap::new();
        for context in &self.contexts {
            for (name, binding) in context.iter_mappings() {
                let value = context.evaluate_binding(binding, input, delta_time);
                let previous_state = self.previous.get(name).copied().unwrap_or_default();
                if triggers_satisfied(binding, value, previous_state, delta_time) {
                    triggered.insert(name.clone(), true);
                }
            }
        }

        let states = action_states_from_values(values, &self.previous, delta_time, &triggered);
        self.previous = states.clone();
        states
    }
}

fn gamepad_axis_value(gamepad: &crate::gamepad::GamepadState, axis: GamepadAxis) -> f32 {
    match axis {
        GamepadAxis::LeftStickX => gamepad.left_stick_x,
        GamepadAxis::LeftStickY => gamepad.left_stick_y,
        GamepadAxis::RightStickX => gamepad.right_stick_x,
        GamepadAxis::RightStickY => gamepad.right_stick_y,
        GamepadAxis::LeftTrigger => gamepad.left_trigger,
        GamepadAxis::RightTrigger => gamepad.right_trigger,
    }
}

fn apply_deadzone(value: f32, deadzone: DeadZone) -> f32 {
    let abs = value.abs();
    if abs <= deadzone.inner {
        return 0.0;
    }
    if abs >= deadzone.outer {
        return value.signum();
    }
    let scaled = (abs - deadzone.inner) / (deadzone.outer - deadzone.inner);
    value.signum() * scaled
}

fn apply_modifiers(value: f32, modifiers: &[InputModifier], delta_time: f32) -> f32 {
    modifiers
        .iter()
        .copied()
        .fold(value, |value, modifier| modifier.apply(value, delta_time))
        .clamp(-1.0, 1.0)
}

fn accumulate_value(
    values: &mut HashMap<String, f32>,
    name: &str,
    value: f32,
    mode: AccumulationMode,
) {
    values
        .entry(name.to_string())
        .and_modify(|current| {
            *current = match mode {
                AccumulationMode::HighestAbsolute => {
                    if value.abs() > current.abs() {
                        value
                    } else {
                        *current
                    }
                }
                AccumulationMode::Cumulative => (*current + value).clamp(-1.0, 1.0),
            };
        })
        .or_insert(value.clamp(-1.0, 1.0));
}

fn action_states_from_values(
    values: HashMap<String, f32>,
    previous: &HashMap<String, ActionState>,
    delta_time: f32,
    triggered_overrides: &HashMap<String, bool>,
) -> HashMap<String, ActionState> {
    let mut states = HashMap::new();
    for (name, value) in values {
        let was_down = previous.get(&name).is_some_and(|state| state.down);
        let down = value.abs() > f32::EPSILON;
        let held_seconds = if down {
            previous
                .get(&name)
                .map(|state| state.held_seconds)
                .unwrap_or_default()
                + delta_time.max(0.0)
        } else {
            0.0
        };
        let pressed = down && !was_down;
        let released = !down && was_down;
        let triggered = triggered_overrides
            .get(&name)
            .copied()
            .unwrap_or(pressed || down);
        states.insert(
            name,
            ActionState {
                value,
                down,
                pressed,
                released,
                triggered,
                held_seconds,
            },
        );
    }

    for (name, previous_state) in previous {
        if previous_state.down && !states.contains_key(name) {
            states.insert(
                name.clone(),
                ActionState {
                    released: true,
                    triggered: triggered_overrides.get(name).copied().unwrap_or(false),
                    ..ActionState::inactive()
                },
            );
        }
    }

    states
}

fn triggers_satisfied(
    binding: &InputBinding,
    value: f32,
    previous: ActionState,
    delta_time: f32,
) -> bool {
    let down = value.abs() > f32::EPSILON;
    if binding.triggers.is_empty() {
        return down && !previous.down || down;
    }

    let held_seconds = if down {
        previous.held_seconds + delta_time.max(0.0)
    } else {
        0.0
    };

    binding.triggers.iter().any(|trigger| match *trigger {
        InputTrigger::Down => down,
        InputTrigger::Pressed => down && !previous.down,
        InputTrigger::Released => !down && previous.down,
        InputTrigger::Hold { seconds } => down && held_seconds >= seconds.max(0.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::InputState;

    #[test]
    fn deadzone_filters_inner_range() {
        let dz = DeadZone::default();
        assert_eq!(apply_deadzone(0.1, dz), 0.0);
        assert_eq!(apply_deadzone(-0.1, dz), 0.0);
    }

    #[test]
    fn deadzone_passes_outer_range() {
        let dz = DeadZone::default();
        assert_eq!(apply_deadzone(1.0, dz), 1.0);
        assert_eq!(apply_deadzone(-1.0, dz), -1.0);
    }

    #[test]
    fn chord_detection_requires_all_keys() {
        let mut input = InputState::default();
        input.apply_event(crate::input::InputEvent::KeyDown(KeyCode::Character('a')));

        let mut map = InputMap::default();
        map.actions.insert(
            "CtrlA".to_string(),
            InputBinding {
                positive_keys: vec![KeyCode::Character('a')],
                chord_keys: vec![KeyCode::Character('z')],
                ..Default::default()
            },
        );

        let values = map.evaluate(&input);
        assert!(
            !values.contains_key("CtrlA"),
            "chord should not fire without all keys"
        );

        input.apply_event(crate::input::InputEvent::KeyDown(KeyCode::Character('z')));
        let values = map.evaluate(&input);
        assert!(
            values.contains_key("CtrlA"),
            "chord should fire with all keys"
        );
    }

    #[test]
    fn gamepad_button_maps_to_action() {
        use crate::gamepad::GamepadState;

        let mut gamepad = GamepadState::default();
        gamepad.press_button(GamepadButton::A);

        let mut input = InputState::default();
        input.apply_gamepad_state(gamepad);

        let mut map = InputMap::default();
        map.actions.insert(
            "Jump".to_string(),
            InputBinding {
                positive_gamepad: vec![GamepadButton::A],
                ..Default::default()
            },
        );

        let values = map.evaluate(&input);
        assert!(values.contains_key("Jump"));
    }

    #[test]
    fn gamepad_axis_maps_to_action_with_deadzone() {
        use crate::gamepad::GamepadState;

        let mut gamepad = GamepadState::connected(0, "Test Controller");
        gamepad.left_stick_x = 0.5;

        let mut input = InputState::default();
        input.apply_gamepad_state(gamepad);

        let mut map = InputMap::default();
        map.actions.insert(
            "MoveX".to_string(),
            InputBinding {
                positive_gamepad_axes: vec![GamepadAxis::LeftStickX],
                axis_type: AxisType::Axis1D,
                ..Default::default()
            },
        );

        let values = map.evaluate(&input);
        assert!(values.get("MoveX").is_some_and(|value| *value > 0.3));
    }

    #[test]
    fn axis_binding_returns_negative_and_positive() {
        let mut input = InputState::default();
        input.apply_event(crate::input::InputEvent::KeyDown(KeyCode::Character('a')));

        let mut map = InputMap::default();
        map.actions.insert(
            "MoveX".to_string(),
            InputBinding::axis([KeyCode::Character('a')], [KeyCode::Character('d')]),
        );

        let values = map.evaluate(&input);
        assert_eq!(values.get("MoveX"), Some(&-1.0));
    }

    #[test]
    fn cumulative_action_values_cancel_each_other() {
        let mut input = InputState::default();
        input.apply_event(crate::input::InputEvent::KeyDown(KeyCode::Character('a')));
        input.apply_event(crate::input::InputEvent::KeyDown(KeyCode::Character('d')));

        let mut map = InputMap::default();
        map.accumulation
            .insert("MoveX".to_string(), AccumulationMode::Cumulative);
        map.mappings.push((
            "MoveX".to_string(),
            InputBinding {
                negative_keys: vec![KeyCode::Character('a')],
                axis_type: AxisType::Axis1D,
                ..Default::default()
            },
        ));
        map.mappings.push((
            "MoveX".to_string(),
            InputBinding {
                positive_keys: vec![KeyCode::Character('d')],
                axis_type: AxisType::Axis1D,
                ..Default::default()
            },
        ));

        let values = map.evaluate(&input);
        assert!(!values.contains_key("MoveX"));
    }

    #[test]
    fn modifiers_apply_in_order() {
        let mut input = InputState::default();
        input.apply_event(crate::input::InputEvent::KeyDown(KeyCode::Character('w')));

        let mut map = InputMap::default();
        map.actions.insert(
            "Throttle".to_string(),
            InputBinding {
                positive_keys: vec![KeyCode::Character('w')],
                modifiers: vec![InputModifier::Scalar(0.5), InputModifier::Negate],
                axis_type: AxisType::Axis1D,
                ..Default::default()
            },
        );

        let values = map.evaluate(&input);
        assert_eq!(values.get("Throttle"), Some(&-0.5));
    }

    #[test]
    fn mapping_stack_consumes_lower_priority_contexts() {
        let mut input = InputState::default();
        input.apply_event(crate::input::InputEvent::KeyDown(KeyCode::Character('e')));
        input.apply_event(crate::input::InputEvent::KeyDown(KeyCode::Character('f')));

        let mut gameplay = InputMap {
            name: "Gameplay".to_string(),
            priority: 0,
            ..Default::default()
        };
        gameplay.actions.insert(
            "Use".to_string(),
            InputBinding {
                positive_keys: vec![KeyCode::Character('f')],
                ..Default::default()
            },
        );

        let mut ui = InputMap {
            name: "Ui".to_string(),
            priority: 10,
            ..Default::default()
        };
        ui.actions.insert(
            "Use".to_string(),
            InputBinding {
                positive_keys: vec![KeyCode::Character('e')],
                consume_lower_priority: true,
                ..Default::default()
            },
        );

        let mut stack = InputMapStack::default();
        stack.add_context(gameplay);
        stack.add_context(ui);

        let states = stack.evaluate(&input, 1.0 / 60.0);
        assert_eq!(states.get("Use").map(|state| state.value), Some(1.0));
    }

    #[test]
    fn triggers_report_pressed_hold_and_released() {
        let mut input = InputState::default();
        let mut map = InputMap::default();
        map.actions.insert(
            "Charge".to_string(),
            InputBinding {
                positive_keys: vec![KeyCode::Space],
                triggers: vec![InputTrigger::Pressed, InputTrigger::Hold { seconds: 0.2 }],
                ..Default::default()
            },
        );

        let mut previous = HashMap::new();
        input.apply_event(crate::input::InputEvent::KeyDown(KeyCode::Space));
        let first = map.evaluate_actions(&input, &previous, 0.1);
        assert!(first.get("Charge").is_some_and(|state| state.pressed));
        assert!(first.get("Charge").is_some_and(|state| state.triggered));

        previous = first;
        input.end_frame();
        let held = map.evaluate_actions(&input, &previous, 0.11);
        assert!(held.get("Charge").is_some_and(|state| state.triggered));

        let mut release_map = InputMap::default();
        release_map.actions.insert(
            "Charge".to_string(),
            InputBinding {
                positive_keys: vec![KeyCode::Space],
                triggers: vec![InputTrigger::Released],
                ..Default::default()
            },
        );
        input.apply_event(crate::input::InputEvent::KeyUp(KeyCode::Space));
        let released = release_map.evaluate_actions(&input, &held, 0.1);
        assert!(
            released
                .get("Charge")
                .is_some_and(|state| state.released && state.triggered)
        );
    }
}
