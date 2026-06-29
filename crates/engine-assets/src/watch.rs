#[cfg(feature = "importers")]
use crate::import_payload::discover_asset_dependencies;
use crate::prelude::*;
use crate::*;

/// Hot-reload tracker based on source modification stamps.
#[derive(Clone, Debug, Default)]
pub struct HotReloadTracker {
    stamps: HashMap<AssetGuid, SystemTime>,
}

impl HotReloadTracker {
    /// Updates a resource stamp and returns true when it changed.
    pub fn observe(&mut self, guid: AssetGuid, modified: SystemTime) -> bool {
        match self.stamps.insert(guid, modified) {
            Some(previous) => previous != modified,
            None => false,
        }
    }

    /// Marks changed resources stale in a registry.
    pub fn reload_changed(
        &mut self,
        registry: &mut AssetRegistry,
        changed: impl IntoIterator<Item = (AssetGuid, SystemTime)>,
    ) -> EngineResult<Vec<ResourceHandle>> {
        let mut reloaded = Vec::new();
        for (guid, modified) in changed {
            if self.observe(guid, modified) {
                if let Some(handle) = registry.handle_for_guid(guid) {
                    registry.mark_stale(handle)?;
                    reloaded.push(handle);
                }
            }
        }
        Ok(reloaded)
    }
}

/// Hot reload coordinator that manages the full reimport flow.
///
/// Handles file change events → mark stale → reimport → GPU upload → swap.
#[cfg(feature = "importers")]
pub struct HotReloadCoordinator {
    /// Import queue for background processing.
    import_queue: ImportQueue,
    /// Import worker thread.
    import_worker: Option<ImportWorker>,
    /// Number of frames to delay GPU resource destruction (default 3).
    gpu_destroy_delay_frames: u32,
}

#[cfg(feature = "importers")]
impl HotReloadCoordinator {
    /// Creates a new hot reload coordinator.
    pub fn new(_asset_root: impl Into<PathBuf>) -> Self {
        let import_queue = ImportQueue::default();
        let import_worker = Some(import_queue.spawn_worker());

        Self {
            import_queue,
            import_worker,
            gpu_destroy_delay_frames: 3,
        }
    }

    /// Processes file events and enqueues reimports for modified/created assets.
    pub fn process_file_events(
        &mut self,
        events: &[FileEvent],
        database: &mut AssetDatabase,
    ) -> EngineResult<Vec<AssetGuid>> {
        let mut affected_guids = Vec::new();

        for event in events {
            if let Some(guid) = database.handle_event(event)? {
                // Get the runtime metadata to create an import task
                if let Some(runtime_meta) = database.entry_for_guid(guid) {
                    // Infer importer from the path
                    if let Some((kind, importer)) = infer_importer(&runtime_meta.path) {
                        let task = ImportTask {
                            guid,
                            source_path: runtime_meta.path.clone(),
                            kind,
                            importer: importer.to_string(),
                        };

                        // Enqueue the import task
                        self.import_queue.push_import(task);
                        affected_guids.push(guid);
                    }
                }
            }
        }

        Ok(affected_guids)
    }

    /// Polls for completed imports and processes them.
    ///
    /// Returns import outcomes with diagnostics for logging to the console.
    pub fn poll_completed_imports(&mut self, registry: &mut AssetRegistry) -> Vec<ImportOutcome> {
        let mut outcomes = Vec::new();

        if let Some(worker) = &self.import_worker {
            while let Some(outcome) = worker.try_recv_outcome() {
                // Process the import outcome
                if outcome.diagnostics.is_empty() {
                    // Import succeeded - the upload task will be processed separately
                    if let Some(upload) = &outcome.upload {
                        self.import_queue.push_upload(upload.clone());
                    }
                } else {
                    // Import failed - mark the resource as failed
                    if let Some(handle) = registry.handle_for_guid(outcome.guid) {
                        let error_msg = outcome
                            .diagnostics
                            .iter()
                            .map(|d| d.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; ");
                        let _ = registry.mark_failed(handle, &error_msg);
                    }
                }

                outcomes.push(outcome);
            }
        }

        outcomes
    }

    /// Processes GPU upload tasks by swapping in new resources.
    ///
    /// The caller must provide a function that performs the actual GPU upload
    /// and returns the backend token for the new GPU resource.
    pub fn process_gpu_uploads<F>(
        &mut self,
        registry: &mut AssetRegistry,
        mut upload_fn: F,
    ) -> EngineResult<Vec<ResourceHandle>>
    where
        F: FnMut(&GpuUploadTask, &CpuResource) -> EngineResult<u64>,
    {
        let mut uploaded = Vec::new();
        let uploads = self.import_queue.drain_gpu_uploads();

        for upload in uploads {
            // Get the CPU resource data
            if let Some(cpu_resource) = registry.cpu_resource(upload.handle) {
                // Perform the GPU upload
                match upload_fn(&upload, cpu_resource) {
                    Ok(backend_token) => {
                        // Swap in the new GPU resource
                        let new_gpu_resource = GpuResource {
                            kind: upload.kind,
                            backend_token,
                        };
                        registry.swap_gpu(
                            upload.handle,
                            new_gpu_resource,
                            self.gpu_destroy_delay_frames,
                        )?;
                        uploaded.push(upload.handle);
                    }
                    Err(error) => {
                        // GPU upload failed - mark as failed
                        registry.mark_failed(upload.handle, &error.to_string())?;
                    }
                }
            }
        }

        Ok(uploaded)
    }

    /// Ticks the GPU destroy queue and returns backend tokens ready for destruction.
    ///
    /// The caller must destroy these GPU resources using the render backend.
    pub fn tick_gpu_destroy_queue(&mut self, registry: &mut AssetRegistry) -> Vec<u64> {
        registry.tick_gpu_destroy_queue()
    }

    /// Enqueues an import job to the worker thread.
    pub fn enqueue_import(&mut self, job: ImportJob) -> EngineResult<()> {
        if let Some(worker) = &self.import_worker {
            worker.enqueue(job)
        } else {
            Err(EngineError::other("import worker not available"))
        }
    }
}

/// Importer backend availability compiled into the current build.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImporterBackend {
    /// Built-in lightweight importer.
    BuiltIn,
    /// FBX importer, present only with `fbx-importer`.
    #[cfg(feature = "fbx-importer")]
    Fbx,
    /// Assimp importer, present only with `assimp-importer`.
    #[cfg(feature = "assimp-importer")]
    Assimp,
}

/// Returns importer backends available in this build.
pub fn available_importers() -> Vec<ImporterBackend> {
    let mut importers = Vec::new();
    importers.push(ImporterBackend::BuiltIn);
    #[cfg(feature = "fbx-importer")]
    importers.push(ImporterBackend::Fbx);
    #[cfg(feature = "assimp-importer")]
    importers.push(ImporterBackend::Assimp);
    importers
}

/// Infers the resource kind and importer name for a source asset path.
pub fn infer_importer(path: &Path) -> Option<(ResourceKind, &'static str)> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)?;
    match extension.as_str() {
        "png" | "jpg" | "jpeg" => Some((ResourceKind::Texture, "image")),
        "gltf" | "glb" => Some((ResourceKind::Model, "gltf")),
        "vmodel" => Some((ResourceKind::Model, "vmodel")),
        "wgsl" | "glsl" => Some((ResourceKind::Shader, "shader-source")),
        "wav" | "ogg" => Some((ResourceKind::Audio, "audio")),
        "varg" => Some((ResourceKind::Script, "script-varg")),
        "vscene" => Some((ResourceKind::Scene, "vscene")),
        "vasset" => Some((ResourceKind::Material, "vasset")),
        "json" => {
            let path_text = path.to_string_lossy();
            if path_text.contains("cubemap") || path_text.contains("skybox") {
                Some((ResourceKind::Texture, "cubemap-json"))
            } else if path_text.contains("material") {
                Some((ResourceKind::Material, "material-json"))
            } else if path_text.contains("prefab") {
                Some((ResourceKind::Prefab, "prefab-json"))
            } else {
                infer_scene_json(path)
            }
        }
        "toml" => {
            if path.to_string_lossy().contains("material") {
                Some((ResourceKind::Material, "material-toml"))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Detects whether a JSON file is a scene by checking for required top-level keys.
pub(crate) fn infer_scene_json(path: &Path) -> Option<(ResourceKind, &'static str)> {
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    match (
        value.get("version").and_then(|v| v.as_u64()),
        value.get("objects").and_then(|o| o.as_array()),
    ) {
        (Some(_), Some(_)) => Some((ResourceKind::Scene, "scene-json")),
        _ => None,
    }
}

static NEXT_GENERATED_GUID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn generate_asset_guid(path: &Path) -> AssetGuid {
    let mut entropy = std::collections::hash_map::DefaultHasher::new();
    "varg-asset-guid-v2".hash(&mut entropy);
    path.hash(&mut entropy);
    std::process::id().hash(&mut entropy);
    let entropy = entropy.finish() as u128;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = NEXT_GENERATED_GUID.fetch_add(1, Ordering::Relaxed) as u128;
    AssetGuid::from_u128(timestamp ^ (counter << 64) ^ entropy)
}

fn meta_path_for_source(path: &Path) -> PathBuf {
    let mut meta_path = path.to_path_buf();
    if let Some(name) = path.file_name() {
        let mut meta_name = name.to_os_string();
        meta_name.push(".meta");
        meta_path.set_file_name(meta_name);
    } else {
        meta_path.set_extension("meta");
    }
    meta_path
}

fn read_resource_meta(path: &Path) -> EngineResult<Option<ResourceMetaFormat>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path).map_err(|source| EngineError::Filesystem {
        path: path.to_path_buf(),
        source,
    })?;
    ResourceMetaFormat::from_toml(&text)
        .map(Some)
        .map_err(EngineError::from)
}

fn write_resource_meta(path: &Path, meta: &ResourceMetaFormat) -> EngineResult<()> {
    let text =
        toml::to_string_pretty(meta).map_err(|error| EngineError::other(error.to_string()))?;
    fs::write(path, text).map_err(|source| EngineError::Filesystem {
        path: path.to_path_buf(),
        source,
    })
}

/// Result of scanning a project asset root.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssetScanReport {
    /// Metadata discovered or generated during the scan.
    pub metas: Vec<ResourceMetaFormat>,
    /// Files ignored because no importer accepts them.
    pub ignored: Vec<PathBuf>,
}

/// Scans a project asset root and registers supported resources in the database.
pub fn scan_project_assets(
    asset_root: impl AsRef<Path>,
    database: &mut AssetDatabase,
) -> EngineResult<AssetScanReport> {
    let asset_root = asset_root.as_ref();
    let mut report = AssetScanReport::default();
    if !asset_root.exists() {
        return Ok(report);
    }

    let mut stack = vec![asset_root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let entries = fs::read_dir(&path).map_err(|source| EngineError::Filesystem {
            path: path.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| EngineError::Filesystem {
                path: path.clone(),
                source,
            })?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) == Some("meta") {
                continue;
            }
            let relative = path.strip_prefix(asset_root).unwrap_or(&path).to_path_buf();
            let detected = infer_importer(&relative).or_else(|| {
                if path.extension().and_then(|value| value.to_str()) == Some("json") {
                    infer_scene_json(&path)
                } else {
                    None
                }
            });
            let Some((kind, importer)) = detected else {
                report.ignored.push(relative);
                continue;
            };
            let meta_path = meta_path_for_source(&path);
            let previous = read_resource_meta(&meta_path)?;
            let meta = match previous.clone() {
                Some(mut meta) => {
                    meta.version = CURRENT_SCHEMA_VERSION;
                    meta.source_path = relative;
                    meta.kind = kind;
                    meta.importer = importer.to_string();
                    meta.dependencies = discover_asset_dependencies(&path, kind, importer)?;
                    meta
                }
                None => ResourceMetaFormat {
                    version: CURRENT_SCHEMA_VERSION,
                    guid: generate_asset_guid(&relative),
                    source_path: relative,
                    kind,
                    importer: importer.to_string(),
                    dependencies: discover_asset_dependencies(&path, kind, importer)?,
                },
            };
            if previous.as_ref() != Some(&meta) {
                write_resource_meta(&meta_path, &meta)?;
            }
            database
                .upsert_meta(meta.clone())
                .map_err(EngineError::from)?;
            report.metas.push(meta);
        }
    }
    report
        .metas
        .sort_by(|left, right| left.source_path.cmp(&right.source_path));
    report.ignored.sort();
    Ok(report)
}

#[cfg(not(feature = "importers"))]
fn discover_asset_dependencies(
    _path: &Path,
    _kind: ResourceKind,
    _importer: &str,
) -> EngineResult<Vec<AssetGuid>> {
    Ok(Vec::new())
}

/// File system event kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileEventKind {
    /// File was created.
    Created,
    /// File was modified.
    Modified,
    /// File was removed.
    Removed,
    /// File was renamed.
    Renamed,
}

/// File system event from the watcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileEvent {
    /// Path relative to the watched root.
    pub path: PathBuf,
    /// Event kind.
    pub kind: FileEventKind,
}

/// File watcher for asset change detection with debouncing.
#[cfg(feature = "watch")]
pub struct FileWatcher {
    _watcher: notify::RecommendedWatcher,
    receiver: Receiver<notify::Result<notify::Event>>,
    root: PathBuf,
    canonical_root: PathBuf,
    pub(crate) debounce_buffer: HashMap<PathBuf, (FileEventKind, SystemTime)>,
    debounce_duration: std::time::Duration,
}

#[cfg(feature = "watch")]
impl FileWatcher {
    /// Starts watching the given directory for file changes.
    pub fn start(asset_root: impl AsRef<Path>) -> EngineResult<Self> {
        use notify::{RecursiveMode, Watcher};

        let asset_root = asset_root.as_ref();
        let (sender, receiver) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(sender)
            .map_err(|e| EngineError::other(format!("Failed to create file watcher: {}", e)))?;

        watcher
            .watch(asset_root, RecursiveMode::Recursive)
            .map_err(|e| {
                EngineError::other(format!(
                    "Failed to watch directory {}: {}",
                    asset_root.display(),
                    e
                ))
            })?;

        Ok(Self {
            _watcher: watcher,
            receiver,
            canonical_root: asset_root
                .canonicalize()
                .unwrap_or_else(|_| asset_root.to_path_buf()),
            root: asset_root.to_path_buf(),
            debounce_buffer: HashMap::new(),
            debounce_duration: std::time::Duration::from_millis(200),
        })
    }

    pub(crate) fn relative_event_path(&self, path: &Path) -> Option<PathBuf> {
        if let Ok(relative) = path.strip_prefix(&self.root) {
            return Some(relative.to_path_buf());
        }

        let canonical_path = path.canonicalize().ok()?;
        canonical_path
            .strip_prefix(&self.canonical_root)
            .ok()
            .map(Path::to_path_buf)
    }

    /// Polls for file events, returning debounced events.
    ///
    /// Modified events within 200ms window are debounced to only emit the latest.
    pub fn poll_events(&mut self) -> Vec<FileEvent> {
        let now = SystemTime::now();

        // Drain all pending events from the channel
        while let Ok(result) = self.receiver.try_recv() {
            if let Ok(event) = result {
                for path in &event.paths {
                    // Skip .meta files
                    if path.extension().and_then(|e| e.to_str()) == Some("meta") {
                        continue;
                    }

                    // Convert to relative path
                    let relative = match self.relative_event_path(path) {
                        Some(relative) => relative,
                        None => continue,
                    };

                    let kind = match event.kind {
                        notify::EventKind::Create(_) => FileEventKind::Created,
                        notify::EventKind::Modify(_) => FileEventKind::Modified,
                        notify::EventKind::Remove(_) => FileEventKind::Removed,
                        _ => continue,
                    };

                    // Buffer the event with timestamp
                    self.debounce_buffer.insert(relative, (kind, now));
                }
            }
        }

        // Collect events that are past the debounce window
        let mut ready_events = Vec::new();
        self.debounce_buffer.retain(|path, (kind, timestamp)| {
            if let Ok(elapsed) = now.duration_since(*timestamp) {
                if elapsed >= self.debounce_duration {
                    ready_events.push(FileEvent {
                        path: path.clone(),
                        kind: kind.clone(),
                    });
                    return false; // Remove from buffer
                }
            }
            true // Keep in buffer
        });

        ready_events
    }
}

impl AssetDatabase {
    /// Handles a file system event by updating the database state.
    ///
    /// - Modified: marks asset as Stale and enqueues reimport
    /// - Created: adds new ResourceMeta with Unloaded state
    /// - Removed: removes ResourceMeta from database
    ///
    /// Returns the GUID of the affected asset if an import should be queued.
    pub fn handle_event(&mut self, event: &FileEvent) -> EngineResult<Option<AssetGuid>> {
        match event.kind {
            FileEventKind::Modified => {
                // Mark existing asset as stale and return GUID for reimport
                if let Some(meta) = self.entries.get_mut(&event.path) {
                    meta.import_state = ResourceState::Stale;
                    return Ok(Some(meta.guid));
                }
                Ok(None)
            }
            FileEventKind::Created => {
                // Add new asset with Unloaded state
                let absolute_path = self.project_root.join(&event.path);
                if let Some((kind, importer)) = infer_importer(&event.path) {
                    let guid = self
                        .path_to_guid
                        .get(&event.path)
                        .copied()
                        .unwrap_or_else(|| generate_asset_guid(&event.path));

                    let meta = ResourceMeta {
                        guid,
                        path: event.path.clone(),
                        kind,
                        import_state: ResourceState::Unloaded,
                    };
                    self.entries.insert(event.path.clone(), meta);

                    // Also register in persistent metadata
                    let meta_format = ResourceMetaFormat {
                        version: CURRENT_SCHEMA_VERSION,
                        guid,
                        source_path: event.path.clone(),
                        kind,
                        importer: importer.to_string(),
                        dependencies: Vec::new(),
                    };
                    self.upsert_meta(meta_format)?;
                    return Ok(Some(guid));
                } else if absolute_path.extension().and_then(|e| e.to_str()) == Some("json") {
                    // Try content-based detection for JSON files
                    if let Some((kind, importer)) = infer_scene_json(&absolute_path) {
                        let guid = self
                            .path_to_guid
                            .get(&event.path)
                            .copied()
                            .unwrap_or_else(|| generate_asset_guid(&event.path));

                        let meta = ResourceMeta {
                            guid,
                            path: event.path.clone(),
                            kind,
                            import_state: ResourceState::Unloaded,
                        };
                        self.entries.insert(event.path.clone(), meta);

                        let meta_format = ResourceMetaFormat {
                            version: CURRENT_SCHEMA_VERSION,
                            guid,
                            source_path: event.path.clone(),
                            kind,
                            importer: importer.to_string(),
                            dependencies: Vec::new(),
                        };
                        self.upsert_meta(meta_format)?;
                        return Ok(Some(guid));
                    }
                }
                Ok(None)
            }
            FileEventKind::Removed => {
                // Remove asset from database and return GUID for cleanup
                if let Some(meta) = self.entries.remove(&event.path) {
                    return Ok(Some(meta.guid));
                }
                Ok(None)
            }
            FileEventKind::Renamed => {
                // Treat as remove + create (handled by separate events)
                Ok(None)
            }
        }
    }
}
