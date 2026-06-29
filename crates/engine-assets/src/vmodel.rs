use crate::mesh_builder::*;
use crate::prelude::*;
use crate::*;

#[cfg(feature = "importers")]
#[derive(Debug, Deserialize)]
struct VModelDocument {
    #[allow(dead_code)]
    schema_version: Option<u32>,
    operations: Vec<VModelOperation>,
}

#[cfg(feature = "importers")]
#[derive(Debug, Deserialize)]
struct VModelOperation {
    #[serde(rename = "type")]
    operation_type: String,
    #[serde(default = "empty_toml_table")]
    params: toml::Value,
}

#[cfg(feature = "importers")]
fn empty_toml_table() -> toml::Value {
    toml::Value::Table(Default::default())
}

#[cfg(feature = "importers")]
#[derive(Clone, Copy, Debug)]
struct ModelBuildState {
    primitive: VModelPrimitive,
    size: [f32; 3],
    translation: [f32; 3],
    scale: [f32; 3],
    rotation: [f32; 3],
    bevel: f32,
    segments: u32,
    array: Option<ArraySpec>,
    radial_array: Option<RadialArraySpec>,
    mirror: Option<MirrorSpec>,
    material_index: Option<usize>,
}

#[cfg(feature = "importers")]
#[derive(Clone, Copy, Debug)]
enum VModelPrimitive {
    Box,
    Cylinder,
    Cone,
    Sphere,
    Plane,
}

#[cfg(feature = "importers")]
#[derive(Clone, Copy, Debug)]
struct ArraySpec {
    count: usize,
    offset: [f32; 3],
}

#[cfg(feature = "importers")]
#[derive(Clone, Copy, Debug)]
struct RadialArraySpec {
    count: usize,
    radius: f32,
    axis: Axis,
    start_angle: f32,
    step_angle: f32,
}

#[cfg(feature = "importers")]
#[derive(Clone, Copy, Debug)]
struct MirrorSpec {
    axis: Axis,
}

#[cfg(feature = "importers")]
#[derive(Clone, Copy, Debug)]
pub(crate) enum Axis {
    X,
    Y,
    Z,
}

#[cfg(feature = "importers")]
impl Default for ModelBuildState {
    fn default() -> Self {
        Self {
            primitive: VModelPrimitive::Box,
            size: [1.0, 1.0, 1.0],
            translation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            rotation: [0.0, 0.0, 0.0],
            bevel: 0.0,
            segments: 24,
            array: None,
            radial_array: None,
            mirror: None,
            material_index: None,
        }
    }
}

#[cfg(feature = "importers")]
pub(crate) fn compile_vmodel(bytes: &[u8]) -> Result<ModelResource, AssetError> {
    let text = std::str::from_utf8(bytes).map_err(|source| AssetError::Parse {
        format: "vmodel",
        diagnostic: AssetDiagnostic::new(source.to_string()),
    })?;
    let document: VModelDocument = toml::from_str(text).map_err(|source| AssetError::Parse {
        format: "vmodel",
        diagnostic: AssetDiagnostic::new(source.to_string()),
    })?;
    if document.operations.is_empty() {
        return Err(AssetError::Parse {
            format: "vmodel",
            diagnostic: AssetDiagnostic::new(".vmodel contains no operations"),
        });
    }

    let mut model = ModelResource::default();
    let mut state = ModelBuildState::default();
    let mut has_base_mesh = false;

    for operation in &document.operations {
        match operation.operation_type.as_str() {
            "cube" | "box" => {
                if has_base_mesh {
                    add_state_meshes(&mut model, &state);
                }
                state = ModelBuildState {
                    primitive: VModelPrimitive::Box,
                    size: vmodel_vec3_param(&operation.params, "size").unwrap_or([1.0, 1.0, 1.0]),
                    translation: vmodel_vec3_param(&operation.params, "position")
                        .or_else(|| vmodel_vec3_param(&operation.params, "translation"))
                        .unwrap_or([0.0, 0.0, 0.0]),
                    scale: vmodel_vec3_param(&operation.params, "scale").unwrap_or([1.0, 1.0, 1.0]),
                    rotation: vmodel_vec3_param(&operation.params, "rotation")
                        .unwrap_or([0.0, 0.0, 0.0]),
                    bevel: vmodel_f32_param(&operation.params, "bevel")
                        .unwrap_or(0.0)
                        .max(0.0),
                    segments: vmodel_u32_param(&operation.params, "segments")
                        .unwrap_or(24)
                        .clamp(3, 96),
                    array: None,
                    radial_array: None,
                    mirror: None,
                    material_index: vmodel_usize_param(&operation.params, "material"),
                };
                has_base_mesh = true;
            }
            "cylinder" | "cone" | "sphere" | "plane" => {
                if has_base_mesh {
                    add_state_meshes(&mut model, &state);
                }
                state = ModelBuildState {
                    primitive: match operation.operation_type.as_str() {
                        "cylinder" => VModelPrimitive::Cylinder,
                        "cone" => VModelPrimitive::Cone,
                        "sphere" => VModelPrimitive::Sphere,
                        _ => VModelPrimitive::Plane,
                    },
                    size: vmodel_vec3_param(&operation.params, "size").unwrap_or([1.0, 1.0, 1.0]),
                    translation: vmodel_vec3_param(&operation.params, "position")
                        .or_else(|| vmodel_vec3_param(&operation.params, "translation"))
                        .unwrap_or([0.0, 0.0, 0.0]),
                    scale: vmodel_vec3_param(&operation.params, "scale").unwrap_or([1.0, 1.0, 1.0]),
                    rotation: vmodel_vec3_param(&operation.params, "rotation")
                        .unwrap_or([0.0, 0.0, 0.0]),
                    bevel: vmodel_f32_param(&operation.params, "bevel")
                        .unwrap_or(0.0)
                        .max(0.0),
                    segments: vmodel_u32_param(&operation.params, "segments")
                        .or_else(|| vmodel_u32_param(&operation.params, "rings"))
                        .unwrap_or(24)
                        .clamp(3, 96),
                    array: None,
                    radial_array: None,
                    mirror: None,
                    material_index: vmodel_usize_param(&operation.params, "material"),
                };
                has_base_mesh = true;
            }
            "bevel" => {
                state.bevel = vmodel_f32_param(&operation.params, "amount")
                    .or_else(|| vmodel_f32_param(&operation.params, "radius"))
                    .unwrap_or(state.bevel)
                    .max(0.0);
            }
            "translate" => {
                let offset = vmodel_vec3_param(&operation.params, "offset")
                    .or_else(|| vmodel_vec3_param(&operation.params, "position"))
                    .unwrap_or([0.0, 0.0, 0.0]);
                state.translation = add_vec3(state.translation, offset);
            }
            "scale" => {
                let scale = vmodel_vec3_param(&operation.params, "value")
                    .or_else(|| vmodel_vec3_param(&operation.params, "scale"))
                    .unwrap_or([1.0, 1.0, 1.0]);
                state.scale = mul_vec3(state.scale, scale);
            }
            "rotate" => {
                let rotation = vmodel_vec3_param(&operation.params, "rotation")
                    .or_else(|| vmodel_vec3_param(&operation.params, "value"))
                    .or_else(|| vmodel_vec3_param(&operation.params, "euler"))
                    .unwrap_or([0.0, 0.0, 0.0]);
                state.rotation = add_vec3(state.rotation, rotation);
            }
            "array" => {
                let count = vmodel_usize_param(&operation.params, "count")
                    .unwrap_or(1)
                    .clamp(1, 256);
                let offset = vmodel_vec3_param(&operation.params, "offset").unwrap_or_else(|| {
                    let axis = vmodel_string_param(&operation.params, "axis")
                        .unwrap_or_else(|| "x".to_string());
                    let spacing = vmodel_f32_param(&operation.params, "spacing").unwrap_or(1.0);
                    axis_offset(&axis, spacing)
                });
                state.array = Some(ArraySpec { count, offset });
            }
            "radial_array" => {
                let count = vmodel_usize_param(&operation.params, "count")
                    .unwrap_or(1)
                    .clamp(1, 256);
                let axis = vmodel_string_param(&operation.params, "axis")
                    .as_deref()
                    .map(axis_from_str)
                    .unwrap_or(Axis::Y);
                let radius = vmodel_f32_param(&operation.params, "radius")
                    .or_else(|| vmodel_f32_param(&operation.params, "spacing"))
                    .unwrap_or(1.0)
                    .abs();
                let start_angle = vmodel_f32_param(&operation.params, "start_angle").unwrap_or(0.0);
                let step_angle = vmodel_f32_param(&operation.params, "step_angle")
                    .unwrap_or_else(|| 360.0 / count as f32);
                state.radial_array = Some(RadialArraySpec {
                    count,
                    radius,
                    axis,
                    start_angle,
                    step_angle,
                });
            }
            "mirror" => {
                let axis = vmodel_string_param(&operation.params, "axis")
                    .as_deref()
                    .map(axis_from_str)
                    .unwrap_or(Axis::X);
                state.mirror = Some(MirrorSpec { axis });
            }
            "material_slot" => {
                state.material_index = Some(upsert_vmodel_material(&mut model, &operation.params));
            }
            "inset_panel" => {
                if !has_base_mesh {
                    return Err(AssetError::Parse {
                        format: "vmodel",
                        diagnostic: AssetDiagnostic::new(
                            "inset_panel requires a cube or box first",
                        ),
                    });
                }
                let face = vmodel_string_param(&operation.params, "face")
                    .unwrap_or_else(|| "+z".to_string());
                let margin = vmodel_f32_param(&operation.params, "margin")
                    .or_else(|| vmodel_f32_param(&operation.params, "amount"))
                    .unwrap_or(0.1)
                    .max(0.0);
                let depth = vmodel_f32_param(&operation.params, "depth")
                    .unwrap_or(0.02)
                    .abs();
                add_inset_panel(&mut model, &state, &face, margin, depth);
            }
            other => {
                return Err(AssetError::Parse {
                    format: "vmodel",
                    diagnostic: AssetDiagnostic::new(format!(
                        "unsupported .vmodel operation `{other}`"
                    )),
                });
            }
        }
    }

    if has_base_mesh {
        add_state_meshes(&mut model, &state);
    }

    if model.meshes.is_empty() {
        return Err(AssetError::Parse {
            format: "vmodel",
            diagnostic: AssetDiagnostic::new(".vmodel produced no mesh primitives"),
        });
    }
    Ok(model)
}

#[cfg(feature = "importers")]
fn add_state_meshes(model: &mut ModelResource, state: &ModelBuildState) {
    let array = state.array.unwrap_or(ArraySpec {
        count: 1,
        offset: [0.0, 0.0, 0.0],
    });
    let count = state
        .radial_array
        .map_or(array.count, |radial| radial.count);

    for index in 0..count {
        let (translation, rotation) = if let Some(radial) = state.radial_array {
            let angle = radial.start_angle + radial.step_angle * index as f32;
            (
                add_vec3(
                    state.translation,
                    radial_offset(radial.axis, radial.radius, angle),
                ),
                add_vec3(state.rotation, radial_rotation(radial.axis, angle)),
            )
        } else {
            (
                add_vec3(state.translation, scale_vec3(array.offset, index as f32)),
                state.rotation,
            )
        };
        let size = mul_vec3(state.size, state.scale);
        let mut mesh = build_primitive_mesh(state.primitive, size, state.bevel, state.segments);
        transform_mesh(&mut mesh, translation, rotation);
        mesh.material_index = state.material_index;

        if let Some(mirror) = state.mirror {
            let mut mirrored = mesh.clone();
            mirror_mesh(&mut mirrored, mirror.axis);
            model.meshes.push(mirrored);
        }
        model.meshes.push(mesh);
    }
}

#[cfg(feature = "importers")]
fn build_primitive_mesh(
    primitive: VModelPrimitive,
    size: [f32; 3],
    bevel: f32,
    segments: u32,
) -> BasicMeshResource {
    match primitive {
        VModelPrimitive::Box => build_box_mesh(size, [0.0, 0.0, 0.0], bevel),
        VModelPrimitive::Cylinder => build_cylinder_mesh(size, segments, false),
        VModelPrimitive::Cone => build_cylinder_mesh(size, segments, true),
        VModelPrimitive::Sphere => build_sphere_mesh(size, segments),
        VModelPrimitive::Plane => build_plane_mesh(size),
    }
}

#[cfg(feature = "importers")]
fn upsert_vmodel_material(model: &mut ModelResource, params: &toml::Value) -> usize {
    if let Some(index) =
        vmodel_usize_param(params, "index").or_else(|| vmodel_usize_param(params, "material"))
    {
        while model.materials.len() <= index {
            model.materials.push(CpuMaterialResource::default());
        }
        update_vmodel_material(&mut model.materials[index], params, index);
        index
    } else {
        let index = model.materials.len();
        let mut material = CpuMaterialResource::default();
        update_vmodel_material(&mut material, params, index);
        model.materials.push(material);
        index
    }
}

#[cfg(feature = "importers")]
fn update_vmodel_material(material: &mut CpuMaterialResource, params: &toml::Value, index: usize) {
    material.name =
        vmodel_string_param(params, "name").unwrap_or_else(|| format!("material_{index}"));
    material.base_color = vmodel_vec4_param(params, "base_color")
        .or_else(|| {
            vmodel_vec3_param(params, "base_color").map(|color| [color[0], color[1], color[2], 1.0])
        })
        .unwrap_or(material.base_color);
    material.metallic = vmodel_f32_param(params, "metallic").unwrap_or(material.metallic);
    material.roughness = vmodel_f32_param(params, "roughness").unwrap_or(material.roughness);
    material.emissive = vmodel_vec3_param(params, "emissive").unwrap_or(material.emissive);
}

#[cfg(feature = "importers")]
fn add_inset_panel(
    model: &mut ModelResource,
    state: &ModelBuildState,
    face: &str,
    margin: f32,
    depth: f32,
) {
    let size = mul_vec3(state.size, state.scale);
    let panel_scale = [
        (size[0] - margin * 2.0).max(size[0] * 0.1),
        (size[1] - margin * 2.0).max(size[1] * 0.1),
        (size[2] - margin * 2.0).max(size[2] * 0.1),
    ];
    let (panel_size, panel_offset) = match face.trim().to_ascii_lowercase().as_str() {
        "+x" => (
            [depth, panel_scale[1], panel_scale[2]],
            [size[0] * 0.5 + depth * 0.5, 0.0, 0.0],
        ),
        "-x" => (
            [depth, panel_scale[1], panel_scale[2]],
            [-size[0] * 0.5 - depth * 0.5, 0.0, 0.0],
        ),
        "+y" => (
            [panel_scale[0], depth, panel_scale[2]],
            [0.0, size[1] * 0.5 + depth * 0.5, 0.0],
        ),
        "-y" => (
            [panel_scale[0], depth, panel_scale[2]],
            [0.0, -size[1] * 0.5 - depth * 0.5, 0.0],
        ),
        "-z" => (
            [panel_scale[0], panel_scale[1], depth],
            [0.0, 0.0, -size[2] * 0.5 - depth * 0.5],
        ),
        _ => (
            [panel_scale[0], panel_scale[1], depth],
            [0.0, 0.0, size[2] * 0.5 + depth * 0.5],
        ),
    };
    let mut mesh = build_box_mesh(panel_size, panel_offset, state.bevel.min(depth * 0.45));
    transform_mesh(&mut mesh, state.translation, state.rotation);
    mesh.material_index = state.material_index;
    model.meshes.push(mesh);
}

#[cfg(feature = "importers")]
fn build_box_mesh(size: [f32; 3], translation: [f32; 3], bevel: f32) -> BasicMeshResource {
    let half = [size[0] * 0.5, size[1] * 0.5, size[2] * 0.5];
    let bevel = bevel
        .min(half[0] * 0.45)
        .min(half[1] * 0.45)
        .min(half[2] * 0.45)
        .max(0.0);

    if bevel <= f32::EPSILON {
        return build_axis_box_mesh(size, translation);
    }

    let x0 = -half[0] + bevel;
    let x1 = half[0] - bevel;
    let y0 = -half[1] + bevel;
    let y1 = half[1] - bevel;
    let z0 = -half[2] + bevel;
    let z1 = half[2] - bevel;
    let hx = half[0];
    let hy = half[1];
    let hz = half[2];

    let mut mesh = MeshBuilder::default();
    mesh.quad_x(1.0, hx, y0, y1, z0, z1, translation);
    mesh.quad_x(-1.0, -hx, y0, y1, z0, z1, translation);
    mesh.quad_y(1.0, hy, x0, x1, z0, z1, translation);
    mesh.quad_y(-1.0, -hy, x0, x1, z0, z1, translation);
    mesh.quad_z(1.0, hz, x0, x1, y0, y1, translation);
    mesh.quad_z(-1.0, -hz, x0, x1, y0, y1, translation);

    for &y in &[y0, y1] {
        for &z in &[z0, z1] {
            mesh.quad_x_edge([x0, y, z], [x1, y, z], half, translation);
        }
    }
    for &x in &[x0, x1] {
        for &z in &[z0, z1] {
            mesh.quad_y_edge([x, y0, z], [x, y1, z], half, translation);
        }
    }
    for &x in &[x0, x1] {
        for &y in &[y0, y1] {
            mesh.quad_z_edge([x, y, z0], [x, y, z1], half, translation);
        }
    }
    mesh.finish()
}

#[cfg(feature = "importers")]
fn build_axis_box_mesh(size: [f32; 3], translation: [f32; 3]) -> BasicMeshResource {
    let half = [size[0] * 0.5, size[1] * 0.5, size[2] * 0.5];
    let mut mesh = MeshBuilder::default();
    mesh.quad_x(
        1.0,
        half[0],
        -half[1],
        half[1],
        -half[2],
        half[2],
        translation,
    );
    mesh.quad_x(
        -1.0,
        -half[0],
        -half[1],
        half[1],
        -half[2],
        half[2],
        translation,
    );
    mesh.quad_y(
        1.0,
        half[1],
        -half[0],
        half[0],
        -half[2],
        half[2],
        translation,
    );
    mesh.quad_y(
        -1.0,
        -half[1],
        -half[0],
        half[0],
        -half[2],
        half[2],
        translation,
    );
    mesh.quad_z(
        1.0,
        half[2],
        -half[0],
        half[0],
        -half[1],
        half[1],
        translation,
    );
    mesh.quad_z(
        -1.0,
        -half[2],
        -half[0],
        half[0],
        -half[1],
        half[1],
        translation,
    );
    mesh.finish()
}

#[cfg(feature = "importers")]
fn build_plane_mesh(size: [f32; 3]) -> BasicMeshResource {
    let half = [size[0] * 0.5, size[2] * 0.5];
    let mut mesh = MeshBuilder::default();
    mesh.quad(
        [
            [-half[0], 0.0, -half[1]],
            [half[0], 0.0, -half[1]],
            [half[0], 0.0, half[1]],
            [-half[0], 0.0, half[1]],
        ],
        [0.0, 1.0, 0.0],
    );
    mesh.finish()
}

#[cfg(feature = "importers")]
fn build_cylinder_mesh(size: [f32; 3], segments: u32, cone: bool) -> BasicMeshResource {
    let segments = segments.clamp(3, 96);
    let radius_x = (size[0] * 0.5).abs().max(0.001);
    let radius_z = (size[2] * 0.5).abs().max(0.001);
    let half_y = (size[1] * 0.5).abs().max(0.001);
    let mut mesh = MeshBuilder::default();
    let top_radius_x = if cone { 0.0 } else { radius_x };
    let top_radius_z = if cone { 0.0 } else { radius_z };

    for index in 0..segments {
        let a0 = index as f32 / segments as f32 * std::f32::consts::TAU;
        let a1 = (index + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let bottom0 = [c0 * radius_x, -half_y, s0 * radius_z];
        let bottom1 = [c1 * radius_x, -half_y, s1 * radius_z];
        let top0 = [c0 * top_radius_x, half_y, s0 * top_radius_z];
        let top1 = [c1 * top_radius_x, half_y, s1 * top_radius_z];

        if cone {
            mesh.triangle(
                [bottom0, bottom1, [0.0, half_y, 0.0]],
                normalize([c0 + c1, radius_x / size[1].max(0.001), s0 + s1]),
            );
        } else {
            mesh.quad(
                [bottom0, bottom1, top1, top0],
                normalize([c0 + c1, 0.0, s0 + s1]),
            );
        }
        mesh.triangle([[0.0, -half_y, 0.0], bottom1, bottom0], [0.0, -1.0, 0.0]);
        if !cone {
            mesh.triangle([[0.0, half_y, 0.0], top0, top1], [0.0, 1.0, 0.0]);
        }
    }
    mesh.finish()
}

#[cfg(feature = "importers")]
fn build_sphere_mesh(size: [f32; 3], segments: u32) -> BasicMeshResource {
    let lon_segments = segments.clamp(8, 96);
    let lat_segments = (lon_segments / 2).clamp(4, 48);
    let radius = [
        (size[0] * 0.5).abs().max(0.001),
        (size[1] * 0.5).abs().max(0.001),
        (size[2] * 0.5).abs().max(0.001),
    ];
    let mut mesh = MeshBuilder::default();

    for lat in 0..lat_segments {
        let theta0 = lat as f32 / lat_segments as f32 * std::f32::consts::PI;
        let theta1 = (lat + 1) as f32 / lat_segments as f32 * std::f32::consts::PI;
        for lon in 0..lon_segments {
            let phi0 = lon as f32 / lon_segments as f32 * std::f32::consts::TAU;
            let phi1 = (lon + 1) as f32 / lon_segments as f32 * std::f32::consts::TAU;
            let p00 = sphere_point(theta0, phi0, radius);
            let p01 = sphere_point(theta0, phi1, radius);
            let p10 = sphere_point(theta1, phi0, radius);
            let p11 = sphere_point(theta1, phi1, radius);
            if lat == 0 {
                mesh.triangle_smooth(
                    [p00, p10, p11],
                    [
                        [0.0, 1.0, 0.0],
                        normalize_ellipsoid(p10, radius),
                        normalize_ellipsoid(p11, radius),
                    ],
                );
            } else if lat + 1 == lat_segments {
                mesh.triangle_smooth(
                    [p00, p10, p01],
                    [
                        normalize_ellipsoid(p00, radius),
                        [0.0, -1.0, 0.0],
                        normalize_ellipsoid(p01, radius),
                    ],
                );
            } else {
                mesh.quad_smooth(
                    [p00, p10, p11, p01],
                    [
                        normalize_ellipsoid(p00, radius),
                        normalize_ellipsoid(p10, radius),
                        normalize_ellipsoid(p11, radius),
                        normalize_ellipsoid(p01, radius),
                    ],
                );
            }
        }
    }
    mesh.finish()
}
