//! Offscreen render targets for Scene View and Game View.

use engine_core::Handle;

use crate::resource::ImageFormat;

/// Identifies the purpose of a render target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewKind {
    /// Editor scene view.
    SceneView,
    /// Game camera view.
    GameView,
    /// Shadow map.
    Shadow,
    /// Post-processing intermediate.
    PostProcess,
    /// Material/mesh preview.
    Preview,
}

/// Render target creation descriptor.
#[derive(Clone, Debug)]
pub struct RenderTargetDesc {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Color attachment format.
    pub color_format: ImageFormat,
    /// Whether a depth attachment is needed.
    pub with_depth: bool,
    /// MSAA sample count.
    pub samples: u32,
    /// View kind.
    pub kind: ViewKind,
    /// Debug label.
    pub label: Option<&'static str>,
}

impl RenderTargetDesc {
    /// Creates a standard scene/game view descriptor.
    pub fn view(width: u32, height: u32, kind: ViewKind) -> Self {
        Self {
            width,
            height,
            color_format: ImageFormat::Rgba8Srgb,
            with_depth: true,
            samples: 1,
            kind,
            label: None,
        }
    }
}

/// A live render target backed by GPU resources.
#[derive(Debug)]
pub struct RenderTarget {
    /// Opaque backend handle.
    pub handle: Handle,
    /// Creation descriptor.
    pub desc: RenderTargetDesc,
}

impl RenderTarget {
    /// Returns the view kind.
    pub fn kind(&self) -> ViewKind {
        self.desc.kind
    }

    /// Returns the pixel dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.desc.width, self.desc.height)
    }
}
