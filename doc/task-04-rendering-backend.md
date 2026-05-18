# Task 04: Rendering Abstraction And First Backend

## Goal

Implement a backend-neutral render abstraction and the first usable rendering backend. Vulkan via `ash` is the recommended first backend; Metal, DX12, and WebGPU must remain possible through the abstraction.

## Requirements

- Render abstraction crate independent of concrete backend.
- Vulkan instance, device, swapchain, command buffers, synchronization.
- GPU memory management through `gpu-allocator`, VMA FFI, or equivalent.
- Explicit GPU resource lifecycle and delayed destruction queue.
- Scene View and Game View offscreen render targets.
- Forward and deferred rendering paths.
- PBR, shadows, MSAA, and post-processing.
- RenderGraph compilation and execution.
- Rust-native RenderGraph orchestration API.
- Optional script-driven render path.
- Shader compilation, reflection, hot reload, and pipeline cache invalidation.
- Material pipeline refresh and texture cache invalidation.
- GUI texture upload and GUI draw integration.
- Gizmo, selected outline, resource preview, material preview, and mesh preview support.

## Design Constraints

- No core crate may depend directly on the concrete backend.
- RenderGraph resources must have explicit lifetimes.
- Shader assets should move toward multi-backend compilation or an intermediate description.
- GPU cleanup cannot depend on a scripting language garbage collector.

## Deliverables

- `engine-render` abstraction.
- `engine-render-vulkan` first backend.
- RenderGraph implementation.
- Default scene rendering.
- Scene View and Game View rendering.
- Shader/material pipeline.
- GUI render integration.

## Acceptance

- Runtime starts and renders a default scene.
- Editor profile starts and renders a default scene.
- RenderGraph tests pass.
- Optional script-driven render path tests pass if script backend is enabled.
- Abstraction does not block future Metal, DX12, or WebGPU backends.

