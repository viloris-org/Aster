#[cfg(feature = "audio")]
use super::*;

#[cfg(feature = "audio")]
pub(crate) fn parse_spatial_mode(value: &str) -> SpatialMode {
    match value {
        "object" => SpatialMode::Object,
        "ambient_field" => SpatialMode::AmbientField,
        _ => SpatialMode::Direct,
    }
}

#[cfg(feature = "audio")]
pub(crate) fn parse_audio_source_shape(source: &AudioSourceComponentData) -> AudioSourceShape {
    match source.shape.as_str() {
        "cone" => AudioSourceShape::Cone {
            inner_angle_degrees: source.inner_angle_degrees,
            outer_angle_degrees: source.outer_angle_degrees.max(source.inner_angle_degrees),
            outer_gain: source.outer_gain.clamp(0.0, 1.0),
        },
        "sphere" => AudioSourceShape::Sphere {
            radius: source.sphere_radius.max(0.0),
        },
        _ => AudioSourceShape::Point,
    }
}

#[cfg(feature = "audio")]
pub(crate) fn parse_synth_waveform(value: &str) -> Waveform {
    match value.to_ascii_lowercase().as_str() {
        "square" => Waveform::Square,
        "saw" | "sawtooth" => Waveform::Sawtooth,
        "triangle" | "tri" => Waveform::Triangle,
        "noise" | "white_noise" | "white-noise" => Waveform::Noise,
        _ => Waveform::Sine,
    }
}

#[cfg(feature = "audio")]
pub(crate) fn generate_loop_pattern(
    waveform: Waveform,
    pattern: &str,
    bpm: f32,
    beats_per_note: f32,
    volume: f32,
    sample_rate: u32,
) -> EngineResult<Vec<f32>> {
    let bpm = bpm.clamp(30.0, 300.0);
    let beats_per_note = beats_per_note.clamp(0.0625, 8.0);
    let volume = volume.clamp(0.0, 1.0);
    let note_seconds = 60.0 / bpm * beats_per_note;
    let note_samples = (note_seconds * sample_rate as f32).round().max(1.0) as usize;
    let tokens = pattern
        .split(|ch: char| ch.is_whitespace() || ch == ',' || ch == '|')
        .filter(|token| !token.trim().is_empty())
        .take(128)
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return Err(EngineError::config(
            "Audio.startLoop pattern must contain at least one note or rest",
        ));
    }

    let mut samples = Vec::with_capacity(note_samples.saturating_mul(tokens.len()));
    for token in tokens {
        if is_rest_token(token) {
            samples.resize(samples.len() + note_samples, 0.0);
            continue;
        }
        let frequency = parse_note_frequency(token).ok_or_else(|| {
            EngineError::config(format!("unsupported Audio.startLoop note `{token}`"))
        })?;
        let mut note = generate_tone(waveform, frequency, note_seconds, volume, sample_rate);
        note.resize(note_samples, 0.0);
        samples.extend(note.into_iter().take(note_samples));
    }
    Ok(samples)
}

#[cfg(feature = "audio")]
fn is_rest_token(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "r" | "rest" | "-" | "_" | "0"
    )
}

#[cfg(feature = "audio")]
fn parse_note_frequency(token: &str) -> Option<f32> {
    if let Ok(frequency) = token.parse::<f32>() {
        return (frequency > 0.0).then_some(frequency.clamp(20.0, 20_000.0));
    }

    let token = token.trim();
    let mut chars = token.chars();
    let note = chars.next()?.to_ascii_uppercase();
    let base = match note {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let mut semitone = base;
    let mut rest = chars.as_str();
    if let Some(stripped) = rest.strip_prefix('#') {
        semitone += 1;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix('b') {
        semitone -= 1;
        rest = stripped;
    }
    let octave = rest.parse::<i32>().ok()?;
    let midi = (octave + 1) * 12 + semitone;
    let frequency = 440.0 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0);
    frequency
        .is_finite()
        .then_some(frequency.clamp(20.0, 20_000.0))
}

#[cfg(feature = "audio")]
pub(crate) fn parse_attenuation_model(source: &AudioSourceComponentData) -> AttenuationModel {
    match source.attenuation.as_str() {
        "inverse_distance" => AttenuationModel::InverseDistance {
            min_distance: source.min_distance.max(0.0),
            max_distance: source.max_distance.max(source.min_distance),
        },
        "linear_distance" => AttenuationModel::LinearDistance {
            min_distance: source.min_distance.max(0.0),
            max_distance: source.max_distance.max(source.min_distance),
        },
        "logarithmic_distance" => AttenuationModel::LogarithmicDistance {
            min_distance: source.min_distance.max(0.0),
            max_distance: source.max_distance.max(source.min_distance),
        },
        _ => AttenuationModel::None,
    }
}

#[cfg(feature = "audio")]
pub(crate) fn extract_acoustic_blockers(scene: &Scene) -> Vec<AcousticAabb> {
    scene
        .iter_objects()
        .filter_map(|(entity, object)| {
            let geometry = object
                .components
                .iter()
                .find_map(|component| match component {
                    ComponentData::AcousticGeometry(geometry) => Some(geometry),
                    _ => None,
                })?;
            let transform = scene.transforms().world(entity).unwrap_or_default();
            let material = geometry
                .material
                .as_ref()
                .map(acoustic_material_from_component)
                .or_else(|| {
                    object
                        .components
                        .iter()
                        .find_map(|component| match component {
                            ComponentData::AcousticMaterial(material) => {
                                Some(acoustic_material_from_component(material))
                            }
                            _ => None,
                        })
                })
                .unwrap_or_default();
            let half_size = geometry.size * 0.5;
            Some(AcousticAabb {
                min: transform.translation - half_size,
                max: transform.translation + half_size,
                material,
                blocks_direct_path: geometry.blocks_direct_path,
            })
        })
        .collect()
}

#[cfg(feature = "audio")]
fn acoustic_material_from_component(
    material: &engine_ecs::AcousticMaterialComponentData,
) -> AcousticMaterial {
    AcousticMaterial {
        absorption: material.absorption.map(|value| value.clamp(0.0, 1.0)),
        transmission: material.transmission.map(|value| value.clamp(0.0, 1.0)),
        scattering: material.scattering.clamp(0.0, 1.0),
    }
}

#[cfg(feature = "audio")]
pub(crate) fn parse_virtualization_policy(value: &str) -> VirtualizationPolicy {
    match value {
        "stop" => VirtualizationPolicy::Stop,
        "protected" => VirtualizationPolicy::Protected,
        _ => VirtualizationPolicy::Virtualize,
    }
}

#[cfg(feature = "audio")]
pub(crate) fn parse_voice_category(value: &str) -> VoiceCategory {
    match value {
        "critical" => VoiceCategory::Critical,
        "dialogue" => VoiceCategory::Dialogue,
        "music" => VoiceCategory::Music,
        "ui" => VoiceCategory::Ui,
        "ambience" => VoiceCategory::Ambience,
        "disposable" => VoiceCategory::Disposable,
        _ => VoiceCategory::Sfx,
    }
}

#[cfg(feature = "audio")]
pub(crate) fn parse_output_mode(value: &str) -> OutputMode {
    match value {
        "binaural" => OutputMode::Binaural,
        _ => OutputMode::Stereo,
    }
}

#[cfg(feature = "audio")]
pub(crate) fn parse_hrtf_quality(value: &str) -> HrtfQuality {
    match value {
        "low" => HrtfQuality::Low,
        "high" => HrtfQuality::High,
        _ => HrtfQuality::Medium,
    }
}
