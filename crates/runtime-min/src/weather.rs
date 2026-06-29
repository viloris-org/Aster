use super::*;

pub(crate) fn apply_runtime_weather_environment(
    world: &mut RenderWorld,
    weather: &RuntimeWeatherState,
) {
    if !weather.enabled {
        return;
    }

    let mut environment = world.resolved_environment();
    blend_weather_into_environment(&mut environment, weather);
    world.environment = Some(environment.clone());
    world.skybox = environment
        .sky_enabled
        .then_some(engine_render::RenderSkybox {
            cubemap: environment.sky_cubemap.clone(),
            zenith_color: environment.sky_zenith_color,
            horizon_color: environment.sky_horizon_color,
            rotation_degrees: environment.sky_rotation_degrees,
            intensity: environment.sky_intensity,
        });
    world.fog = environment.fog.enabled.then_some(environment.fog);
}

fn blend_weather_into_environment(
    environment: &mut RenderEnvironment,
    weather: &RuntimeWeatherState,
) {
    let preset = normalize_weather_preset(&weather.preset);
    let cloud_cover = weather.cloud_cover.clamp(0.0, 1.0);
    let precipitation = weather.precipitation.clamp(0.0, 1.0);
    let sun = daylight_factor(weather.time_of_day);
    let dusk = dusk_factor(weather.time_of_day);

    let clear_zenith = mix_color([0.035, 0.05, 0.09], [0.18, 0.42, 0.78], sun);
    let clear_horizon = mix_color([0.09, 0.08, 0.12], [0.62, 0.75, 0.92], sun);
    let storm_zenith = [0.055, 0.065, 0.08];
    let storm_horizon = [0.18, 0.2, 0.22];
    let dusk_tint = [1.0, 0.55, 0.28];

    let storm_mix = match preset.as_str() {
        "storm" => 0.85_f32,
        "rain" => 0.65_f32,
        "overcast" => 0.5_f32,
        "night" => 0.25_f32,
        _ => 0.0_f32,
    }
    .max(cloud_cover * 0.75)
    .max(precipitation * 0.7)
    .clamp(0.0, 1.0);

    environment.sky_enabled = true;
    environment.sky_cubemap = None;
    environment.sky_zenith_color = mix_color(clear_zenith, storm_zenith, storm_mix);
    environment.sky_horizon_color = mix_color(clear_horizon, storm_horizon, storm_mix);
    environment.sky_horizon_color =
        mix_color(environment.sky_horizon_color, dusk_tint, dusk * 0.35);
    environment.sky_rotation_degrees = weather.wind.x.atan2(weather.wind.z).to_degrees();
    environment.sky_intensity =
        (0.18 + sun * 0.95) * (1.0 - cloud_cover * 0.42) * (1.0 - precipitation * 0.18);

    let ambient = 0.18 + sun * 0.82;
    environment.ambient_color = mix_color([0.025, 0.03, 0.04], [0.08, 0.09, 0.1], sun);
    environment.ambient_intensity = ambient * (1.0 - cloud_cover * 0.35);

    let fog_density = 0.00025 + cloud_cover * 0.0008 + precipitation * 0.0014;
    environment.fog = RenderFog {
        enabled: fog_density > 0.0003,
        density: fog_density,
        color: mix_color(
            environment.sky_horizon_color,
            [0.12, 0.13, 0.15],
            precipitation * 0.5,
        ),
    };
    environment.exposure = (0.78 + sun * 0.42).max(0.45) * (1.0 - cloud_cover * 0.15);
    environment.bloom_intensity = (0.03 + dusk * 0.08) * (1.0 - precipitation * 0.3);
    environment.ssgi_intensity = (0.35 + sun * 0.35) * (1.0 - cloud_cover * 0.2);
    environment.ssr_intensity = if precipitation > 0.0 {
        (0.22 + precipitation * 0.28).min(0.65)
    } else {
        environment.ssr_intensity
    };
}

pub(crate) fn normalize_weather_preset(preset: &str) -> String {
    match preset.trim().to_ascii_lowercase().as_str() {
        "overcast" | "cloudy" => "overcast".to_string(),
        "rain" | "rainy" => "rain".to_string(),
        "storm" | "stormy" | "thunder" => "storm".to_string(),
        "night" => "night".to_string(),
        _ => "clear".to_string(),
    }
}

pub(crate) fn apply_weather_preset_defaults(weather: &mut RuntimeWeatherState) {
    match weather.preset.as_str() {
        "overcast" => {
            weather.cloud_cover = weather.cloud_cover.max(0.75);
            weather.precipitation = weather.precipitation.min(0.2);
        }
        "rain" => {
            weather.cloud_cover = weather.cloud_cover.max(0.82);
            weather.precipitation = weather.precipitation.max(0.45);
        }
        "storm" => {
            weather.cloud_cover = weather.cloud_cover.max(0.95);
            weather.precipitation = weather.precipitation.max(0.75);
        }
        "night" => {
            weather.time_of_day = 22.0;
            weather.cloud_cover = weather.cloud_cover.max(0.2);
        }
        _ => {
            weather.cloud_cover = weather.cloud_cover.min(0.25);
            weather.precipitation = weather.precipitation.min(0.05);
        }
    }
}

pub(crate) fn normalize_time_of_day(time_of_day: f32) -> f32 {
    time_of_day.rem_euclid(24.0)
}

fn daylight_factor(time_of_day: f32) -> f32 {
    let hour = normalize_time_of_day(time_of_day);
    let angle = ((hour - 6.0) / 12.0) * std::f32::consts::PI;
    angle.sin().clamp(0.0, 1.0)
}

fn dusk_factor(time_of_day: f32) -> f32 {
    let hour = normalize_time_of_day(time_of_day);
    let sunrise = (1.0 - ((hour - 6.0).abs() / 2.0)).clamp(0.0, 1.0);
    let sunset = (1.0 - ((hour - 18.0).abs() / 2.0)).clamp(0.0, 1.0);
    sunrise.max(sunset)
}

fn mix_color(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] * (1.0 - t) + b[0] * t,
        a[1] * (1.0 - t) + b[1] * t,
        a[2] * (1.0 - t) + b[2] * t,
    ]
}
