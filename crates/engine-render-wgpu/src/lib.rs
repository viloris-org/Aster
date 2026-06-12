#![deny(missing_docs)]

//! WebGPU rendering backend for Aster.

use std::{collections::HashMap, sync::Arc};

use bytemuck::{Pod, Zeroable};
use engine_core::{EngineError, EngineResult, Handle, HandleAllocator};
use engine_render::{
    BufferDesc, BufferHandle, BufferUsage, GuiDrawList, GuiTextureId, ImageDesc, ImageFormat,
    ImageHandle, ImageUsage, RenderApi, RenderDevice, RenderFrame, RenderGraph, RenderLight,
    RenderLightKind, RenderMaterialTextures, RenderTarget, RenderTargetDesc, RenderWorld, ViewKind,
};
use wgpu::util::DeviceExt;

/// Re-exported wgpu API used by window hosts that need to create surfaces.
pub use wgpu;

const DEFAULT_WIDTH: u32 = 960;
const DEFAULT_HEIGHT: u32 = 540;
const CUBE_INDEX_COUNT: u32 = 36;
const MAX_FORWARD_LIGHTS: usize = 8;
const MAX_DIRECTIONAL_LIGHTS: usize = 2;
const DEFAULT_AMBIENT_LIGHT: [f32; 4] = [0.16, 0.16, 0.16, 1.0];
const CSM_CASCADE_COUNT: usize = 3;
const CSM_SHADOW_RESOLUTION: u32 = 2048;
const CSM_CASCADE_SPLITS: [f32; CSM_CASCADE_COUNT] = [15.0, 50.0, 150.0];

/// GPU vertex layout: position (3×f32), normal (3×f32), UV (2×f32).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    offset: [f32; 3],
    scale: [f32; 3],
    color: [f32; 4],
    rotation: [f32; 4],
    metallic: f32,
    roughness: f32,
    emissive: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_projection: [[f32; 4]; 4],
    camera_position: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ModelUniform {
    model: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ForwardLightUniform {
    position_type: [f32; 4],
    direction_range: [f32; 4],
    color_intensity: [f32; 4],
    spot_angles: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LightingUniform {
    ambient: [f32; 4],
    params: [u32; 4],
    lights: [ForwardLightUniform; MAX_FORWARD_LIGHTS],
}

impl Default for LightingUniform {
    fn default() -> Self {
        Self {
            ambient: DEFAULT_AMBIENT_LIGHT,
            params: [0, 0, 0, 0],
            lights: [ForwardLightUniform::zeroed(); MAX_FORWARD_LIGHTS],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShadowUniform {
    light_view_projection: [[f32; 4]; 4],
}

impl ShadowUniform {
    #[allow(dead_code)]
    fn zeroed() -> Self {
        Self {
            light_view_projection: IDENTITY_MAT4,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CsmUniform {
    /// Light-space VP matrix for each cascade.
    cascade_vps: [[[f32; 4]; 4]; CSM_CASCADE_COUNT],
    /// Split depth for each cascade in view space (vec4 for alignment, only first 3 used).
    cascade_splits: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SkyboxUniform {
    view_rotation_only: [[f32; 4]; 4],
    zenith_color: [f32; 4],
    horizon_color: [f32; 4],
    rotation_intensity: [f32; 4],
    use_cubemap: [u32; 4],
}

struct GpuImage {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    _desc: ImageDesc,
}

struct MaterialGpuData {
    bind_group: wgpu::BindGroup,
}

struct GpuTarget {
    _color: wgpu::Texture,
    color_view: wgpu::TextureView,
    _depth: Option<wgpu::Texture>,
    depth_view: Option<wgpu::TextureView>,
    _desc: RenderTargetDesc,
}

/// Configuration for creating an offscreen wgpu renderer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WgpuOffscreenConfig {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Color attachment format.
    pub format: ImageFormat,
}

impl Default for WgpuOffscreenConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            format: ImageFormat::Rgba8Srgb,
        }
    }
}

/// Procedural debug mesh shapes for quick visualisation without external assets.
#[derive(Clone, Debug, PartialEq)]
pub enum DebugMesh {
    /// Unit cube centred at origin, edge length 1, hard normals.
    Cube,
    /// UV sphere with the given longitudinal/latitudinal segment count.
    Sphere(u32),
    /// Quad on the XY plane from (-0.5, -0.5, 0) to (0.5, 0.5, 0).
    Plane,
}

/// GPU buffers for a single indexed mesh, ready for drawing.
#[derive(Debug)]
pub struct MeshBuffers {
    /// Vertex buffer uploaded to the GPU.
    pub vertex_buffer: wgpu::Buffer,
    /// Index buffer uploaded to the GPU.
    pub index_buffer: wgpu::Buffer,
    /// Number of indices to draw.
    pub index_count: u32,
}

/// Real wgpu render device with an offscreen default target.
pub struct WgpuRenderDevice {
    _instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    image_allocator: HandleAllocator,
    buffer_allocator: HandleAllocator,
    target_allocator: HandleAllocator,
    images: HashMap<Handle, GpuImage>,
    buffers: HashMap<Handle, wgpu::Buffer>,
    targets: HashMap<Handle, GpuTarget>,
    default_target: RenderTarget,
    game_target: RenderTarget,
    preview_target: RenderTarget,
    pipeline: wgpu::RenderPipeline,
    camera_bind_group: wgpu::BindGroup,
    camera_uniform: wgpu::Buffer,
    lighting_uniform: wgpu::Buffer,
    _default_texture: wgpu::Texture,
    default_texture_view: wgpu::TextureView,
    _default_normal_texture: wgpu::Texture,
    default_normal_texture_view: wgpu::TextureView,
    _default_mra_texture: wgpu::Texture,
    default_mra_texture_view: wgpu::TextureView,
    _default_sampler: wgpu::Sampler,
    material_bind_group_layout: wgpu::BindGroupLayout,
    default_material_bind_group: wgpu::BindGroup,
    material_gpu: HashMap<String, MaterialGpuData>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    mesh_cache: HashMap<String, MeshBuffers>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    surface_depth: Option<wgpu::Texture>,
    surface_depth_view: Option<wgpu::TextureView>,
    surface_suspended: bool,
    next_gui_texture: u64,
    submitted_worlds: u64,
    grid_pipeline: wgpu::RenderPipeline,
    grid_bind_group: wgpu::BindGroup,
    grid_vertex_buffer: wgpu::Buffer,
    grid_vertex_count: u32,
    csm_depth_views: [wgpu::TextureView; CSM_CASCADE_COUNT],
    _csm_depth_textures: [wgpu::Texture; CSM_CASCADE_COUNT],
    _csm_sampler: wgpu::Sampler,
    csm_uniform: wgpu::Buffer,
    shadow_pipeline: wgpu::RenderPipeline,
    shadow_bind_group_layout: wgpu::BindGroupLayout,
    material_cache: HashMap<String, ([f32; 4], f32, f32, [f32; 3])>,
    skybox_pipeline: wgpu::RenderPipeline,
    skybox_bind_group: wgpu::BindGroup,
    skybox_uniform: wgpu::Buffer,
    _skybox_default_cubemap: wgpu::Texture,
    _skybox_default_cubemap_view: wgpu::TextureView,
    _skybox_sampler: wgpu::Sampler,
    /// Frame-lagged destruction queue: (frame_index, resource).
    destroy_queue: Vec<(u64, DestroyResource)>,
}

/// A GPU resource pending deferred destruction.
#[allow(dead_code)]
enum DestroyResource {
    /// wgpu Texture (dropped when all GPU command buffers referencing it have completed).
    Texture(wgpu::Texture),
    /// wgpu Buffer.
    Buffer(wgpu::Buffer),
    /// wgpu TextureView.
    TextureView(wgpu::TextureView),
}

impl std::fmt::Debug for WgpuRenderDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuRenderDevice")
            .field("adapter", &self.adapter.get_info().name)
            .field("default_target", &self.default_target)
            .field("game_target", &self.game_target)
            .field("preview_target", &self.preview_target)
            .field("image_count", &self.images.len())
            .field("buffer_count", &self.buffers.len())
            .field("target_count", &self.targets.len())
            .field("submitted_worlds", &self.submitted_worlds)
            .finish()
    }
}

impl WgpuRenderDevice {
    /// Creates a wgpu device and an offscreen render target.
    pub fn new_offscreen(config: WgpuOffscreenConfig) -> EngineResult<Self> {
        pollster::block_on(Self::new_offscreen_async(config))
    }

    /// Creates a wgpu device and an offscreen render target asynchronously.
    pub async fn new_offscreen_async(config: WgpuOffscreenConfig) -> EngineResult<Self> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await;
        let adapter = match adapter {
            Ok(a) => a,
            Err(_) => instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::None,
                    compatible_surface: None,
                    force_fallback_adapter: true,
                })
                .await
                .map_err(|error| {
                    EngineError::other(format!("no suitable wgpu adapter found: {error}"))
                })?,
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|error| EngineError::other(format!("request wgpu device failed: {error}")))?;

        Self::from_device(
            instance,
            adapter,
            Arc::new(device),
            Arc::new(queue),
            config.width,
            config.height,
            config.format,
            None,
        )
    }

    /// Creates a wgpu device configured to present into a surface.
    pub fn new_surface(
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> EngineResult<Self> {
        pollster::block_on(Self::new_surface_async(surface, width, height))
    }

    /// Creates a wgpu device from a winit window, creating a surface automatically.
    pub fn new(window: &winit::window::Window) -> EngineResult<Self> {
        pollster::block_on(Self::new_async(window))
    }

    /// Creates a wgpu device from a winit window asynchronously.
    pub async fn new_async(window: &winit::window::Window) -> EngineResult<Self> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .map_err(|error| EngineError::other(format!("create wgpu surface failed: {error}")))?;
        // SAFETY: instance is moved into the returned struct and outlives the surface.
        let surface: wgpu::Surface<'static> = unsafe { std::mem::transmute(surface) };
        let width = window.inner_size().width;
        let height = window.inner_size().height;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await;
        let adapter = match adapter {
            Ok(a) => a,
            Err(_) => instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::None,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: true,
                })
                .await
                .map_err(|error| {
                    EngineError::other(format!("no suitable wgpu adapter found: {error}"))
                })?,
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|error| EngineError::other(format!("request wgpu device failed: {error}")))?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| *f == wgpu::TextureFormat::Bgra8UnormSrgb)
            .or_else(|| {
                caps.formats
                    .iter()
                    .copied()
                    .find(wgpu::TextureFormat::is_srgb)
            })
            .unwrap_or(caps.formats[0]);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);
        let image_format = from_wgpu_format(format).unwrap_or(ImageFormat::Rgba8Srgb);

        Self::from_device(
            instance,
            adapter,
            Arc::new(device),
            Arc::new(queue),
            width,
            height,
            image_format,
            Some((surface, surface_config)),
        )
    }

    /// Creates a wgpu device configured to present into a surface asynchronously.
    pub async fn new_surface_async(
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> EngineResult<Self> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await;
        let adapter = match adapter {
            Ok(a) => a,
            Err(_) => instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::None,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: true,
                })
                .await
                .map_err(|error| {
                    EngineError::other(format!("no suitable wgpu adapter found: {error}"))
                })?,
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|error| EngineError::other(format!("request wgpu device failed: {error}")))?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| *f == wgpu::TextureFormat::Bgra8UnormSrgb)
            .or_else(|| {
                caps.formats
                    .iter()
                    .copied()
                    .find(wgpu::TextureFormat::is_srgb)
            })
            .unwrap_or(caps.formats[0]);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);
        let image_format = from_wgpu_format(format).unwrap_or(ImageFormat::Rgba8Srgb);

        Self::from_device(
            instance,
            adapter,
            Arc::new(device),
            Arc::new(queue),
            width,
            height,
            image_format,
            Some((surface, surface_config)),
        )
    }

    /// Creates a wgpu render device from pre-created shared device and queue.
    ///
    /// Use this when the host (e.g. CLI editor) already owns a wgpu device/queue
    /// that must be shared between the 3D renderer and the host compositor.
    pub fn from_arc_device(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        width: u32,
        height: u32,
        format: ImageFormat,
        surface_state: Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)>,
    ) -> EngineResult<Self> {
        Self::from_device(
            instance,
            adapter,
            device,
            queue,
            width,
            height,
            format,
            surface_state,
        )
    }

    /// Returns a shared reference to the wgpu device.
    pub fn device_arc(&self) -> Arc<wgpu::Device> {
        Arc::clone(&self.device)
    }

    /// Returns a shared reference to the wgpu queue.
    pub fn queue_arc(&self) -> Arc<wgpu::Queue> {
        Arc::clone(&self.queue)
    }

    /// Returns a reference to the default offscreen render target's color texture view.
    pub fn default_target_view(&self) -> &wgpu::TextureView {
        self.targets
            .get(&self.default_target.handle)
            .map(|t| &t.color_view)
            .expect("default target exists")
    }

    /// Returns the pixel dimensions of the default offscreen render target.
    pub fn default_target_size(&self) -> (u32, u32) {
        self.default_target.size()
    }

    /// Returns a reference to the game offscreen render target's color texture view.
    pub fn game_target_view(&self) -> &wgpu::TextureView {
        self.targets
            .get(&self.game_target.handle)
            .map(|t| &t.color_view)
            .expect("game target exists")
    }

    /// Returns the pixel dimensions of the game offscreen render target.
    pub fn game_target_size(&self) -> (u32, u32) {
        self.game_target.size()
    }

    /// Returns a reference to the preview offscreen render target's color texture view.
    pub fn preview_target_view(&self) -> &wgpu::TextureView {
        self.targets
            .get(&self.preview_target.handle)
            .map(|t| &t.color_view)
            .expect("preview target exists")
    }

    /// Returns the pixel dimensions of the preview offscreen render target.
    pub fn preview_target_size(&self) -> (u32, u32) {
        self.preview_target.size()
    }

    /// Register PBR material parameters for an asset material name.
    ///
    /// Material names match the format used in `RenderObject::material`, e.g.
    /// `"asset:0123456789abcdef"`. Parameters registered here override the
    /// default built-in material lookups.
    pub fn register_material_params(
        &mut self,
        name: &str,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        emissive: [f32; 3],
    ) {
        self.material_cache
            .insert(name.to_owned(), (base_color, metallic, roughness, emissive));
    }

    /// Prepares instance buffer from mesh batches for rendering.
    fn prepare_render_batches(&mut self, world: &RenderWorld) -> Vec<(String, u32, String)> {
        let batches = self.mesh_batches_from_world(world);
        let total_instances: usize = batches.iter().map(|(_, inst, _)| inst.len()).sum();
        if total_instances > self.instance_capacity {
            self.instance_capacity = total_instances.next_power_of_two();
            self.instance_buffer = create_instance_buffer(&self.device, self.instance_capacity);
        }
        let mut all_instances = Vec::with_capacity(total_instances);
        for (_, instances, _) in &batches {
            all_instances.extend_from_slice(instances);
        }
        if !all_instances.is_empty() {
            self.queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&all_instances),
            );
        }
        batches
            .into_iter()
            .map(|(name, instances, mat)| (name, instances.len() as u32, mat))
            .collect()
    }

    /// Renders a render world to the default offscreen target, bypassing any surface.
    ///
    /// Use this when the host composites the result into its own UI.
    pub fn render_world_offscreen(&mut self, world: &RenderWorld) -> EngineResult<()> {
        let handle = self.default_target.handle;
        let (tw, th) = self.default_target.size();
        self.render_world_to_target(
            world,
            handle,
            tw as f32 / th.max(1) as f32,
            "aster offscreen render world encoder",
            "default wgpu target is missing",
        )
    }

    /// Renders a render world to the game offscreen target, bypassing any surface.
    ///
    /// Use this when the host composites the game view result into its own UI.
    pub fn render_world_offscreen_game(&mut self, world: &RenderWorld) -> EngineResult<()> {
        let handle = self.game_target.handle;
        let (tw, th) = self.game_target.size();
        self.render_world_to_target(
            world,
            handle,
            tw as f32 / th.max(1) as f32,
            "aster game offscreen render world encoder",
            "game wgpu target is missing",
        )
    }

    /// Renders a render world to the preview offscreen target.
    pub fn render_world_offscreen_preview(&mut self, world: &RenderWorld) -> EngineResult<()> {
        let handle = self.preview_target.handle;
        let (tw, th) = self.preview_target.size();
        self.render_world_to_target(
            world,
            handle,
            tw as f32 / th.max(1) as f32,
            "aster preview offscreen render world encoder",
            "preview wgpu target is missing",
        )
    }

    /// Read back the game offscreen target as RGBA pixels.
    ///
    /// Returns `(width, height, rgba_bytes)`. Uses a staging buffer and GPU readback.
    /// This is a synchronous blocking call — it waits for the GPU to finish.
    pub fn readback_game_target(&mut self) -> EngineResult<(u32, u32, Vec<u8>)> {
        let (w, h) = self.game_target.size();
        let format = self.game_target.desc.color_format;
        self.readback_target(w, h, format, self.game_target.handle)
    }

    /// Read back the default offscreen target as RGBA pixels.
    ///
    /// Returns `(width, height, rgba_bytes)`. Uses a staging buffer and GPU readback.
    /// This is a synchronous blocking call — it waits for the GPU to finish.
    pub fn readback_default_target(&mut self) -> EngineResult<(u32, u32, Vec<u8>)> {
        let (w, h) = self.default_target.size();
        let format = self.default_target.desc.color_format;
        self.readback_target(w, h, format, self.default_target.handle)
    }

    fn readback_target(
        &mut self,
        w: u32,
        h: u32,
        format: engine_render::ImageFormat,
        handle: Handle,
    ) -> EngineResult<(u32, u32, Vec<u8>)> {
        let bytes_per_pixel = format.bytes_per_pixel() as u64;
        // wgpu requires bytes_per_row to be a multiple of 256
        let unpadded = w as u64 * bytes_per_pixel;
        let padding = (256 - (unpadded % 256)) % 256;
        let bytes_per_row = unpadded + padding;
        let total_bytes = bytes_per_row * h as u64;

        let target = self
            .targets
            .get(&handle)
            .ok_or_else(|| EngineError::invalid_handle("readback target missing"))?;

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aster viewport readback staging"),
            size: total_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("aster viewport readback encoder"),
            });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target._color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row as u32),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        // Synchronous readback: map + wait
        let buffer_slice = staging.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok();
        receiver
            .recv()
            .map_err(|_| EngineError::other("viewport readback channel closed"))?
            .map_err(|e| EngineError::other(format!("viewport readback map failed: {e}")))?;

        let mapped = buffer_slice.get_mapped_range();
        // Strip padding: copy only the actual RGBA bytes per row
        let mut pixels = Vec::with_capacity((w * h * bytes_per_pixel as u32) as usize);
        for row in 0..h as usize {
            let start = row * bytes_per_row as usize;
            let end = start + w as usize * bytes_per_pixel as usize;
            pixels.extend_from_slice(&mapped[start..end]);
        }
        drop(mapped);
        staging.unmap();

        Ok((w, h, pixels))
    }

    fn render_world_to_target(
        &mut self,
        world: &RenderWorld,
        target_handle: Handle,
        aspect: f32,
        encoder_label: &str,
        missing_error: &str,
    ) -> EngineResult<()> {
        let batches = self.prepare_render_batches(world);
        let uniform = camera_uniform_from_world(world, aspect);
        self.queue
            .write_buffer(&self.camera_uniform, 0, bytemuck::bytes_of(&uniform));
        let lighting = lighting_uniform_from_world(world);
        self.queue
            .write_buffer(&self.lighting_uniform, 0, bytemuck::bytes_of(&lighting));
        let csm = csm_uniform_from_world(world, aspect);
        self.queue
            .write_buffer(&self.csm_uniform, 0, bytemuck::bytes_of(&csm));
        let skybox = skybox_uniform_from_world(world);
        self.queue
            .write_buffer(&self.skybox_uniform, 0, bytemuck::bytes_of(&skybox));

        let target = self
            .targets
            .get(&target_handle)
            .ok_or_else(|| EngineError::invalid_handle(missing_error))?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(encoder_label),
            });

        // CSM shadow passes (one per cascade)
        for cascade_idx in 0..CSM_CASCADE_COUNT {
            let cascade_vp = ShadowUniform {
                light_view_projection: csm.cascade_vps[cascade_idx],
            };
            let cascade_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("aster csm cascade {cascade_idx} uniform")),
                contents: bytemuck::bytes_of(&cascade_vp),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let cascade_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("aster csm cascade {cascade_idx} bind group")),
                layout: &self.shadow_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cascade_buf.as_entire_binding(),
                }],
            });
            encode_shadow_pass(
                &mut encoder,
                &self.csm_depth_views[cascade_idx],
                &self.shadow_pipeline,
                &cascade_bg,
                &self.vertex_buffer,
                &self.index_buffer,
                &self.instance_buffer,
                &batches,
                &self.mesh_cache,
            );
        }

        encode_skybox_pass(
            &mut encoder,
            &target.color_view,
            target.depth_view.as_ref(),
            &self.skybox_pipeline,
            &self.skybox_bind_group,
        );
        encode_batched_forward_pass(
            &mut encoder,
            &target.color_view,
            target.depth_view.as_ref(),
            &self.pipeline,
            &self.camera_bind_group,
            &self.default_material_bind_group,
            &self.material_gpu,
            &self.mesh_cache,
            &self.vertex_buffer,
            &self.index_buffer,
            &self.instance_buffer,
            &batches,
        );
        encode_grid_pass(
            &mut encoder,
            &target.color_view,
            target.depth_view.as_ref(),
            &self.grid_pipeline,
            &self.grid_bind_group,
            &self.grid_vertex_buffer,
            self.grid_vertex_count,
        );
        self.queue.submit(std::iter::once(encoder.finish()));
        self.submitted_worlds = self.submitted_worlds.saturating_add(1);
        Ok(())
    }

    fn from_device(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        width: u32,
        height: u32,
        format: ImageFormat,
        surface_state: Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)>,
    ) -> EngineResult<Self> {
        let mut target_allocator = HandleAllocator::default();
        let default_target = create_target(
            &device,
            &mut target_allocator,
            RenderTargetDesc {
                width: width.max(1),
                height: height.max(1),
                color_format: format,
                with_depth: true,
                samples: 1,
                kind: ViewKind::SceneView,
                label: Some("aster default offscreen target"),
            },
        )?;
        let game_target = create_target(
            &device,
            &mut target_allocator,
            RenderTargetDesc {
                width: width.max(1),
                height: height.max(1),
                color_format: format,
                with_depth: true,
                samples: 1,
                kind: ViewKind::GameView,
                label: Some("aster game offscreen target"),
            },
        )?;
        let preview_target = create_target(
            &device,
            &mut target_allocator,
            RenderTargetDesc {
                width: 320,
                height: 180,
                color_format: format,
                with_depth: true,
                samples: 1,
                kind: ViewKind::Preview,
                label: Some("aster camera preview offscreen target"),
            },
        )?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aster forward shader"),
            source: wgpu::ShaderSource::Wgsl(FORWARD_SHADER.into()),
        });
        let camera_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aster camera uniform"),
            contents: bytemuck::bytes_of(&CameraUniform {
                view_projection: IDENTITY_MAT4,
                camera_position: [0.0, 0.0, 5.0, 1.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let lighting_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aster lighting uniform"),
            contents: bytemuck::bytes_of(&LightingUniform::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let default_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aster default white texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let default_texture_view =
            default_texture.create_view(&wgpu::TextureViewDescriptor::default());
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &default_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("aster default sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        // CSM cascade shadow maps
        let mut csm_depth_textures = Vec::with_capacity(CSM_CASCADE_COUNT);
        let mut csm_depth_views = Vec::with_capacity(CSM_CASCADE_COUNT);
        for i in 0..CSM_CASCADE_COUNT {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("aster csm cascade {i} depth")),
                size: wgpu::Extent3d {
                    width: CSM_SHADOW_RESOLUTION,
                    height: CSM_SHADOW_RESOLUTION,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            csm_depth_textures.push(tex);
            csm_depth_views.push(view);
        }
        let csm_depth_textures: [wgpu::Texture; CSM_CASCADE_COUNT] =
            csm_depth_textures.try_into().unwrap();
        let csm_depth_views: [wgpu::TextureView; CSM_CASCADE_COUNT] =
            csm_depth_views.try_into().unwrap();
        let csm_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("aster csm sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let csm_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aster csm uniform"),
            contents: bytemuck::bytes_of(&CsmUniform {
                cascade_vps: [IDENTITY_MAT4; CSM_CASCADE_COUNT],
                cascade_splits: [0.0; 4],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Group 0: camera + lighting + CSM
        let scene_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("aster scene bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        count: None,
                    },
                ],
            });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aster scene bind group"),
            layout: &scene_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lighting_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: csm_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&csm_depth_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&csm_depth_views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&csm_depth_views[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&csm_sampler),
                },
            ],
        });

        // Group 1: material textures
        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("aster material bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // Default normal map (flat normal 128,128,255,255 = (0,0,1) in tangent space)
        let default_normal_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aster default normal texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let default_normal_texture_view =
            default_normal_texture.create_view(&wgpu::TextureViewDescriptor::default());
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &default_normal_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[128, 128, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        // Default metallic-roughness-AO texture (R=AO=1, G=roughness=0.5, B=metallic=0)
        let default_mra_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aster default MRA texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let default_mra_texture_view =
            default_mra_texture.create_view(&wgpu::TextureViewDescriptor::default());
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &default_mra_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 128, 0, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let default_material_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("aster default material bind group"),
                layout: &material_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&default_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&default_normal_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&default_mra_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&default_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&default_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&default_sampler),
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("aster forward pipeline layout"),
            bind_group_layouts: &[Some(&scene_bind_group_layout), Some(&material_bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("aster forward pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3,
                            1 => Float32x3,
                            2 => Float32x2
                        ],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Instance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            3 => Float32x3,
                            4 => Float32x3,
                            5 => Float32x4,
                            6 => Float32x4,
                            7 => Float32,
                            8 => Float32,
                            9 => Float32x3
                        ],
                    },
                ],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_state
                        .as_ref()
                        .map(|(_, config)| config.format)
                        .unwrap_or_else(|| to_wgpu_format(format)),
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // --- Grid pipeline ---
        let grid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aster grid shader"),
            source: wgpu::ShaderSource::Wgsl(GRID_SHADER.into()),
        });
        let grid_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("aster grid bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let grid_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aster grid bind group"),
            layout: &grid_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_uniform.as_entire_binding(),
            }],
        });
        let grid_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("aster grid pipeline layout"),
            bind_group_layouts: &[Some(&grid_bind_group_layout)],
            immediate_size: 0,
        });
        let grid_color_format = surface_state
            .as_ref()
            .map(|(_, config)| config.format)
            .unwrap_or_else(|| to_wgpu_format(format));
        let grid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("aster grid pipeline"),
            layout: Some(&grid_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &grid_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
                }],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &grid_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: grid_color_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let grid_vertices = generate_grid();
        let grid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aster grid vertices"),
            contents: bytemuck::cast_slice(&grid_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let grid_vertex_count = grid_vertices.len() as u32;

        // Shadow pipeline
        let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aster shadow shader"),
            source: wgpu::ShaderSource::Wgsl(SHADOW_SHADER.into()),
        });
        let shadow_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("aster shadow bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("aster shadow pipeline layout"),
                bind_group_layouts: &[Some(&shadow_bind_group_layout)],
                immediate_size: 0,
            });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("aster shadow pipeline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shadow_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3,
                            1 => Float32x3,
                            2 => Float32x2
                        ],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Instance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            3 => Float32x3,
                            4 => Float32x3,
                            5 => Float32x4,
                            6 => Float32x4,
                            7 => Float32,
                            8 => Float32,
                            9 => Float32x3
                        ],
                    },
                ],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });

        // Skybox pipeline
        let skybox_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aster skybox shader"),
            source: wgpu::ShaderSource::Wgsl(SKYBOX_SHADER.into()),
        });
        let skybox_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aster skybox uniform"),
            contents: bytemuck::bytes_of(&SkyboxUniform {
                view_rotation_only: IDENTITY_MAT4,
                zenith_color: [0.15, 0.35, 0.65, 1.0],
                horizon_color: [0.55, 0.7, 0.85, 1.0],
                rotation_intensity: [0.0, 1.0, 0.0, 0.0],
                use_cubemap: [0, 0, 0, 0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let skybox_default_cubemap = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aster skybox default cubemap"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for face in 0..6u32 {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &skybox_default_cubemap,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: face,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &[128, 128, 200, 255],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        let skybox_default_cubemap_view =
            skybox_default_cubemap.create_view(&wgpu::TextureViewDescriptor {
                label: Some("aster skybox default cubemap view"),
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            });
        let skybox_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("aster skybox sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let skybox_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("aster skybox bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let skybox_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aster skybox bind group"),
            layout: &skybox_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: skybox_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&skybox_default_cubemap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&skybox_sampler),
                },
            ],
        });
        let skybox_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("aster skybox pipeline layout"),
                bind_group_layouts: &[Some(&skybox_bind_group_layout)],
                immediate_size: 0,
            });
        let skybox_color_format = surface_state
            .as_ref()
            .map(|(_, config)| config.format)
            .unwrap_or_else(|| to_wgpu_format(format));
        let skybox_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("aster skybox pipeline"),
            layout: Some(&skybox_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &skybox_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &skybox_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: skybox_color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aster cube vertices"),
            contents: bytemuck::cast_slice(CUBE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aster cube indices"),
            contents: bytemuck::cast_slice(CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let instance_capacity = 1;
        let instance_buffer = create_instance_buffer(&device, instance_capacity);

        let CreatedTarget(color, color_view, depth, depth_view, default_target) = default_target;
        let CreatedTarget(game_color, game_color_view, game_depth, game_depth_view, game_target) =
            game_target;
        let CreatedTarget(
            preview_color,
            preview_color_view,
            preview_depth,
            preview_depth_view,
            preview_target,
        ) = preview_target;
        let mut targets = HashMap::new();
        targets.insert(
            default_target.handle,
            GpuTarget {
                _color: color,
                color_view,
                _depth: depth,
                depth_view,
                _desc: default_target.desc.clone(),
            },
        );
        targets.insert(
            game_target.handle,
            GpuTarget {
                _color: game_color,
                color_view: game_color_view,
                _depth: game_depth,
                depth_view: game_depth_view,
                _desc: game_target.desc.clone(),
            },
        );
        targets.insert(
            preview_target.handle,
            GpuTarget {
                _color: preview_color,
                color_view: preview_color_view,
                _depth: preview_depth,
                depth_view: preview_depth_view,
                _desc: preview_target.desc.clone(),
            },
        );

        let (surface, surface_config) = surface_state
            .map(|(surface, config)| (Some(surface), Some(config)))
            .unwrap_or((None, None));

        let mut renderer = Self {
            _instance: instance,
            adapter,
            device,
            queue,
            image_allocator: HandleAllocator::default(),
            buffer_allocator: HandleAllocator::default(),
            target_allocator,
            images: HashMap::new(),
            buffers: HashMap::new(),
            targets,
            default_target,
            game_target,
            preview_target,
            pipeline,
            camera_bind_group,
            camera_uniform,
            lighting_uniform,
            _default_texture: default_texture,
            default_texture_view,
            _default_normal_texture: default_normal_texture,
            default_normal_texture_view,
            _default_mra_texture: default_mra_texture,
            default_mra_texture_view,
            _default_sampler: default_sampler,
            material_bind_group_layout,
            default_material_bind_group,
            material_gpu: HashMap::new(),
            vertex_buffer,
            index_buffer,
            instance_buffer,
            instance_capacity,
            mesh_cache: HashMap::new(),
            surface,
            surface_config,
            surface_depth: None,
            surface_depth_view: None,
            surface_suspended: false,
            next_gui_texture: 1,
            submitted_worlds: 0,
            grid_pipeline,
            grid_bind_group,
            grid_vertex_buffer,
            grid_vertex_count,
            csm_depth_views,
            _csm_depth_textures: csm_depth_textures,
            _csm_sampler: csm_sampler,
            csm_uniform,
            shadow_pipeline,
            shadow_bind_group_layout,
            material_cache: HashMap::new(),
            skybox_pipeline,
            skybox_bind_group,
            skybox_uniform,
            _skybox_default_cubemap: skybox_default_cubemap,
            _skybox_default_cubemap_view: skybox_default_cubemap_view,
            _skybox_sampler: skybox_sampler,
            destroy_queue: Vec::new(),
        };
        renderer.upload_debug_meshes();
        Ok(renderer)
    }

    /// Number of render worlds submitted to this backend.
    pub fn submitted_worlds(&self) -> u64 {
        self.submitted_worlds
    }

    /// Resizes the default offscreen render target (scene view).
    ///
    /// No-op when the target already matches the requested dimensions.
    pub fn resize_default_target(&mut self, width: u32, height: u32) -> EngineResult<()> {
        let w = width.max(1);
        let h = height.max(1);
        if self.default_target.desc.width == w && self.default_target.desc.height == h {
            return Ok(());
        }
        let old_handle = self.default_target.handle;
        let desc = RenderTargetDesc {
            width: w,
            height: h,
            ..self.default_target.desc.clone()
        };
        self.default_target = self.create_resized_target(old_handle, desc)?;
        Ok(())
    }

    /// Resizes the game offscreen render target.
    ///
    /// No-op when the target already matches the requested dimensions.
    pub fn resize_game_target(&mut self, width: u32, height: u32) -> EngineResult<()> {
        let w = width.max(1);
        let h = height.max(1);
        if self.game_target.desc.width == w && self.game_target.desc.height == h {
            return Ok(());
        }
        let old_handle = self.game_target.handle;
        let desc = RenderTargetDesc {
            width: w,
            height: h,
            ..self.game_target.desc.clone()
        };
        self.game_target = self.create_resized_target(old_handle, desc)?;
        Ok(())
    }

    /// Resizes the preview offscreen render target.
    ///
    /// No-op when the target already matches the requested dimensions.
    pub fn resize_preview_target(&mut self, width: u32, height: u32) -> EngineResult<()> {
        let w = width.max(1);
        let h = height.max(1);
        if self.preview_target.desc.width == w && self.preview_target.desc.height == h {
            return Ok(());
        }
        let old_handle = self.preview_target.handle;
        let desc = RenderTargetDesc {
            width: w,
            height: h,
            ..self.preview_target.desc.clone()
        };
        self.preview_target = self.create_resized_target(old_handle, desc)?;
        Ok(())
    }

    fn create_resized_target(
        &mut self,
        old_handle: Handle,
        desc: RenderTargetDesc,
    ) -> EngineResult<RenderTarget> {
        let CreatedTarget(color, color_view, depth, depth_view, new_target) =
            create_target(&self.device, &mut self.target_allocator, desc)?;
        self.targets.remove(&old_handle);
        self.targets.insert(
            new_target.handle,
            GpuTarget {
                _color: color,
                color_view,
                _depth: depth,
                depth_view,
                _desc: new_target.desc.clone(),
            },
        );
        Ok(new_target)
    }

    /// Resizes the configured presentation surface.
    ///
    /// When either dimension is zero, rendering is suspended until valid dimensions
    /// are provided. Old depth resources are dropped before reconfiguration.
    pub fn resize_surface(&mut self, width: u32, height: u32) {
        let (Some(surface), Some(config)) = (self.surface.as_ref(), self.surface_config.as_mut())
        else {
            return;
        };
        if width == 0 || height == 0 {
            self.surface_suspended = true;
            return;
        }
        self.surface_depth = None;
        self.surface_depth_view = None;
        config.width = width;
        config.height = height;
        surface.configure(&self.device, config);
        self.surface_suspended = false;
    }

    /// Creates GPU vertex and index buffers from a procedural debug mesh.
    pub fn create_mesh_buffers(&self, mesh: &DebugMesh) -> MeshBuffers {
        let (vertices, indices) = generate_mesh(mesh);
        Self::buffers_from_data(&self.device, &vertices, &indices)
    }

    /// Uploads a mesh from vertex/index data into the mesh cache.
    pub fn upload_mesh(&mut self, name: &str, vertices: &[Vertex], indices: &[u32]) {
        let buffers = Self::buffers_from_data(&self.device, vertices, indices);
        self.mesh_cache.insert(name.to_string(), buffers);
    }

    /// Pre-loads procedural debug meshes into the cache.
    pub fn upload_debug_meshes(&mut self) {
        for mesh in &[DebugMesh::Cube, DebugMesh::Sphere(8), DebugMesh::Plane] {
            let name = mesh_name(mesh);
            let buffers = self.create_mesh_buffers(mesh);
            self.mesh_cache.insert(name, buffers);
        }
    }

    /// Returns true when a mesh is available in the cache.
    pub fn has_mesh(&self, name: &str) -> bool {
        self.mesh_cache.contains_key(name) || name == "debug/cube"
    }

    fn buffers_from_data(
        device: &wgpu::Device,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> MeshBuffers {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aster mesh vertices"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aster mesh indices"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        MeshBuffers {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }

    fn ensure_surface_depth(&mut self) {
        let Some(config) = self.surface_config.as_ref() else {
            return;
        };
        if self.surface_depth_view.is_some() {
            return;
        }
        let depth = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aster surface depth"),
            size: wgpu::Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        self.surface_depth = Some(depth);
        self.surface_depth_view = Some(view);
    }

    /// Groups render objects by mesh name for batched instanced rendering.
    fn mesh_batches_from_world(&self, world: &RenderWorld) -> Vec<(String, Vec<Instance>, String)> {
        let batch_capacity = (world.objects.len()
            + usize::from(!world.sprites.is_empty())
            + usize::from(!world.particles.is_empty()))
        .min(32);
        let mut batches: HashMap<(String, String), Vec<Instance>> = HashMap::with_capacity(batch_capacity);
        for object in &world.objects {
            let (color, metallic, roughness, emissive) = self.pbr_for_material(&object.material);
            let t = object.transform;
            let mesh = if object.mesh.is_empty() {
                "debug/cube".to_string()
            } else {
                object.mesh.clone()
            };
            let mat = object.material.clone();
            batches.entry((mesh, mat)).or_default().push(Instance {
                offset: [t.translation.x, t.translation.y, t.translation.z],
                scale: [
                    t.scale.x.max(0.05),
                    t.scale.y.max(0.05),
                    t.scale.z.max(0.05),
                ],
                color,
                rotation: [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w],
                metallic,
                roughness,
                emissive,
            });
        }
        if !world.sprites.is_empty() {
            let mut sprites = world.sprites.iter().collect::<Vec<_>>();
            sprites.sort_by(|left, right| {
                left.layer
                    .cmp(&right.layer)
                    .then(left.order_in_layer.cmp(&right.order_in_layer))
            });
            let sprite_instances = sprites.into_iter().map(|sprite| {
                let t = sprite.transform;
                let x = t.scale.x.abs().max(0.01) * if sprite.flip_h { -1.0 } else { 1.0 };
                let y = t.scale.y.abs().max(0.01) * if sprite.flip_v { -1.0 } else { 1.0 };
                Instance {
                    offset: [
                        t.translation.x,
                        t.translation.y,
                        t.translation.z + sprite.order_in_layer as f32 * 0.0001,
                    ],
                    scale: [x, y, t.scale.z.abs().max(0.01)],
                    color: sprite.color,
                    rotation: [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w],
                    metallic: 0.0,
                    roughness: 0.5,
                    emissive: [0.0; 3],
                }
            });
            batches
                .entry(("debug/plane".to_string(), String::new()))
                .or_default()
                .extend(sprite_instances);
        }
        if !world.particles.is_empty() {
            let particle_instances: Vec<Instance> = world
                .particles
                .iter()
                .map(|particle| {
                    let t = particle.transform;
                    Instance {
                        offset: [t.translation.x, t.translation.y, t.translation.z],
                        scale: [
                            t.scale.x.max(0.01),
                            t.scale.y.max(0.01),
                            t.scale.z.max(0.01),
                        ],
                        color: particle.color,
                        rotation: [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w],
                        metallic: 0.0,
                        roughness: 0.5,
                        emissive: [0.0; 3],
                    }
                })
                .collect();
            batches
                .entry(("debug/plane".to_string(), String::new()))
                .or_default()
                .extend(particle_instances);
        }
        batches
            .into_iter()
            .map(|((mesh, mat), instances)| (mesh, instances, mat))
            .collect()
    }

    fn pbr_for_material(&self, material: &str) -> ([f32; 4], f32, f32, [f32; 3]) {
        if let Some(&params) = self.material_cache.get(material) {
            return params;
        }
        if material.contains("debug") {
            ([0.2, 0.65, 1.0, 1.0], 0.0, 0.5, [0.0, 0.0, 0.0])
        } else if material.contains("error") {
            ([1.0, 0.2, 0.45, 1.0], 0.0, 0.5, [0.0, 0.0, 0.0])
        } else {
            ([0.82, 0.86, 0.72, 1.0], 0.0, 0.5, [0.0, 0.0, 0.0])
        }
    }
}

impl RenderDevice for WgpuRenderDevice {
    fn api(&self) -> RenderApi {
        RenderApi::WebGpu
    }

    fn render(&mut self, frame: RenderFrame) -> EngineResult<()> {
        self.submit_render_world(&RenderWorld::default(), frame)
    }

    fn submit_render_world(
        &mut self,
        world: &RenderWorld,
        _frame: RenderFrame,
    ) -> EngineResult<()> {
        let batches = self.prepare_render_batches(world);
        let aspect = self
            .surface_config
            .as_ref()
            .map(|cfg| cfg.width as f32 / cfg.height.max(1) as f32)
            .unwrap_or(16.0 / 9.0);
        let uniform = camera_uniform_from_world(world, aspect);
        self.queue
            .write_buffer(&self.camera_uniform, 0, bytemuck::bytes_of(&uniform));
        let lighting = lighting_uniform_from_world(world);
        self.queue
            .write_buffer(&self.lighting_uniform, 0, bytemuck::bytes_of(&lighting));
        let csm = csm_uniform_from_world(world, aspect);
        self.queue
            .write_buffer(&self.csm_uniform, 0, bytemuck::bytes_of(&csm));
        let skybox = skybox_uniform_from_world(world);
        self.queue
            .write_buffer(&self.skybox_uniform, 0, bytemuck::bytes_of(&skybox));

        if self.surface.is_some() {
            if self.surface_suspended {
                return Ok(());
            }
            self.ensure_surface_depth();
            let surface = self
                .surface
                .as_ref()
                .ok_or_else(|| EngineError::invalid_handle("wgpu surface is missing"))?;
            let frame = match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    if let Some(config) = self.surface_config.as_ref() {
                        surface.configure(&self.device, config);
                    }
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Timeout
                | wgpu::CurrentSurfaceTexture::Occluded
                | wgpu::CurrentSurfaceTexture::Validation => return Ok(()),
            };
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("aster surface render world encoder"),
                });
            // CSM shadow passes
            for cascade_idx in 0..CSM_CASCADE_COUNT {
                let cascade_vp = ShadowUniform {
                    light_view_projection: csm.cascade_vps[cascade_idx],
                };
                let cascade_buf =
                    self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("aster csm cascade {cascade_idx} uniform")),
                        contents: bytemuck::bytes_of(&cascade_vp),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                let cascade_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("aster csm cascade {cascade_idx} bind group")),
                    layout: &self.shadow_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: cascade_buf.as_entire_binding(),
                    }],
                });
                encode_shadow_pass(
                    &mut encoder,
                    &self.csm_depth_views[cascade_idx],
                    &self.shadow_pipeline,
                    &cascade_bg,
                    &self.vertex_buffer,
                    &self.index_buffer,
                    &self.instance_buffer,
                    &batches,
                    &self.mesh_cache,
                );
            }
            encode_skybox_pass(
                &mut encoder,
                &view,
                self.surface_depth_view.as_ref(),
                &self.skybox_pipeline,
                &self.skybox_bind_group,
            );
            encode_batched_forward_pass(
                &mut encoder,
                &view,
                self.surface_depth_view.as_ref(),
                &self.pipeline,
                &self.camera_bind_group,
                &self.default_material_bind_group,
                &self.material_gpu,
                &self.mesh_cache,
                &self.vertex_buffer,
                &self.index_buffer,
                &self.instance_buffer,
                &batches,
            );
            encode_grid_pass(
                &mut encoder,
                &view,
                self.surface_depth_view.as_ref(),
                &self.grid_pipeline,
                &self.grid_bind_group,
                &self.grid_vertex_buffer,
                self.grid_vertex_count,
            );
            self.queue.submit(std::iter::once(encoder.finish()));
            frame.present();
            self.submitted_worlds = self.submitted_worlds.saturating_add(1);
            return Ok(());
        }

        let target = self
            .targets
            .get(&self.default_target.handle)
            .ok_or_else(|| EngineError::invalid_handle("default wgpu target is missing"))?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("aster render world encoder"),
            });
        // CSM shadow passes
        for cascade_idx in 0..CSM_CASCADE_COUNT {
            let cascade_vp = ShadowUniform {
                light_view_projection: csm.cascade_vps[cascade_idx],
            };
            let cascade_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("aster csm cascade {cascade_idx} uniform")),
                contents: bytemuck::bytes_of(&cascade_vp),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let cascade_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("aster csm cascade {cascade_idx} bind group")),
                layout: &self.shadow_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cascade_buf.as_entire_binding(),
                }],
            });
            encode_shadow_pass(
                &mut encoder,
                &self.csm_depth_views[cascade_idx],
                &self.shadow_pipeline,
                &cascade_bg,
                &self.vertex_buffer,
                &self.index_buffer,
                &self.instance_buffer,
                &batches,
                &self.mesh_cache,
            );
        }
        encode_skybox_pass(
            &mut encoder,
            &target.color_view,
            target.depth_view.as_ref(),
            &self.skybox_pipeline,
            &self.skybox_bind_group,
        );
        encode_batched_forward_pass(
            &mut encoder,
            &target.color_view,
            target.depth_view.as_ref(),
            &self.pipeline,
            &self.camera_bind_group,
            &self.default_material_bind_group,
            &self.material_gpu,
            &self.mesh_cache,
            &self.vertex_buffer,
            &self.index_buffer,
            &self.instance_buffer,
            &batches,
        );
        encode_grid_pass(
            &mut encoder,
            &target.color_view,
            target.depth_view.as_ref(),
            &self.grid_pipeline,
            &self.grid_bind_group,
            &self.grid_vertex_buffer,
            self.grid_vertex_count,
        );
        self.queue.submit(std::iter::once(encoder.finish()));
        self.submitted_worlds = self.submitted_worlds.saturating_add(1);
        Ok(())
    }

    /// Prepares instance buffer from mesh batches for rendering.
    fn execute_graph(&mut self, _graph: &RenderGraph, _frame: RenderFrame) -> EngineResult<()> {
        Ok(())
    }

    fn create_render_target(&mut self, desc: RenderTargetDesc) -> EngineResult<RenderTarget> {
        let created = create_target(&self.device, &mut self.target_allocator, desc)?;
        self.targets.insert(
            created.4.handle,
            GpuTarget {
                _color: created.0,
                color_view: created.1,
                _depth: created.2,
                depth_view: created.3,
                _desc: created.4.desc.clone(),
            },
        );
        Ok(created.4)
    }

    fn destroy_render_target(&mut self, target: RenderTarget) {
        self.targets.remove(&target.handle);
    }

    fn create_image(&mut self, desc: ImageDesc) -> EngineResult<ImageHandle> {
        let handle = self.image_allocator.allocate()?;
        let texture = self.device.create_texture(&texture_desc(&desc));
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.images.insert(
            handle,
            GpuImage {
                _texture: texture,
                view: view,
                _desc: desc,
            },
        );
        Ok(ImageHandle::new(handle))
    }

    fn upload_texture(&mut self, desc: ImageDesc, data: &[u8]) -> EngineResult<ImageHandle> {
        let handle = self.image_allocator.allocate()?;
        let texture = self.device.create_texture(&texture_desc(&desc));
        if !data.is_empty() {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(desc.width.max(1) * desc.format.bytes_per_pixel()),
                    rows_per_image: Some(desc.height.max(1)),
                },
                wgpu::Extent3d {
                    width: desc.width.max(1),
                    height: desc.height.max(1),
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.images.insert(
            handle,
            GpuImage {
                _texture: texture,
                view: view,
                _desc: desc,
            },
        );
        Ok(ImageHandle::new(handle))
    }

    fn destroy_image(&mut self, handle: ImageHandle) {
        if let Some(image) = self.images.remove(&handle.raw()) {
            let _ = self.image_allocator.free(handle.raw());
            let frame = self.submitted_worlds;
            self.destroy_queue
                .push((frame, DestroyResource::Texture(image._texture)));
        }
    }

    fn create_buffer(&mut self, desc: BufferDesc) -> EngineResult<BufferHandle> {
        let handle = self.buffer_allocator.allocate()?;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: desc.label,
            size: desc.size.max(1),
            usage: to_wgpu_buffer_usage(desc.usage, desc.host_visible),
            mapped_at_creation: false,
        });
        self.buffers.insert(handle, buffer);
        Ok(BufferHandle::new(handle))
    }

    fn destroy_buffer(&mut self, handle: BufferHandle) {
        if let Some(buffer) = self.buffers.remove(&handle.raw()) {
            let _ = self.buffer_allocator.free(handle.raw());
            let frame = self.submitted_worlds;
            self.destroy_queue
                .push((frame, DestroyResource::Buffer(buffer)));
        }
    }

    fn upload_gui_texture(&mut self, desc: ImageDesc, data: &[u8]) -> EngineResult<GuiTextureId> {
        let texture = self.device.create_texture(&texture_desc(&desc));
        if !data.is_empty() {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(desc.width.max(1) * desc.format.bytes_per_pixel()),
                    rows_per_image: Some(desc.height.max(1)),
                },
                wgpu::Extent3d {
                    width: desc.width.max(1),
                    height: desc.height.max(1),
                    depth_or_array_layers: 1,
                },
            );
        }
        let handle = self.image_allocator.allocate()?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.images.insert(
            handle,
            GpuImage {
                _texture: texture,
                view: view,
                _desc: desc,
            },
        );
        let id = GuiTextureId(self.next_gui_texture);
        self.next_gui_texture = self.next_gui_texture.saturating_add(1);
        Ok(id)
    }

    fn draw_gui(&mut self, _draw_list: &GuiDrawList) -> EngineResult<()> {
        Ok(())
    }

    fn upload_mesh_data(
        &mut self,
        mesh_name: &str,
        positions: &[[f32; 3]],
        normals: &[[f32; 3]],
        texcoords: &[[f32; 2]],
        indices: &[u32],
    ) -> EngineResult<()> {
        let vertex_count = positions.len().min(normals.len()).min(texcoords.len());
        let vertices: Vec<Vertex> = (0..vertex_count)
            .map(|i| Vertex {
                position: positions[i],
                normal: normals[i],
                uv: texcoords[i],
            })
            .collect();
        self.upload_mesh(mesh_name, &vertices, indices);
        Ok(())
    }

    fn flush_destroy_queue(&mut self, frame_index: u64) {
        // Drop resources whose frame index is at least 2 frames behind the
        // current frame, ensuring GPU command buffers referencing them have
        // completed.
        let threshold = frame_index.saturating_sub(2);
        self.destroy_queue
            .retain(|(idx, _resource)| *idx > threshold);
    }

    fn register_material_params(
        &mut self,
        name: &str,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        emissive: [f32; 3],
    ) {
        WgpuRenderDevice::register_material_params(
            self, name, base_color, metallic, roughness, emissive,
        );
    }

    fn register_material_textures(
        &mut self,
        name: &str,
        textures: &RenderMaterialTextures,
    ) {
        let view_for = |handle: &Option<ImageHandle>| -> &wgpu::TextureView {
            match handle {
                Some(h) => match self.images.get(&h.raw()) {
                    Some(img) => &img.view,
                    None => &self.default_texture_view,
                },
                None => &self.default_texture_view,
            }
        };
        let base_color_view = view_for(&textures.base_color);
        let normal_view = if textures.normal.is_some() {
            view_for(&textures.normal)
        } else {
            &self.default_normal_texture_view
        };
        let mra_view = if textures.metallic_roughness.is_some() {
            view_for(&textures.metallic_roughness)
        } else {
            &self.default_mra_texture_view
        };
        let emissive_view = view_for(&textures.emissive);
        let occlusion_view = view_for(&textures.occlusion);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("aster material bind group: {name}")),
            layout: &self.material_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(base_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(mra_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(emissive_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(occlusion_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self._default_sampler),
                },
            ],
        });
        self.material_gpu
            .insert(name.to_owned(), MaterialGpuData { bind_group });
    }
}

struct CreatedTarget(
    wgpu::Texture,
    wgpu::TextureView,
    Option<wgpu::Texture>,
    Option<wgpu::TextureView>,
    RenderTarget,
);

fn create_target(
    device: &wgpu::Device,
    allocator: &mut HandleAllocator,
    desc: RenderTargetDesc,
) -> EngineResult<CreatedTarget> {
    let handle = allocator.allocate()?;
    let color_desc = ImageDesc {
        width: desc.width.max(1),
        height: desc.height.max(1),
        mip_levels: 1,
        samples: desc.samples.max(1),
        format: desc.color_format,
        usage: ImageUsage::COLOR_ATTACHMENT
            .or(ImageUsage::SAMPLED)
            .or(ImageUsage::TRANSFER_SRC),
        label: desc.label,
    };
    let color = device.create_texture(&texture_desc(&color_desc));
    let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
    let (depth, depth_view) = if desc.with_depth {
        let depth_desc = ImageDesc::depth_2d(desc.width.max(1), desc.height.max(1));
        let depth = device.create_texture(&texture_desc(&depth_desc));
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        (Some(depth), Some(depth_view))
    } else {
        (None, None)
    };
    let target = RenderTarget { handle, desc };
    Ok(CreatedTarget(color, color_view, depth, depth_view, target))
}

fn texture_desc(desc: &ImageDesc) -> wgpu::TextureDescriptor<'_> {
    wgpu::TextureDescriptor {
        label: desc.label,
        size: wgpu::Extent3d {
            width: desc.width.max(1),
            height: desc.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: desc.mip_levels.max(1),
        sample_count: desc.samples.max(1),
        dimension: wgpu::TextureDimension::D2,
        format: to_wgpu_format(desc.format),
        usage: to_wgpu_texture_usage(desc.usage),
        view_formats: &[],
    }
}

fn to_wgpu_format(format: ImageFormat) -> wgpu::TextureFormat {
    match format {
        ImageFormat::Rgba8Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        ImageFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        ImageFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
        ImageFormat::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
        ImageFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
        ImageFormat::Depth24Stencil8 => wgpu::TextureFormat::Depth24PlusStencil8,
        ImageFormat::Bc7Srgb => wgpu::TextureFormat::Bc7RgbaUnormSrgb,
    }
}

fn from_wgpu_format(format: wgpu::TextureFormat) -> Option<ImageFormat> {
    match format {
        wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Bgra8UnormSrgb => {
            Some(ImageFormat::Rgba8Srgb)
        }
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm => {
            Some(ImageFormat::Rgba8Unorm)
        }
        wgpu::TextureFormat::Rgba16Float => Some(ImageFormat::Rgba16Float),
        wgpu::TextureFormat::Rgba32Float => Some(ImageFormat::Rgba32Float),
        _ => None,
    }
}

fn encode_batched_forward_pass<'a>(
    encoder: &mut wgpu::CommandEncoder,
    color_view: &wgpu::TextureView,
    depth_view: Option<&wgpu::TextureView>,
    pipeline: &wgpu::RenderPipeline,
    camera_bind_group: &wgpu::BindGroup,
    default_material_bind_group: &wgpu::BindGroup,
    material_gpu: &HashMap<String, MaterialGpuData>,
    mesh_cache: &'a HashMap<String, MeshBuffers>,
    default_vertex_buffer: &'a wgpu::Buffer,
    default_index_buffer: &'a wgpu::Buffer,
    instance_buffer: &wgpu::Buffer,
    batches: &[(String, u32, String)],
) {
    let color_attachment = Some(wgpu::RenderPassColorAttachment {
        view: color_view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
        },
    });
    let depth_attachment = depth_view.map(|view| wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
        }),
        stencil_ops: None,
    });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("aster forward pass"),
        color_attachments: &[color_attachment],
        depth_stencil_attachment: depth_attachment,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, camera_bind_group, &[]);

    let mut instance_offset = 0u32;
    for (mesh_name, count, material_name) in batches {
        if *count == 0 {
            continue;
        }
        let mat_bg = material_gpu
            .get(material_name)
            .map(|m| &m.bind_group)
            .unwrap_or(default_material_bind_group);
        pass.set_bind_group(1, mat_bg, &[]);
        let buffers = mesh_cache.get(mesh_name);
        let (vertex_buf, index_buf, index_count) = match buffers {
            Some(b) => (&b.vertex_buffer, &b.index_buffer, b.index_count),
            None => (
                default_vertex_buffer,
                default_index_buffer,
                CUBE_INDEX_COUNT,
            ),
        };
        pass.set_vertex_buffer(0, vertex_buf.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..index_count, 0, instance_offset..instance_offset + count);
        instance_offset += count;
    }
}

fn csm_uniform_from_world(world: &RenderWorld, aspect: f32) -> CsmUniform {
    let light_dir = world
        .lights
        .iter()
        .find(|l| l.kind == RenderLightKind::Directional)
        .map(|l| l.transform.translation.normalized())
        .unwrap_or_else(|| engine_core::math::Vec3::new(-0.5, -1.0, -0.25).normalized());

    let cam = match &world.camera {
        Some(c) => c,
        None => {
            return CsmUniform {
                cascade_vps: [IDENTITY_MAT4; CSM_CASCADE_COUNT],
                cascade_splits: [0.0; 4],
            };
        }
    };

    let cam_pos = cam.transform.translation;
    let cam_forward = engine_core::math::Vec3::new(
        -cam.transform.rotation.x * 2.0 * cam.transform.rotation.w,
        -cam.transform.rotation.y * 2.0 * cam.transform.rotation.w,
        -cam.transform.rotation.z * 2.0 * cam.transform.rotation.w,
    )
    .normalized();

    let fov_rad = cam.vertical_fov_degrees.to_radians();
    let tan_half_fov = (fov_rad * 0.5).tan();

    let mut cascade_vps = [IDENTITY_MAT4; CSM_CASCADE_COUNT];
    let mut cascade_splits = [0.0f32; 4];

    for i in 0..CSM_CASCADE_COUNT {
        let near = if i == 0 { cam.near } else { CSM_CASCADE_SPLITS[i - 1] };
        let far = CSM_CASCADE_SPLITS[i];
        cascade_splits[i] = far;

        let half_height_near = tan_half_fov * near;
        let half_width_near = half_height_near * aspect;
        let half_height_far = tan_half_fov * far;
        let half_width_far = half_height_far * aspect;

        let near_center = cam_pos + cam_forward * near;
        let far_center = cam_pos + cam_forward * far;

        let right = engine_core::math::Vec3::new(0.0, 1.0, 0.0)
            .cross(cam_forward)
            .normalized();
        let up = cam_forward.cross(right);

        let corners = [
            near_center + right * half_width_near + up * half_height_near,
            near_center - right * half_width_near + up * half_height_near,
            near_center - right * half_width_near - up * half_height_near,
            near_center + right * half_width_near - up * half_height_near,
            far_center + right * half_width_far + up * half_height_far,
            far_center - right * half_width_far + up * half_height_far,
            far_center - right * half_width_far - up * half_height_far,
            far_center + right * half_width_far - up * half_height_far,
        ];

        let mut center = engine_core::math::Vec3::ZERO;
        for corner in &corners {
            center = center + *corner;
        }
        center = center * (1.0 / 8.0);

        let up = if light_dir.x.abs() < 0.99 {
            engine_core::math::Vec3::new(0.0, 1.0, 0.0)
        } else {
            engine_core::math::Vec3::new(0.0, 0.0, 1.0)
        };
        let light_view = look_at_rh(center - light_dir * 50.0, center, up);

        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut min_z = f32::MAX;
        let mut max_z = f32::MIN;
        for corner in &corners {
            let p = mul_mat4_vec3(&light_view, *corner);
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
        }

        let z_padding = 10.0;
        let light_proj = orthographic_rh_custom(
            min_x, max_x, min_y, max_y, min_z - z_padding, max_z + z_padding,
        );
        cascade_vps[i] = mul_mat4(&light_proj, &light_view);
    }

    CsmUniform {
        cascade_vps,
        cascade_splits,
    }
}

fn encode_shadow_pass(
    encoder: &mut wgpu::CommandEncoder,
    depth_view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
    vertex_buffer: &wgpu::Buffer,
    index_buffer: &wgpu::Buffer,
    instance_buffer: &wgpu::Buffer,
    batches: &[(String, u32, String)],
    mesh_cache: &HashMap<String, MeshBuffers>,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("aster shadow pass"),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);

    let mut instance_offset = 0u32;
    for (mesh_name, count, _) in batches {
        if *count == 0 {
            continue;
        }
        let buffers = mesh_cache.get(mesh_name);
        let (vertex_buf, index_buf, index_count) = match buffers {
            Some(b) => (&b.vertex_buffer, &b.index_buffer, b.index_count),
            None => (vertex_buffer, index_buffer, CUBE_INDEX_COUNT),
        };
        pass.set_vertex_buffer(0, vertex_buf.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..index_count, 0, instance_offset..instance_offset + count);
        instance_offset += count;
    }
}

fn encode_grid_pass(
    encoder: &mut wgpu::CommandEncoder,
    color_view: &wgpu::TextureView,
    depth_view: Option<&wgpu::TextureView>,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
    vertex_buffer: &wgpu::Buffer,
    vertex_count: u32,
) {
    let color_attachment = Some(wgpu::RenderPassColorAttachment {
        view: color_view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
        },
    });
    let depth_attachment = depth_view.map(|view| wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
        }),
        stencil_ops: None,
    });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("aster grid pass"),
        color_attachments: &[color_attachment],
        depth_stencil_attachment: depth_attachment,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    pass.draw(0..vertex_count, 0..1);
}

fn skybox_uniform_from_world(world: &RenderWorld) -> SkyboxUniform {
    let skybox = match &world.skybox {
        Some(s) => s,
        None => {
            return SkyboxUniform {
                view_rotation_only: IDENTITY_MAT4,
                zenith_color: [0.15, 0.35, 0.65, 1.0],
                horizon_color: [0.55, 0.7, 0.85, 1.0],
                rotation_intensity: [0.0, 1.0, 0.0, 0.0],
                use_cubemap: [0, 0, 0, 0],
            };
        }
    };

    let eye = world
        .camera
        .as_ref()
        .map(|c| c.transform.translation)
        .unwrap_or(engine_core::math::Vec3::new(0.0, 0.0, 5.0));
    let target = world
        .camera
        .as_ref()
        .and_then(|c| c.look_at_target)
        .unwrap_or_else(|| {
            let q = world
                .camera
                .as_ref()
                .map(|c| c.transform.rotation)
                .unwrap_or(engine_core::math::Quat::IDENTITY);
            let fwd = engine_core::math::Vec3::new(
                2.0 * (q.x * q.z + q.w * q.y),
                2.0 * (q.y * q.z - q.w * q.x),
                1.0 - 2.0 * (q.x * q.x + q.y * q.y),
            );
            engine_core::math::Vec3::new(eye.x - fwd.x, eye.y - fwd.y, eye.z - fwd.z)
        });
    let view = look_at_rh(eye, target, engine_core::math::Vec3::new(0.0, 1.0, 0.0));

    SkyboxUniform {
        view_rotation_only: view,
        zenith_color: [
            skybox.zenith_color[0],
            skybox.zenith_color[1],
            skybox.zenith_color[2],
            1.0,
        ],
        horizon_color: [
            skybox.horizon_color[0],
            skybox.horizon_color[1],
            skybox.horizon_color[2],
            1.0,
        ],
        rotation_intensity: [skybox.rotation_degrees, skybox.intensity, 0.0, 0.0],
        use_cubemap: [0, 0, 0, 0],
    }
}

fn encode_skybox_pass(
    encoder: &mut wgpu::CommandEncoder,
    color_view: &wgpu::TextureView,
    depth_view: Option<&wgpu::TextureView>,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
) {
    let color_attachment = Some(wgpu::RenderPassColorAttachment {
        view: color_view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
            store: wgpu::StoreOp::Store,
        },
    });
    let depth_attachment = depth_view.map(|view| wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(1.0),
            store: wgpu::StoreOp::Store,
        }),
        stencil_ops: None,
    });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("aster skybox pass"),
        color_attachments: &[color_attachment],
        depth_stencil_attachment: depth_attachment,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.draw(0..3, 0..1);
}

fn to_wgpu_texture_usage(usage: ImageUsage) -> wgpu::TextureUsages {
    let mut out = wgpu::TextureUsages::empty();
    if usage.contains(ImageUsage::SAMPLED) {
        out |= wgpu::TextureUsages::TEXTURE_BINDING;
    }
    if usage.contains(ImageUsage::COLOR_ATTACHMENT) {
        out |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    if usage.contains(ImageUsage::DEPTH_STENCIL_ATTACHMENT) {
        out |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    if usage.contains(ImageUsage::STORAGE) {
        out |= wgpu::TextureUsages::STORAGE_BINDING;
    }
    if usage.contains(ImageUsage::TRANSFER_SRC) {
        out |= wgpu::TextureUsages::COPY_SRC;
    }
    if usage.contains(ImageUsage::TRANSFER_DST) {
        out |= wgpu::TextureUsages::COPY_DST;
    }
    out
}

fn to_wgpu_buffer_usage(usage: BufferUsage, host_visible: bool) -> wgpu::BufferUsages {
    let mut out = wgpu::BufferUsages::empty();
    if usage.contains(BufferUsage::VERTEX) {
        out |= wgpu::BufferUsages::VERTEX;
    }
    if usage.contains(BufferUsage::INDEX) {
        out |= wgpu::BufferUsages::INDEX;
    }
    if usage.contains(BufferUsage::UNIFORM) {
        out |= wgpu::BufferUsages::UNIFORM;
    }
    if usage.contains(BufferUsage::STORAGE) {
        out |= wgpu::BufferUsages::STORAGE;
    }
    if usage.contains(BufferUsage::TRANSFER_SRC) || host_visible {
        out |= wgpu::BufferUsages::COPY_SRC;
    }
    if usage.contains(BufferUsage::TRANSFER_DST) || host_visible {
        out |= wgpu::BufferUsages::COPY_DST;
    }
    out
}

fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("aster instance buffer"),
        size: (capacity.max(1) * std::mem::size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn camera_uniform_from_world(world: &RenderWorld, aspect: f32) -> CameraUniform {
    let eye = world
        .camera
        .as_ref()
        .map(|camera| camera.transform.translation)
        .unwrap_or_else(|| engine_core::math::Vec3::new(0.0, 0.0, 5.0));

    // Use the explicit look-at pivot if provided (editor orbit camera sets this),
    // otherwise fall back to deriving the target from the camera's transform rotation.
    let target = world
        .camera
        .as_ref()
        .and_then(|camera| camera.look_at_target)
        .unwrap_or_else(|| {
            // Extract the local +Z axis from the rotation quaternion in world space.
            // q * (0,0,1) gives the camera's local +Z in world space.
            // Since the camera looks along local -Z, the view direction is
            // -(+Z) which is achieved by target = eye - fwd below.
            let q = world
                .camera
                .as_ref()
                .map(|c| c.transform.rotation)
                .unwrap_or(engine_core::math::Quat::IDENTITY);
            let fwd = engine_core::math::Vec3::new(
                2.0 * (q.x * q.z + q.w * q.y),
                2.0 * (q.y * q.z - q.w * q.x),
                1.0 - 2.0 * (q.x * q.x + q.y * q.y),
            );
            // Negate because camera looks along -Z in its local space.
            engine_core::math::Vec3::new(eye.x - fwd.x, eye.y - fwd.y, eye.z - fwd.z)
        });

    let view = look_at_rh(eye, target, engine_core::math::Vec3::new(0.0, 1.0, 0.0));
    let fov = world
        .camera
        .as_ref()
        .map(|camera| camera.vertical_fov_degrees)
        .unwrap_or(60.0);
    let near = world
        .camera
        .as_ref()
        .map(|camera| camera.near)
        .unwrap_or(0.1);
    let far = world
        .camera
        .as_ref()
        .map(|camera| camera.far)
        .unwrap_or(100.0);
    let proj = match world.camera.as_ref().map(|camera| camera.projection) {
        Some(engine_render::RenderProjection::Orthographic { vertical_size }) => {
            orthographic_rh(vertical_size.max(0.001), aspect, near, far)
        }
        _ => perspective_rh(fov.to_radians(), aspect, near, far),
    };
    let vp = mul_mat4(&proj, &view);
    CameraUniform {
        view_projection: vp,
        camera_position: [eye.x, eye.y, eye.z, 1.0],
    }
}

fn lighting_uniform_from_world(world: &RenderWorld) -> LightingUniform {
    let mut uniform = LightingUniform::default();
    let mut count = 0usize;

    for light in select_forward_lights(world) {
        uniform.lights[count] = forward_light_uniform(light);
        count += 1;
    }

    if count == 0 {
        uniform.lights[0] = ForwardLightUniform {
            position_type: [0.0, 0.0, 0.0, 0.0],
            direction_range: [-0.5, -1.0, -0.25, 0.0],
            color_intensity: [1.0, 1.0, 1.0, 1.0],
            spot_angles: [1.0, 1.0, 0.0, 0.0],
        };
        count = 1;
    }

    uniform.params = [count as u32, 0, 0, 0];
    uniform
}

fn select_forward_lights(world: &RenderWorld) -> Vec<&RenderLight> {
    let mut selected = Vec::with_capacity(MAX_FORWARD_LIGHTS);
    let mut directional: Vec<&RenderLight> = world
        .lights
        .iter()
        .filter(|light| light.kind == RenderLightKind::Directional && light.intensity > 0.0)
        .collect();
    directional.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));

    selected.extend(directional.into_iter().take(MAX_DIRECTIONAL_LIGHTS));

    let remaining = MAX_FORWARD_LIGHTS.saturating_sub(selected.len());
    if remaining == 0 {
        return selected;
    }

    let mut local: Vec<(&RenderLight, f32)> = world
        .lights
        .iter()
        .filter(|light| light.kind != RenderLightKind::Directional)
        .filter_map(|light| local_light_score(world, light).map(|score| (light, score)))
        .collect();
    local.sort_by(|(_, a), (_, b)| b.total_cmp(a));

    selected.extend(local.into_iter().take(remaining).map(|(light, _)| light));
    selected
}

fn local_light_score(world: &RenderWorld, light: &RenderLight) -> Option<f32> {
    if light.intensity <= 0.0 || light.range <= 0.0 {
        return None;
    }

    let range = light.range.max(0.001);
    let camera = world.camera.as_ref();
    let Some(camera) = camera else {
        return Some(light.intensity * range);
    };

    let to_light = light.transform.translation - camera.transform.translation;
    let distance = to_light.length();
    if distance - range > camera.far {
        return None;
    }

    let distance_sq = to_light.length_squared().max(1.0);
    Some(light.intensity * range * range / distance_sq)
}

fn forward_light_uniform(light: &RenderLight) -> ForwardLightUniform {
    let light_type = match light.kind {
        RenderLightKind::Point => 1.0,
        RenderLightKind::Spot => 2.0,
        RenderLightKind::Directional => 0.0,
    };
    let direction = rotate_vec3(
        light.transform.rotation,
        engine_core::math::Vec3::new(0.0, 0.0, -1.0),
    )
    .normalized();
    let direction = if direction.length_squared() <= f32::EPSILON {
        engine_core::math::Vec3::new(0.0, -1.0, 0.0)
    } else {
        direction
    };
    let range = light.range.max(0.001);
    let outer_half_angle = (light.spot_angle.clamp(1.0, 179.0) * 0.5).to_radians();
    let inner_half_angle = outer_half_angle * 0.75;

    ForwardLightUniform {
        position_type: [
            light.transform.translation.x,
            light.transform.translation.y,
            light.transform.translation.z,
            light_type,
        ],
        direction_range: [direction.x, direction.y, direction.z, range],
        color_intensity: [
            light.color.x.clamp(0.0, 1.0),
            light.color.y.clamp(0.0, 1.0),
            light.color.z.clamp(0.0, 1.0),
            light.intensity.max(0.0),
        ],
        spot_angles: [inner_half_angle.cos(), outer_half_angle.cos(), 0.0, 0.0],
    }
}

fn rotate_vec3(
    rotation: engine_core::math::Quat,
    vector: engine_core::math::Vec3,
) -> engine_core::math::Vec3 {
    let q = engine_core::math::Vec3::new(rotation.x, rotation.y, rotation.z);
    let t = cross(q, vector) * 2.0;
    vector + t * rotation.w + cross(q, t)
}

const IDENTITY_MAT4: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn look_at_rh(
    eye: engine_core::math::Vec3,
    target: engine_core::math::Vec3,
    up: engine_core::math::Vec3,
) -> [[f32; 4]; 4] {
    let f = (target - eye).normalized();
    let r = cross(f, up).normalized();
    let u = cross(r, f);
    [
        [r.x, u.x, -f.x, 0.0],
        [r.y, u.y, -f.y, 0.0],
        [r.z, u.z, -f.z, 0.0],
        [-r.dot(eye), -u.dot(eye), f.dot(eye), 1.0],
    ]
}

fn cross(a: engine_core::math::Vec3, b: engine_core::math::Vec3) -> engine_core::math::Vec3 {
    engine_core::math::Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn perspective_rh(fov_y: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_y * 0.5).tan();
    let range_inv = 1.0 / (near - far);
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, (far + near) * range_inv, -1.0],
        [0.0, 0.0, 2.0 * far * near * range_inv, 0.0],
    ]
}

fn orthographic_rh(vertical_size: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let top = vertical_size * 0.5;
    let bottom = -top;
    let right = top * aspect;
    let left = -right;
    let range_inv = 1.0 / (near - far);
    [
        [2.0 / (right - left), 0.0, 0.0, 0.0],
        [0.0, 2.0 / (top - bottom), 0.0, 0.0],
        [0.0, 0.0, 2.0 * range_inv, 0.0],
        [
            -(right + left) / (right - left),
            -(top + bottom) / (top - bottom),
            (far + near) * range_inv,
            1.0,
        ],
    ]
}

fn mul_mat4(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            result[col][row] = a[0][row] * b[col][0]
                + a[1][row] * b[col][1]
                + a[2][row] * b[col][2]
                + a[3][row] * b[col][3];
        }
    }
    result
}

fn mul_mat4_vec3(m: &[[f32; 4]; 4], v: engine_core::math::Vec3) -> engine_core::math::Vec3 {
    let x = m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z + m[3][0];
    let y = m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z + m[3][1];
    let z = m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z + m[3][2];
    engine_core::math::Vec3::new(x, y, z)
}

fn orthographic_rh_custom(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> [[f32; 4]; 4] {
    let range_inv = 1.0 / (near - far);
    [
        [2.0 / (right - left), 0.0, 0.0, 0.0],
        [0.0, 2.0 / (top - bottom), 0.0, 0.0],
        [0.0, 0.0, range_inv, 0.0],
        [
            -(right + left) / (right - left),
            -(top + bottom) / (top - bottom),
            near * range_inv,
            1.0,
        ],
    ]
}

fn mesh_name(mesh: &DebugMesh) -> String {
    match mesh {
        DebugMesh::Cube => "debug/cube".to_string(),
        DebugMesh::Sphere(_) => "debug/sphere".to_string(),
        DebugMesh::Plane => "debug/plane".to_string(),
    }
}

// Cube vertices with hard normals (24 vertices, 4 per face × 6 faces).
const CUBE_VERTICES: &[Vertex] = &[
    // Front face (+Z)
    Vertex {
        position: [-0.5, -0.5, 0.5],
        normal: [0.0, 0.0, 1.0],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.5],
        normal: [0.0, 0.0, 1.0],
        uv: [1.0, 0.0],
    },
    Vertex {
        position: [0.5, 0.5, 0.5],
        normal: [0.0, 0.0, 1.0],
        uv: [1.0, 1.0],
    },
    Vertex {
        position: [-0.5, 0.5, 0.5],
        normal: [0.0, 0.0, 1.0],
        uv: [0.0, 1.0],
    },
    // Back face (-Z)
    Vertex {
        position: [0.5, -0.5, -0.5],
        normal: [0.0, 0.0, -1.0],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, -0.5],
        normal: [0.0, 0.0, -1.0],
        uv: [1.0, 0.0],
    },
    Vertex {
        position: [-0.5, 0.5, -0.5],
        normal: [0.0, 0.0, -1.0],
        uv: [1.0, 1.0],
    },
    Vertex {
        position: [0.5, 0.5, -0.5],
        normal: [0.0, 0.0, -1.0],
        uv: [0.0, 1.0],
    },
    // Right face (+X)
    Vertex {
        position: [0.5, -0.5, 0.5],
        normal: [1.0, 0.0, 0.0],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, -0.5],
        normal: [1.0, 0.0, 0.0],
        uv: [1.0, 0.0],
    },
    Vertex {
        position: [0.5, 0.5, -0.5],
        normal: [1.0, 0.0, 0.0],
        uv: [1.0, 1.0],
    },
    Vertex {
        position: [0.5, 0.5, 0.5],
        normal: [1.0, 0.0, 0.0],
        uv: [0.0, 1.0],
    },
    // Left face (-X)
    Vertex {
        position: [-0.5, -0.5, -0.5],
        normal: [-1.0, 0.0, 0.0],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.5],
        normal: [-1.0, 0.0, 0.0],
        uv: [1.0, 0.0],
    },
    Vertex {
        position: [-0.5, 0.5, 0.5],
        normal: [-1.0, 0.0, 0.0],
        uv: [1.0, 1.0],
    },
    Vertex {
        position: [-0.5, 0.5, -0.5],
        normal: [-1.0, 0.0, 0.0],
        uv: [0.0, 1.0],
    },
    // Top face (+Y)
    Vertex {
        position: [-0.5, 0.5, 0.5],
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [0.5, 0.5, 0.5],
        normal: [0.0, 1.0, 0.0],
        uv: [1.0, 0.0],
    },
    Vertex {
        position: [0.5, 0.5, -0.5],
        normal: [0.0, 1.0, 0.0],
        uv: [1.0, 1.0],
    },
    Vertex {
        position: [-0.5, 0.5, -0.5],
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 1.0],
    },
    // Bottom face (-Y)
    Vertex {
        position: [-0.5, -0.5, -0.5],
        normal: [0.0, -1.0, 0.0],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, -0.5],
        normal: [0.0, -1.0, 0.0],
        uv: [1.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.5],
        normal: [0.0, -1.0, 0.0],
        uv: [1.0, 1.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.5],
        normal: [0.0, -1.0, 0.0],
        uv: [0.0, 1.0],
    },
];

const CUBE_INDICES: &[u32] = &[
    0, 1, 2, 2, 3, 0, // front
    4, 5, 6, 6, 7, 4, // back
    8, 9, 10, 10, 11, 8, // right
    12, 13, 14, 14, 15, 12, // left
    16, 17, 18, 18, 19, 16, // top
    20, 21, 22, 22, 23, 20, // bottom
];

fn generate_mesh(mesh: &DebugMesh) -> (Vec<Vertex>, Vec<u32>) {
    match mesh {
        DebugMesh::Cube => generate_cube(),
        DebugMesh::Sphere(segments) => generate_sphere(*segments),
        DebugMesh::Plane => generate_plane(),
    }
}

fn generate_cube() -> (Vec<Vertex>, Vec<u32>) {
    (CUBE_VERTICES.to_vec(), CUBE_INDICES.to_vec())
}

fn generate_sphere(segments: u32) -> (Vec<Vertex>, Vec<u32>) {
    let segs = segments.max(3);
    let lat = segs;
    let lon = segs * 2;

    let mut vertices = Vec::with_capacity(((lat + 1) * (lon + 1)) as usize);
    let mut indices = Vec::with_capacity((lat * lon * 6) as usize);

    for i in 0..=lat {
        let v = i as f32 / lat as f32;
        let theta = v * std::f32::consts::PI;
        let y = theta.cos();
        let r = theta.sin();

        for j in 0..=lon {
            let u = j as f32 / lon as f32;
            let phi = u * 2.0 * std::f32::consts::PI;
            let x = r * phi.cos();
            let z = r * phi.sin();

            vertices.push(Vertex {
                position: [x * 0.5, y * 0.5, z * 0.5],
                normal: [x, y, z],
                uv: [u, v],
            });
        }
    }

    for i in 0..lat {
        for j in 0..lon {
            let a = i * (lon + 1) + j;
            let b = a + lon + 1;
            let c = a + 1;
            let d = b + 1;
            indices.push(a);
            indices.push(b);
            indices.push(c);
            indices.push(c);
            indices.push(b);
            indices.push(d);
        }
    }

    (vertices, indices)
}

fn generate_plane() -> (Vec<Vertex>, Vec<u32>) {
    let vertices = vec![
        Vertex {
            position: [-0.5, -0.5, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.5, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 1.0],
        },
    ];
    let indices = vec![0, 1, 2, 2, 3, 0];
    (vertices, indices)
}

fn generate_grid() -> Vec<Vertex> {
    let half = 50.0;
    let mut vertices = Vec::with_capacity(404);

    for i in -50..=50 {
        let x = i as f32;
        let alpha = if i % 5 == 0 { 0.35 } else { 0.15 };
        vertices.push(Vertex {
            position: [x, 0.0, -half],
            normal: [0.0, 1.0, 0.0],
            uv: [alpha, 0.0],
        });
        vertices.push(Vertex {
            position: [x, 0.0, half],
            normal: [0.0, 1.0, 0.0],
            uv: [alpha, 0.0],
        });
    }
    for i in -50..=50 {
        let z = i as f32;
        let alpha = if i % 5 == 0 { 0.35 } else { 0.15 };
        vertices.push(Vertex {
            position: [-half, 0.0, z],
            normal: [0.0, 1.0, 0.0],
            uv: [alpha, 0.0],
        });
        vertices.push(Vertex {
            position: [half, 0.0, z],
            normal: [0.0, 1.0, 0.0],
            uv: [alpha, 0.0],
        });
    }

    vertices
}

const FORWARD_SHADER: &str = r#"
// Group 0: scene-level uniforms
struct CameraUniform {
    view_projection: mat4x4<f32>,
    camera_position: vec4<f32>,
};

struct ForwardLight {
    position_type: vec4<f32>,
    direction_range: vec4<f32>,
    color_intensity: vec4<f32>,
    spot_angles: vec4<f32>,
};

struct LightingUniform {
    ambient: vec4<f32>,
    params: vec4<u32>,
    lights: array<ForwardLight, 8>,
};

struct CsmUniform {
    cascade_vps: array<mat4x4<f32>, 3>,
    cascade_splits: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> lighting: LightingUniform;
@group(0) @binding(2) var<uniform> csm: CsmUniform;
@group(0) @binding(3) var csm_shadow_0: texture_depth_2d;
@group(0) @binding(4) var csm_shadow_1: texture_depth_2d;
@group(0) @binding(5) var csm_shadow_2: texture_depth_2d;
@group(0) @binding(6) var csm_sampler: sampler_comparison;

// Group 1: material textures
@group(1) @binding(0) var base_color_tex: texture_2d<f32>;
@group(1) @binding(1) var normal_tex: texture_2d<f32>;
@group(1) @binding(2) var metallic_roughness_tex: texture_2d<f32>;
@group(1) @binding(3) var emissive_tex: texture_2d<f32>;
@group(1) @binding(4) var occlusion_tex: texture_2d<f32>;
@group(1) @binding(5) var mat_sampler: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) offset: vec3<f32>,
    @location(4) scale: vec3<f32>,
    @location(5) color: vec4<f32>,
    @location(6) rotation: vec4<f32>,
    @location(7) metallic: f32,
    @location(8) roughness: f32,
    @location(9) emissive: vec3<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) world_position: vec3<f32>,
    @location(4) metallic: f32,
    @location(5) roughness: f32,
    @location(6) emissive: vec3<f32>,
    @location(7) world_tangent: vec3<f32>,
    @location(8) world_bitangent: vec3<f32>,
};

const PI: f32 = 3.14159265359;
const EPSILON: f32 = 0.001;

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let ndoth = max(dot(n, h), 0.0);
    let ndoth2 = ndoth * ndoth;
    let denom = ndoth2 * (a2 - 1.0) + 1.0;
    return a2 / max(PI * denom * denom, EPSILON);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = r * r / 8.0;
    let ndotv = max(dot(n, v), 0.0);
    let ndotl = max(dot(n, l), 0.0);
    let g1v = ndotv / (ndotv * (1.0 - k) + k);
    let g1l = ndotl / (ndotl * (1.0 - k) + k);
    return g1v * g1l;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn aces_tonemap(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return saturate((color * (a * color + b)) / (color * (c * color + d) + e));
}

fn sample_csm_shadow(world_pos: vec3<f32>, view_depth: f32) -> f32 {
    var cascade_idx = 2u;
    if (view_depth < csm.cascade_splits.x) {
        cascade_idx = 0u;
    } else if (view_depth < csm.cascade_splits.y) {
        cascade_idx = 1u;
    }

    var shadow_coord: vec4<f32>;
    if (cascade_idx == 0u) {
        shadow_coord = csm.cascade_vps[0] * vec4<f32>(world_pos, 1.0);
    } else if (cascade_idx == 1u) {
        shadow_coord = csm.cascade_vps[1] * vec4<f32>(world_pos, 1.0);
    } else {
        shadow_coord = csm.cascade_vps[2] * vec4<f32>(world_pos, 1.0);
    }

    let ndc = shadow_coord.xyz / shadow_coord.w;
    let uv = ndc.xy * 0.5 + 0.5;
    let depth = ndc.z;

    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || depth < 0.0 || depth > 1.0) {
        return 1.0;
    }

    let bias = 0.0005;
    var shadow_factor = 0.0;
    let texel = 1.0 / 2048.0;
    for (var dx = -1; dx <= 1; dx++) {
        for (var dy = -1; dy <= 1; dy++) {
            let offset = vec2<f32>(f32(dx), f32(dy)) * texel;
            if (cascade_idx == 0u) {
                shadow_factor += textureSampleCompare(csm_shadow_0, csm_sampler, uv + offset, depth - bias);
            } else if (cascade_idx == 1u) {
                shadow_factor += textureSampleCompare(csm_shadow_1, csm_sampler, uv + offset, depth - bias);
            } else {
                shadow_factor += textureSampleCompare(csm_shadow_2, csm_sampler, uv + offset, depth - bias);
            }
        }
    }
    return shadow_factor / 9.0;
}

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    let scaled_position = input.position * input.scale;
    let rotated_position = scaled_position
        + 2.0 * cross(input.rotation.xyz, cross(input.rotation.xyz, scaled_position)
        + input.rotation.w * scaled_position);
    let world_pos = rotated_position + input.offset;
    out.position = camera.view_projection * vec4<f32>(world_pos, 1.0);

    let rotated_normal = input.normal
        + 2.0 * cross(input.rotation.xyz, cross(input.rotation.xyz, input.normal)
        + input.rotation.w * input.normal);
    let n = normalize(rotated_normal);

    // Compute tangent and bitangent from UV derivatives (flat plane approximation)
    // For general meshes, tangents should come from vertex data; this is a fallback.
    var t = vec3<f32>(1.0, 0.0, 0.0);
    if (abs(dot(n, t)) > 0.99) {
        t = vec3<f32>(0.0, 1.0, 0.0);
    }
    let b = normalize(cross(n, t));
    let tt = normalize(cross(b, n));

    out.world_normal = n;
    out.world_tangent = tt;
    out.world_bitangent = b;
    out.uv = input.uv;
    out.color = input.color;
    out.world_position = world_pos;
    out.metallic = input.metallic;
    out.roughness = input.roughness;
    out.emissive = input.emissive;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    // Sample material textures
    let tex_color = textureSample(base_color_tex, mat_sampler, input.uv);
    let normal_sample = textureSample(normal_tex, mat_sampler, input.uv).rgb;
    let mra = textureSample(metallic_roughness_tex, mat_sampler, input.uv);
    let emissive_tex_color = textureSample(emissive_tex, mat_sampler, input.uv).rgb;
    let ao = textureSample(occlusion_tex, mat_sampler, input.uv).r;

    // Base color: vertex tint * texture
    let base_color = input.color.rgb * tex_color.rgb;

    // PBR parameters: per-instance fallback * texture modulation
    let metallic = clamp(input.metallic * mra.b, 0.0, 1.0);
    let roughness = clamp(input.roughness * mra.g, 0.04, 1.0);

    // Normal mapping: reconstruct TBN and transform sampled normal
    let tbn_t = normalize(input.world_tangent);
    let tbn_b = normalize(input.world_bitangent);
    let tbn_n = normalize(input.world_normal);
    let tbn = mat3x3<f32>(tbn_t, tbn_b, tbn_n);
    let tangent_normal = normalize(normal_sample * 2.0 - 1.0);
    let n = normalize(tbn * tangent_normal);

    let v = normalize(camera.camera_position.xyz - input.world_position);

    let f0 = mix(vec3<f32>(0.04), base_color, metallic);

    // Ambient with occlusion
    var color = lighting.ambient.rgb * base_color * ao;

    // CSM shadow
    let view_pos = camera.view_projection * vec4<f32>(input.world_position, 1.0);
    let view_depth = view_pos.z / view_pos.w;
    let shadow_factor = sample_csm_shadow(input.world_position, view_depth);

    for (var i: u32 = 0u; i < lighting.params.x; i = i + 1u) {
        let light = lighting.lights[i];
        let light_type = light.position_type.w;
        let light_color = light.color_intensity.rgb;
        let intensity = light.color_intensity.w;
        var light_dir = vec3<f32>(0.0, 1.0, 0.0);
        var attenuation = 1.0;
        var spot = 1.0;

        if (light_type < 0.5) {
            light_dir = normalize(-light.direction_range.xyz);
        } else {
            let to_light = light.position_type.xyz - input.world_position;
            let distance = length(to_light);
            light_dir = to_light / max(distance, EPSILON);
            let range = max(light.direction_range.w, EPSILON);
            let falloff = max(1.0 - distance / range, 0.0);
            attenuation = falloff * falloff;

            if (light_type > 1.5) {
                let spot_alignment = dot(normalize(-light_dir), normalize(light.direction_range.xyz));
                spot = smoothstep(light.spot_angles.y, light.spot_angles.x, spot_alignment);
            }
        }

        let ndotl = max(dot(n, light_dir), 0.0);
        if (ndotl <= 0.0) {
            continue;
        }

        let h = normalize(v + light_dir);
        let ndotv = max(dot(n, v), 0.0);
        let vdoth = max(dot(v, h), 0.0);

        let d = distribution_ggx(n, h, roughness);
        let g = geometry_smith(n, v, light_dir, roughness);
        let f = fresnel_schlick(vdoth, f0);

        let specular = (d * g * f) / max(4.0 * ndotv * ndotl, EPSILON);
        let kd = (1.0 - f) * (1.0 - metallic);
        let diffuse = kd * base_color / PI;

        var radiance = (diffuse + specular) * light_color * intensity * ndotl;

        if (light_type < 0.5) {
            radiance = radiance * shadow_factor;
        }

        color = color + radiance * attenuation * spot;
    }

    // Emissive: per-instance factor * emissive texture
    color = color + input.emissive * emissive_tex_color;

    color = aces_tonemap(color);

    let alpha = input.color.a * tex_color.a;
    return vec4<f32>(color, alpha);
}
"#;

const GRID_SHADER: &str = r#"
struct CameraUniform {
    view_projection: mat4x4<f32>,
    camera_position: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) alpha_factor: f32,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.position = camera.view_projection * vec4<f32>(input.position, 1.0);
    out.world_pos = input.position;
    out.alpha_factor = input.uv.x;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let half_extent = 50.0;
    let fade_start = half_extent * 0.7;
    let dist = length(input.world_pos.xz);
    let fade = 1.0 - smoothstep(fade_start, half_extent, dist);
    let alpha = input.alpha_factor * fade;
    return vec4<f32>(vec3<f32>(0.6), alpha);
}
"#;

const SHADOW_SHADER: &str = r#"
struct ShadowUniform {
    light_view_projection: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> shadow: ShadowUniform;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) offset: vec3<f32>,
    @location(4) scale: vec3<f32>,
    @location(5) color: vec4<f32>,
    @location(6) rotation: vec4<f32>,
    @location(7) metallic: f32,
    @location(8) roughness: f32,
    @location(9) emissive: vec3<f32>,
};

@vertex
fn vs_main(input: VsIn) -> @builtin(position) vec4<f32> {
    let scaled_position = input.position * input.scale;
    let rotated_position = scaled_position
        + 2.0 * cross(input.rotation.xyz, cross(input.rotation.xyz, scaled_position)
        + input.rotation.w * scaled_position);
    let world_pos = rotated_position + input.offset;
    return shadow.light_view_projection * vec4<f32>(world_pos, 1.0);
}
"#;

const SKYBOX_SHADER: &str = r#"
struct SkyboxUniform {
    view_rotation_only: mat4x4<f32>,
    zenith_color: vec4<f32>,
    horizon_color: vec4<f32>,
    rotation_intensity: vec4<f32>,
    use_cubemap: vec4<u32>,
};

@group(0) @binding(0) var<uniform> skybox: SkyboxUniform;
@group(0) @binding(1) var cubemap_texture: texture_cube<f32>;
@group(0) @binding(2) var cubemap_sampler: sampler;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) direction: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var out: VsOut;
    let x = f32(i32(vertex_index) - 1);
    let y = f32(i32(vertex_index & 1u) * 2 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    let inv_proj = vec4<f32>(x, y, 1.0, 1.0);
    let view_dir = (skybox.view_rotation_only * inv_proj).xyz;
    out.direction = view_dir;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let dir = normalize(input.direction);
    let rotation_rad = skybox.rotation_intensity.x * 3.14159265 / 180.0;
    let cos_r = cos(rotation_rad);
    let sin_r = sin(rotation_rad);
    let rotated_dir = vec3<f32>(
        dir.x * cos_r - dir.z * sin_r,
        dir.y,
        dir.x * sin_r + dir.z * cos_r
    );
    let intensity = skybox.rotation_intensity.y;
    var color: vec3<f32>;
    if (skybox.use_cubemap.x != 0u) {
        color = textureSample(cubemap_texture, cubemap_sampler, rotated_dir).rgb * intensity;
    } else {
        let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
        color = mix(skybox.horizon_color.rgb, skybox.zenith_color.rgb, t) * intensity;
    }
    return vec4<f32>(color, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_has_24_vertices_and_36_indices() {
        let (verts, indices) = generate_cube();
        assert_eq!(
            verts.len(),
            24,
            "cube must have 24 vertices with hard normals"
        );
        assert_eq!(
            indices.len(),
            36,
            "cube must have 36 indices (6 faces × 2 triangles × 3)"
        );
    }

    #[test]
    fn cube_vertices_have_correct_data() {
        let (verts, _indices) = generate_cube();
        // Front face vertices should have normal +Z
        for v in &verts[0..4] {
            assert!(
                (v.normal[2] - 1.0).abs() < 0.001,
                "front face normal should be +Z"
            );
        }
        // Back face vertices should have normal -Z
        for v in &verts[4..8] {
            assert!(
                (v.normal[2] + 1.0).abs() < 0.001,
                "back face normal should be -Z"
            );
        }
    }

    #[test]
    fn sphere_generates_expected_counts() {
        let (verts, indices) = generate_sphere(8);
        let expected_verts = (8 + 1) * (16 + 1); // lat+1 × lon+1
        let expected_indices = 8 * 16 * 6; // lat × lon × 6
        assert_eq!(verts.len(), expected_verts as usize);
        assert_eq!(indices.len(), expected_indices as usize);
    }

    #[test]
    fn sphere_min_segments_clamped() {
        let (verts, _) = generate_sphere(1);
        // Min segments is 3, so (3+1)*(6+1) = 28
        assert_eq!(verts.len(), 28);
    }

    #[test]
    fn plane_has_4_vertices_and_6_indices() {
        let (verts, indices) = generate_plane();
        assert_eq!(verts.len(), 4);
        assert_eq!(indices.len(), 6);
        // All normals point up
        for v in &verts {
            assert!((v.normal[2] - 1.0).abs() < 0.001);
        }
    }

    #[test]
    fn debug_mesh_enum_variants() {
        // Verify the enum can be constructed and matched
        let cube = DebugMesh::Cube;
        let sphere = DebugMesh::Sphere(8);
        let plane = DebugMesh::Plane;
        assert_eq!(cube, DebugMesh::Cube);
        assert_eq!(sphere, DebugMesh::Sphere(8));
        assert_eq!(plane, DebugMesh::Plane);
    }

    #[test]
    fn packs_scene_lights_into_forward_uniform() {
        let light = RenderLight {
            object: engine_core::EntityId::from_u128(7),
            transform: engine_core::math::Transform {
                translation: engine_core::math::Vec3::new(1.0, 2.0, 3.0),
                rotation: engine_core::math::Quat::IDENTITY,
                scale: engine_core::math::Vec3::ONE,
            },
            kind: RenderLightKind::Point,
            color: engine_core::math::Vec3::new(0.5, 0.75, 1.0),
            intensity: 3.0,
            range: 12.0,
            spot_angle: 45.0,
        };
        let world = RenderWorld {
            camera: None,
            objects: Vec::new(),
            sprites: Vec::new(),
            lights: vec![light],
            particles: vec![],
            skybox: None,
        };

        let uniform = lighting_uniform_from_world(&world);

        assert_eq!(uniform.params[0], 1);
        assert_eq!(uniform.lights[0].position_type, [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(uniform.lights[0].color_intensity, [0.5, 0.75, 1.0, 3.0]);
        assert_eq!(uniform.lights[0].direction_range[3], 12.0);
    }

    #[test]
    fn mesh_batches_group_objects_without_per_object_mesh_names() {
        let world = RenderWorld {
            camera: None,
            objects: vec![
                test_render_object(1, "debug/cube"),
                test_render_object(2, "debug/cube"),
                test_render_object(3, "debug/sphere"),
            ],
            sprites: Vec::new(),
            lights: Vec::new(),
            particles: Vec::new(),
            skybox: None,
        };

        let batches = test_mesh_batches(&world);

        assert_eq!(batch_len(&batches, "debug/cube"), Some(2));
        assert_eq!(batch_len(&batches, "debug/sphere"), Some(1));
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn mesh_batches_merge_particles_with_plane_objects() {
        let world = RenderWorld {
            camera: None,
            objects: vec![test_render_object(1, "debug/plane")],
            sprites: Vec::new(),
            lights: Vec::new(),
            particles: vec![engine_render::RenderParticle {
                object: engine_core::EntityId::from_u128(2),
                transform: engine_core::math::Transform::IDENTITY,
                color: [1.0, 1.0, 1.0, 1.0],
                age_fraction: 0.5,
            }],
            skybox: None,
        };

        let batches = test_mesh_batches(&world);

        assert_eq!(batch_len(&batches, "debug/plane"), Some(2));
    }

    #[test]
    fn mesh_batches_render_sprites_as_colored_planes() {
        let mut transform = engine_core::math::Transform::IDENTITY;
        transform.rotation = engine_core::math::Quat::from_euler_deg(0.0, 0.0, 90.0);
        let world = RenderWorld {
            sprites: vec![engine_render::RenderSprite {
                object: engine_core::EntityId::from_u128(2),
                transform,
                texture: None,
                color: [0.2, 0.4, 0.6, 0.5],
                order_in_layer: 7,
                layer: "Default".to_string(),
                flip_h: true,
                flip_v: false,
            }],
            ..RenderWorld::default()
        };

        let batches = test_mesh_batches(&world);
        let instances = &batches
            .iter()
            .find(|(mesh, _)| mesh == "debug/plane")
            .unwrap()
            .1;

        assert_eq!(instances.len(), 1);
        assert!(instances[0].scale[0] < 0.0);
        assert_eq!(instances[0].color, [0.2, 0.4, 0.6, 0.5]);
        assert_ne!(instances[0].rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    fn batch_len(batches: &[(String, Vec<Instance>)], mesh: &str) -> Option<usize> {
        batches
            .iter()
            .find(|(name, _)| name == mesh)
            .map(|(_, instances)| instances.len())
    }

    fn test_mesh_batches(world: &RenderWorld) -> Vec<(String, Vec<Instance>)> {
        use std::collections::HashMap;
        let batch_capacity = (world.objects.len()
            + usize::from(!world.sprites.is_empty())
            + usize::from(!world.particles.is_empty()))
        .min(32);
        let mut batches: HashMap<&str, Vec<Instance>> = HashMap::with_capacity(batch_capacity);
        for object in &world.objects {
            let (color, metallic, roughness, emissive) = test_pbr(&object.material);
            let t = object.transform;
            let mesh = if object.mesh.is_empty() {
                "debug/cube"
            } else {
                object.mesh.as_str()
            };
            batches.entry(mesh).or_default().push(Instance {
                offset: [t.translation.x, t.translation.y, t.translation.z],
                scale: [
                    t.scale.x.max(0.05),
                    t.scale.y.max(0.05),
                    t.scale.z.max(0.05),
                ],
                color,
                rotation: [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w],
                metallic,
                roughness,
                emissive,
            });
        }
        if !world.sprites.is_empty() {
            let mut sprites = world.sprites.iter().collect::<Vec<_>>();
            sprites.sort_by(|left, right| {
                left.layer
                    .cmp(&right.layer)
                    .then(left.order_in_layer.cmp(&right.order_in_layer))
            });
            let sprite_instances = sprites.into_iter().map(|sprite| {
                let t = sprite.transform;
                let x = t.scale.x.abs().max(0.01) * if sprite.flip_h { -1.0 } else { 1.0 };
                let y = t.scale.y.abs().max(0.01) * if sprite.flip_v { -1.0 } else { 1.0 };
                Instance {
                    offset: [
                        t.translation.x,
                        t.translation.y,
                        t.translation.z + sprite.order_in_layer as f32 * 0.0001,
                    ],
                    scale: [x, y, t.scale.z.abs().max(0.01)],
                    color: sprite.color,
                    rotation: [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w],
                    metallic: 0.0,
                    roughness: 0.5,
                    emissive: [0.0; 3],
                }
            });
            batches
                .entry("debug/plane")
                .or_default()
                .extend(sprite_instances);
        }
        if !world.particles.is_empty() {
            let particle_instances: Vec<Instance> = world
                .particles
                .iter()
                .map(|particle| {
                    let t = particle.transform;
                    Instance {
                        offset: [t.translation.x, t.translation.y, t.translation.z],
                        scale: [
                            t.scale.x.max(0.01),
                            t.scale.y.max(0.01),
                            t.scale.z.max(0.01),
                        ],
                        color: particle.color,
                        rotation: [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w],
                        metallic: 0.0,
                        roughness: 0.5,
                        emissive: [0.0; 3],
                    }
                })
                .collect();
            batches
                .entry("debug/plane")
                .or_default()
                .extend(particle_instances);
        }
        batches
            .into_iter()
            .map(|(mesh, instances)| (mesh.to_owned(), instances))
            .collect()
    }

    fn test_pbr(material: &str) -> ([f32; 4], f32, f32, [f32; 3]) {
        if material.contains("debug") {
            ([0.2, 0.65, 1.0, 1.0], 0.0, 0.5, [0.0, 0.0, 0.0])
        } else if material.contains("error") {
            ([1.0, 0.2, 0.45, 1.0], 0.0, 0.5, [0.0, 0.0, 0.0])
        } else {
            ([0.82, 0.86, 0.72, 1.0], 0.0, 0.5, [0.0, 0.0, 0.0])
        }
    }

    fn test_render_object(id: u128, mesh: &str) -> engine_render::RenderObject {
        engine_render::RenderObject {
            object: engine_core::EntityId::from_u128(id),
            transform: engine_core::math::Transform::IDENTITY,
            mesh: mesh.to_owned(),
            material: "debug/material".to_owned(),
        }
    }

    #[test]
    fn uses_fallback_directional_light_when_scene_has_no_lights() {
        let uniform = lighting_uniform_from_world(&RenderWorld::default());

        assert_eq!(uniform.params[0], 1);
        assert_eq!(uniform.lights[0].position_type[3], 0.0);
        assert_eq!(uniform.lights[0].color_intensity[3], 1.0);
    }

    #[test]
    fn selects_directional_budget_then_highest_scored_local_lights() {
        let camera = engine_render::RenderCamera {
            object: engine_core::EntityId::from_u128(1),
            transform: engine_core::math::Transform::IDENTITY,
            projection: engine_render::RenderProjection::Perspective,
            vertical_fov_degrees: 60.0,
            near: 0.1,
            far: 50.0,
            look_at_target: None,
        };
        let mut lights = vec![
            test_light(
                2,
                RenderLightKind::Directional,
                engine_core::math::Vec3::ZERO,
                1.0,
                1.0,
            ),
            test_light(
                3,
                RenderLightKind::Directional,
                engine_core::math::Vec3::ZERO,
                5.0,
                1.0,
            ),
            test_light(
                4,
                RenderLightKind::Directional,
                engine_core::math::Vec3::ZERO,
                3.0,
                1.0,
            ),
            test_light(
                5,
                RenderLightKind::Point,
                engine_core::math::Vec3::new(100.0, 0.0, 0.0),
                100.0,
                4.0,
            ),
        ];
        for index in 0..10 {
            lights.push(test_light(
                10 + index,
                RenderLightKind::Point,
                engine_core::math::Vec3::new(2.0 + index as f32, 0.0, 0.0),
                1.0,
                5.0,
            ));
        }
        let world = RenderWorld {
            camera: Some(camera),
            objects: Vec::new(),
            sprites: Vec::new(),
            lights,
            particles: Vec::new(),
            skybox: None,
        };

        let selected = select_forward_lights(&world);

        assert_eq!(selected.len(), MAX_FORWARD_LIGHTS);
        assert_eq!(selected[0].object, engine_core::EntityId::from_u128(3));
        assert_eq!(selected[1].object, engine_core::EntityId::from_u128(4));
        assert!(selected
            .iter()
            .all(|light| light.object != engine_core::EntityId::from_u128(5)));
        assert!(selected
            .iter()
            .any(|light| light.object == engine_core::EntityId::from_u128(10)));
    }

    fn test_light(
        id: u128,
        kind: RenderLightKind,
        translation: engine_core::math::Vec3,
        intensity: f32,
        range: f32,
    ) -> RenderLight {
        RenderLight {
            object: engine_core::EntityId::from_u128(id),
            transform: engine_core::math::Transform {
                translation,
                rotation: engine_core::math::Quat::IDENTITY,
                scale: engine_core::math::Vec3::ONE,
            },
            kind,
            color: engine_core::math::Vec3::ONE,
            intensity,
            range,
            spot_angle: 45.0,
        }
    }

    #[test]
    fn grid_generates_404_vertices() {
        let vertices = generate_grid();
        assert_eq!(
            vertices.len(),
            404,
            "grid must have 404 vertices (202 lines × 2)"
        );
    }

    #[test]
    fn grid_vertices_lie_on_y_zero() {
        let vertices = generate_grid();
        for v in &vertices {
            assert!(
                (v.position[1] - 0.0).abs() < f32::EPSILON,
                "every grid vertex must lie on Y=0"
            );
        }
    }

    #[test]
    fn grid_major_lines_have_alpha_0_35() {
        let vertices = generate_grid();
        assert!((vertices[0].uv[0] - 0.35).abs() < 0.001);
        assert!((vertices[202].uv[0] - 0.35).abs() < 0.001);
    }

    #[test]
    fn grid_minor_lines_have_alpha_0_15() {
        let vertices = generate_grid();
        assert!((vertices[2].uv[0] - 0.15).abs() < 0.001);
    }

    #[test]
    fn grid_vertices_within_extent() {
        let vertices = generate_grid();
        for v in &vertices {
            assert!(v.position[0].abs() <= 50.0 + f32::EPSILON);
            assert!(v.position[2].abs() <= 50.0 + f32::EPSILON);
        }
    }
}
