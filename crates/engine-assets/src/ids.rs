use crate::prelude::*;

/// Current schema version for asset-side files.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Stable resource GUID serialized as 128 bits.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetGuid(u128);

impl AssetGuid {
    /// Creates a GUID from raw bits.
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }

    /// Creates a GUID from the core asset identifier type.
    pub const fn from_asset_id(id: AssetId) -> Self {
        Self(id.as_u128())
    }

    /// Returns the raw GUID bits.
    pub const fn as_u128(self) -> u128 {
        self.0
    }

    /// Converts this GUID to the core asset identifier type.
    pub const fn as_asset_id(self) -> AssetId {
        AssetId::from_u128(self.0)
    }
}

impl fmt::Display for AssetGuid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:032x}", self.0)
    }
}

impl Serialize for AssetGuid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for AssetGuid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AssetGuidVisitor;

        impl serde::de::Visitor<'_> for AssetGuidVisitor {
            type Value = AssetGuid;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a 128-bit asset GUID as hex string or unsigned integer")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let value = value.strip_prefix("0x").unwrap_or(value);
                u128::from_str_radix(value, 16)
                    .or_else(|_| value.parse::<u128>())
                    .map(AssetGuid::from_u128)
                    .map_err(E::custom)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(AssetGuid::from_u128(value as u128))
            }

            fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(AssetGuid::from_u128(value))
            }
        }

        deserializer.deserialize_any(AssetGuidVisitor)
    }
}

/// Engine asset path with explicit UTF-8 boundary handling.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct AssetPath {
    path: PathBuf,
}

impl AssetPath {
    /// Creates an asset path from a native path buffer.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the native path representation.
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    /// Returns a UTF-8 string if the platform path can be represented as UTF-8.
    pub fn to_utf8(&self) -> EngineResult<&str> {
        self.path
            .to_str()
            .ok_or_else(|| EngineError::other("asset path is not valid UTF-8"))
    }
}

/// Supported high-level resource types.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    /// 2D, 3D, or cube texture data.
    Texture,
    /// Material parameter and binding data.
    Material,
    /// Shader source and specialization configuration.
    Shader,
    /// Audio clip or stream metadata.
    Audio,
    /// Static model geometry.
    Model,
    /// Skinned model geometry.
    SkinnedModel,
    /// Animation clip or animation set.
    Animation,
    /// Varg script source for the runtime.
    Script,
    /// Reusable scene object subset.
    Prefab,
    /// Scene definition data.
    Scene,
}

/// Runtime resource load state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceState {
    /// Known to the registry but not loaded.
    Unloaded,
    /// CPU-side data is being loaded or imported.
    LoadingCpu,
    /// CPU-side data is available.
    CpuReady,
    /// GPU upload has been queued.
    UploadQueued,
    /// GPU-side data is available.
    GpuReady,
    /// The resource must be reloaded before use.
    Stale,
    /// Loading or importing failed.
    Failed,
}
