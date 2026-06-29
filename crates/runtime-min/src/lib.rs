#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Minimal Varg runtime and first playable game runner.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use engine_assets::{AssetDatabase, AssetRegistry, MaterialFormat, ModelResource};
#[cfg(feature = "asset-import")]
use engine_assets::{
    AssetGuid, DecodedCubemapResource, DecodedTextureResource, GpuResource, HotReloadTracker,
    ImportTask, ResourceKind, import_builtin_asset, scan_project_assets,
};
#[cfg(feature = "audio")]
use engine_audio::{
    AcousticAabb, AcousticMaterial, AcousticSceneSnapshot, AcousticSolverConfig,
    AcousticSourceSample, AttenuationModel, AudioContext, AudioListenerDesc, AudioObjectTransform,
    AudioSourceDesc, AudioSourceShape, ClipHandle, HrtfQuality, MemoryAudioBackend, OutputMode,
    SourceHandle, SpatialMode, VirtualizationPolicy, VoiceCategory, solve_direct_propagation,
    synth::{Waveform, generate_tone},
};
use engine_core::math::{Transform, Vec3};
use engine_core::{EngineConfig, EngineError, EngineResult, FrameCounter, TimeState, logging};
#[cfg(feature = "audio")]
use engine_ecs::AudioSourceComponentData;
use engine_ecs::{
    BuildConfiguration, ColliderComponentData, ComponentData, MaterialRef,
    MeshRendererComponentData, ProjectManifest, Scene, ScriptComponent, project_manifest_path,
};
#[cfg(feature = "physics")]
use engine_ecs::{BuoyancyProbeSetComponentData, FluidVolumeComponentData};
#[cfg(feature = "physics")]
use engine_physics::{
    BodyHandle, BodyKind, BuoyancyBodySample, BuoyancyProbeSet, CharacterControllerDesc,
    ColliderDesc, ColliderShape, ColliderShapeRef, FluidForce, FluidSurfaceModel, FluidVolumeDesc,
    FluidVolumeSample, PhysicsBackend, PhysicsWorld, QueryFilter, RapierPhysicsBackend,
    RigidbodyDesc, built_in_physical_material, collider_displacement_volume, solve_probe_buoyancy,
    solve_volume_buoyancy,
};
#[cfg(feature = "runtime-game")]
use engine_platform::GamepadProvider;
use engine_platform::{
    ActionMap, AxisType, DeadZone, GamepadAxis, GamepadButton, InputBindingV2, InputMap,
    InputModifier, InputState, InputTrigger, KeyCode, MouseButton,
};
#[cfg(feature = "asset-import")]
use engine_render::RenderMaterialTextures;
use engine_render::{
    AntiAliasingMode, BatteryPolicy, FrameGenerationKind, GuiDrawCmd, GuiDrawList, GuiTextureId,
    GuiVertex, HeadlessRenderDevice, ImageDesc, ImageFormat, ImageHandle, ImageUsage,
    PresentStrategy, RenderApi, RenderDevice, RenderEnvironment, RenderFog, RenderFrame,
    RenderGlobalIllumination, RenderGraph, RenderGraphBuilder, RenderPerformanceConfig,
    RenderPlatformClass, RenderProbeVolume, RenderQualityMode, RenderScalingContext,
    RenderScalingSettings, RenderWorld, ThermalState, UiCompositionPolicy, UpscalerKind,
};
#[cfg(feature = "wgpu")]
pub use engine_render_wgpu::WgpuRenderDevice;
use engine_script_varg::{
    VargAudioCommand, VargDestroyNearestRequest, VargRenderCommand, VargRuntimeContextRef,
    VargSceneBounds, VargSceneContext, VargScript, VargSpawnRequest, VargUiCommand,
    compile_script_source, compile_vscene_source_to_scene,
};
#[cfg(feature = "audio")]
use std::collections::HashSet;

/// Explicit runtime services. There is no hidden global mutable state.
mod audio;
mod input;
mod physics;
mod project;
mod render;
mod run;
mod scene;
mod services;
mod ui;
mod weather;

#[cfg(feature = "audio")]
use audio::*;
use input::*;
#[cfg(feature = "physics")]
use physics::*;
use render::*;
use scene::*;
use ui::*;
use weather::*;

#[cfg(feature = "runtime-game")]
pub use input::apply_winit_input_capture;
pub use project::{RuntimeProject, load_runtime_project};
pub use render::{
    build_default_render_graph, render_scaling_settings_from_build,
    runtime_performance_config_from_env, runtime_scaling_context,
    runtime_scaling_settings_from_env, smoke_runtime_min,
};
pub use run::run_project;
pub use scene::extract_render_world;
pub use services::{
    RuntimeDiagnostic, RuntimeInputCapture, RuntimeRenderEnvironment, RuntimeServices,
    RuntimeStats, RuntimeUserPreferences, RuntimeWeatherState, headless_services_from_scene,
};
