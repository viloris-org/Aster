//! Window abstraction.

use engine_core::EngineResult;

/// Window creation descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowDescriptor {
    /// Window title.
    pub title: String,
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
    pub height: u32,
}

impl Default for WindowDescriptor {
    fn default() -> Self {
        Self {
            title: "Aster".to_owned(),
            width: 1280,
            height: 720,
        }
    }
}

/// Window creation boundary implemented by concrete platform backends later.
pub trait WindowProvider {
    /// Window handle type.
    type Window;

    /// Creates a window.
    fn create_window(&self, descriptor: &WindowDescriptor) -> EngineResult<Self::Window>;
}
