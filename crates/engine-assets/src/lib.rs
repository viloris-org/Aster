#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Asset database, registry, manifest, dependency, import, and reload primitives.

mod database;
mod error;
mod ids;
mod import;
#[cfg(feature = "importers")]
mod import_payload;
mod manifest;
#[cfg(feature = "importers")]
mod mesh_builder;
mod prelude;
pub mod registry;
pub mod resource_trait;
pub mod resource_types;
mod resources;
mod runtime_registry;
#[cfg(feature = "importers")]
mod vmodel;
mod watch;

pub use database::{AssetDatabase, DependencyGraph};
pub use error::{AssetDiagnostic, AssetError};
pub use ids::{AssetGuid, AssetPath, CURRENT_SCHEMA_VERSION, ResourceKind, ResourceState};
#[cfg(feature = "importers")]
pub use import::{GltfImporter, ImportWorker, PngImporter};
pub use import::{GpuUploadTask, ImportJob, ImportOutcome, ImportQueue, ImportTask};
#[cfg(feature = "importers")]
pub use import_payload::import_builtin_asset;
pub use manifest::{
    AssetManifestEntry, ImportCacheEntry, ImportCacheFormat, ResourceManifestFormat,
};
pub use registry::ResourceTypeRegistry;
pub use resource_trait::{Resource, ResourceHandle as TypedResourceHandle};
pub use resource_types::{
    CurveLoopMode, CurvePoint, CurveResource, FontResource, InputActionDef, InputMapResource,
    ThemeResource,
};
pub use resources::{
    BasicMeshResource, CpuMaterialResource, CpuTextureResource, CubemapSource,
    DecodedCubemapResource, DecodedTextureResource, ImportOptions, MaterialFormat, ModelResource,
    ResourceMeta, ResourceMetaFormat, ShaderConfigFormat, TextureResource,
};
pub use runtime_registry::{
    AssetRegistry, CpuResource, GpuResource, PreviewData, ResourceHandle, ResourceRecord,
};
#[cfg(feature = "watch")]
pub use watch::FileWatcher;
#[cfg(feature = "importers")]
pub use watch::HotReloadCoordinator;
pub use watch::{
    AssetScanReport, FileEvent, FileEventKind, HotReloadTracker, ImporterBackend,
    available_importers, infer_importer, scan_project_assets,
};

#[cfg(test)]
mod tests;
