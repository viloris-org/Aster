use std::collections::HashMap;
use std::path::Path;

use engine_core::math::{Quat, Transform, Vec3};
use engine_ecs::{
    CameraComponentData, CameraRole, ColliderComponentData, ComponentData, GameObject,
    LightComponentData, MaterialRef, MeshRendererComponentData, RigidbodyComponentData,
    SCENE_FILE_VERSION, Scene, SceneFile, ScriptComponent, SerializedGameObject,
};

use crate::diagnostics::{VargDiagnostic, VargDiagnosticSeverity};
use crate::parser::parse_source;
use crate::syntax::{
    parse_expression_call, parse_header, parse_string_literal, split_top_level_commas,
    strip_line_comment,
};

/// Parses a `.vscene` world file into the native scene file structure.
///
/// This is the preferred load path for Varg scenes. It parses the authoring
/// source directly into the engine's typed ECS scene model.
pub fn compile_vscene_source_to_scene_file(
    path: impl AsRef<Path>,
    source: &str,
) -> (Option<SceneFile>, Vec<VargDiagnostic>) {
    let path = path.as_ref();
    let (ast, mut diagnostics) = parse_source(path, source);
    if ast.is_none() {
        return (None, diagnostics);
    }
    let document = match parse_vscene_document(source) {
        Ok(document) => document,
        Err(error) => {
            diagnostics.push(error);
            return (None, diagnostics);
        }
    };
    let Some(scene_block) = document.children.iter().find(|block| block.kind == "scene") else {
        diagnostics.push(vscene_error(
            source,
            1,
            1,
            "VSCENE1000",
            ".vscene file does not contain a scene declaration",
            "`scene Name { ... }`",
            "Add a top-level scene declaration.",
        ));
        return (None, diagnostics);
    };

    match compile_vscene_scene(scene_block) {
        Ok(file) => (Some(file), diagnostics),
        Err(mut error) => {
            error.source_line = source
                .lines()
                .nth(error.line.unwrap_or(1).saturating_sub(1))
                .map(str::to_string);
            diagnostics.push(error);
            (None, diagnostics)
        }
    }
}

/// Parses a `.vscene` world file directly into an executable ECS [`Scene`].
pub fn compile_vscene_source_to_scene(
    path: impl AsRef<Path>,
    source: &str,
) -> (Option<Scene>, Vec<VargDiagnostic>) {
    let (file, mut diagnostics) = compile_vscene_source_to_scene_file(path, source);
    let Some(file) = file else {
        return (None, diagnostics);
    };
    match Scene::from_scene_file(file) {
        Ok(scene) => (Some(scene), diagnostics),
        Err(error) => {
            diagnostics.push(VargDiagnostic {
                code: "VSCENE9001".to_string(),
                severity: VargDiagnosticSeverity::Error,
                line: Some(1),
                column: Some(1),
                message: format!("scene construction failed: {error}"),
                expected: "valid ECS scene".to_string(),
                suggestion: "Check generated object IDs, hierarchy, and component data."
                    .to_string(),
                blocking: true,
                source_line: source.lines().next().map(str::to_string),
            });
            (None, diagnostics)
        }
    }
}

/// Serializes an ECS [`Scene`] as native `.vscene` source.
pub fn serialize_scene_to_vscene(
    scene: &Scene,
    name: impl AsRef<str>,
) -> engine_core::EngineResult<String> {
    serialize_scene_file_to_vscene(&scene.to_scene_file(name.as_ref())?)
}

/// Serializes a typed scene file as native `.vscene` source.
pub fn serialize_scene_file_to_vscene(file: &SceneFile) -> engine_core::EngineResult<String> {
    let mut output = String::new();
    output.push_str("scene ");
    output.push_str(&vscene_block_name(&file.name));
    output.push_str(" {\n");

    for record in &file.objects {
        write_vscene_object(&mut output, record, 1)?;
    }

    output.push_str("}\n");
    Ok(output)
}

#[derive(Clone, Debug, Default, PartialEq)]
struct VsceneDocument {
    children: Vec<VsceneBlock>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct VsceneBlock {
    kind: String,
    name: Option<String>,
    line: usize,
    properties: HashMap<String, VsceneValue>,
    children: Vec<VsceneBlock>,
}

#[derive(Clone, Debug, PartialEq)]
enum VsceneValue {
    Number(f32),
    Bool(bool),
    String(String),
    Identifier(String),
    Vec3(Vec3),
    Color(Vec3),
    Call {
        function: String,
        args: HashMap<String, VsceneValue>,
    },
}

fn parse_vscene_document(source: &str) -> Result<VsceneDocument, VargDiagnostic> {
    let mut stack: Vec<VsceneBlock> = Vec::new();
    let mut document = VsceneDocument::default();

    for (line_index, raw_line) in source.lines().enumerate() {
        let line = line_index + 1;
        let without_comment = strip_line_comment(raw_line);
        let trimmed = without_comment.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == "}" {
            let Some(block) = stack.pop() else {
                return Err(vscene_error(
                    source,
                    line,
                    1,
                    "VSCENE1001",
                    "unexpected closing brace",
                    "A closing brace must match an open block.",
                    "Remove this brace or add the missing block header before it.",
                ));
            };
            if let Some(parent) = stack.last_mut() {
                parent.children.push(block);
            } else {
                document.children.push(block);
            }
            continue;
        }

        if let Some(header) = parse_header(trimmed) {
            stack.push(VsceneBlock {
                kind: header.kind,
                name: header.name,
                line,
                properties: HashMap::new(),
                children: Vec::new(),
            });
            continue;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let Some(block) = stack.last_mut() else {
                return Err(vscene_error(
                    source,
                    line,
                    1,
                    "VSCENE1002",
                    "property declared outside a block",
                    "`property: value` inside a block",
                    "Move this property inside a scene, entity, or component block.",
                ));
            };
            let parsed = parse_vscene_value(value.trim()).ok_or_else(|| {
                vscene_error(
                    source,
                    line,
                    raw_line.find(':').map(|index| index + 2).unwrap_or(1),
                    "VSCENE1003",
                    "unsupported .vscene value syntax",
                    "Use numbers, booleans, strings, identifiers, Vec3(...), Color(...), or Constructor(key: value).",
                    "Simplify the value or add compiler support for this construct.",
                )
            })?;
            block.properties.insert(key.trim().to_string(), parsed);
            continue;
        }

        return Err(vscene_error(
            source,
            line,
            1,
            "VSCENE1004",
            "unsupported .vscene statement",
            "Use `block Name { ... }`, `property: value`, or `}`.",
            "Rewrite this line using the declarative .vscene block syntax.",
        ));
    }

    if let Some(block) = stack.last() {
        return Err(vscene_error(
            source,
            block.line,
            1,
            "VSCENE1005",
            "unclosed .vscene block",
            "Every `{` must be paired with a closing `}`.",
            "Add a closing brace for this block.",
        ));
    }

    Ok(document)
}

fn compile_vscene_scene(scene_block: &VsceneBlock) -> Result<SceneFile, VargDiagnostic> {
    let mut objects = Vec::new();
    let mut next_id = 1_u64;

    for child in &scene_block.children {
        match child.kind.as_str() {
            "camera" | "entity" | "light" => {
                objects.push(compile_vscene_object(child, next_id)?);
                next_id += 1;
            }
            "group" => {
                for nested in &child.children {
                    if matches!(nested.kind.as_str(), "camera" | "entity" | "light") {
                        objects.push(compile_vscene_object(nested, next_id)?);
                        next_id += 1;
                    }
                }
            }
            "intent" | "constraints" | "scatter" => {}
            _ => {
                return Err(vscene_compile_error(
                    child,
                    "VSCENE2000",
                    &format!("unsupported scene child block `{}`", child.kind),
                    "`camera`, `entity`, `light`, `group`, or future generator blocks",
                    "Use an entity-like block supported by the compiler.",
                ));
            }
        }
    }

    Ok(SceneFile {
        version: SCENE_FILE_VERSION,
        name: scene_block
            .name
            .clone()
            .unwrap_or_else(|| "Scene".to_string()),
        objects,
    })
}

fn compile_vscene_object(
    block: &VsceneBlock,
    id: u64,
) -> Result<SerializedGameObject, VargDiagnostic> {
    let name = block
        .name
        .clone()
        .unwrap_or_else(|| format!("{} {id}", block.kind));
    let mut tag = string_property(block, "tag").unwrap_or_else(|| "Untagged".to_string());
    let mut camera_role = None;
    let mut components = Vec::new();
    let mut transform = Transform::IDENTITY;
    let mut mesh_renderer_child = None;
    let mut material_child = None;
    let mut has_explicit_collider_size = false;

    if block.kind == "camera" {
        tag = "MainCamera".to_string();
        camera_role = Some(CameraRole::Main);
        components.push(ComponentData::Camera(compile_camera_component(block)));
    }
    if block.kind == "light" {
        tag = "Light".to_string();
        components.push(ComponentData::Light(compile_light_component(block)));
    }
    apply_vscene_transform_properties(block, &mut transform);

    for child in &block.children {
        match child.kind.as_str() {
            "transform" => {
                transform = Transform::IDENTITY;
                apply_vscene_transform_properties(child, &mut transform);
            }
            "perspective" => upsert_component(
                &mut components,
                ComponentData::Camera(compile_camera_component(child)),
            ),
            "mesh" | "geometry" => mesh_renderer_child = Some(child),
            "material" => material_child = Some(child),
            "rigidbody" => {
                components.push(ComponentData::Rigidbody(compile_rigidbody_component(child)))
            }
            "collider" => {
                has_explicit_collider_size |= child.properties.contains_key("size");
                components.push(ComponentData::Collider(compile_collider_component(child)))
            }
            "script" => components.push(ComponentData::Script(compile_script_component(child))),
            "light" => components.push(ComponentData::Light(compile_light_component(child))),
            _ => {
                return Err(vscene_compile_error(
                    child,
                    "VSCENE2001",
                    &format!("unsupported object child block `{}`", child.kind),
                    "`transform`, `perspective`, `mesh`, `geometry`, `material`, `rigidbody`, `collider`, `script`, or `light`",
                    "Use a supported component block or extend the .vscene compiler.",
                ));
            }
        }
    }

    if block.properties.contains_key("mesh")
        || block.properties.contains_key("geometry")
        || block.properties.contains_key("material")
        || mesh_renderer_child.is_some()
        || material_child.is_some()
    {
        upsert_component(
            &mut components,
            ComponentData::MeshRenderer(compile_mesh_renderer_component(
                block,
                mesh_renderer_child,
                material_child,
            )),
        );
    }

    if let Some(primitive_scale) = vscene_mesh_primitive_scale(block, mesh_renderer_child) {
        if has_explicit_collider_size {
            preserve_explicit_collider_size(&mut components, primitive_scale);
        }
        transform.scale = Vec3::new(
            transform.scale.x * primitive_scale.x,
            transform.scale.y * primitive_scale.y,
            transform.scale.z * primitive_scale.z,
        );
    }

    Ok(SerializedGameObject {
        object: GameObject {
            id: engine_core::EntityId::from_u128(u128::from(id)),
            name,
            tag,
            layer: 0,
            camera_role,
            active: true,
            scripts: Vec::new(),
            components,
        },
        local_transform: transform,
        parent: None,
        sibling_index: (id - 1) as usize,
    })
}

fn apply_vscene_transform_properties(block: &VsceneBlock, transform: &mut Transform) {
    if let Some(position) = vec3_property(block, "position") {
        transform.translation = position;
    }
    if let Some(rotation) = vec3_property(block, "rotation") {
        transform.rotation = Quat::from_euler_deg(rotation.z, rotation.y, rotation.x);
    }
    if let Some(scale) = vec3_property(block, "scale") {
        transform.scale = scale;
    }
}
fn compile_camera_component(block: &VsceneBlock) -> CameraComponentData {
    CameraComponentData {
        vertical_fov_degrees: number_property(block, "fov").unwrap_or(60.0),
        near: number_property(block, "near").unwrap_or(0.01),
        far: number_property(block, "far").unwrap_or(1000.0),
        aspect_ratio: None,
        primary: bool_property(block, "primary").unwrap_or(true),
        clear_color: Vec3::new(0.1, 0.1, 0.1),
    }
}

fn compile_mesh_renderer_component(
    object: &VsceneBlock,
    mesh: Option<&VsceneBlock>,
    material: Option<&VsceneBlock>,
) -> MeshRendererComponentData {
    let builtin_material = material
        .and_then(vscene_material_builtin)
        .or_else(|| string_property(object, "material"))
        .unwrap_or_else(|| "debug/default".to_string());
    MeshRendererComponentData {
        mesh: None,
        builtin_mesh: Some(
            vscene_mesh_builtin(object, mesh).unwrap_or_else(|| "debug/cube".to_string()),
        ),
        material: MaterialRef {
            asset: None,
            builtin: Some(builtin_material),
        },
        casts_shadows: true,
        receive_shadows: true,
    }
}

fn vscene_mesh_builtin(object: &VsceneBlock, mesh: Option<&VsceneBlock>) -> Option<String> {
    mesh.and_then(vscene_mesh_builtin_from_block)
        .or_else(|| string_property(object, "mesh").and_then(|value| normalize_vscene_mesh(&value)))
        .or_else(|| {
            string_property(object, "geometry").and_then(|value| normalize_vscene_mesh(&value))
        })
        .or_else(|| call_property(object, "mesh").and_then(vscene_mesh_builtin_from_call))
        .or_else(|| call_property(object, "geometry").and_then(vscene_mesh_builtin_from_call))
}

fn vscene_mesh_builtin_from_block(block: &VsceneBlock) -> Option<String> {
    string_property(block, "builtin")
        .or_else(|| string_property(block, "type").and_then(|value| normalize_vscene_mesh(&value)))
        .or_else(|| string_property(block, "kind").and_then(|value| normalize_vscene_mesh(&value)))
        .or_else(|| string_property(block, "path").map(|value| format!("model:{value}")))
        .or_else(|| call_property(block, "type").and_then(vscene_mesh_builtin_from_call))
        .or_else(|| call_property(block, "kind").and_then(vscene_mesh_builtin_from_call))
}

fn normalize_vscene_mesh(value: &str) -> Option<String> {
    let normalized = match value {
        "box" | "cube" | "primitive.box" | "debug/cube" => "debug/cube".to_string(),
        "sphere" | "primitive.sphere" | "debug/sphere" => "debug/sphere".to_string(),
        "plane" | "primitive.plane" | "debug/plane" => "debug/plane".to_string(),
        "cylinder" | "primitive.cylinder" | "debug/cylinder" => "debug/cylinder".to_string(),
        "cone" | "primitive.cone" | "debug/cone" => "debug/cone".to_string(),
        other if other.starts_with("debug/") => other.to_string(),
        other if other.starts_with("model:") => other.to_string(),
        other if other.ends_with(".gltf") || other.ends_with(".glb") => format!("model:{other}"),
        _ => return None,
    };
    Some(normalized)
}

fn vscene_mesh_builtin_from_call(value: &VsceneValue) -> Option<String> {
    let VsceneValue::Call { function, args } = value else {
        return None;
    };
    match function.as_str() {
        "Box" | "Cube" | "primitive.box" => Some("debug/cube".to_string()),
        "Sphere" | "primitive.sphere" => Some("debug/sphere".to_string()),
        "Plane" | "primitive.plane" => Some("debug/plane".to_string()),
        "Cylinder" | "primitive.cylinder" => Some("debug/cylinder".to_string()),
        "Cone" | "primitive.cone" => Some("debug/cone".to_string()),
        "Model" => args
            .get("path")
            .and_then(vscene_value_string)
            .map(|path| format!("model:{path}")),
        _ => None,
    }
}

fn vscene_mesh_primitive_scale(object: &VsceneBlock, mesh: Option<&VsceneBlock>) -> Option<Vec3> {
    mesh.and_then(vscene_mesh_primitive_scale_from_block)
        .or_else(|| call_property(object, "mesh").and_then(vscene_mesh_primitive_scale_from_call))
        .or_else(|| {
            call_property(object, "geometry").and_then(vscene_mesh_primitive_scale_from_call)
        })
}

fn vscene_mesh_primitive_scale_from_block(block: &VsceneBlock) -> Option<Vec3> {
    let kind = string_property(block, "builtin")
        .or_else(|| string_property(block, "type"))
        .or_else(|| string_property(block, "kind"))?;
    primitive_scale_from_kind(
        &kind,
        vec3_property(block, "size"),
        number_property(block, "radius"),
        number_property(block, "height").or_else(|| number_property(block, "depth")),
    )
}

fn vscene_mesh_primitive_scale_from_call(value: &VsceneValue) -> Option<Vec3> {
    let VsceneValue::Call { function, args } = value else {
        return None;
    };
    primitive_scale_from_kind(
        function,
        vscene_arg_vec3(args, "size"),
        vscene_arg_number(args, "radius"),
        vscene_arg_number(args, "height").or_else(|| vscene_arg_number(args, "depth")),
    )
}

fn primitive_scale_from_kind(
    kind: &str,
    size: Option<Vec3>,
    radius: Option<f32>,
    height: Option<f32>,
) -> Option<Vec3> {
    match kind {
        "Box" | "Cube" | "box" | "cube" | "primitive.box" | "debug/cube" => size,
        "Sphere" | "sphere" | "primitive.sphere" | "debug/sphere" => {
            radius.map(|radius| Vec3::new(radius * 2.0, radius * 2.0, radius * 2.0))
        }
        "Cylinder" | "cylinder" | "primitive.cylinder" | "debug/cylinder" => {
            let diameter = radius.unwrap_or(0.5) * 2.0;
            Some(Vec3::new(diameter, height.unwrap_or(1.0), diameter))
        }
        "Cone" | "cone" | "primitive.cone" | "debug/cone" => {
            let diameter = radius.unwrap_or(0.5) * 2.0;
            Some(Vec3::new(diameter, height.unwrap_or(1.0), diameter))
        }
        "Plane" | "plane" | "primitive.plane" | "debug/plane" => size,
        _ => None,
    }
}

fn preserve_explicit_collider_size(components: &mut [ComponentData], primitive_scale: Vec3) {
    for component in components {
        let ComponentData::Collider(collider) = component else {
            continue;
        };
        collider.size = Vec3::new(
            divide_or_zero(collider.size.x, primitive_scale.x),
            divide_or_zero(collider.size.y, primitive_scale.y),
            divide_or_zero(collider.size.z, primitive_scale.z),
        );
    }
}

fn divide_or_zero(value: f32, divisor: f32) -> f32 {
    if divisor.abs() <= f32::EPSILON {
        0.0
    } else {
        value / divisor
    }
}

fn vscene_material_builtin(block: &VsceneBlock) -> Option<String> {
    if block.properties.contains_key("baseColor")
        || block.properties.contains_key("color")
        || block.properties.contains_key("emissive")
        || block.properties.contains_key("metallic")
        || block.properties.contains_key("roughness")
    {
        return Some(vscene_inline_material_name(block));
    }
    string_property(block, "builtin")
        .or_else(|| string_property(block, "name"))
        .or_else(|| string_property(block, "type"))
        .or_else(|| string_property(block, "kind"))
}

fn vscene_inline_material_name(block: &VsceneBlock) -> String {
    let base_color = vec3_property(block, "baseColor")
        .or_else(|| vec3_property(block, "color"))
        .unwrap_or(Vec3::ONE);
    let alpha = number_property(block, "alpha").unwrap_or(1.0);
    let metallic = number_property(block, "metallic").unwrap_or(0.0);
    let roughness = number_property(block, "roughness").unwrap_or(0.5);
    let emissive = vec3_property(block, "emissive").unwrap_or(Vec3::ZERO);
    format!(
        "@vscene-material:base={},{},{},{};metallic={};roughness={};emissive={},{},{}",
        base_color.x,
        base_color.y,
        base_color.z,
        alpha,
        metallic,
        roughness,
        emissive.x,
        emissive.y,
        emissive.z
    )
}

fn compile_rigidbody_component(block: &VsceneBlock) -> RigidbodyComponentData {
    RigidbodyComponentData {
        body_type: identifier_property(block, "mode").unwrap_or_else(|| "dynamic".to_string()),
        mass: number_property(block, "mass").unwrap_or(1.0),
        use_gravity: bool_property(block, "useGravity").unwrap_or(true),
        linear_damping: 0.0,
        angular_damping: 0.05,
        lock_position: [false, false, false],
        lock_rotation: [false, false, false],
    }
}

fn compile_collider_component(block: &VsceneBlock) -> ColliderComponentData {
    ColliderComponentData {
        shape: identifier_property(block, "shape").unwrap_or_else(|| "box".to_string()),
        size: vec3_property(block, "size").unwrap_or(Vec3::ONE),
        is_trigger: bool_property(block, "isTrigger").unwrap_or(false),
        mask: u32::MAX,
        physics_material: "default".to_string(),
    }
}

fn compile_script_component(block: &VsceneBlock) -> ScriptComponent {
    let mut exported = HashMap::new();
    for (key, value) in &block.properties {
        if key == "source" {
            continue;
        }
        exported.insert(key.clone(), vscene_value_to_json(value));
    }
    ScriptComponent {
        source: string_property(block, "source").unwrap_or_default(),
        exported_values: exported,
        state: HashMap::new(),
    }
}

fn compile_light_component(block: &VsceneBlock) -> LightComponentData {
    let mut light = LightComponentData {
        kind: identifier_property(block, "kind")
            .or_else(|| identifier_property(block, "type"))
            .unwrap_or_else(|| "point".to_string()),
        ..LightComponentData::default()
    };
    light.color = vec3_property(block, "color").unwrap_or(Vec3::ONE);
    light.intensity = number_property(block, "intensity").unwrap_or(1.0);
    light.range = number_property(block, "range").unwrap_or(10.0);
    light.spot_angle = number_property(block, "spotAngle").unwrap_or(30.0);
    light.casts_shadow = bool_property(block, "castsShadow").unwrap_or(true);
    light.source_radius = number_property(block, "sourceRadius").unwrap_or(0.0);
    light.temperature_kelvin = number_property(block, "temperatureKelvin").unwrap_or(0.0);
    light.contact_shadow_strength = number_property(block, "contactShadowStrength").unwrap_or(0.0);
    light.indirect_energy = number_property(block, "indirectEnergy").unwrap_or(1.0);
    light.specular = number_property(block, "specular").unwrap_or(1.0);
    light.attenuation = number_property(block, "attenuation").unwrap_or(2.0);
    light.shadow_bias = number_property(block, "shadowBias").unwrap_or(0.0008);
    light.shadow_normal_bias = number_property(block, "shadowNormalBias").unwrap_or(0.0025);
    light.shadow_fade_start = number_property(block, "shadowFadeStart").unwrap_or(0.8);
    light.shadow_max_distance = number_property(block, "shadowMaxDistance").unwrap_or(200.0);
    light.cull_mask = u32_property(block, "cullMask").unwrap_or(u32::MAX);
    light.shadow_caster_mask = u32_property(block, "shadowCasterMask").unwrap_or(u32::MAX);
    light.directional_shadow_blend_splits =
        bool_property(block, "directionalShadowBlendSplits").unwrap_or(true);
    light.directional_shadow_split_1 =
        number_property(block, "directionalShadowSplit1").unwrap_or(0.1);
    light.directional_shadow_split_2 =
        number_property(block, "directionalShadowSplit2").unwrap_or(0.28);
    light.directional_shadow_split_3 =
        number_property(block, "directionalShadowSplit3").unwrap_or(0.55);
    light.projector = string_property(block, "projector").filter(|value| !value.is_empty());
    if let Some(mode) = identifier_property(block, "bakeMode") {
        light.bake_mode = parse_light_bake_mode(&mode);
    }
    if let Some(mode) = identifier_property(block, "directionalShadowMode") {
        light.directional_shadow_mode = parse_directional_shadow_mode(&mode);
    }
    light
}

fn parse_light_bake_mode(value: &str) -> engine_ecs::LightBakeMode {
    match value {
        "disabled" => engine_ecs::LightBakeMode::Disabled,
        "static" => engine_ecs::LightBakeMode::Static,
        _ => engine_ecs::LightBakeMode::Dynamic,
    }
}

fn parse_directional_shadow_mode(value: &str) -> engine_ecs::DirectionalShadowMode {
    match value {
        "orthogonal" => engine_ecs::DirectionalShadowMode::Orthogonal,
        "parallel-2-splits" | "parallel2" | "pssm2" => {
            engine_ecs::DirectionalShadowMode::Parallel2Splits
        }
        _ => engine_ecs::DirectionalShadowMode::Parallel4Splits,
    }
}

fn light_bake_mode_name(value: engine_ecs::LightBakeMode) -> &'static str {
    match value {
        engine_ecs::LightBakeMode::Disabled => "disabled",
        engine_ecs::LightBakeMode::Static => "static",
        engine_ecs::LightBakeMode::Dynamic => "dynamic",
    }
}

fn directional_shadow_mode_name(value: engine_ecs::DirectionalShadowMode) -> &'static str {
    match value {
        engine_ecs::DirectionalShadowMode::Orthogonal => "orthogonal",
        engine_ecs::DirectionalShadowMode::Parallel2Splits => "parallel-2-splits",
        engine_ecs::DirectionalShadowMode::Parallel4Splits => "parallel-4-splits",
    }
}

fn upsert_component(components: &mut Vec<ComponentData>, component: ComponentData) {
    let component_type = component.type_id();
    if let Some(existing) = components
        .iter_mut()
        .find(|candidate| candidate.type_id() == component_type)
    {
        *existing = component;
    } else {
        components.push(component);
    }
}

fn parse_vscene_value(source: &str) -> Option<VsceneValue> {
    let source = source.trim();
    if source == "true" {
        return Some(VsceneValue::Bool(true));
    }
    if source == "false" {
        return Some(VsceneValue::Bool(false));
    }
    if let Ok(number) = source.parse::<f32>() {
        return Some(VsceneValue::Number(number));
    }
    if let Some(value) = parse_string_literal(source) {
        return Some(VsceneValue::String(value));
    }
    if let Some(args) = source
        .strip_prefix("Vec3(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let parts = split_top_level_commas(args);
        if parts.len() == 3 {
            return Some(VsceneValue::Vec3(Vec3::new(
                parts[0].trim().parse().ok()?,
                parts[1].trim().parse().ok()?,
                parts[2].trim().parse().ok()?,
            )));
        }
    }
    if let Some(args) = source
        .strip_prefix("Color(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let raw = parse_string_literal(args.trim())?;
        return parse_hex_color(&raw).map(VsceneValue::Color);
    }
    if let Some((function, args)) = parse_expression_call(source) {
        let mut parsed_args = HashMap::new();
        for arg in split_top_level_commas(args) {
            let (key, value) = arg.split_once(':')?;
            parsed_args.insert(key.trim().to_string(), parse_vscene_value(value.trim())?);
        }
        return Some(VsceneValue::Call {
            function: function.to_string(),
            args: parsed_args,
        });
    }
    is_vscene_identifier(source).then(|| VsceneValue::Identifier(source.to_string()))
}

fn is_vscene_identifier(source: &str) -> bool {
    let mut chars = source.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| {
            ch == '_' || ch == '-' || ch == '/' || ch == '.' || ch.is_ascii_alphanumeric()
        })
}

fn parse_hex_color(source: &str) -> Option<Vec3> {
    let hex = source.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    Some(Vec3::new(r, g, b))
}

fn number_property(block: &VsceneBlock, key: &str) -> Option<f32> {
    match block.properties.get(key)? {
        VsceneValue::Number(value) => Some(*value),
        _ => None,
    }
}

fn u32_property(block: &VsceneBlock, key: &str) -> Option<u32> {
    match block.properties.get(key)? {
        VsceneValue::Number(value) if *value >= 0.0 => Some(*value as u32),
        VsceneValue::String(value) | VsceneValue::Identifier(value) => value.parse().ok(),
        _ => None,
    }
}

fn bool_property(block: &VsceneBlock, key: &str) -> Option<bool> {
    match block.properties.get(key)? {
        VsceneValue::Bool(value) => Some(*value),
        _ => None,
    }
}

fn string_property(block: &VsceneBlock, key: &str) -> Option<String> {
    match block.properties.get(key)? {
        VsceneValue::String(value) | VsceneValue::Identifier(value) => Some(value.clone()),
        _ => None,
    }
}

fn identifier_property(block: &VsceneBlock, key: &str) -> Option<String> {
    match block.properties.get(key)? {
        VsceneValue::Identifier(value) | VsceneValue::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn call_property<'a>(block: &'a VsceneBlock, key: &str) -> Option<&'a VsceneValue> {
    match block.properties.get(key)? {
        value @ VsceneValue::Call { .. } => Some(value),
        _ => None,
    }
}

fn vscene_value_string(value: &VsceneValue) -> Option<String> {
    match value {
        VsceneValue::String(value) | VsceneValue::Identifier(value) => Some(value.clone()),
        _ => None,
    }
}

fn vscene_arg_number(args: &HashMap<String, VsceneValue>, key: &str) -> Option<f32> {
    match args.get(key)? {
        VsceneValue::Number(value) => Some(*value),
        _ => None,
    }
}

fn vscene_arg_vec3(args: &HashMap<String, VsceneValue>, key: &str) -> Option<Vec3> {
    match args.get(key)? {
        VsceneValue::Vec3(value) | VsceneValue::Color(value) => Some(*value),
        _ => None,
    }
}

fn vec3_property(block: &VsceneBlock, key: &str) -> Option<Vec3> {
    match block.properties.get(key)? {
        VsceneValue::Vec3(value) | VsceneValue::Color(value) => Some(*value),
        _ => None,
    }
}

fn vscene_value_to_json(value: &VsceneValue) -> serde_json::Value {
    match value {
        VsceneValue::Number(value) => serde_json::json!(value),
        VsceneValue::Bool(value) => serde_json::json!(value),
        VsceneValue::String(value) | VsceneValue::Identifier(value) => serde_json::json!(value),
        VsceneValue::Vec3(value) | VsceneValue::Color(value) => vec3_json(*value),
        VsceneValue::Call { function, args } => {
            let mut object = serde_json::Map::new();
            object.insert("type".to_string(), serde_json::json!(function));
            for (key, value) in args {
                object.insert(key.clone(), vscene_value_to_json(value));
            }
            serde_json::Value::Object(object)
        }
    }
}

fn vec3_json(value: Vec3) -> serde_json::Value {
    serde_json::json!({
        "x": value.x,
        "y": value.y,
        "z": value.z,
    })
}

fn vscene_compile_error(
    block: &VsceneBlock,
    code: &str,
    message: &str,
    expected: &str,
    suggestion: &str,
) -> VargDiagnostic {
    VargDiagnostic {
        code: code.to_string(),
        severity: VargDiagnosticSeverity::Error,
        line: Some(block.line),
        column: Some(1),
        message: message.to_string(),
        expected: expected.to_string(),
        suggestion: suggestion.to_string(),
        blocking: true,
        source_line: None,
    }
}

fn write_vscene_object(
    output: &mut String,
    record: &SerializedGameObject,
    indent: usize,
) -> engine_core::EngineResult<()> {
    let is_camera = record.object.camera_role == Some(CameraRole::Main)
        || record
            .object
            .components
            .iter()
            .any(|component| matches!(component, ComponentData::Camera(_)));
    let standalone_light = (!is_camera)
        .then(|| {
            record
                .object
                .components
                .iter()
                .find_map(|component| match component {
                    ComponentData::Light(light) if record.object.components.len() == 1 => {
                        Some(light)
                    }
                    _ => None,
                })
        })
        .flatten();
    write_indent(output, indent);
    output.push_str(if is_camera {
        "camera "
    } else if standalone_light.is_some() {
        "light "
    } else {
        "entity "
    });
    output.push_str(&vscene_quoted(&record.object.name));
    output.push_str(" {\n");

    if !is_camera && standalone_light.is_none() && record.object.tag != "Untagged" {
        write_property(
            output,
            indent + 1,
            "tag",
            &vscene_quoted(&record.object.tag),
        );
    }

    write_transform_block(output, indent + 1, record.local_transform);
    if let Some(light) = standalone_light {
        write_light_properties(output, indent + 1, light);
    }

    for component in &record.object.components {
        if standalone_light.is_some() && matches!(component, ComponentData::Light(_)) {
            continue;
        }
        match component {
            ComponentData::Camera(camera) => write_camera_block(output, indent + 1, camera),
            ComponentData::MeshRenderer(mesh) => {
                write_mesh_renderer_block(output, indent + 1, mesh)?
            }
            ComponentData::Rigidbody(rigidbody) => {
                write_rigidbody_block(output, indent + 1, rigidbody);
            }
            ComponentData::Collider(collider) => write_collider_block(output, indent + 1, collider),
            ComponentData::Script(script) => write_script_block(output, indent + 1, script),
            ComponentData::Light(light) => write_light_block(output, indent + 1, light),
            other => {
                return Err(engine_core::EngineError::config(format!(
                    "native .vscene writer does not support {} components yet",
                    other.type_id()
                )));
            }
        }
    }

    write_indent(output, indent);
    output.push_str("}\n\n");
    Ok(())
}

fn write_transform_block(output: &mut String, indent: usize, transform: Transform) {
    write_indent(output, indent);
    output.push_str("transform {\n");
    write_property(
        output,
        indent + 1,
        "position",
        &vscene_vec3(transform.translation),
    );
    let (yaw, pitch, roll) = transform.rotation.to_euler_deg();
    write_property(
        output,
        indent + 1,
        "rotation",
        &vscene_vec3(Vec3::new(roll, pitch, yaw)),
    );
    write_property(output, indent + 1, "scale", &vscene_vec3(transform.scale));
    write_indent(output, indent);
    output.push_str("}\n");
}

fn write_camera_block(output: &mut String, indent: usize, camera: &CameraComponentData) {
    write_indent(output, indent);
    output.push_str("perspective {\n");
    write_property(
        output,
        indent + 1,
        "fov",
        &vscene_number(camera.vertical_fov_degrees),
    );
    write_property(output, indent + 1, "near", &vscene_number(camera.near));
    write_property(output, indent + 1, "far", &vscene_number(camera.far));
    write_indent(output, indent);
    output.push_str("}\n");
    write_property(
        output,
        indent,
        "primary",
        if camera.primary { "true" } else { "false" },
    );
}

fn write_mesh_renderer_block(
    output: &mut String,
    indent: usize,
    mesh: &MeshRendererComponentData,
) -> engine_core::EngineResult<()> {
    if mesh.mesh.is_some() {
        return Err(engine_core::EngineError::config(
            "native .vscene writer does not support asset mesh references yet",
        ));
    }
    let builtin_mesh = mesh
        .builtin_mesh
        .as_deref()
        .unwrap_or("debug/cube")
        .to_string();
    write_property(output, indent, "mesh", &builtin_mesh);
    if let Some(builtin_material) = mesh.material.builtin.as_deref() {
        write_indent(output, indent);
        output.push_str("material {\n");
        write_property(
            output,
            indent + 1,
            "builtin",
            &vscene_quoted(builtin_material),
        );
        write_indent(output, indent);
        output.push_str("}\n");
    }
    Ok(())
}

fn write_rigidbody_block(output: &mut String, indent: usize, rigidbody: &RigidbodyComponentData) {
    write_indent(output, indent);
    output.push_str("rigidbody {\n");
    write_property(output, indent + 1, "mode", &rigidbody.body_type);
    write_property(output, indent + 1, "mass", &vscene_number(rigidbody.mass));
    write_property(
        output,
        indent + 1,
        "useGravity",
        if rigidbody.use_gravity {
            "true"
        } else {
            "false"
        },
    );
    write_indent(output, indent);
    output.push_str("}\n");
}

fn write_collider_block(output: &mut String, indent: usize, collider: &ColliderComponentData) {
    write_indent(output, indent);
    output.push_str("collider {\n");
    write_property(output, indent + 1, "shape", &collider.shape);
    write_property(output, indent + 1, "size", &vscene_vec3(collider.size));
    write_property(
        output,
        indent + 1,
        "isTrigger",
        if collider.is_trigger { "true" } else { "false" },
    );
    write_indent(output, indent);
    output.push_str("}\n");
}

fn write_script_block(output: &mut String, indent: usize, script: &ScriptComponent) {
    write_indent(output, indent);
    output.push_str("script ");
    output.push_str(&vscene_block_name(
        script
            .source
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".varg"))
            .unwrap_or("Script"),
    ));
    output.push_str(" {\n");
    write_property(output, indent + 1, "source", &vscene_quoted(&script.source));
    let mut exported = script.exported_values.iter().collect::<Vec<_>>();
    exported.sort_by(|left, right| left.0.cmp(right.0));
    for (key, value) in exported {
        write_property(output, indent + 1, key, &json_value_to_vscene(value));
    }
    write_indent(output, indent);
    output.push_str("}\n");
}

fn write_light_block(output: &mut String, indent: usize, light: &LightComponentData) {
    write_indent(output, indent);
    output.push_str("light {\n");
    write_light_properties(output, indent + 1, light);
    write_indent(output, indent);
    output.push_str("}\n");
}

fn write_light_properties(output: &mut String, indent: usize, light: &LightComponentData) {
    write_property(output, indent, "kind", &light.kind);
    write_property(output, indent, "color", &vscene_vec3(light.color));
    write_property(output, indent, "intensity", &vscene_number(light.intensity));
    write_property(output, indent, "range", &vscene_number(light.range));
    write_property(
        output,
        indent,
        "spotAngle",
        &vscene_number(light.spot_angle),
    );
    write_property(
        output,
        indent,
        "castsShadow",
        if light.casts_shadow { "true" } else { "false" },
    );
    write_property(
        output,
        indent,
        "sourceRadius",
        &vscene_number(light.source_radius),
    );
    write_property(
        output,
        indent,
        "temperatureKelvin",
        &vscene_number(light.temperature_kelvin),
    );
    write_property(
        output,
        indent,
        "contactShadowStrength",
        &vscene_number(light.contact_shadow_strength),
    );
    write_property(
        output,
        indent,
        "indirectEnergy",
        &vscene_number(light.indirect_energy),
    );
    write_property(output, indent, "specular", &vscene_number(light.specular));
    write_property(
        output,
        indent,
        "attenuation",
        &vscene_number(light.attenuation),
    );
    write_property(
        output,
        indent,
        "shadowBias",
        &vscene_number(light.shadow_bias),
    );
    write_property(
        output,
        indent,
        "shadowNormalBias",
        &vscene_number(light.shadow_normal_bias),
    );
    write_property(
        output,
        indent,
        "shadowFadeStart",
        &vscene_number(light.shadow_fade_start),
    );
    write_property(
        output,
        indent,
        "shadowMaxDistance",
        &vscene_number(light.shadow_max_distance),
    );
    write_property(output, indent, "cullMask", &light.cull_mask.to_string());
    write_property(
        output,
        indent,
        "shadowCasterMask",
        &light.shadow_caster_mask.to_string(),
    );
    write_property(
        output,
        indent,
        "bakeMode",
        light_bake_mode_name(light.bake_mode),
    );
    write_property(
        output,
        indent,
        "directionalShadowMode",
        directional_shadow_mode_name(light.directional_shadow_mode),
    );
    write_property(
        output,
        indent,
        "directionalShadowBlendSplits",
        if light.directional_shadow_blend_splits {
            "true"
        } else {
            "false"
        },
    );
    write_property(
        output,
        indent,
        "directionalShadowSplit1",
        &vscene_number(light.directional_shadow_split_1),
    );
    write_property(
        output,
        indent,
        "directionalShadowSplit2",
        &vscene_number(light.directional_shadow_split_2),
    );
    write_property(
        output,
        indent,
        "directionalShadowSplit3",
        &vscene_number(light.directional_shadow_split_3),
    );
    if let Some(projector) = &light.projector {
        write_property(output, indent, "projector", &vscene_quoted(projector));
    }
}

fn write_property(output: &mut String, indent: usize, key: &str, value: &str) {
    write_indent(output, indent);
    output.push_str(key);
    output.push_str(": ");
    output.push_str(value);
    output.push('\n');
}

fn write_indent(output: &mut String, indent: usize) {
    for _ in 0..indent {
        output.push_str("    ");
    }
}

fn vscene_block_name(name: &str) -> String {
    if is_vscene_identifier(name) {
        name.to_string()
    } else {
        vscene_quoted(name)
    }
}

fn vscene_quoted(value: &str) -> String {
    format!("{:?}", value)
}

fn vscene_vec3(value: Vec3) -> String {
    format!(
        "Vec3({}, {}, {})",
        vscene_number(value.x),
        vscene_number(value.y),
        vscene_number(value.z)
    )
}

fn vscene_number(value: f32) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn json_value_to_vscene(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => vscene_quoted(value),
        serde_json::Value::Object(object) => {
            if let (Some(x), Some(y), Some(z)) = (object.get("x"), object.get("y"), object.get("z"))
            {
                return format!(
                    "Vec3({}, {}, {})",
                    json_value_to_vscene(x),
                    json_value_to_vscene(y),
                    json_value_to_vscene(z)
                );
            }
            vscene_quoted(&value.to_string())
        }
        _ => vscene_quoted(&value.to_string()),
    }
}

fn vscene_error(
    source: &str,
    line: usize,
    column: usize,
    code: &str,
    message: &str,
    expected: &str,
    suggestion: &str,
) -> VargDiagnostic {
    VargDiagnostic {
        code: code.to_string(),
        severity: VargDiagnosticSeverity::Error,
        line: Some(line),
        column: Some(column),
        message: message.to_string(),
        expected: expected.to_string(),
        suggestion: suggestion.to_string(),
        blocking: true,
        source_line: source
            .lines()
            .nth(line.saturating_sub(1))
            .map(str::to_string),
    }
}
