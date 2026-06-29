use crate::error::ensure_schema;
use crate::prelude::*;
use crate::*;

/// Import cache entry produced by importers.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ImportCacheEntry {
    /// Source asset GUID.
    pub guid: AssetGuid,
    /// Source content hash recorded by the importer.
    pub source_hash: String,
    /// Imported artifact path.
    pub artifact_path: PathBuf,
    /// Imported resource kind.
    pub kind: ResourceKind,
    /// Importer identifier and version.
    pub importer_version: String,
}

/// Import cache file format.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct ImportCacheFormat {
    /// Schema version.
    pub version: u32,
    /// Cached imports.
    #[serde(default)]
    pub entries: Vec<ImportCacheEntry>,
}

impl ImportCacheFormat {
    /// Parses an import cache from JSON.
    pub fn from_json(input: &str) -> Result<Self, AssetError> {
        let parsed: Self = serde_json::from_str(input).map_err(|source| AssetError::Parse {
            format: "import cache",
            diagnostic: AssetDiagnostic::new(source.to_string()),
        })?;
        ensure_schema("import cache", parsed.version)?;
        Ok(parsed)
    }
}

/// Manifest entry stored in resource manifests.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct AssetManifestEntry {
    /// Stable asset GUID.
    pub guid: AssetGuid,
    /// Asset path relative to the manifest root.
    pub path: AssetPath,
    /// Resource kind.
    pub kind: ResourceKind,
    /// Direct dependency GUIDs.
    #[serde(default)]
    pub dependencies: Vec<AssetGuid>,
}

/// Versioned resource manifest file format.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ResourceManifestFormat {
    /// Schema version.
    pub version: u32,
    /// Manifest entries.
    #[serde(default)]
    pub entries: Vec<AssetManifestEntry>,
}

impl Default for ResourceManifestFormat {
    fn default() -> Self {
        Self {
            version: CURRENT_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

impl ResourceManifestFormat {
    /// Parses a resource manifest from JSON.
    pub fn from_json(input: &str) -> Result<Self, AssetError> {
        let parsed: Self = serde_json::from_str(input).map_err(|source| AssetError::Parse {
            format: "resource manifest",
            diagnostic: AssetDiagnostic::new(source.to_string()),
        })?;
        ensure_schema("resource manifest", parsed.version)?;
        Ok(parsed)
    }

    /// Adds or replaces an entry by GUID.
    pub fn upsert(&mut self, entry: AssetManifestEntry) {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|candidate| candidate.guid == entry.guid)
        {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// Looks up an entry by GUID.
    pub fn get(&self, guid: AssetGuid) -> Option<&AssetManifestEntry> {
        self.entries.iter().find(|entry| entry.guid == guid)
    }
}
