//! Filesystem abstraction.

use std::path::{Path, PathBuf};

use engine_core::{EngineError, EngineResult};

/// Filesystem operations required by the core runtime.
pub trait FileSystem {
    /// Reads a file into memory.
    fn read(&self, path: &Path) -> EngineResult<Vec<u8>>;

    /// Returns whether a path exists.
    fn exists(&self, path: &Path) -> bool;
}

/// Host filesystem implementation using `std`.
#[derive(Clone, Debug, Default)]
pub struct HostFileSystem;

impl FileSystem for HostFileSystem {
    fn read(&self, path: &Path) -> EngineResult<Vec<u8>> {
        std::fs::read(path).map_err(|source| EngineError::Filesystem {
            path: PathBuf::from(path),
            source,
        })
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}
