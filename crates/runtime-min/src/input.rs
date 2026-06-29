use super::*;

pub(crate) fn parse_input_binding_table(
    binding: &toml::map::Map<String, toml::Value>,
) -> InputBindingV2 {
    let axis_type = binding
        .get("axis")
        .and_then(toml::Value::as_str)
        .and_then(parse_axis_type)
        .unwrap_or(AxisType::Digital);

    let mut parsed = InputBindingV2 {
        axis_type,
        dead_zone: binding
            .get("deadzone")
            .and_then(toml::Value::as_float)
            .map(|inner| DeadZone {
                inner: inner as f32,
                outer: 1.0,
            }),
        buffer_frames: binding
            .get("buffer_frames")
            .and_then(toml::Value::as_integer)
            .unwrap_or_default()
            .max(0) as u32,
        consume_lower_priority: binding
            .get("consume")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        ..Default::default()
    };

    parsed.positive_keys = parse_key_array(binding.get("keys"));
    parsed.positive_mouse = parse_mouse_button_array(binding.get("mouse"));
    parsed.positive_gamepad = parse_gamepad_button_array(binding.get("gamepad"));
    parsed.positive_gamepad_axes = parse_gamepad_axis_array(binding.get("gamepad_axes"));
    parsed.negative_keys = parse_key_array(binding.get("negative_keys"));
    parsed.negative_gamepad = parse_gamepad_button_array(binding.get("negative_gamepad"));
    parsed.negative_gamepad_axes = parse_gamepad_axis_array(binding.get("negative_gamepad_axes"));
    parsed.chord_keys = parse_key_array(binding.get("chords"));
    parsed.modifiers = parse_modifier_array(binding.get("modifiers"));
    parsed.triggers = parse_trigger_array(binding.get("triggers"));
    parsed
}

fn parse_string_array(value: Option<&toml::Value>) -> Vec<&str> {
    value
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .collect()
}

fn parse_key_array(value: Option<&toml::Value>) -> Vec<KeyCode> {
    parse_string_array(value)
        .into_iter()
        .filter_map(ActionMap::parse_key_name)
        .collect()
}

fn parse_mouse_button_array(value: Option<&toml::Value>) -> Vec<MouseButton> {
    parse_string_array(value)
        .into_iter()
        .filter_map(parse_mouse_button)
        .collect()
}

fn parse_gamepad_button_array(value: Option<&toml::Value>) -> Vec<GamepadButton> {
    parse_string_array(value)
        .into_iter()
        .filter_map(parse_gamepad_button)
        .collect()
}

fn parse_gamepad_axis_array(value: Option<&toml::Value>) -> Vec<GamepadAxis> {
    parse_string_array(value)
        .into_iter()
        .filter_map(parse_gamepad_axis)
        .collect()
}

fn parse_modifier_array(value: Option<&toml::Value>) -> Vec<InputModifier> {
    parse_string_array(value)
        .into_iter()
        .filter_map(parse_input_modifier)
        .collect()
}

fn parse_trigger_array(value: Option<&toml::Value>) -> Vec<InputTrigger> {
    parse_string_array(value)
        .into_iter()
        .filter_map(parse_input_trigger)
        .collect()
}

fn parse_axis_type(value: &str) -> Option<AxisType> {
    match value {
        "digital" | "Digital" => Some(AxisType::Digital),
        "axis1d" | "Axis1D" | "1d" => Some(AxisType::Axis1D),
        "axis2d" | "Axis2D" | "2d" => Some(AxisType::Axis2D),
        _ => None,
    }
}

fn parse_mouse_button(value: &str) -> Option<MouseButton> {
    match value {
        "Left" | "left" => Some(MouseButton::Left),
        "Right" | "right" => Some(MouseButton::Right),
        "Middle" | "middle" => Some(MouseButton::Middle),
        _ => value
            .strip_prefix("Other")
            .and_then(|number| number.parse::<u16>().ok())
            .map(MouseButton::Other),
    }
}

fn parse_gamepad_button(value: &str) -> Option<GamepadButton> {
    match value {
        "A" => Some(GamepadButton::A),
        "B" => Some(GamepadButton::B),
        "X" => Some(GamepadButton::X),
        "Y" => Some(GamepadButton::Y),
        "LB" => Some(GamepadButton::LB),
        "RB" => Some(GamepadButton::RB),
        "LT" => Some(GamepadButton::LT),
        "RT" => Some(GamepadButton::RT),
        "Start" => Some(GamepadButton::Start),
        "Select" | "Back" => Some(GamepadButton::Select),
        "LeftStick" => Some(GamepadButton::LeftStick),
        "RightStick" => Some(GamepadButton::RightStick),
        "DPadUp" => Some(GamepadButton::DPadUp),
        "DPadDown" => Some(GamepadButton::DPadDown),
        "DPadLeft" => Some(GamepadButton::DPadLeft),
        "DPadRight" => Some(GamepadButton::DPadRight),
        _ => None,
    }
}

fn parse_gamepad_axis(value: &str) -> Option<GamepadAxis> {
    match value {
        "LeftStickX" => Some(GamepadAxis::LeftStickX),
        "LeftStickY" => Some(GamepadAxis::LeftStickY),
        "RightStickX" => Some(GamepadAxis::RightStickX),
        "RightStickY" => Some(GamepadAxis::RightStickY),
        "LeftTrigger" => Some(GamepadAxis::LeftTrigger),
        "RightTrigger" => Some(GamepadAxis::RightTrigger),
        _ => None,
    }
}

fn parse_input_modifier(value: &str) -> Option<InputModifier> {
    match value {
        "Negate" | "negate" => Some(InputModifier::Negate),
        "ScaleByDeltaTime" | "scale_by_delta_time" | "delta_time" => {
            Some(InputModifier::ScaleByDeltaTime)
        }
        _ => value
            .strip_prefix("Scalar:")
            .or_else(|| value.strip_prefix("scalar:"))
            .and_then(|number| number.parse::<f32>().ok())
            .map(InputModifier::Scalar)
            .or_else(|| {
                value
                    .strip_prefix("ResponseCurve:")
                    .or_else(|| value.strip_prefix("response_curve:"))
                    .and_then(|number| number.parse::<f32>().ok())
                    .map(|exponent| InputModifier::ResponseCurve { exponent })
            }),
    }
}

fn parse_input_trigger(value: &str) -> Option<InputTrigger> {
    match value {
        "Down" | "down" => Some(InputTrigger::Down),
        "Pressed" | "pressed" => Some(InputTrigger::Pressed),
        "Released" | "released" => Some(InputTrigger::Released),
        _ => value
            .strip_prefix("Hold:")
            .or_else(|| value.strip_prefix("hold:"))
            .and_then(|number| number.parse::<f32>().ok())
            .map(|seconds| InputTrigger::Hold { seconds }),
    }
}

/// Applies runtime input capture state to a winit window.
#[cfg(feature = "runtime-game")]
pub fn apply_winit_input_capture(
    window: &winit::window::Window,
    capture: RuntimeInputCapture,
) -> Result<(), String> {
    use winit::window::CursorGrabMode;

    if capture.mouse {
        window.focus_window();
        window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
            .map_err(|error| format!("cursor grab: {error}"))?;
        window.set_cursor_visible(false);
    } else {
        window
            .set_cursor_grab(CursorGrabMode::None)
            .map_err(|error| format!("cursor release: {error}"))?;
        window.set_cursor_visible(true);
    }
    Ok(())
}

/// Converts a winit physical key to an engine KeyCode.
#[cfg(feature = "runtime-game")]
fn convert_winit_key_static(key: winit::keyboard::PhysicalKey) -> Option<engine_platform::KeyCode> {
    use engine_platform::KeyCode;
    use winit::keyboard::{KeyCode as WinitKeyCode, PhysicalKey};

    match key {
        PhysicalKey::Code(WinitKeyCode::Escape) => Some(KeyCode::Escape),
        PhysicalKey::Code(WinitKeyCode::Enter) => Some(KeyCode::Enter),
        PhysicalKey::Code(WinitKeyCode::Backspace) => Some(KeyCode::Backspace),
        PhysicalKey::Code(WinitKeyCode::Space) => Some(KeyCode::Space),
        PhysicalKey::Code(WinitKeyCode::ArrowUp) => Some(KeyCode::ArrowUp),
        PhysicalKey::Code(WinitKeyCode::ArrowDown) => Some(KeyCode::ArrowDown),
        PhysicalKey::Code(WinitKeyCode::ArrowLeft) => Some(KeyCode::ArrowLeft),
        PhysicalKey::Code(WinitKeyCode::ArrowRight) => Some(KeyCode::ArrowRight),
        PhysicalKey::Code(WinitKeyCode::KeyA) => Some(KeyCode::Character('a')),
        PhysicalKey::Code(WinitKeyCode::KeyB) => Some(KeyCode::Character('b')),
        PhysicalKey::Code(WinitKeyCode::KeyC) => Some(KeyCode::Character('c')),
        PhysicalKey::Code(WinitKeyCode::KeyD) => Some(KeyCode::Character('d')),
        PhysicalKey::Code(WinitKeyCode::KeyE) => Some(KeyCode::Character('e')),
        PhysicalKey::Code(WinitKeyCode::KeyF) => Some(KeyCode::Character('f')),
        PhysicalKey::Code(WinitKeyCode::KeyG) => Some(KeyCode::Character('g')),
        PhysicalKey::Code(WinitKeyCode::KeyH) => Some(KeyCode::Character('h')),
        PhysicalKey::Code(WinitKeyCode::KeyI) => Some(KeyCode::Character('i')),
        PhysicalKey::Code(WinitKeyCode::KeyJ) => Some(KeyCode::Character('j')),
        PhysicalKey::Code(WinitKeyCode::KeyK) => Some(KeyCode::Character('k')),
        PhysicalKey::Code(WinitKeyCode::KeyL) => Some(KeyCode::Character('l')),
        PhysicalKey::Code(WinitKeyCode::KeyM) => Some(KeyCode::Character('m')),
        PhysicalKey::Code(WinitKeyCode::KeyN) => Some(KeyCode::Character('n')),
        PhysicalKey::Code(WinitKeyCode::KeyO) => Some(KeyCode::Character('o')),
        PhysicalKey::Code(WinitKeyCode::KeyP) => Some(KeyCode::Character('p')),
        PhysicalKey::Code(WinitKeyCode::KeyQ) => Some(KeyCode::Character('q')),
        PhysicalKey::Code(WinitKeyCode::KeyR) => Some(KeyCode::Character('r')),
        PhysicalKey::Code(WinitKeyCode::KeyS) => Some(KeyCode::Character('s')),
        PhysicalKey::Code(WinitKeyCode::KeyT) => Some(KeyCode::Character('t')),
        PhysicalKey::Code(WinitKeyCode::KeyU) => Some(KeyCode::Character('u')),
        PhysicalKey::Code(WinitKeyCode::KeyV) => Some(KeyCode::Character('v')),
        PhysicalKey::Code(WinitKeyCode::KeyW) => Some(KeyCode::Character('w')),
        PhysicalKey::Code(WinitKeyCode::KeyX) => Some(KeyCode::Character('x')),
        PhysicalKey::Code(WinitKeyCode::KeyY) => Some(KeyCode::Character('y')),
        PhysicalKey::Code(WinitKeyCode::KeyZ) => Some(KeyCode::Character('z')),
        PhysicalKey::Code(WinitKeyCode::Digit0) => Some(KeyCode::Character('0')),
        PhysicalKey::Code(WinitKeyCode::Digit1) => Some(KeyCode::Character('1')),
        PhysicalKey::Code(WinitKeyCode::Digit2) => Some(KeyCode::Character('2')),
        PhysicalKey::Code(WinitKeyCode::Digit3) => Some(KeyCode::Character('3')),
        PhysicalKey::Code(WinitKeyCode::Digit4) => Some(KeyCode::Character('4')),
        PhysicalKey::Code(WinitKeyCode::Digit5) => Some(KeyCode::Character('5')),
        PhysicalKey::Code(WinitKeyCode::Digit6) => Some(KeyCode::Character('6')),
        PhysicalKey::Code(WinitKeyCode::Digit7) => Some(KeyCode::Character('7')),
        PhysicalKey::Code(WinitKeyCode::Digit8) => Some(KeyCode::Character('8')),
        PhysicalKey::Code(WinitKeyCode::Digit9) => Some(KeyCode::Character('9')),
        PhysicalKey::Code(WinitKeyCode::Minus) => Some(KeyCode::Character('-')),
        PhysicalKey::Code(WinitKeyCode::Period) => Some(KeyCode::Character('.')),
        _ => None,
    }
}

/// Converts a winit mouse button to an engine MouseButton.
#[cfg(feature = "runtime-game")]
fn convert_winit_mouse_button_static(
    button: winit::event::MouseButton,
) -> Option<engine_platform::MouseButton> {
    use engine_platform::MouseButton;
    match button {
        winit::event::MouseButton::Left => Some(MouseButton::Left),
        winit::event::MouseButton::Right => Some(MouseButton::Right),
        winit::event::MouseButton::Middle => Some(MouseButton::Middle),
        winit::event::MouseButton::Other(id) => Some(MouseButton::Other(id)),
        _ => None,
    }
}
