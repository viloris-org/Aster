use crate::error::ensure_schema;
use crate::prelude::*;
use crate::watch::{generate_asset_guid, infer_scene_json};
use crate::*;

/// Resource dependency graph keyed by asset GUID.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencyGraph {
    outgoing: HashMap<AssetGuid, BTreeSet<AssetGuid>>,
    incoming: HashMap<AssetGuid, BTreeSet<AssetGuid>>,
}

impl DependencyGraph {
    /// Replaces all direct dependencies for a GUID.
    pub fn set_dependencies(
        &mut self,
        guid: AssetGuid,
        dependencies: impl IntoIterator<Item = AssetGuid>,
    ) {
        if let Some(previous) = self.outgoing.remove(&guid) {
            for dependency in previous {
                if let Some(dependents) = self.incoming.get_mut(&dependency) {
                    dependents.remove(&guid);
                }
            }
        }

        let dependencies = dependencies.into_iter().collect::<BTreeSet<_>>();
        for dependency in &dependencies {
            self.incoming.entry(*dependency).or_default().insert(guid);
        }
        self.outgoing.insert(guid, dependencies);
    }

    /// Returns direct dependencies for a GUID.
    pub fn dependencies(&self, guid: AssetGuid) -> Vec<AssetGuid> {
        self.outgoing
            .get(&guid)
            .map(|items| items.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Returns resources that directly depend on a GUID.
    pub fn dependents(&self, guid: AssetGuid) -> Vec<AssetGuid> {
        self.incoming
            .get(&guid)
            .map(|items| items.iter().copied().collect())
            .unwrap_or_default()
    }
}

/// Asset database for GUID/path resolution across project and built-in roots.
#[derive(Clone, Debug)]
pub struct AssetDatabase {
    pub(crate) project_root: PathBuf,
    builtin_root: PathBuf,
    guid_to_path: HashMap<AssetGuid, AssetPath>,
    pub(crate) path_to_guid: HashMap<PathBuf, AssetGuid>,
    meta: HashMap<AssetGuid, ResourceMetaFormat>,
    dependencies: DependencyGraph,
    /// Runtime resource metadata keyed by project-relative path.
    pub(crate) entries: HashMap<PathBuf, ResourceMeta>,
    /// Folder paths discovered during asset scan.
    folders: BTreeSet<PathBuf>,
}

impl AssetDatabase {
    /// Creates an empty asset database.
    pub fn new(project_root: impl Into<PathBuf>, builtin_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            builtin_root: builtin_root.into(),
            guid_to_path: HashMap::new(),
            path_to_guid: HashMap::new(),
            meta: HashMap::new(),
            dependencies: DependencyGraph::default(),
            entries: HashMap::new(),
            folders: BTreeSet::new(),
        }
    }

    /// Registers or updates metadata and GUID/path mappings.
    pub fn upsert_meta(&mut self, meta: ResourceMetaFormat) -> Result<(), AssetError> {
        ensure_schema("resource meta", meta.version)?;
        let path = AssetPath::new(meta.source_path.clone());
        if let Some(existing_guid) = self.path_to_guid.get(path.as_path()) {
            if *existing_guid != meta.guid {
                return Err(AssetError::Conflict {
                    diagnostic: AssetDiagnostic::new("path is already mapped to a different GUID")
                        .with_path(path.as_path()),
                });
            }
        }

        self.dependencies
            .set_dependencies(meta.guid, meta.dependencies.iter().copied());
        self.path_to_guid
            .insert(meta.source_path.clone(), meta.guid);
        self.guid_to_path.insert(meta.guid, path);
        self.meta.insert(meta.guid, meta);
        Ok(())
    }

    /// Creates a project resource record with no dependencies.
    pub fn create_project_resource(
        &mut self,
        guid: AssetGuid,
        path: impl Into<PathBuf>,
        kind: ResourceKind,
        importer: impl Into<String>,
    ) -> Result<(), AssetError> {
        self.upsert_meta(ResourceMetaFormat {
            version: CURRENT_SCHEMA_VERSION,
            guid,
            source_path: path.into(),
            kind,
            importer: importer.into(),
            dependencies: Vec::new(),
        })
    }

    /// Resolves a GUID to a project-relative path.
    pub fn resolve_guid(&self, guid: AssetGuid) -> Result<&AssetPath, AssetError> {
        self.guid_to_path
            .get(&guid)
            .ok_or_else(|| AssetError::NotFound {
                diagnostic: AssetDiagnostic::new("GUID is not present in the asset database")
                    .with_guid(guid),
            })
    }

    /// Resolves a project-relative path to a GUID.
    pub fn guid_for_path(&self, path: impl AsRef<Path>) -> Result<AssetGuid, AssetError> {
        let path = path.as_ref();
        self.path_to_guid
            .get(path)
            .copied()
            .ok_or_else(|| AssetError::NotFound {
                diagnostic: AssetDiagnostic::new("path is not present in the asset database")
                    .with_path(path),
            })
    }

    /// Resolves a project-relative path to a GUID, returning `None` when unknown.
    pub fn get_guid_for_path(&self, path: impl AsRef<Path>) -> Option<AssetGuid> {
        self.path_to_guid.get(path.as_ref()).copied()
    }

    /// Resolves `builtin:/x` or `project:/x` resource references to native paths.
    ///
    /// Rejects references whose resolved path escapes the intended root directory.
    pub fn resolve_resource_reference(&self, reference: &str) -> Result<PathBuf, AssetError> {
        let (root, rest) = if let Some(rest) = reference.strip_prefix("builtin:/") {
            (&self.builtin_root, rest)
        } else if let Some(rest) = reference.strip_prefix("project:/") {
            (&self.project_root, rest)
        } else {
            return Err(AssetError::NotFound {
                diagnostic: AssetDiagnostic::new(
                    "resource reference must use builtin:/ or project:/",
                ),
            });
        };

        let resolved = root.join(rest);
        // Canonicalize to resolve ../ components and symlinks.
        let canonical = resolved.canonicalize().map_err(|_| AssetError::NotFound {
            diagnostic: AssetDiagnostic::new("resource reference resolves to a non-existent path")
                .with_path(&resolved),
        })?;
        let canonical_root = root.canonicalize().map_err(|_| AssetError::NotFound {
            diagnostic: AssetDiagnostic::new("root directory does not exist").with_path(root),
        })?;

        if !canonical.starts_with(&canonical_root) {
            return Err(AssetError::NotFound {
                diagnostic: AssetDiagnostic::new("resource reference escapes its root directory")
                    .with_path(&resolved),
            });
        }

        Ok(canonical)
    }

    /// Returns the dependency graph.
    pub fn dependencies(&self) -> &DependencyGraph {
        &self.dependencies
    }

    /// Builds a versioned manifest from registered database entries.
    pub fn manifest(&self) -> ResourceManifestFormat {
        let mut manifest = ResourceManifestFormat::default();
        for meta in self.meta.values() {
            manifest.upsert(AssetManifestEntry {
                guid: meta.guid,
                path: AssetPath::new(meta.source_path.clone()),
                kind: meta.kind,
                dependencies: meta.dependencies.clone(),
            });
        }
        manifest
    }

    /// Scans an asset root directory tree, registering resources and folders.
    ///
    /// New files are added with `import_state` set to `Unloaded`. Existing entries
    /// matching the same project-relative path are preserved (GUID stays stable).
    /// Entries whose paths no longer exist on disk are removed.
    pub fn scan(&mut self, root: &Path) -> EngineResult<()> {
        let asset_root = self.project_root.clone();
        let root = if root.is_absolute() {
            root.to_path_buf()
        } else {
            asset_root.join(root)
        };

        let mut current_paths: HashSet<PathBuf> = HashSet::new();
        let mut current_folders: BTreeSet<PathBuf> = BTreeSet::new();

        if !root.exists() {
            self.entries.clear();
            self.folders.clear();
            return Ok(());
        }

        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let dir_entries = fs::read_dir(&dir).map_err(|source| EngineError::Filesystem {
                path: dir.clone(),
                source,
            })?;
            for entry in dir_entries {
                let entry = entry.map_err(|source| EngineError::Filesystem {
                    path: dir.clone(),
                    source,
                })?;
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path.clone());
                    if let Ok(relative) = path.strip_prefix(&root) {
                        current_folders.insert(relative.to_path_buf());
                    }
                    continue;
                }
                if path.extension().and_then(|value| value.to_str()) == Some("meta") {
                    continue;
                }
                let relative = path.strip_prefix(&root).unwrap_or(&path).to_path_buf();
                // Try extension-based inference first, then content-based for JSON files
                let (kind, importer) = match infer_importer(&relative) {
                    Some(result) => result,
                    None => {
                        if relative.extension().and_then(|v| v.to_str()) == Some("json") {
                            match infer_scene_json(&path) {
                                Some(result) => result,
                                None => continue,
                            }
                        } else {
                            continue;
                        }
                    }
                };
                current_paths.insert(relative.clone());

                // Preserve existing GUID or generate a new one
                let guid = self
                    .path_to_guid
                    .get(&relative)
                    .copied()
                    .unwrap_or_else(|| generate_asset_guid(&relative));

                let meta = ResourceMeta {
                    guid,
                    path: relative.clone(),
                    kind,
                    import_state: ResourceState::Unloaded,
                };
                self.entries.insert(relative.clone(), meta);

                // Also register in the persistent metadata tables
                let meta_format = ResourceMetaFormat {
                    version: CURRENT_SCHEMA_VERSION,
                    guid,
                    source_path: relative,
                    kind,
                    importer: importer.to_string(),
                    dependencies: Vec::new(),
                };
                let _ = self.upsert_meta(meta_format);
            }
        }

        // Remove entries whose paths no longer exist on disk
        self.entries.retain(|path, _| current_paths.contains(path));
        self.folders = current_folders;

        Ok(())
    }

    /// Returns all runtime resource entries.
    pub fn iter_entries(&self) -> impl Iterator<Item = &ResourceMeta> {
        self.entries.values()
    }

    /// Returns the runtime metadata for a specific path.
    pub fn entry_for_path(&self, path: &Path) -> Option<&ResourceMeta> {
        self.entries.get(path)
    }

    /// Returns the runtime metadata for a specific GUID.
    pub fn entry_for_guid(&self, guid: AssetGuid) -> Option<&ResourceMeta> {
        self.guid_to_path
            .get(&guid)
            .and_then(|asset_path| self.entries.get(asset_path.as_path()))
    }

    /// Returns discovered folder paths.
    pub fn folders(&self) -> &BTreeSet<PathBuf> {
        &self.folders
    }

    /// Returns mutable access to runtime resource entries.
    pub fn entries_mut(&mut self) -> &mut HashMap<PathBuf, ResourceMeta> {
        &mut self.entries
    }
}
