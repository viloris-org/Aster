use crate::prelude::*;
use crate::*;

/// Project panel preview and thumbnail metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewData {
    /// Optional thumbnail bytes in an implementation-defined encoded format.
    pub thumbnail: Option<Arc<[u8]>>,
    /// Human-readable preview summary.
    pub summary: String,
}

/// Stable Rust-native resource handle.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceHandle {
    id: ResourceId,
    handle: Handle,
}

impl ResourceHandle {
    /// Creates a resource handle from raw parts.
    pub const fn new(id: ResourceId, handle: Handle) -> Self {
        Self { id, handle }
    }

    /// Stable resource ID.
    pub const fn id(self) -> ResourceId {
        self.id
    }

    /// Generational handle value.
    pub const fn raw_handle(self) -> Handle {
        self.handle
    }
}

/// Registered resource record.
#[derive(Clone, Debug)]
pub struct ResourceRecord {
    /// Stable resource ID.
    pub id: ResourceId,
    /// Source asset GUID.
    pub guid: AssetGuid,
    /// Resource kind.
    pub kind: ResourceKind,
    /// Current load state.
    pub state: ResourceState,
    /// Direct dependency GUIDs.
    pub dependencies: Vec<AssetGuid>,
    /// Optional project panel preview data.
    pub preview: Option<PreviewData>,
}

/// CPU-side cached resource data.
#[derive(Clone, Debug)]
pub struct CpuResource {
    /// Resource kind.
    pub kind: ResourceKind,
    /// Implementation-defined CPU payload.
    pub bytes: Arc<[u8]>,
}

/// GPU-side cached resource data.
#[derive(Clone, Debug)]
pub struct GpuResource {
    /// Resource kind.
    pub kind: ResourceKind,
    /// Backend-owned opaque token.
    pub backend_token: u64,
}

/// Registry for stable resource handles and CPU/GPU cache lifetimes.
#[derive(Clone, Debug, Default)]
pub struct AssetRegistry {
    handles: HandleAllocator,
    by_handle: HashMap<Handle, ResourceRecord>,
    by_guid: HashMap<AssetGuid, ResourceHandle>,
    pub(crate) cpu_cache: HashMap<ResourceHandle, CpuResource>,
    pub(crate) gpu_cache: HashMap<ResourceHandle, GpuResource>,
    /// GPU resources pending deferred destruction (handle, backend_token, frames_remaining)
    pub(crate) gpu_destroy_queue: VecDeque<(ResourceHandle, u64, u32)>,
}

impl AssetRegistry {
    /// Registers a resource and returns its stable native handle.
    pub fn register(
        &mut self,
        guid: AssetGuid,
        kind: ResourceKind,
    ) -> EngineResult<ResourceHandle> {
        if let Some(handle) = self.by_guid.get(&guid) {
            return Ok(*handle);
        }

        let raw = self.handles.allocate()?;
        let id = ResourceId::from_u128(guid.as_u128());
        let handle = ResourceHandle::new(id, raw);
        self.by_handle.insert(
            raw,
            ResourceRecord {
                id,
                guid,
                kind,
                state: ResourceState::Unloaded,
                dependencies: Vec::new(),
                preview: None,
            },
        );
        self.by_guid.insert(guid, handle);
        Ok(handle)
    }

    /// Looks up a handle by GUID.
    pub fn handle_for_guid(&self, guid: AssetGuid) -> Option<ResourceHandle> {
        self.by_guid.get(&guid).copied()
    }

    /// Returns a registered resource record.
    pub fn record(&self, handle: ResourceHandle) -> Option<&ResourceRecord> {
        if self.handles.is_live(handle.raw_handle()) {
            self.by_handle.get(&handle.raw_handle())
        } else {
            None
        }
    }

    /// Updates resource state.
    pub fn set_state(&mut self, handle: ResourceHandle, state: ResourceState) -> EngineResult<()> {
        let record = self
            .by_handle
            .get_mut(&handle.raw_handle())
            .ok_or_else(|| EngineError::invalid_handle("resource handle does not exist"))?;
        record.state = state;
        Ok(())
    }

    /// Updates project panel preview data.
    pub fn set_preview(
        &mut self,
        handle: ResourceHandle,
        preview: PreviewData,
    ) -> EngineResult<()> {
        let record = self
            .by_handle
            .get_mut(&handle.raw_handle())
            .ok_or_else(|| EngineError::invalid_handle("resource handle does not exist"))?;
        record.preview = Some(preview);
        Ok(())
    }

    /// Inserts or replaces CPU cache data without changing GPU lifetime.
    pub fn put_cpu(&mut self, handle: ResourceHandle, resource: CpuResource) -> EngineResult<()> {
        self.ensure_live(handle)?;
        self.cpu_cache.insert(handle, resource);
        self.set_state(handle, ResourceState::CpuReady)
    }

    /// Returns CPU cache data for a resource.
    pub fn cpu_resource(&self, handle: ResourceHandle) -> Option<&CpuResource> {
        self.cpu_cache.get(&handle)
    }

    /// Returns GPU cache data for a resource.
    pub fn gpu_resource(&self, handle: ResourceHandle) -> Option<&GpuResource> {
        self.gpu_cache.get(&handle)
    }

    /// Inserts or replaces GPU cache data without changing CPU lifetime.
    pub fn put_gpu(&mut self, handle: ResourceHandle, resource: GpuResource) -> EngineResult<()> {
        self.ensure_live(handle)?;
        self.gpu_cache.insert(handle, resource);
        self.set_state(handle, ResourceState::GpuReady)
    }

    /// Drops only CPU-side cache data for a resource.
    pub fn drop_cpu(&mut self, handle: ResourceHandle) {
        self.cpu_cache.remove(&handle);
    }

    /// Drops only GPU-side cache data for a resource.
    pub fn drop_gpu(&mut self, handle: ResourceHandle) {
        self.gpu_cache.remove(&handle);
    }

    /// Marks a resource stale and drops both cache tiers.
    pub fn mark_stale(&mut self, handle: ResourceHandle) -> EngineResult<()> {
        self.drop_cpu(handle);
        self.drop_gpu(handle);
        self.set_state(handle, ResourceState::Stale)
    }

    /// Replaces GPU resource with a new one, enqueuing the old one for deferred destruction.
    ///
    /// The old GPU resource is kept alive for `frames` frames (default 3) to allow
    /// in-flight rendering commands to complete before the backend destroys it.
    pub fn swap_gpu(
        &mut self,
        handle: ResourceHandle,
        new_resource: GpuResource,
        frames: u32,
    ) -> EngineResult<()> {
        self.ensure_live(handle)?;

        // If there's an old GPU resource, enqueue it for deferred destruction
        if let Some(old_resource) = self.gpu_cache.get(&handle) {
            self.gpu_destroy_queue
                .push_back((handle, old_resource.backend_token, frames));
        }

        // Insert the new GPU resource
        self.gpu_cache.insert(handle, new_resource);
        self.set_state(handle, ResourceState::GpuReady)
    }

    /// Ticks the deferred GPU destroy queue, decrementing frame counters.
    ///
    /// Returns backend tokens that reached 0 during this tick.
    /// Decrements all counters first, then removes items that are now at 0.
    pub fn tick_gpu_destroy_queue(&mut self) -> Vec<u64> {
        let mut ready_to_destroy = Vec::new();

        // Check if any items are already at 0 before we start
        let had_zeros_before = self.gpu_destroy_queue.iter().any(|(_, _, f)| *f == 0);

        // If there were items at 0, remove them without decrementing
        if had_zeros_before {
            self.gpu_destroy_queue
                .retain(|(_handle, token, frames_remaining)| {
                    if *frames_remaining == 0 {
                        ready_to_destroy.push(*token);
                        false // Remove from queue
                    } else {
                        true // Keep in queue
                    }
                });
        } else {
            // No items at 0, so decrement all and then remove any that reached 0
            for (_handle, _token, frames_remaining) in &mut self.gpu_destroy_queue {
                *frames_remaining -= 1;
            }

            self.gpu_destroy_queue
                .retain(|(_handle, token, frames_remaining)| {
                    if *frames_remaining == 0 {
                        ready_to_destroy.push(*token);
                        false // Remove from queue
                    } else {
                        true // Keep in queue
                    }
                });
        }

        ready_to_destroy
    }

    /// Marks a resource as failed and logs the error.
    pub fn mark_failed(&mut self, handle: ResourceHandle, error: &str) -> EngineResult<()> {
        self.drop_cpu(handle);
        self.drop_gpu(handle);
        self.set_state(handle, ResourceState::Failed)?;

        // Store error in preview for display in the editor
        if let Some(record) = self.by_handle.get_mut(&handle.raw_handle()) {
            record.preview = Some(PreviewData {
                thumbnail: None,
                summary: format!("Import failed: {}", error),
            });
        }

        Ok(())
    }

    fn ensure_live(&self, handle: ResourceHandle) -> EngineResult<()> {
        if self.handles.is_live(handle.raw_handle()) {
            Ok(())
        } else {
            Err(EngineError::invalid_handle("resource handle is stale"))
        }
    }
}
