//! Input abstraction.

/// Keyboard key codes used by the platform abstraction.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum KeyCode {
    /// Escape key.
    Escape,
    /// Enter key.
    Enter,
    /// Space key.
    Space,
    /// Character key.
    Character(char),
}

/// Input event emitted by a platform backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputEvent {
    /// Key pressed.
    KeyDown(KeyCode),
    /// Key released.
    KeyUp(KeyCode),
    /// Mouse moved in logical pixels.
    MouseMove {
        /// X position.
        x: i32,
        /// Y position.
        y: i32,
    },
}
