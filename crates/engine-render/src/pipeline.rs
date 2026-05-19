//! Shader, material, and GUI pipeline abstractions.

use engine_core::Handle;

/// Typed handle for a compiled shader.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ShaderHandle(pub(crate) Handle);

/// Typed handle for a material pipeline.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MaterialHandle(pub(crate) Handle);

/// Shader stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShaderStage {
    /// Vertex shader.
    Vertex,
    /// Fragment shader.
    Fragment,
    /// Compute shader.
    Compute,
}

/// Shader source format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShaderSource<'a> {
    /// SPIR-V bytecode.
    SpirV(&'a [u32]),
    /// GLSL source text.
    Glsl(&'a str),
    /// WGSL source text.
    Wgsl(&'a str),
}

/// Pipeline creation descriptor.
#[derive(Clone, Debug)]
pub struct PipelineDesc {
    /// Vertex shader handle.
    pub vertex: ShaderHandle,
    /// Fragment shader handle.
    pub fragment: ShaderHandle,
    /// Whether depth testing is enabled.
    pub depth_test: bool,
    /// Whether depth writing is enabled.
    pub depth_write: bool,
    /// Whether alpha blending is enabled.
    pub alpha_blend: bool,
    /// MSAA sample count.
    pub samples: u32,
    /// Debug label.
    pub label: Option<&'static str>,
}

/// Opaque GUI texture identifier returned by the backend.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GuiTextureId(pub u64);

/// A single GUI draw vertex.
#[derive(Clone, Copy, Debug)]
pub struct GuiVertex {
    /// Screen-space position.
    pub pos: [f32; 2],
    /// UV coordinates.
    pub uv: [f32; 2],
    /// RGBA color packed as u32.
    pub color: u32,
}

/// A GUI draw command referencing a texture and index range.
#[derive(Clone, Debug)]
pub struct GuiDrawCmd {
    /// Texture to bind.
    pub texture: GuiTextureId,
    /// Scissor rectangle `[x, y, w, h]` in pixels.
    pub scissor: [u32; 4],
    /// Index offset.
    pub index_offset: u32,
    /// Number of indices.
    pub index_count: u32,
}

/// Complete GUI draw list for one frame.
#[derive(Clone, Debug, Default)]
pub struct GuiDrawList {
    /// Vertex buffer.
    pub vertices: Vec<GuiVertex>,
    /// Index buffer.
    pub indices: Vec<u32>,
    /// Draw commands.
    pub commands: Vec<GuiDrawCmd>,
}
