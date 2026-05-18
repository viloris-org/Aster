//! Dynamic library abstraction.

use std::path::Path;

use engine_core::{EngineError, EngineResult};

/// Dynamic library capability boundary.
pub trait DynamicLibraryProvider {
    /// Library handle type.
    type Library;

    /// Loads a dynamic library.
    fn load(&self, path: &Path) -> EngineResult<Self::Library>;
}

/// Provider used when dynamic loading is unavailable for the selected profile.
#[derive(Clone, Debug, Default)]
pub struct UnsupportedDynamicLibraryProvider;

impl DynamicLibraryProvider for UnsupportedDynamicLibraryProvider {
    type Library = ();

    fn load(&self, _path: &Path) -> EngineResult<Self::Library> {
        Err(EngineError::UnsupportedCapability {
            capability: "dynamic-library",
        })
    }
}
