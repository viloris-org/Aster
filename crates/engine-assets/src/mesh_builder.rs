use crate::vmodel::Axis;
use crate::*;

#[cfg(feature = "importers")]
#[derive(Default)]
pub(crate) struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    texcoords: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

#[cfg(feature = "importers")]
impl MeshBuilder {
    pub(crate) fn quad(&mut self, points: [[f32; 3]; 4], normal: [f32; 3]) {
        self.quad_smooth(points, [normal; 4]);
    }

    pub(crate) fn quad_smooth(&mut self, points: [[f32; 3]; 4], normals: [[f32; 3]; 4]) {
        let base = self.positions.len() as u32;
        self.positions.extend(points);
        self.normals.extend(normals);
        self.texcoords
            .extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    }

    pub(crate) fn triangle(&mut self, points: [[f32; 3]; 3], normal: [f32; 3]) {
        self.triangle_smooth(points, [normal; 3]);
    }

    pub(crate) fn triangle_smooth(&mut self, points: [[f32; 3]; 3], normals: [[f32; 3]; 3]) {
        let base = self.positions.len() as u32;
        self.positions.extend(points);
        self.normals.extend(normals);
        self.texcoords.extend([[0.5, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    pub(crate) fn quad_x(
        &mut self,
        sign: f32,
        x: f32,
        y0: f32,
        y1: f32,
        z0: f32,
        z1: f32,
        t: [f32; 3],
    ) {
        self.quad(
            translate_points([[x, y0, z0], [x, y1, z0], [x, y1, z1], [x, y0, z1]], t),
            [sign, 0.0, 0.0],
        );
    }

    pub(crate) fn quad_y(
        &mut self,
        sign: f32,
        y: f32,
        x0: f32,
        x1: f32,
        z0: f32,
        z1: f32,
        t: [f32; 3],
    ) {
        self.quad(
            translate_points([[x0, y, z0], [x1, y, z0], [x1, y, z1], [x0, y, z1]], t),
            [0.0, sign, 0.0],
        );
    }

    pub(crate) fn quad_z(
        &mut self,
        sign: f32,
        z: f32,
        x0: f32,
        x1: f32,
        y0: f32,
        y1: f32,
        t: [f32; 3],
    ) {
        self.quad(
            translate_points([[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]], t),
            [0.0, 0.0, sign],
        );
    }

    pub(crate) fn quad_x_edge(&mut self, a: [f32; 3], b: [f32; 3], half: [f32; 3], t: [f32; 3]) {
        let pa = [
            clamp_to_half(a[0], half[0]),
            sign_half(a[1], half[1]),
            sign_half(a[2], half[2]),
        ];
        let pb = [
            clamp_to_half(b[0], half[0]),
            sign_half(b[1], half[1]),
            sign_half(b[2], half[2]),
        ];
        self.quad(
            translate_points([a, b, pb, pa], t),
            normalize([0.0, a[1], a[2]]),
        );
    }

    pub(crate) fn quad_y_edge(&mut self, a: [f32; 3], b: [f32; 3], half: [f32; 3], t: [f32; 3]) {
        let pa = [
            sign_half(a[0], half[0]),
            clamp_to_half(a[1], half[1]),
            sign_half(a[2], half[2]),
        ];
        let pb = [
            sign_half(b[0], half[0]),
            clamp_to_half(b[1], half[1]),
            sign_half(b[2], half[2]),
        ];
        self.quad(
            translate_points([a, b, pb, pa], t),
            normalize([a[0], 0.0, a[2]]),
        );
    }

    pub(crate) fn quad_z_edge(&mut self, a: [f32; 3], b: [f32; 3], half: [f32; 3], t: [f32; 3]) {
        let pa = [
            sign_half(a[0], half[0]),
            sign_half(a[1], half[1]),
            clamp_to_half(a[2], half[2]),
        ];
        let pb = [
            sign_half(b[0], half[0]),
            sign_half(b[1], half[1]),
            clamp_to_half(b[2], half[2]),
        ];
        self.quad(
            translate_points([a, b, pb, pa], t),
            normalize([a[0], a[1], 0.0]),
        );
    }

    pub(crate) fn finish(self) -> BasicMeshResource {
        BasicMeshResource {
            positions: self.positions,
            normals: self.normals,
            texcoords: self.texcoords,
            indices: self.indices,
            material_index: None,
        }
    }
}

#[cfg(feature = "importers")]
pub(crate) fn vmodel_vec3_param(params: &toml::Value, key: &str) -> Option<[f32; 3]> {
    let array = params.get(key)?.as_array()?;
    if array.len() != 3 {
        return None;
    }
    Some([
        toml_number_as_f32(&array[0])?,
        toml_number_as_f32(&array[1])?,
        toml_number_as_f32(&array[2])?,
    ])
}

#[cfg(feature = "importers")]
pub(crate) fn vmodel_vec4_param(params: &toml::Value, key: &str) -> Option<[f32; 4]> {
    let array = params.get(key)?.as_array()?;
    if array.len() != 4 {
        return None;
    }
    Some([
        toml_number_as_f32(&array[0])?,
        toml_number_as_f32(&array[1])?,
        toml_number_as_f32(&array[2])?,
        toml_number_as_f32(&array[3])?,
    ])
}

#[cfg(feature = "importers")]
pub(crate) fn vmodel_f32_param(params: &toml::Value, key: &str) -> Option<f32> {
    toml_number_as_f32(params.get(key)?)
}

#[cfg(feature = "importers")]
pub(crate) fn toml_number_as_f32(value: &toml::Value) -> Option<f32> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|value| value as f64))
        .map(|value| value as f32)
}

#[cfg(feature = "importers")]
pub(crate) fn vmodel_usize_param(params: &toml::Value, key: &str) -> Option<usize> {
    params
        .get(key)?
        .as_integer()
        .and_then(|value| usize::try_from(value).ok())
}

#[cfg(feature = "importers")]
pub(crate) fn vmodel_u32_param(params: &toml::Value, key: &str) -> Option<u32> {
    params
        .get(key)?
        .as_integer()
        .and_then(|value| u32::try_from(value).ok())
}

#[cfg(feature = "importers")]
pub(crate) fn vmodel_string_param(params: &toml::Value, key: &str) -> Option<String> {
    params.get(key)?.as_str().map(ToOwned::to_owned)
}

#[cfg(feature = "importers")]
pub(crate) fn axis_offset(axis: &str, spacing: f32) -> [f32; 3] {
    match axis.trim().to_ascii_lowercase().as_str() {
        "y" | "+y" => [0.0, spacing, 0.0],
        "-y" => [0.0, -spacing, 0.0],
        "z" | "+z" => [0.0, 0.0, spacing],
        "-z" => [0.0, 0.0, -spacing],
        "-x" => [-spacing, 0.0, 0.0],
        _ => [spacing, 0.0, 0.0],
    }
}

#[cfg(feature = "importers")]
pub(crate) fn axis_from_str(axis: &str) -> Axis {
    match axis.trim().to_ascii_lowercase().as_str() {
        "y" | "+y" | "-y" => Axis::Y,
        "z" | "+z" | "-z" => Axis::Z,
        _ => Axis::X,
    }
}

#[cfg(feature = "importers")]
pub(crate) fn radial_offset(axis: Axis, radius: f32, angle_degrees: f32) -> [f32; 3] {
    let angle = angle_degrees.to_radians();
    let (sin, cos) = angle.sin_cos();
    match axis {
        Axis::X => [0.0, cos * radius, sin * radius],
        Axis::Y => [cos * radius, 0.0, sin * radius],
        Axis::Z => [cos * radius, sin * radius, 0.0],
    }
}

#[cfg(feature = "importers")]
pub(crate) fn radial_rotation(axis: Axis, angle_degrees: f32) -> [f32; 3] {
    match axis {
        Axis::X => [angle_degrees, 0.0, 0.0],
        Axis::Y => [0.0, angle_degrees, 0.0],
        Axis::Z => [0.0, 0.0, angle_degrees],
    }
}

#[cfg(feature = "importers")]
pub(crate) fn add_vec3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

#[cfg(feature = "importers")]
pub(crate) fn mul_vec3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] * right[0], left[1] * right[1], left[2] * right[2]]
}

#[cfg(feature = "importers")]
pub(crate) fn scale_vec3(value: [f32; 3], scalar: f32) -> [f32; 3] {
    [value[0] * scalar, value[1] * scalar, value[2] * scalar]
}

#[cfg(feature = "importers")]
pub(crate) fn translate_points(points: [[f32; 3]; 4], translation: [f32; 3]) -> [[f32; 3]; 4] {
    points.map(|point| add_vec3(point, translation))
}

#[cfg(feature = "importers")]
pub(crate) fn transform_mesh(
    mesh: &mut BasicMeshResource,
    translation: [f32; 3],
    rotation: [f32; 3],
) {
    for position in &mut mesh.positions {
        *position = add_vec3(rotate_vec3(*position, rotation), translation);
    }
    for normal in &mut mesh.normals {
        *normal = normalize(rotate_vec3(*normal, rotation));
    }
}

#[cfg(feature = "importers")]
pub(crate) fn mirror_mesh(mesh: &mut BasicMeshResource, axis: Axis) {
    for position in &mut mesh.positions {
        match axis {
            Axis::X => position[0] = -position[0],
            Axis::Y => position[1] = -position[1],
            Axis::Z => position[2] = -position[2],
        }
    }
    for normal in &mut mesh.normals {
        match axis {
            Axis::X => normal[0] = -normal[0],
            Axis::Y => normal[1] = -normal[1],
            Axis::Z => normal[2] = -normal[2],
        }
    }
    for triangle in mesh.indices.chunks_exact_mut(3) {
        triangle.swap(1, 2);
    }
}

#[cfg(feature = "importers")]
pub(crate) fn rotate_vec3(value: [f32; 3], rotation: [f32; 3]) -> [f32; 3] {
    let (sx, cx) = rotation[0].to_radians().sin_cos();
    let (sy, cy) = rotation[1].to_radians().sin_cos();
    let (sz, cz) = rotation[2].to_radians().sin_cos();

    let mut output = value;
    output = [
        output[0],
        output[1] * cx - output[2] * sx,
        output[1] * sx + output[2] * cx,
    ];
    output = [
        output[0] * cy + output[2] * sy,
        output[1],
        -output[0] * sy + output[2] * cy,
    ];
    [
        output[0] * cz - output[1] * sz,
        output[0] * sz + output[1] * cz,
        output[2],
    ]
}

#[cfg(feature = "importers")]
pub(crate) fn sphere_point(theta: f32, phi: f32, radius: [f32; 3]) -> [f32; 3] {
    let (sin_theta, cos_theta) = theta.sin_cos();
    let (sin_phi, cos_phi) = phi.sin_cos();
    [
        radius[0] * sin_theta * cos_phi,
        radius[1] * cos_theta,
        radius[2] * sin_theta * sin_phi,
    ]
}

#[cfg(feature = "importers")]
pub(crate) fn normalize_ellipsoid(point: [f32; 3], radius: [f32; 3]) -> [f32; 3] {
    normalize([
        point[0] / (radius[0] * radius[0]),
        point[1] / (radius[1] * radius[1]),
        point[2] / (radius[2] * radius[2]),
    ])
}

#[cfg(feature = "importers")]
pub(crate) fn sign_half(value: f32, half: f32) -> f32 {
    if value < 0.0 { -half } else { half }
}

#[cfg(feature = "importers")]
pub(crate) fn clamp_to_half(value: f32, half: f32) -> f32 {
    value.clamp(-half, half)
}

#[cfg(feature = "importers")]
pub(crate) fn normalize(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if length <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        [value[0] / length, value[1] / length, value[2] / length]
    }
}
