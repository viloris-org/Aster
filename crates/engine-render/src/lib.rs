#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Render abstraction only. Concrete backends live outside `runtime-min`.

use engine_core::{EngineError, EngineResult};

/// Render API selected by a concrete backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderApi {
    /// No rendering backend.
    Headless,
    /// Vulkan backend.
    Vulkan,
    /// Metal backend.
    Metal,
}

/// Render frame context passed to backends.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderFrame {
    /// Frame index.
    pub frame_index: u64,
}

/// Render backend abstraction.
pub trait RenderDevice {
    /// Returns the concrete API exposed by this device.
    fn api(&self) -> RenderApi;

    /// Renders one frame.
    fn render(&mut self, frame: RenderFrame) -> EngineResult<()>;
}

/// Null renderer used by minimal runtime builds.
#[derive(Clone, Debug, Default)]
pub struct HeadlessRenderDevice;

impl RenderDevice for HeadlessRenderDevice {
    fn api(&self) -> RenderApi {
        RenderApi::Headless
    }

    fn render(&mut self, _frame: RenderFrame) -> EngineResult<()> {
        Ok(())
    }
}

/// Placeholder for profiles that request a concrete backend before one is linked.
#[derive(Clone, Debug, Default)]
pub struct MissingRenderDevice;

impl RenderDevice for MissingRenderDevice {
    fn api(&self) -> RenderApi {
        RenderApi::Headless
    }

    fn render(&mut self, _frame: RenderFrame) -> EngineResult<()> {
        Err(EngineError::UnsupportedCapability {
            capability: "render-backend",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_renderer_accepts_frame() {
        let mut renderer = HeadlessRenderDevice;
        renderer.render(RenderFrame { frame_index: 0 }).unwrap();
        assert_eq!(renderer.api(), RenderApi::Headless);
    }
}
