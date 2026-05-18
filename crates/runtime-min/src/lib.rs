#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Minimal Aster runtime without editor, Python, importers, physics, audio, or concrete rendering.

use engine_core::{logging, EngineConfig, EngineResult, FrameCounter};
use engine_ecs::Scene;
use engine_render::{HeadlessRenderDevice, RenderDevice, RenderFrame};

/// Explicit runtime services. There is no hidden global mutable state.
#[derive(Debug)]
pub struct RuntimeServices<R = HeadlessRenderDevice> {
    /// Runtime configuration.
    pub config: EngineConfig,
    /// Scene storage.
    pub scene: Scene,
    /// Render abstraction.
    pub renderer: R,
    frame_counter: FrameCounter,
}

impl RuntimeServices<HeadlessRenderDevice> {
    /// Creates minimal runtime services with a headless renderer.
    pub fn minimal(config: EngineConfig) -> Self {
        Self {
            config,
            scene: Scene::default(),
            renderer: HeadlessRenderDevice,
            frame_counter: FrameCounter::default(),
        }
    }
}

impl<R: RenderDevice> RuntimeServices<R> {
    /// Ticks one runtime frame.
    pub fn tick(&mut self) -> EngineResult<()> {
        logging::log_frame(self.frame_counter.get());
        self.renderer.render(RenderFrame {
            frame_index: self.frame_counter.get(),
        })?;
        self.frame_counter.advance();
        Ok(())
    }

    /// Current frame index.
    pub fn frame_index(&self) -> u64 {
        self.frame_counter.get()
    }
}

/// Runs a one-frame native smoke path for the minimal runtime.
pub fn smoke_runtime_min() -> EngineResult<u64> {
    let config = EngineConfig::default();
    logging::log_runtime_start(&config.app_name, config.profile.as_str());
    let mut services = RuntimeServices::minimal(config);
    services.tick()?;
    Ok(services.frame_index())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_min_ticks_one_frame() {
        assert_eq!(smoke_runtime_min().unwrap(), 1);
    }
}
