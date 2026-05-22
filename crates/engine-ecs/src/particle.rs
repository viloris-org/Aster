//! Serializable particle emitter data and deterministic CPU sampling.

use engine_core::math::{Transform, Vec3};

/// Serializable particle emitter component.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ParticleEmitterComponentData {
    /// Maximum live particles sampled from this emitter.
    pub max_particles: u32,
    /// Particles emitted per second.
    pub emission_rate: f32,
    /// Lifetime of each particle in seconds.
    pub lifetime: f32,
    /// Initial particle speed.
    pub start_speed: f32,
    /// Initial particle size.
    pub start_size: f32,
    /// Particle size at death.
    #[serde(default = "default_end_size")]
    pub end_size: f32,
    /// RGB color at birth.
    pub start_color: Vec3,
    /// RGB color at death.
    #[serde(default = "default_end_color")]
    pub end_color: Vec3,
    /// Acceleration applied over particle lifetime.
    #[serde(default = "default_gravity")]
    pub gravity: Vec3,
    /// Direction spread in degrees around the emitter up axis.
    #[serde(default = "default_spread_degrees")]
    pub spread_degrees: f32,
    /// Whether emission wraps after the live window.
    pub looping: bool,
    /// Deterministic sampling seed.
    #[serde(default = "default_particle_seed")]
    pub seed: u32,
    /// Simulated time in seconds.
    #[serde(default)]
    pub elapsed: f32,
}

fn default_end_size() -> f32 {
    0.02
}

fn default_end_color() -> Vec3 {
    Vec3::new(1.0, 0.35, 0.08)
}

fn default_gravity() -> Vec3 {
    Vec3::new(0.0, -9.8, 0.0)
}

fn default_spread_degrees() -> f32 {
    35.0
}

fn default_particle_seed() -> u32 {
    1
}

impl Default for ParticleEmitterComponentData {
    fn default() -> Self {
        Self {
            max_particles: 128,
            emission_rate: 32.0,
            lifetime: 2.0,
            start_speed: 2.5,
            start_size: 0.12,
            end_size: default_end_size(),
            start_color: Vec3::new(1.0, 0.85, 0.25),
            end_color: default_end_color(),
            gravity: default_gravity(),
            spread_degrees: default_spread_degrees(),
            looping: true,
            seed: default_particle_seed(),
            elapsed: 0.0,
        }
    }
}

impl ParticleEmitterComponentData {
    /// Advances emitter time.
    pub fn tick(&mut self, delta_seconds: f32) {
        if delta_seconds.is_finite() && delta_seconds > 0.0 {
            self.elapsed = (self.elapsed + delta_seconds).max(0.0);
        }
    }
}

/// Runtime particle instance generated from an emitter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleInstance {
    /// Particle world-space position.
    pub position: Vec3,
    /// Particle display size.
    pub size: f32,
    /// Particle color as RGBA.
    pub color: [f32; 4],
    /// Normalized lifetime age, from 0 at birth to 1 at death.
    pub age_fraction: f32,
}

/// Stateless deterministic CPU particle sampler.
#[derive(Clone, Copy, Debug, Default)]
pub struct ParticleSystem;

impl ParticleSystem {
    /// Samples live particles for an emitter at its current elapsed time.
    pub fn sample(
        emitter: &ParticleEmitterComponentData,
        transform: Transform,
    ) -> Vec<ParticleInstance> {
        if emitter.max_particles == 0
            || emitter.emission_rate <= 0.0
            || emitter.lifetime <= f32::EPSILON
        {
            return Vec::new();
        }

        let live_capacity = emitter
            .max_particles
            .min((emitter.emission_rate * emitter.lifetime).ceil() as u32)
            .max(1);
        let emitted = if emitter.looping {
            live_capacity
        } else {
            ((emitter.elapsed * emitter.emission_rate).floor() as u32).min(live_capacity)
        };
        let interval = 1.0 / emitter.emission_rate;
        let live_window = interval * live_capacity as f32;
        let phase_time = if emitter.looping && live_window > f32::EPSILON {
            emitter.elapsed.rem_euclid(live_window)
        } else {
            emitter.elapsed
        };

        let mut particles = Vec::with_capacity(emitted as usize);
        for index in 0..emitted {
            let age = if emitter.looping {
                (phase_time - index as f32 * interval).rem_euclid(live_window)
            } else {
                phase_time - index as f32 * interval
            };
            if !(0.0..=emitter.lifetime).contains(&age) {
                continue;
            }

            let t = (age / emitter.lifetime).clamp(0.0, 1.0);
            let direction = particle_direction(emitter.seed, index, emitter.spread_degrees);
            let velocity = direction * emitter.start_speed;
            let position =
                transform.translation + velocity * age + emitter.gravity * (0.5 * age * age);
            let size = lerp(emitter.start_size, emitter.end_size, t).max(0.001);
            let color = [
                lerp(emitter.start_color.x, emitter.end_color.x, t),
                lerp(emitter.start_color.y, emitter.end_color.y, t),
                lerp(emitter.start_color.z, emitter.end_color.z, t),
                1.0 - t,
            ];

            particles.push(ParticleInstance {
                position,
                size,
                color,
                age_fraction: t,
            });
        }
        particles
    }
}

fn particle_direction(seed: u32, index: u32, spread_degrees: f32) -> Vec3 {
    let yaw = random_unit(seed, index, 0) * std::f32::consts::TAU;
    let spread = spread_degrees.clamp(0.0, 180.0).to_radians();
    let cone = random_unit(seed, index, 1) * spread;
    let (yaw_sin, yaw_cos) = yaw.sin_cos();
    let radius = cone.sin();
    Vec3::new(radius * yaw_cos, cone.cos(), radius * yaw_sin).normalized()
}

fn random_unit(seed: u32, index: u32, salt: u32) -> f32 {
    let mut value = seed ^ index.wrapping_mul(0x9e37_79b9) ^ salt.wrapping_mul(0x85eb_ca6b);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^= value >> 16;
    value as f32 / u32::MAX as f32
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_particles_deterministically() {
        let emitter = ParticleEmitterComponentData {
            emission_rate: 4.0,
            lifetime: 1.0,
            elapsed: 0.5,
            ..ParticleEmitterComponentData::default()
        };

        let first = ParticleSystem::sample(&emitter, Transform::IDENTITY);
        let second = ParticleSystem::sample(&emitter, Transform::IDENTITY);

        assert_eq!(first, second);
        assert!(!first.is_empty());
        assert!(first.iter().all(|particle| particle.size > 0.0));
    }
}
