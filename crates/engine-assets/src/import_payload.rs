use crate::prelude::*;
#[cfg(feature = "importers")]
use crate::vmodel::compile_vmodel;
use crate::*;

pub(crate) fn discover_asset_dependencies(
    path: &Path,
    kind: ResourceKind,
    importer: &str,
) -> EngineResult<Vec<AssetGuid>> {
    if kind != ResourceKind::Material {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|source| EngineError::Filesystem {
        path: path.to_path_buf(),
        source,
    })?;
    let material = parse_material_format(&text, importer).map_err(EngineError::from)?;
    let mut dependencies = material.textures.values().copied().collect::<Vec<_>>();
    if material.shader != AssetGuid::from_u128(0) {
        dependencies.push(material.shader);
    }
    dependencies.sort();
    dependencies.dedup();
    Ok(dependencies)
}

/// Runs a built-in import task into CPU cache and queues a GPU upload.
#[cfg(feature = "importers")]
pub fn import_builtin_asset(
    project_asset_root: impl AsRef<Path>,
    registry: &mut AssetRegistry,
    task: ImportTask,
) -> EngineResult<ImportOutcome> {
    let handle = registry.register(task.guid, task.kind)?;
    registry.set_state(handle, ResourceState::LoadingCpu)?;
    let path = project_asset_root.as_ref().join(&task.source_path);
    let mut file = fs::File::open(&path).map_err(|source| EngineError::Filesystem {
        path: path.clone(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| EngineError::Filesystem {
            path: path.clone(),
            source,
        })?;
    let imported = import_cpu_payload(&path, task.kind, &task.importer, &bytes);
    registry.put_cpu(
        handle,
        CpuResource {
            kind: task.kind,
            bytes: imported.bytes,
        },
    )?;
    registry.set_preview(
        handle,
        PreviewData {
            thumbnail: None,
            summary: imported.summary,
        },
    )?;
    Ok(ImportOutcome {
        guid: task.guid,
        diagnostics: imported.diagnostics,
        upload: Some(GpuUploadTask {
            handle,
            kind: task.kind,
        }),
    })
}

#[cfg(feature = "importers")]
struct ImportedCpuPayload {
    bytes: Arc<[u8]>,
    summary: String,
    diagnostics: Vec<AssetDiagnostic>,
}

#[cfg(feature = "importers")]
fn import_cpu_payload(
    path: &Path,
    kind: ResourceKind,
    importer: &str,
    bytes: &[u8],
) -> ImportedCpuPayload {
    match kind {
        ResourceKind::Texture => import_texture_payload(path, importer, bytes),
        ResourceKind::Model | ResourceKind::SkinnedModel => {
            import_model_payload(path, importer, bytes)
        }
        ResourceKind::Shader => import_shader_payload(path, importer, bytes),
        ResourceKind::Material => import_material_payload(path, importer, bytes),
        ResourceKind::Audio
        | ResourceKind::Animation
        | ResourceKind::Script
        | ResourceKind::Prefab
        | ResourceKind::Scene => ImportedCpuPayload {
            bytes: Arc::from(bytes),
            summary: format!("{} bytes imported by {}", bytes.len(), importer),
            diagnostics: Vec::new(),
        },
    }
}

#[cfg(feature = "importers")]
fn import_texture_payload(path: &Path, importer: &str, bytes: &[u8]) -> ImportedCpuPayload {
    if importer == "cubemap-json" {
        return import_cubemap_payload(path, importer, bytes);
    }

    let mut diagnostics = Vec::new();
    let (payload, summary) = match image::load_from_memory(bytes) {
        Ok(image) => {
            let rgba = image.to_rgba8();
            let width = rgba.width();
            let height = rgba.height();
            let texture = DecodedTextureResource {
                width,
                height,
                format: "rgba8_srgb".to_string(),
                pixels: rgba.into_raw(),
            };
            match texture.to_bytes() {
                Ok(bytes) => (
                    bytes,
                    format!("decoded {width}x{height} rgba8_srgb texture by {importer}"),
                ),
                Err(error) => {
                    diagnostics.push(
                        AssetDiagnostic::new(format!("texture encode failed: {error}"))
                            .with_path(path),
                    );
                    (
                        Arc::from(bytes),
                        format!(
                            "{} bytes texture source imported by {importer}",
                            bytes.len()
                        ),
                    )
                }
            }
        }
        Err(error) => {
            diagnostics.push(
                AssetDiagnostic::new(format!("texture decode failed: {error}")).with_path(path),
            );
            let summary = if let Some((format, width, height)) = parse_image_dimensions(bytes) {
                format!("{format} {width}x{height} texture source imported by {importer}")
            } else {
                format!(
                    "{} bytes texture source imported by {importer}",
                    bytes.len()
                )
            };
            (Arc::from(bytes), summary)
        }
    };
    ImportedCpuPayload {
        bytes: payload,
        summary,
        diagnostics,
    }
}

#[cfg(feature = "importers")]
fn import_cubemap_payload(path: &Path, importer: &str, bytes: &[u8]) -> ImportedCpuPayload {
    let mut diagnostics = Vec::new();
    let source = match serde_json::from_slice::<CubemapSource>(bytes) {
        Ok(source) => source,
        Err(error) => {
            diagnostics.push(
                AssetDiagnostic::new(format!("cubemap manifest parse failed: {error}"))
                    .with_path(path),
            );
            return ImportedCpuPayload {
                bytes: Arc::from(bytes),
                summary: format!(
                    "{} bytes cubemap source imported by {importer}",
                    bytes.len()
                ),
                diagnostics,
            };
        }
    };

    let base_dir = path.parent().unwrap_or_else(|| Path::new(""));
    let faces = [
        source.positive_x,
        source.negative_x,
        source.positive_y,
        source.negative_y,
        source.positive_z,
        source.negative_z,
    ];
    let mut face_size = None;
    let mut pixels = Vec::new();
    for face in faces {
        let face_path = base_dir.join(&face);
        let face_bytes = match fs::read(&face_path) {
            Ok(bytes) => bytes,
            Err(source) => {
                diagnostics.push(
                    AssetDiagnostic::new(format!("cubemap face read failed: {source}"))
                        .with_path(face_path),
                );
                return ImportedCpuPayload {
                    bytes: Arc::from(bytes),
                    summary: format!(
                        "{} bytes cubemap source imported by {importer}",
                        bytes.len()
                    ),
                    diagnostics,
                };
            }
        };
        let image = match image::load_from_memory(&face_bytes) {
            Ok(image) => image.to_rgba8(),
            Err(error) => {
                diagnostics.push(
                    AssetDiagnostic::new(format!("cubemap face decode failed: {error}"))
                        .with_path(face_path),
                );
                return ImportedCpuPayload {
                    bytes: Arc::from(bytes),
                    summary: format!(
                        "{} bytes cubemap source imported by {importer}",
                        bytes.len()
                    ),
                    diagnostics,
                };
            }
        };
        if image.width() != image.height() {
            diagnostics
                .push(AssetDiagnostic::new("cubemap face must be square").with_path(face_path));
            return ImportedCpuPayload {
                bytes: Arc::from(bytes),
                summary: format!(
                    "{} bytes cubemap source imported by {importer}",
                    bytes.len()
                ),
                diagnostics,
            };
        }
        match face_size {
            Some(size) if size != image.width() => {
                diagnostics.push(
                    AssetDiagnostic::new("all cubemap faces must have identical dimensions")
                        .with_path(face_path),
                );
                return ImportedCpuPayload {
                    bytes: Arc::from(bytes),
                    summary: format!(
                        "{} bytes cubemap source imported by {importer}",
                        bytes.len()
                    ),
                    diagnostics,
                };
            }
            Some(_) => {}
            None => face_size = Some(image.width()),
        }
        pixels.extend_from_slice(&image.into_raw());
    }

    let face_size = face_size.unwrap_or(1);
    let cubemap = DecodedCubemapResource {
        face_size,
        format: "cubemap_rgba8_srgb".to_string(),
        pixels,
    };
    let payload = match cubemap.to_bytes() {
        Ok(bytes) => bytes,
        Err(error) => {
            diagnostics.push(
                AssetDiagnostic::new(format!("cubemap encode failed: {error}")).with_path(path),
            );
            Arc::from(bytes)
        }
    };

    ImportedCpuPayload {
        bytes: payload,
        summary: format!("decoded {face_size}x{face_size}x6 rgba8_srgb cubemap by {importer}"),
        diagnostics,
    }
}

#[cfg(feature = "importers")]
fn import_model_payload(path: &Path, importer: &str, bytes: &[u8]) -> ImportedCpuPayload {
    let mut diagnostics = Vec::new();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let (payload, summary) = if extension.eq_ignore_ascii_case("vmodel") || importer == "vmodel" {
        match compile_vmodel(bytes) {
            Ok(model) => {
                let primitive_count = model.meshes.len();
                let vertex_count = model
                    .meshes
                    .iter()
                    .map(|mesh| mesh.positions.len())
                    .sum::<usize>();
                match model.to_bytes() {
                    Ok(bytes) => (
                        bytes,
                        format!(
                            ".vmodel compiled by {importer}: {primitive_count} mesh primitive{}, {vertex_count} vertices",
                            if primitive_count == 1 { "" } else { "s" }
                        ),
                    ),
                    Err(error) => {
                        diagnostics.push(
                            AssetDiagnostic::new(format!("model encode failed: {error}"))
                                .with_path(path),
                        );
                        (
                            Arc::from(bytes),
                            format!("{} bytes model source imported by {importer}", bytes.len()),
                        )
                    }
                }
            }
            Err(error) => {
                diagnostics.push(AssetDiagnostic::new(error.to_string()).with_path(path));
                (
                    Arc::from(bytes),
                    format!("{} bytes model source imported by {importer}", bytes.len()),
                )
            }
        }
    } else if extension.eq_ignore_ascii_case("gltf") || extension.eq_ignore_ascii_case("glb") {
        match import_gltf_model(path) {
            Ok(model) => {
                let primitive_count = model.meshes.len();
                match model.to_bytes() {
                    Ok(bytes) => (
                        bytes,
                        format!(
                            "glTF model imported by {importer}: {primitive_count} mesh primitives"
                        ),
                    ),
                    Err(error) => {
                        diagnostics.push(
                            AssetDiagnostic::new(format!("model encode failed: {error}"))
                                .with_path(path),
                        );
                        (
                            Arc::from(bytes),
                            format!("{} bytes model source imported by {importer}", bytes.len()),
                        )
                    }
                }
            }
            Err(error) => {
                diagnostics.push(
                    AssetDiagnostic::new(format!("glTF import failed: {error}")).with_path(path),
                );
                (
                    Arc::from(bytes),
                    format!("{} bytes model source imported by {importer}", bytes.len()),
                )
            }
        }
    } else {
        (
            Arc::from(bytes),
            format!("{} bytes model source imported by {importer}", bytes.len()),
        )
    };
    ImportedCpuPayload {
        bytes: payload,
        summary,
        diagnostics,
    }
}

#[cfg(feature = "importers")]
pub(crate) fn import_gltf_model(path: &Path) -> EngineResult<ModelResource> {
    let (document, buffers, _) =
        gltf::import(path).map_err(|error| EngineError::other(error.to_string()))?;
    let mut model = ModelResource::default();

    // Extract materials
    for material in document.materials() {
        let pbr = material.pbr_metallic_roughness();
        let base_color = pbr.base_color_factor();
        let metallic = pbr.metallic_factor();
        let roughness = pbr.roughness_factor();
        let emissive = material.emissive_factor();

        let alpha_mode = match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => "OPAQUE",
            gltf::material::AlphaMode::Blend => "BLEND",
            gltf::material::AlphaMode::Mask => "MASK",
        };
        let alpha_cutoff = material.alpha_cutoff().unwrap_or(0.5);

        // Extract texture references (store as relative paths for AssetDatabase resolution)
        let base_color_texture_ref = pbr.base_color_texture().and_then(|info| {
            let source = info.texture().source().source();
            match source {
                gltf::image::Source::Uri { uri, .. } => Some(uri.to_string()),
                _ => None,
            }
        });

        let normal_texture_ref = material.normal_texture().and_then(|info| {
            let source = info.texture().source().source();
            match source {
                gltf::image::Source::Uri { uri, .. } => Some(uri.to_string()),
                _ => None,
            }
        });

        let metallic_roughness_texture_ref = pbr.metallic_roughness_texture().and_then(|info| {
            let source = info.texture().source().source();
            match source {
                gltf::image::Source::Uri { uri, .. } => Some(uri.to_string()),
                _ => None,
            }
        });

        model.materials.push(CpuMaterialResource {
            name: material.name().unwrap_or("").to_string(),
            base_color,
            metallic,
            roughness,
            emissive,
            alpha_mode: alpha_mode.to_string(),
            alpha_cutoff,
            base_color_texture_ref,
            normal_texture_ref,
            metallic_roughness_texture_ref,
        });
    }

    // Extract meshes
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|data| &**data));
            let positions = reader
                .read_positions()
                .map(|items| items.collect::<Vec<_>>())
                .unwrap_or_default();
            if positions.is_empty() {
                continue;
            }
            // Read normals, or fill with default (0, 1, 0) if missing
            let normals = reader
                .read_normals()
                .map(|items| items.collect::<Vec<_>>())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let texcoords = reader
                .read_tex_coords(0)
                .map(|items| items.into_f32().collect::<Vec<_>>())
                .unwrap_or_default();
            let indices = reader
                .read_indices()
                .map(|items| items.into_u32().collect::<Vec<_>>())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            model.meshes.push(BasicMeshResource {
                positions,
                normals,
                texcoords,
                indices,
                material_index: primitive.material().index(),
            });
        }
    }
    Ok(model)
}

#[cfg(feature = "importers")]
fn import_material_payload(path: &Path, importer: &str, bytes: &[u8]) -> ImportedCpuPayload {
    let mut diagnostics = Vec::new();
    let material = std::str::from_utf8(bytes)
        .map_err(|error| AssetError::Parse {
            format: "material",
            diagnostic: AssetDiagnostic::new(error.to_string()).with_path(path),
        })
        .and_then(|text| parse_material_format(text, importer));
    let summary = match material {
        Ok(material) => format!(
            "material imported by {importer}: {} textures, {} parameters",
            material.textures.len(),
            material.parameters.len()
        ),
        Err(error) => {
            diagnostics.push(AssetDiagnostic::new(error.to_string()).with_path(path));
            format!(
                "{} bytes material source imported by {importer}",
                bytes.len()
            )
        }
    };
    ImportedCpuPayload {
        bytes: Arc::from(bytes),
        summary,
        diagnostics,
    }
}

fn parse_material_format(input: &str, importer: &str) -> Result<MaterialFormat, AssetError> {
    match importer {
        "material-toml" => MaterialFormat::from_toml(input),
        "vasset" => MaterialFormat::from_vasset(input),
        _ => MaterialFormat::from_json(input),
    }
}

#[cfg(feature = "importers")]
fn import_shader_payload(path: &Path, importer: &str, bytes: &[u8]) -> ImportedCpuPayload {
    let mut diagnostics = Vec::new();
    if std::str::from_utf8(bytes).is_err() {
        diagnostics.push(
            AssetDiagnostic::new("shader source is not valid UTF-8; queued raw bytes")
                .with_path(path),
        );
    }
    ImportedCpuPayload {
        bytes: Arc::from(bytes),
        summary: format!(
            "{} bytes shader source imported by {}",
            bytes.len(),
            importer
        ),
        diagnostics,
    }
}

#[cfg(feature = "importers")]
fn parse_image_dimensions(bytes: &[u8]) -> Option<(&'static str, u32, u32)> {
    parse_png_dimensions(bytes).or_else(|| parse_jpeg_dimensions(bytes))
}

#[cfg(feature = "importers")]
fn parse_png_dimensions(bytes: &[u8]) -> Option<(&'static str, u32, u32)> {
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some(("png", width, height))
}

#[cfg(feature = "importers")]
fn parse_jpeg_dimensions(bytes: &[u8]) -> Option<(&'static str, u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }
    let mut cursor = 2;
    while cursor + 9 < bytes.len() {
        if bytes[cursor] != 0xff {
            cursor += 1;
            continue;
        }
        let marker = bytes[cursor + 1];
        cursor += 2;
        if marker == 0xd8 || marker == 0xd9 {
            continue;
        }
        if cursor + 2 > bytes.len() {
            return None;
        }
        let segment_len = u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]) as usize;
        if segment_len < 2 || cursor + segment_len > bytes.len() {
            return None;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            let height = u16::from_be_bytes([bytes[cursor + 3], bytes[cursor + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[cursor + 5], bytes[cursor + 6]]) as u32;
            return Some(("jpeg", width, height));
        }
        cursor += segment_len;
    }
    None
}
