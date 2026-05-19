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

/// winit-backed window provider. Only available with the `editor` feature.
#[cfg(feature = "editor")]
pub struct WinitWindowProvider;

#[cfg(feature = "editor")]
impl WindowProvider for WinitWindowProvider {
    type Window = winit::window::Window;

    fn create_window(&self, descriptor: &WindowDescriptor) -> EngineResult<Self::Window> {
        use engine_core::EngineError;
        use winit::{dpi::LogicalSize, event_loop::EventLoop, window::WindowAttributes};

        let event_loop = EventLoop::new().map_err(|e| EngineError::other(e.to_string()))?;
        let attrs = WindowAttributes::default()
            .with_title(&descriptor.title)
            .with_inner_size(LogicalSize::new(descriptor.width, descriptor.height));

        #[allow(deprecated)]
        let window = event_loop
            .create_window(attrs)
            .map_err(|e| EngineError::other(e.to_string()))?;
        Ok(window)
    }
}
