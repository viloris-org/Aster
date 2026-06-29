#[cfg(feature = "physics")]
use super::*;

#[cfg(feature = "physics")]
pub(crate) fn collider_desc_from_scene(
    collider: &engine_ecs::ColliderComponentData,
    layer: u32,
) -> ColliderDesc {
    let material = built_in_physical_material(&collider.physics_material);
    ColliderDesc {
        shape: collider_shape_from_scene(collider, Vec3::ONE),
        is_trigger: collider.is_trigger,
        layer,
        mask: collider.mask,
        friction: material.friction,
        restitution: material.restitution,
        friction_combine: material.friction_combine,
        restitution_combine: material.restitution_combine,
        ..ColliderDesc::default()
    }
}

#[cfg(feature = "physics")]
pub(crate) fn collider_shape_from_scene(
    collider: &engine_ecs::ColliderComponentData,
    scale: Vec3,
) -> ColliderShape {
    let half = (collider.size * scale) * 0.5;
    match collider.shape.as_str() {
        "sphere" => ColliderShape::Sphere {
            radius: half.x.abs().max(half.y.abs()).max(half.z.abs()),
        },
        "capsule" => ColliderShape::Capsule {
            half_height: half.y.abs(),
            radius: half.x.abs().max(half.z.abs()),
        },
        _ => ColliderShape::Box {
            half_extents: Vec3::new(half.x.abs(), half.y.abs(), half.z.abs()),
        },
    }
}

#[cfg(feature = "physics")]
pub(crate) fn fluid_volume_desc_from_scene(fluid: &FluidVolumeComponentData) -> FluidVolumeDesc {
    FluidVolumeDesc {
        size: fluid.size,
        density: fluid.density,
        buoyancy_scale: fluid.buoyancy_scale,
        linear_drag: fluid.linear_drag,
        flow_velocity: fluid.flow_velocity,
        surface_offset: fluid.surface_offset,
        surface_model: match fluid.surface_profile.as_str() {
            "river" => FluidSurfaceModel::River,
            "ocean" => FluidSurfaceModel::Ocean,
            "tidal" => FluidSurfaceModel::Tidal,
            _ => FluidSurfaceModel::Still,
        },
        wave_direction: fluid.wave_direction,
        wave_amplitude: fluid.wave_amplitude,
        wave_length: fluid.wave_length,
        wave_speed: fluid.wave_speed,
        chop_amplitude: fluid.chop_amplitude,
        chop_length: fluid.chop_length,
        river_slope: fluid.river_slope,
        tide_amplitude: fluid.tide_amplitude,
        tide_period_seconds: fluid.tide_period_seconds,
        tide_phase_seconds: fluid.tide_phase_seconds,
    }
}

#[cfg(feature = "physics")]
pub(crate) fn buoyancy_probe_set_from_scene(
    probe_set: &BuoyancyProbeSetComponentData,
) -> BuoyancyProbeSet {
    BuoyancyProbeSet {
        probes: probe_set.probes.clone(),
        buoyancy: probe_set.buoyancy,
        damping: probe_set.damping,
        angular_response: probe_set.angular_response,
    }
}

#[cfg(feature = "physics")]
pub(crate) fn apply_fluid_force(
    backend: &mut dyn PhysicsBackend,
    body: BodyHandle,
    force: FluidForce,
) -> EngineResult<()> {
    if force.force.length_squared() > f32::EPSILON {
        backend.apply_force(body, force.force)?;
    }
    if force.torque.length_squared() > f32::EPSILON {
        backend.apply_torque(body, force.torque)?;
    }
    Ok(())
}
