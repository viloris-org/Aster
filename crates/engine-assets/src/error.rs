use crate::prelude::*;
use crate::*;

/// Structured diagnostic for failed load, import, or migration operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetDiagnostic {
    /// Optional path related to the failure.
    pub path: Option<PathBuf>,
    /// Optional GUID related to the failure.
    pub guid: Option<AssetGuid>,
    /// Human-readable error context.
    pub message: String,
}

impl AssetDiagnostic {
    /// Creates a diagnostic with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            path: None,
            guid: None,
            message: message.into(),
        }
    }

    /// Adds path context.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Adds GUID context.
    pub fn with_guid(mut self, guid: AssetGuid) -> Self {
        self.guid = Some(guid);
        self
    }
}

/// Asset-layer error with diagnostics suitable for editor surfacing.
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    /// A file format failed to parse.
    #[error("failed to parse {format}: {diagnostic:?}")]
    Parse {
        /// Format name.
        format: &'static str,
        /// Structured diagnostic.
        diagnostic: AssetDiagnostic,
    },
    /// A format version cannot be loaded by this build.
    #[error("unsupported {format} schema version {version}, expected {expected}")]
    UnsupportedVersion {
        /// Format name.
        format: &'static str,
        /// Version found in the file.
        version: u32,
        /// Version supported by this build.
        expected: u32,
    },
    /// A requested resource or path was not found.
    #[error("asset was not found: {diagnostic:?}")]
    NotFound {
        /// Structured diagnostic.
        diagnostic: AssetDiagnostic,
    },
    /// A requested operation conflicts with existing database state.
    #[error("asset conflict: {diagnostic:?}")]
    Conflict {
        /// Structured diagnostic.
        diagnostic: AssetDiagnostic,
    },
}

impl From<AssetError> for EngineError {
    fn from(error: AssetError) -> Self {
        EngineError::other(error.to_string())
    }
}

pub(crate) fn ensure_schema(format: &'static str, version: u32) -> Result<(), AssetError> {
    if version == CURRENT_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(AssetError::UnsupportedVersion {
            format,
            version,
            expected: CURRENT_SCHEMA_VERSION,
        })
    }
}
