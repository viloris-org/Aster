#[cfg(feature = "importers")]
use crate::import_payload::import_gltf_model;
use crate::prelude::*;
#[cfg(feature = "importers")]
use crate::watch::generate_asset_guid;
use crate::*;

/// Import task handled by CPU loading/import workers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportTask {
    /// Source GUID.
    pub guid: AssetGuid,
    /// Source path.
    pub source_path: PathBuf,
    /// Resource kind to import.
    pub kind: ResourceKind,
    /// Importer name.
    pub importer: String,
}

/// GPU upload task separated from CPU loading/import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuUploadTask {
    /// Destination resource handle.
    pub handle: ResourceHandle,
    /// Resource kind.
    pub kind: ResourceKind,
}

/// Result of an import task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportOutcome {
    /// Source GUID.
    pub guid: AssetGuid,
    /// Import diagnostics.
    pub diagnostics: Vec<AssetDiagnostic>,
    /// Optional upload task produced by the import.
    pub upload: Option<GpuUploadTask>,
}

/// glTF importer for mesh extraction.
#[cfg(feature = "importers")]
pub struct GltfImporter;

#[cfg(feature = "importers")]
impl GltfImporter {
    /// Imports a glTF file into a model resource with mesh primitives.
    ///
    /// Returns an `ImportOutcome` with diagnostics. On success, the model resource
    /// contains all mesh primitives with positions, normals, texcoords, and indices.
    pub fn import(path: &Path) -> EngineResult<ImportOutcome> {
        let mut diagnostics = Vec::new();

        // Import the glTF model
        let model = match import_gltf_model(path) {
            Ok(model) => model,
            Err(error) => {
                diagnostics.push(
                    AssetDiagnostic::new(format!("glTF import failed: {}", error)).with_path(path),
                );
                return Ok(ImportOutcome {
                    guid: generate_asset_guid(path),
                    diagnostics,
                    upload: None,
                });
            }
        };

        // Validate that we have at least one mesh
        if model.meshes.is_empty() {
            diagnostics.push(
                AssetDiagnostic::new("glTF file contains no mesh primitives").with_path(path),
            );
        }

        Ok(ImportOutcome {
            guid: generate_asset_guid(path),
            diagnostics,
            upload: None,
        })
    }

    /// Imports a glTF file and stores the result in the asset registry.
    pub fn import_to_registry(
        path: &Path,
        registry: &mut AssetRegistry,
        guid: AssetGuid,
    ) -> EngineResult<ImportOutcome> {
        let mut diagnostics = Vec::new();

        // Import the glTF model
        let model = match import_gltf_model(path) {
            Ok(model) => model,
            Err(error) => {
                diagnostics.push(
                    AssetDiagnostic::new(format!("glTF import failed: {}", error)).with_path(path),
                );
                return Ok(ImportOutcome {
                    guid,
                    diagnostics,
                    upload: None,
                });
            }
        };

        // Validate that we have at least one mesh
        if model.meshes.is_empty() {
            diagnostics.push(
                AssetDiagnostic::new("glTF file contains no mesh primitives").with_path(path),
            );
        }

        // Register and store in registry
        let handle = registry.register(guid, ResourceKind::Model)?;
        registry.set_state(handle, ResourceState::LoadingCpu)?;

        let model_bytes = model.to_bytes()?;
        registry.put_cpu(
            handle,
            CpuResource {
                kind: ResourceKind::Model,
                bytes: model_bytes,
            },
        )?;

        registry.set_preview(
            handle,
            PreviewData {
                thumbnail: None,
                summary: format!(
                    "glTF model with {} mesh primitive{}",
                    model.meshes.len(),
                    if model.meshes.len() == 1 { "" } else { "s" }
                ),
            },
        )?;

        Ok(ImportOutcome {
            guid,
            diagnostics,
            upload: Some(GpuUploadTask {
                handle,
                kind: ResourceKind::Model,
            }),
        })
    }
}

/// PNG importer with mip chain generation.
#[cfg(feature = "importers")]
pub struct PngImporter;

#[cfg(feature = "importers")]
impl PngImporter {
    /// Imports a PNG file into a CPU texture resource with mip chain.
    ///
    /// Returns an `ImportOutcome` with diagnostics. On success, the CPU texture resource
    /// is serialized and can be retrieved via the asset registry after calling
    /// `import_png_to_registry`.
    pub fn import(path: &Path, options: &ImportOptions) -> EngineResult<ImportOutcome> {
        let mut diagnostics = Vec::new();

        // Read the file
        let bytes = fs::read(path).map_err(|source| EngineError::Filesystem {
            path: path.to_path_buf(),
            source,
        })?;

        // Decode the PNG
        let image = match image::load_from_memory(&bytes) {
            Ok(img) => img,
            Err(error) => {
                diagnostics.push(
                    AssetDiagnostic::new(format!("PNG decode failed: {}", error)).with_path(path),
                );
                // Return outcome with diagnostics but no upload
                return Ok(ImportOutcome {
                    guid: generate_asset_guid(path),
                    diagnostics,
                    upload: None,
                });
            }
        };

        // Convert to RGBA8
        let rgba = image.to_rgba8();
        let width = rgba.width();
        let height = rgba.height();

        // Generate mip chain
        let mip_levels = if options.generate_mips {
            generate_mip_chain(&rgba)
        } else {
            vec![rgba.into_raw()]
        };

        let _cpu_texture = CpuTextureResource {
            width,
            height,
            format: "Rgba8UnormSrgb".to_string(),
            mip_levels,
        };

        Ok(ImportOutcome {
            guid: generate_asset_guid(path),
            diagnostics,
            upload: None, // Caller will set this if needed
        })
    }

    /// Imports a PNG file and stores the result in the asset registry.
    pub fn import_to_registry(
        path: &Path,
        options: &ImportOptions,
        registry: &mut AssetRegistry,
        guid: AssetGuid,
    ) -> EngineResult<ImportOutcome> {
        let mut diagnostics = Vec::new();

        // Read the file
        let bytes = fs::read(path).map_err(|source| EngineError::Filesystem {
            path: path.to_path_buf(),
            source,
        })?;

        // Decode the PNG
        let image = match image::load_from_memory(&bytes) {
            Ok(img) => img,
            Err(error) => {
                diagnostics.push(
                    AssetDiagnostic::new(format!("PNG decode failed: {}", error)).with_path(path),
                );
                return Ok(ImportOutcome {
                    guid,
                    diagnostics,
                    upload: None,
                });
            }
        };

        // Convert to RGBA8
        let rgba = image.to_rgba8();
        let width = rgba.width();
        let height = rgba.height();

        // Generate mip chain
        let mip_levels = if options.generate_mips {
            generate_mip_chain(&rgba)
        } else {
            vec![rgba.into_raw()]
        };

        let cpu_texture = CpuTextureResource {
            width,
            height,
            format: "Rgba8UnormSrgb".to_string(),
            mip_levels,
        };

        // Register and store in registry
        let handle = registry.register(guid, ResourceKind::Texture)?;
        registry.set_state(handle, ResourceState::LoadingCpu)?;

        let texture_bytes = cpu_texture.to_bytes()?;
        registry.put_cpu(
            handle,
            CpuResource {
                kind: ResourceKind::Texture,
                bytes: texture_bytes,
            },
        )?;

        registry.set_preview(
            handle,
            PreviewData {
                thumbnail: None,
                summary: format!(
                    "{}x{} {} texture with {} mip levels",
                    width,
                    height,
                    cpu_texture.format,
                    cpu_texture.mip_levels.len()
                ),
            },
        )?;

        Ok(ImportOutcome {
            guid,
            diagnostics,
            upload: Some(GpuUploadTask {
                handle,
                kind: ResourceKind::Texture,
            }),
        })
    }
}

/// Generates a mip chain from a base RGBA8 image using box filtering.
#[cfg(feature = "importers")]
pub(crate) fn generate_mip_chain(base: &image::RgbaImage) -> Vec<Vec<u8>> {
    let mut mip_levels = Vec::new();

    // Level 0: original image
    mip_levels.push(base.clone().into_raw());

    let mut current = base.clone();

    // Generate subsequent levels until we reach 1x1
    while current.width() > 1 || current.height() > 1 {
        let new_width = (current.width() / 2).max(1);
        let new_height = (current.height() / 2).max(1);

        let downsampled = downsample_rgba8(&current, new_width, new_height);
        mip_levels.push(downsampled.clone().into_raw());
        current = downsampled;
    }

    mip_levels
}

/// Downsamples an RGBA8 image to a smaller size using box filtering.
#[cfg(feature = "importers")]
fn downsample_rgba8(
    source: &image::RgbaImage,
    target_width: u32,
    target_height: u32,
) -> image::RgbaImage {
    use image::imageops::FilterType;
    image::imageops::resize(source, target_width, target_height, FilterType::Triangle)
}

/// Job sent to the import worker thread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportJob {
    /// Asset path to import.
    pub asset_path: PathBuf,
    /// Resource kind.
    pub resource_kind: ResourceKind,
    /// Import options (currently unused, reserved for future).
    pub import_options: ImportOptions,
}

/// Handle to a background import worker thread.
#[cfg(feature = "importers")]
pub struct ImportWorker {
    outcome_sender: Sender<ImportOutcome>,
    outcome_receiver: Receiver<ImportOutcome>,
}

#[cfg(feature = "importers")]
impl ImportWorker {
    /// Creates a background import worker backed by the shared engine task runtime.
    pub fn spawn() -> Self {
        let (outcome_sender, outcome_receiver) = mpsc::channel::<ImportOutcome>();

        Self {
            outcome_sender,
            outcome_receiver,
        }
    }

    /// Enqueues an import job to be processed on the shared background task runtime.
    pub fn enqueue(&self, job: ImportJob) -> EngineResult<()> {
        let outcome_sender = self.outcome_sender.clone();
        shared_task_runtime().spawn("asset.import", TaskPriority::Background, move || {
            let outcome = Self::process_job(job);
            let _ = outcome_sender.send(outcome);
        });
        Ok(())
    }

    /// Polls for completed import outcomes without blocking.
    pub fn try_recv_outcome(&self) -> Option<ImportOutcome> {
        self.outcome_receiver.try_recv().ok()
    }

    /// Processes a single import job by dispatching to the appropriate importer.
    fn process_job(job: ImportJob) -> ImportOutcome {
        let guid = generate_asset_guid(&job.asset_path);

        // Dispatch to the correct importer based on resource kind and extension
        match job.resource_kind {
            ResourceKind::Texture => {
                // Use PngImporter for texture imports
                PngImporter::import(&job.asset_path, &job.import_options).unwrap_or_else(|error| {
                    ImportOutcome {
                        guid,
                        diagnostics: vec![
                            AssetDiagnostic::new(format!("Texture import failed: {}", error))
                                .with_path(&job.asset_path),
                        ],
                        upload: None,
                    }
                })
            }
            ResourceKind::Model => {
                // Use GltfImporter for model imports
                GltfImporter::import(&job.asset_path).unwrap_or_else(|error| ImportOutcome {
                    guid,
                    diagnostics: vec![
                        AssetDiagnostic::new(format!("Model import failed: {}", error))
                            .with_path(&job.asset_path),
                    ],
                    upload: None,
                })
            }
            _ => {
                // Unsupported resource kind
                ImportOutcome {
                    guid,
                    diagnostics: vec![
                        AssetDiagnostic::new(format!(
                            "Unsupported resource kind for import: {:?}",
                            job.resource_kind
                        ))
                        .with_path(&job.asset_path),
                    ],
                    upload: None,
                }
            }
        }
    }
}

/// Import and upload queues with separated CPU and GPU work.
#[derive(Clone, Debug, Default)]
pub struct ImportQueue {
    imports: VecDeque<ImportTask>,
    uploads: Arc<Mutex<VecDeque<GpuUploadTask>>>,
}

impl ImportQueue {
    /// Queues a CPU import/load task.
    pub fn push_import(&mut self, task: ImportTask) {
        self.imports.push_back(task);
    }

    /// Queues a GPU upload task.
    pub fn push_upload(&mut self, task: GpuUploadTask) {
        if let Ok(mut uploads) = self.uploads.lock() {
            uploads.push_back(task);
        }
    }

    /// Pops one GPU upload task.
    pub fn pop_upload(&mut self) -> Option<GpuUploadTask> {
        self.uploads.lock().ok()?.pop_front()
    }

    /// Drains all pending GPU upload tasks.
    pub fn drain_gpu_uploads(&mut self) -> Vec<GpuUploadTask> {
        match self.uploads.lock() {
            Ok(mut uploads) => uploads.drain(..).collect(),
            _ => Vec::new(),
        }
    }

    #[cfg(feature = "importers")]
    /// Spawns a background worker thread for processing imports.
    ///
    /// The worker will process import jobs and produce GPU upload tasks
    /// that can be consumed via `drain_gpu_uploads()`.
    pub fn spawn_worker(&self) -> ImportWorker {
        ImportWorker::spawn()
    }

    /// Drains imports across worker threads and appends produced upload tasks.
    pub fn drain_imports_parallel<F>(
        &mut self,
        worker_count: usize,
        import: F,
    ) -> Vec<ImportOutcome>
    where
        F: Fn(ImportTask) -> ImportOutcome + Send + Sync + 'static,
    {
        let worker_count = worker_count.max(1);
        let tasks = self.imports.drain(..).collect::<Vec<_>>();
        if tasks.is_empty() {
            return Vec::new();
        }

        let runtime = shared_task_runtime();
        let limiter = runtime.concurrency_limiter(worker_count, TaskPriority::Background);
        let import = Arc::new(import);
        let (outcome_sender, outcome_receiver) = mpsc::channel();

        for task in tasks {
            let import = Arc::clone(&import);
            let outcome_sender = outcome_sender.clone();
            limiter.push("asset.import.batch", move |_| {
                let _ = outcome_sender.send(import(task));
            });
        }
        drop(outcome_sender);
        limiter.wait();

        let mut outcomes = outcome_receiver.into_iter().collect::<Vec<_>>();

        for outcome in &outcomes {
            if let Some(upload) = &outcome.upload {
                self.push_upload(upload.clone());
            }
        }
        outcomes.sort_by_key(|outcome| outcome.guid);
        outcomes
    }
}
