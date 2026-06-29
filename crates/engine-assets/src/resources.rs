use crate::error::ensure_schema;
use crate::prelude::*;
use crate::*;

/// Resource metadata stored beside source assets.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ResourceMetaFormat {
    /// Schema version.
    pub version: u32,
    /// Stable project GUID.
    pub guid: AssetGuid,
    /// Source path relative to the project asset root.
    pub source_path: PathBuf,
    /// Resource kind.
    pub kind: ResourceKind,
    /// Importer identifier.
    pub importer: String,
    /// GUID dependencies declared by the asset or importer.
    #[serde(default)]
    pub dependencies: Vec<AssetGuid>,
}

impl ResourceMetaFormat {
    /// Parses resource metadata from TOML.
    pub fn from_toml(input: &str) -> Result<Self, AssetError> {
        let parsed: Self = toml::from_str(input).map_err(|source| AssetError::Parse {
            format: "resource meta",
            diagnostic: AssetDiagnostic::new(source.to_string()),
        })?;
        ensure_schema("resource meta", parsed.version)?;
        Ok(parsed)
    }
}

/// Runtime resource metadata including import state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceMeta {
    /// Stable asset GUID.
    pub guid: AssetGuid,
    /// Path relative to the asset root.
    pub path: PathBuf,
    /// Resource kind.
    pub kind: ResourceKind,
    /// Current import / load state.
    pub import_state: ResourceState,
}

/// Texture resource metadata.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TextureResource {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Mipmap count.
    pub mip_levels: u32,
    /// Pixel format name.
    pub format: String,
}

/// Material file format.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct MaterialFormat {
    /// Schema version.
    pub version: u32,
    /// Shader dependency.
    pub shader: AssetGuid,
    /// Texture dependencies keyed by slot name.
    #[serde(default)]
    pub textures: HashMap<String, AssetGuid>,
    /// Numeric material parameters keyed by name.
    #[serde(default)]
    pub parameters: HashMap<String, f32>,
}

impl MaterialFormat {
    /// Parses a material from JSON.
    pub fn from_json(input: &str) -> Result<Self, AssetError> {
        let parsed: Self = serde_json::from_str(input).map_err(|source| AssetError::Parse {
            format: "material",
            diagnostic: AssetDiagnostic::new(source.to_string()),
        })?;
        ensure_schema("material", parsed.version)?;
        Ok(parsed)
    }

    /// Parses a material from TOML.
    pub fn from_toml(input: &str) -> Result<Self, AssetError> {
        let parsed: Self = toml::from_str(input).map_err(|source| AssetError::Parse {
            format: "material",
            diagnostic: AssetDiagnostic::new(source.to_string()),
        })?;
        ensure_schema("material", parsed.version)?;
        Ok(parsed)
    }

    /// Parses a native Varg material asset.
    pub fn from_vasset(input: &str) -> Result<Self, AssetError> {
        let mut shader = AssetGuid::from_u128(0);
        let mut parameters = HashMap::new();

        for raw_line in input.lines() {
            let line = raw_line
                .split_once("//")
                .map_or(raw_line, |(line, _)| line)
                .trim();
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim().trim_end_matches(',');
                match key {
                    "shader" => {
                        shader = parse_vasset_shader(value)?;
                    }
                    "roughness" | "metallic" => {
                        let parsed = value.parse::<f32>().map_err(|source| AssetError::Parse {
                            format: "vasset material",
                            diagnostic: AssetDiagnostic::new(format!(
                                "invalid {key} value `{value}`: {source}"
                            )),
                        })?;
                        parameters.insert(key.to_string(), parsed);
                    }
                    _ => {}
                }
            }
        }

        Ok(Self {
            version: CURRENT_SCHEMA_VERSION,
            shader,
            textures: HashMap::new(),
            parameters,
        })
    }
}

fn parse_vasset_shader(value: &str) -> Result<AssetGuid, AssetError> {
    let value = value.trim_matches('"');
    if value.eq_ignore_ascii_case("pbr") || value.eq_ignore_ascii_case("builtin/pbr") {
        return Ok(AssetGuid::from_u128(0));
    }
    u128::from_str_radix(value, 16)
        .map(AssetGuid::from_u128)
        .map_err(|source| AssetError::Parse {
            format: "vasset material",
            diagnostic: AssetDiagnostic::new(format!(
                "shader must be `pbr`, `builtin/pbr`, or a 128-bit hex GUID: {source}"
            )),
        })
}

/// Decoded texture payload ready for a render backend upload.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct DecodedTextureResource {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Runtime pixel format.
    pub format: String,
    /// Tightly packed RGBA pixels.
    pub pixels: Vec<u8>,
}

/// Decoded cubemap resource with six tightly packed square RGBA faces.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct DecodedCubemapResource {
    /// Width and height in pixels for every face.
    pub face_size: u32,
    /// Runtime pixel format.
    pub format: String,
    /// Six tightly packed RGBA faces in +X, -X, +Y, -Y, +Z, -Z order.
    pub pixels: Vec<u8>,
}

/// Source JSON for a six-image cubemap.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct CubemapSource {
    /// Positive X face path, relative to the cubemap source file.
    pub positive_x: PathBuf,
    /// Negative X face path, relative to the cubemap source file.
    pub negative_x: PathBuf,
    /// Positive Y face path, relative to the cubemap source file.
    pub positive_y: PathBuf,
    /// Negative Y face path, relative to the cubemap source file.
    pub negative_y: PathBuf,
    /// Positive Z face path, relative to the cubemap source file.
    pub positive_z: PathBuf,
    /// Negative Z face path, relative to the cubemap source file.
    pub negative_z: PathBuf,
}

/// CPU-side texture resource with mip chain for GPU upload.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct CpuTextureResource {
    /// Width in pixels (base mip level).
    pub width: u32,
    /// Height in pixels (base mip level).
    pub height: u32,
    /// Pixel format name.
    pub format: String,
    /// Mip levels, each containing tightly packed pixel data.
    /// Level 0 is the full resolution, each subsequent level is half resolution.
    pub mip_levels: Vec<Vec<u8>>,
}

impl CpuTextureResource {
    /// Serializes to JSON bytes.
    pub fn to_bytes(&self) -> EngineResult<Arc<[u8]>> {
        serde_json::to_vec(self)
            .map(Arc::from)
            .map_err(|error| EngineError::other(error.to_string()))
    }

    /// Parses from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AssetError> {
        serde_json::from_slice(bytes).map_err(|source| AssetError::Parse {
            format: "cpu texture resource",
            diagnostic: AssetDiagnostic::new(source.to_string()),
        })
    }
}

/// Import options for asset importers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImportOptions {
    /// Whether to generate mip chains for textures.
    pub generate_mips: bool,
    /// Maximum texture dimension (width or height).
    pub max_texture_size: Option<u32>,
}

impl DecodedTextureResource {
    /// Parses a decoded texture payload from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AssetError> {
        serde_json::from_slice(bytes).map_err(|source| AssetError::Parse {
            format: "decoded texture",
            diagnostic: AssetDiagnostic::new(source.to_string()),
        })
    }

    /// Serializes to JSON bytes.
    pub fn to_bytes(&self) -> EngineResult<Arc<[u8]>> {
        serde_json::to_vec(self)
            .map(Arc::from)
            .map_err(|error| EngineError::other(error.to_string()))
    }
}

impl DecodedCubemapResource {
    /// Parses a decoded cubemap payload from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AssetError> {
        serde_json::from_slice(bytes).map_err(|source| AssetError::Parse {
            format: "decoded cubemap",
            diagnostic: AssetDiagnostic::new(source.to_string()),
        })
    }

    /// Serializes to JSON bytes.
    pub fn to_bytes(&self) -> EngineResult<Arc<[u8]>> {
        serde_json::to_vec(self)
            .map(Arc::from)
            .map_err(|error| EngineError::other(error.to_string()))
    }
}

/// CPU-side mesh payload imported from a model file.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct BasicMeshResource {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals, if present.
    #[serde(default)]
    pub normals: Vec<[f32; 3]>,
    /// First texture coordinate set, if present.
    #[serde(default)]
    pub texcoords: Vec<[f32; 2]>,
    /// Triangle indices.
    #[serde(default)]
    pub indices: Vec<u32>,
    /// Material index referenced by the primitive, if present.
    pub material_index: Option<usize>,
}

/// CPU-side PBR material resource extracted from glTF.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct CpuMaterialResource {
    /// Material name from glTF.
    pub name: String,
    /// Base color factor (RGBA, default white).
    pub base_color: [f32; 4],
    /// Metallic factor (0.0 = dielectric, 1.0 = metal).
    pub metallic: f32,
    /// Roughness factor (0.0 = smooth, 1.0 = rough).
    pub roughness: f32,
    /// Emissive factor (RGB).
    #[serde(default)]
    pub emissive: [f32; 3],
    /// Alpha mode: "OPAQUE", "BLEND", or "MASK".
    #[serde(default = "default_alpha_mode")]
    pub alpha_mode: String,
    /// Alpha cutoff threshold for MASK mode.
    #[serde(default = "default_alpha_cutoff")]
    pub alpha_cutoff: f32,
    /// Base color texture reference (relative asset path).
    pub base_color_texture_ref: Option<String>,
    /// Normal map texture reference (relative asset path).
    pub normal_texture_ref: Option<String>,
    /// Metallic-roughness texture reference (relative asset path).
    pub metallic_roughness_texture_ref: Option<String>,
}

fn default_alpha_mode() -> String {
    "OPAQUE".to_string()
}

fn default_alpha_cutoff() -> f32 {
    0.5
}

impl Default for CpuMaterialResource {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0, 0.0, 0.0],
            alpha_mode: "OPAQUE".to_string(),
            alpha_cutoff: 0.5,
            base_color_texture_ref: None,
            normal_texture_ref: None,
            metallic_roughness_texture_ref: None,
        }
    }
}

/// Imported model payload containing basic static meshes.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct ModelResource {
    /// Mesh primitives available to runtime rendering.
    pub meshes: Vec<BasicMeshResource>,
    /// Materials extracted from the model.
    #[serde(default)]
    pub materials: Vec<CpuMaterialResource>,
}

impl ModelResource {
    /// Parses a model payload from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AssetError> {
        serde_json::from_slice(bytes).map_err(|source| AssetError::Parse {
            format: "model resource",
            diagnostic: AssetDiagnostic::new(source.to_string()),
        })
    }

    #[cfg(feature = "importers")]
    pub(crate) fn to_bytes(&self) -> EngineResult<Arc<[u8]>> {
        serde_json::to_vec(self)
            .map(Arc::from)
            .map_err(|error| EngineError::other(error.to_string()))
    }
}

/// Shader configuration file format.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ShaderConfigFormat {
    /// Schema version.
    pub version: u32,
    /// Shader stage entry points keyed by stage name.
    pub stages: HashMap<String, PathBuf>,
    /// Compile-time defines.
    #[serde(default)]
    pub defines: HashMap<String, String>,
}

impl ShaderConfigFormat {
    /// Parses shader configuration from TOML.
    pub fn from_toml(input: &str) -> Result<Self, AssetError> {
        let parsed: Self = toml::from_str(input).map_err(|source| AssetError::Parse {
            format: "shader config",
            diagnostic: AssetDiagnostic::new(source.to_string()),
        })?;
        ensure_schema("shader config", parsed.version)?;
        Ok(parsed)
    }
}
