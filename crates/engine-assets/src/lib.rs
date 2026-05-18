#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Asset identifier, path, and manifest primitives.

use std::path::{Path, PathBuf};

use engine_core::{AssetId, EngineError, EngineResult};

/// Engine asset path with explicit UTF-8 boundary handling.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

/// Minimal manifest entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetManifestEntry {
    /// Stable asset ID.
    pub id: AssetId,
    /// Asset path relative to the manifest root.
    pub path: AssetPath,
}

/// Minimal asset manifest subset.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssetManifest {
    entries: Vec<AssetManifestEntry>,
}

impl AssetManifest {
    /// Adds or replaces an entry by ID.
    pub fn upsert(&mut self, entry: AssetManifestEntry) {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|candidate| candidate.id == entry.id)
        {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// Looks up an entry by ID.
    pub fn get(&self, id: AssetId) -> Option<&AssetManifestEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Returns all entries in insertion order.
    pub fn entries(&self) -> &[AssetManifestEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_upsert_replaces_by_id() {
        let id = AssetId::from_u128(7);
        let mut manifest = AssetManifest::default();
        manifest.upsert(AssetManifestEntry {
            id,
            path: AssetPath::new("old.mesh"),
        });
        manifest.upsert(AssetManifestEntry {
            id,
            path: AssetPath::new("new.mesh"),
        });

        assert_eq!(manifest.entries().len(), 1);
        assert_eq!(
            manifest.get(id).unwrap().path.to_utf8().unwrap(),
            "new.mesh"
        );
    }
}
