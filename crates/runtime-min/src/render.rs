use super::*;

pub(crate) fn script_probe_count(value: f32) -> u32 {
    if !value.is_finite() {
        return 1;
    }
    value.round().clamp(1.0, 16.0) as u32
}

/// Builds the native runtime performance policy from environment overrides.
///
/// Supported variables are `VARG_PRESENT_MODE`, `VARG_TARGET_FPS`,
/// `VARG_RENDER_SCALE`, and `VARG_DYNAMIC_RESOLUTION`.
pub fn runtime_performance_config_from_env() -> RenderPerformanceConfig {
    let mut config = RenderPerformanceConfig::competitive_120hz();
    if let Ok(value) = std::env::var("VARG_PRESENT_MODE") {
        config.present_strategy = match value.as_str() {
            "vsync" => PresentStrategy::VSync,
            "uncapped" => PresentStrategy::Uncapped,
            _ => PresentStrategy::LowLatency,
        };
    }
    if let Ok(value) = std::env::var("VARG_TARGET_FPS") {
        if let Ok(target_fps) = value.parse::<u32>() {
            config.dynamic_resolution.target_fps = target_fps.max(1);
        }
    }
    if let Ok(value) = std::env::var("VARG_RENDER_SCALE") {
        if let Ok(scale) = value.parse::<f32>() {
            config.render_scale = scale.clamp(
                config.dynamic_resolution.min_scale,
                config.dynamic_resolution.max_scale,
            );
        }
    }
    if let Ok(value) = std::env::var("VARG_DYNAMIC_RESOLUTION") {
        config.dynamic_resolution.enabled = matches!(value.as_str(), "1" | "true" | "on");
    }
    config
}

/// Builds render scaling settings from environment overrides.
///
/// Supported variables are `VARG_UPSCALER`, `VARG_RENDER_QUALITY`,
/// `VARG_RENDER_SCALE_MIN`, `VARG_RENDER_SCALE_MAX`, `VARG_UPSCALE_SHARPNESS`,
/// `VARG_TARGET_FPS`, `VARG_DYNAMIC_RESOLUTION`, `VARG_BATTERY_POLICY`,
/// `VARG_FRAME_GENERATION`, `VARG_UI_COMPOSITION`, and `VARG_ANTI_ALIASING`.
pub fn runtime_scaling_settings_from_env() -> RenderScalingSettings {
    apply_runtime_scaling_env(RenderScalingSettings::default())
}

/// Converts persisted build settings to the engine render scaling model.
pub fn render_scaling_settings_from_build(build: &BuildConfiguration) -> RenderScalingSettings {
    let render = &build.render;
    let settings = RenderScalingSettings {
        quality: parse_render_quality(&render.quality),
        preferred_upscaler: Some(parse_upscaler(&render.upscaler)),
        dynamic_resolution: render.dynamic_resolution,
        min_render_scale: f32::from(render.min_render_scale_percent) / 100.0,
        max_render_scale: f32::from(render.max_render_scale_percent) / 100.0,
        sharpness: f32::from(render.sharpness_percent) / 100.0,
        target_fps: render.target_fps,
        battery_policy: parse_battery_policy(&render.battery_policy),
        frame_generation: parse_frame_generation(&render.frame_generation),
        ui_composition: parse_ui_composition(&render.ui_composition),
        anti_aliasing: parse_anti_aliasing(&render.anti_aliasing),
        ..RenderScalingSettings::default()
    };
    apply_runtime_scaling_env(settings)
}

fn apply_runtime_scaling_env(mut settings: RenderScalingSettings) -> RenderScalingSettings {
    if let Ok(value) = std::env::var("VARG_UPSCALER") {
        settings.preferred_upscaler = Some(parse_upscaler(&value));
    }
    if let Ok(value) = std::env::var("VARG_RENDER_QUALITY") {
        settings.quality = parse_render_quality(&value);
    }
    if let Ok(value) = std::env::var("VARG_RENDER_SCALE_MIN") {
        if let Ok(scale) = value.parse::<f32>() {
            settings.min_render_scale = scale;
        }
    }
    if let Ok(value) = std::env::var("VARG_RENDER_SCALE_MAX") {
        if let Ok(scale) = value.parse::<f32>() {
            settings.max_render_scale = scale;
        }
    }
    if let Ok(value) = std::env::var("VARG_UPSCALE_SHARPNESS") {
        if let Ok(sharpness) = value.parse::<f32>() {
            settings.sharpness = sharpness;
        }
    }
    if let Ok(value) = std::env::var("VARG_TARGET_FPS") {
        if let Ok(target_fps) = value.parse::<u32>() {
            settings.target_fps = target_fps;
        }
    }
    if let Ok(value) = std::env::var("VARG_DYNAMIC_RESOLUTION") {
        settings.dynamic_resolution = matches!(value.as_str(), "1" | "true" | "on");
    }
    if let Ok(value) = std::env::var("VARG_BATTERY_POLICY") {
        settings.battery_policy = parse_battery_policy(&value);
    }
    if let Ok(value) = std::env::var("VARG_FRAME_GENERATION") {
        settings.frame_generation = parse_frame_generation(&value);
    }
    if let Ok(value) = std::env::var("VARG_UI_COMPOSITION") {
        settings.ui_composition = parse_ui_composition(&value);
    }
    if let Ok(value) =
        std::env::var("VARG_ANTI_ALIASING").or_else(|_| std::env::var("VARG_RENDER_AA"))
    {
        settings.anti_aliasing = parse_anti_aliasing(&value);
    }
    settings.normalized()
}

fn parse_upscaler(value: &str) -> UpscalerKind {
    match value.to_ascii_lowercase().as_str() {
        "native" | "off" => UpscalerKind::Native,
        "temporal" => UpscalerKind::BuiltInTemporal,
        "fsr" => UpscalerKind::Fsr,
        "dlss" => UpscalerKind::Dlss,
        "xess" => UpscalerKind::Xess,
        "metalfx" => UpscalerKind::MetalFx,
        "gsr" | "snapdragon-gsr" => UpscalerKind::SnapdragonGsr,
        "directsr" => UpscalerKind::DirectSr,
        "streamline" => UpscalerKind::Streamline,
        _ => UpscalerKind::BuiltInSpatial,
    }
}

fn parse_render_quality(value: &str) -> RenderQualityMode {
    match value.to_ascii_lowercase().as_str() {
        "native" => RenderQualityMode::Native,
        "ultra-quality" => RenderQualityMode::UltraQuality,
        "quality" => RenderQualityMode::Quality,
        "performance" => RenderQualityMode::Performance,
        "ultra-performance" => RenderQualityMode::UltraPerformance,
        "auto" => RenderQualityMode::Auto,
        _ => RenderQualityMode::Balanced,
    }
}

fn parse_battery_policy(value: &str) -> BatteryPolicy {
    match value.to_ascii_lowercase().as_str() {
        "quality" => BatteryPolicy::Quality,
        "saver" => BatteryPolicy::Saver,
        _ => BatteryPolicy::Balanced,
    }
}

fn parse_frame_generation(value: &str) -> FrameGenerationKind {
    match value.to_ascii_lowercase().as_str() {
        "fsr" => FrameGenerationKind::Fsr,
        "dlss" => FrameGenerationKind::Dlss,
        "xess" => FrameGenerationKind::Xess,
        "metalfx" | "metal-fx" => FrameGenerationKind::MetalFx,
        _ => FrameGenerationKind::Disabled,
    }
}

fn parse_ui_composition(value: &str) -> UiCompositionPolicy {
    match value.to_ascii_lowercase().as_str() {
        "before-frame-generation" | "before-fg" => UiCompositionPolicy::BeforeFrameGeneration,
        "separate-texture" | "separate" => UiCompositionPolicy::SeparateTexture,
        _ => UiCompositionPolicy::AfterFrameGeneration,
    }
}

fn parse_anti_aliasing(value: &str) -> AntiAliasingMode {
    match value.to_ascii_lowercase().as_str() {
        "off" | "none" | "disabled" => AntiAliasingMode::Off,
        "taa" | "temporal" => AntiAliasingMode::Taa,
        _ => AntiAliasingMode::Taa,
    }
}

/// Detects broad runtime conditions used by automatic scaling policy.
pub fn runtime_scaling_context() -> RenderScalingContext {
    let platform = if cfg!(target_os = "android") {
        RenderPlatformClass::Android
    } else if cfg!(any(
        target_os = "ios",
        target_os = "tvos",
        target_os = "visionos"
    )) {
        RenderPlatformClass::AppleMobile
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
        RenderPlatformClass::WindowsOnArm
    } else {
        RenderPlatformClass::Desktop
    };
    let thermal_state = match std::env::var("VARG_THERMAL_STATE")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "warm" => ThermalState::Warm,
        "throttling" => ThermalState::Throttling,
        "critical" => ThermalState::Critical,
        _ => ThermalState::Nominal,
    };
    let battery_saver = std::env::var("VARG_BATTERY_SAVER")
        .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "on"));
    RenderScalingContext {
        platform,
        thermal_state,
        battery_saver,
    }
}

/// Builds the default forward render graph used by the minimal runtime.
pub fn build_default_render_graph() -> RenderGraph {
    use engine_render::RenderStage;

    let mut builder = RenderGraphBuilder::new();
    let shadow = builder.add_pass("shadow");
    let forward = builder.add_pass("forward");
    let temporal_inputs = builder.add_pass_at_stage("temporal-inputs", RenderStage::TemporalInputs);
    let upscale = builder.add_pass_at_stage("upscale", RenderStage::Upscale);
    let post = builder.add_pass_at_stage("post", RenderStage::PostUpscale);
    let ui = builder.add_pass_at_stage("ui", RenderStage::UiComposition);
    builder.order_before(shadow, forward);
    builder.order_before(forward, temporal_inputs);
    builder.order_before(temporal_inputs, upscale);
    builder.order_before(upscale, post);
    builder.order_before(post, ui);
    builder.build()
}

/// Runs a one-frame native smoke path for the minimal runtime.
pub fn smoke_runtime_min() -> EngineResult<u64> {
    let config = EngineConfig::default();
    logging::log_runtime_start(&config.app_name, config.profile.as_str());
    let mut services = RuntimeServices::minimal(config);
    services.tick()?;
    Ok(services.frame_index())
}
