# Editor Viewport Zero-Copy Presentation Plan

状态：Draft  
目标版本：分阶段交付  
最后更新：2026-06-23

## Problem Statement

Aster Editor 的 Scene View 需要低延迟、高刷新、可预测布局的显示路径。当前稳定 fallback 是 WebView canvas readback：WGPU 离屏渲染后读回 CPU，再通过 Tauri IPC 发送给前端 canvas。这条路径可靠、容易和 React 布局对齐，但不满足性能目标：

- GPU 输出经过 CPU staging buffer 和 IPC，带来额外 copy、同步等待和内存带宽消耗。
- 视口越大、刷新率越高，readback 成本越明显。
- 读回路径会把渲染、传输和 WebView compositing 串在一起，难以达到低延迟目标。
- Web UI overlay、dock panel、透明窗口和不同平台 compositor 行为让“直接把 native child surface 塞进 DOM 位置”变得不稳定。

本计划补充 ADR 0001。ADR 记录架构决策；本文定义可落地的跨平台技术方案、能力语义和交付顺序。

## Terminology

不要只用一个 `zero_copy: bool` 描述 editor viewport。实际显示路径至少需要区分这些能力：

- `cpu_readback`: 是否从 GPU texture/buffer 读回 CPU。
- `gpu_native_surface`: Scene View 是否直接渲染到平台 native GPU surface 或 compositor-imported buffer。
- `gpu_composited`: Scene View 和 Web UI 是否由系统 compositor / native view tree 在 GPU 侧合成。
- `direct_scanout_possible`: 当前帧是否理论上可由 display controller 直接扫描输出。

推荐产品口径：

- **No CPU readback**：Aster 必须保证的 editor 性能目标。
- **GPU-native composition**：跨平台 editor host 的常规目标。
- **Direct scanout**：机会性优化，不作为 UI 覆盖场景下的稳定承诺。

原因是 editor 通常有 Web UI、overlays、selection gizmos、dock panels 和透明区域。只要有覆盖、裁剪、缩放、圆角、alpha 或格式不匹配，系统就可能退回 GPU composition。即使没有 CPU copy，也不等于每帧都能 direct scanout。

## Target Architecture

目标架构是 native host owns root：

1. 原生 host window/view 拥有顶层 editor presentation。
2. Scene View 是 host 管理的 WGPU native surface 或 compositor-imported render buffer。
3. Web UI 作为 panel、dock view、overlay 或 input layer 嵌入 host。
4. React 负责布局意图、editor 状态和输入语义，不负责最终 root surface ownership。
5. 平台 adapter 负责把 Scene View 和 Web UI 放入同一个 native composition tree。

这避免两个失败模式：

- WebView root 和 native child surface 属于不同 compositor/lifecycle，导致 resize、DPI 和 DOM rect 跟踪竞态。
- canvas readback 可靠但高成本，无法作为最终性能路径。

## Capability Model

Editor viewport presentation capability 应从当前布尔模型扩展为结构化状态：

```rust
pub struct ViewportPresentationAdapter {
    pub mode: ViewportPresentationMode,
    pub available: bool,
    pub default: bool,
    pub experimental: bool,
    pub backend: &'static str,
    pub cpu_readback: bool,
    pub gpu_native_surface: bool,
    pub gpu_composited: bool,
    pub direct_scanout_possible: DirectScanoutSupport,
    pub reason: &'static str,
}

pub enum DirectScanoutSupport {
    No,
    Maybe,
    YesWhenUnobscured,
}
```

Initial mapping:

| Adapter | CPU readback | GPU native surface | GPU composited | Direct scanout |
| --- | --- | --- | --- | --- |
| `canvas-readback` | Yes | No | WebView-dependent | No |
| `native-host-window` | No | Yes | Yes | Maybe |
| `wayland-embedded-compositor` | No | DMA-BUF/imported buffer | Yes | Maybe |
| `floating-native-scene-view` | No | Yes | OS-dependent | Yes when unobscured |

`zero_copy` may remain as a compatibility field temporarily, but it should mean `!cpu_readback`, not guaranteed direct scanout.

## Platform Strategy

### Linux X11

Use the existing native host adapter around a GTK DrawingArea/X11 native window:

- Create or reuse a host-owned native drawing surface.
- Extract Xlib/XCB raw display/window handles.
- Create a WGPU surface through raw-window-handle.
- Present Scene View directly to that surface.
- Put Web UI in the same host window as overlays/panels, avoiding DOM-driven child window movement.

This path should be described as no-CPU-readback native presentation. It may still be GPU-composited by the X11 compositor, so direct scanout is only opportunistic.

Risks:

- X11 compositor and window manager behavior varies.
- DPI and fractional scaling must be tested.
- Input hit testing must stay owned by the host, not by visual overlap accidents.

### Linux Wayland

Wayland does not provide an X11-style stable foreign child-window embedding model. The production path should be an application-owned embedded compositor or equivalent toolkit path that imports render buffers through DMA-BUF:

- Host owns editor root.
- Scene View renderer exports or presents buffers that the embedded compositor can import.
- Embedded compositor exposes/imports `zwp_linux_dmabuf_v1` buffers and handles explicit synchronization.
- Web UI surfaces are composed with Scene View inside the same host-owned presentation tree.
- Viewport movement is a compositor state update, not OS child-window repositioning.

The current `wayland-embedded-compositor` feature boundary is the right shape. It should not claim availability until DMA-BUF import, synchronization, input routing and WebView panel surfaces are wired.

Risks:

- DMA-BUF format/modifier negotiation is hardware and driver dependent.
- Explicit sync must be correct to avoid tearing, stalls or undefined buffer reuse.
- WebKitGTK/WebView integration with embedded composition must be validated carefully.
- Direct scanout is rarely guaranteed when editor overlays are visible.

### Windows

The preferred Windows architecture is DirectComposition plus WebView2 visual hosting:

- Native host owns the HWND.
- Scene View renders into a Direct3D/DXGI composition swapchain or WGPU-managed native surface.
- WebView2 uses CompositionController / visual hosting instead of ordinary windowed hosting where possible.
- DirectComposition visual tree combines Scene View and Web UI.
- Host handles layout, scale, pointer, keyboard and focus routing explicitly.

This gives the cleanest Windows model: one composition tree, no CPU readback, and no DOM-owned scene surface. The hard part is connecting WGPU output to a DirectComposition-friendly swapchain. If WGPU cannot expose the needed swapchain control cleanly, create a Windows-specific presentation adapter behind `engine-render-wgpu` instead of leaking D3D details into editor UI code.

Risks:

- WebView2 visual hosting requires explicit input and accessibility handling.
- WGPU's public surface abstraction may not expose all DirectComposition swapchain knobs.
- Multi-monitor DPI and fractional scale need dedicated tests.

### macOS

The preferred macOS architecture is an NSWindow/NSView root with Core Animation layers:

- Native host owns NSWindow and root NSView.
- Scene View is a Metal-backed view/layer, such as CAMetalLayer or an MTKView-style presentation surface behind WGPU.
- Web UI is WKWebView and AppKit panels layered in the native view tree.
- Prefer dock/panel composition over arbitrary transparent hole-punching through WKWebView.

This should achieve no CPU readback and GPU-native composition. As on other platforms, direct scanout is opportunistic and depends on overlay state and compositor decisions.

Risks:

- Transparent WKWebView behavior must be validated per macOS version.
- Layer ordering, backing scale and color space must be explicit.
- WGPU/AppKit lifetime and resizing must remain on the correct threads.

## Implementation Phases

### Phase 0: Capability Semantics

- Replace or extend `zero_copy` capability with `cpu_readback`, `gpu_native_surface`, `gpu_composited` and `direct_scanout_possible`.
- Keep `zero_copy` as compatibility alias for `!cpu_readback` until frontend code is migrated.
- Update frontend labels to say `native GPU` or `no CPU readback`, not guaranteed `direct scanout`.
- Add tests for capability selection on Linux, Wayland feature-disabled, Windows and macOS.

### Phase 1: Stabilize Linux X11 Native Host

- Treat current GTK/X11 native host path as the first production no-readback path.
- Verify resize, DPI, panel overlay, focus, input and scene restart behavior.
- Add telemetry showing active adapter, surface size, viewport rect and CPU readback status.
- Keep canvas readback as fallback when `ASTER_EDITOR_COMPOSITOR` is not enabled or native host setup fails.

### Phase 2: Wayland Embedded Compositor Backend

- Implement the feature-gated embedded compositor backend behind the existing seam.
- Add DMA-BUF import/export or compatible WGPU buffer presentation path.
- Implement explicit synchronization, buffer lifecycle and format/modifier negotiation.
- Route input through the host/compositor boundary.
- Keep refusal semantics strict when backend requirements are missing.

### Phase 3: macOS Native Host

- Create the NSWindow/NSView host adapter.
- Render Scene View into a Metal-backed native surface.
- Embed WKWebView panels/overlays in the native view tree.
- Validate transparency, scale factor, color space and input focus.

### Phase 4: Windows DirectComposition Host

- Create HWND root host with DirectComposition visual tree.
- Integrate WebView2 visual hosting.
- Add Scene View presentation visual through WGPU-compatible DXGI/D3D adapter.
- Validate DPI, multi-monitor, resize, input, accessibility and GPU device loss behavior.

## Verification Matrix

Each adapter must pass these checks before it can become default:

| Area | Required checks |
| --- | --- |
| Presentation | no CPU readback, stable resize, stable device loss recovery |
| Layout | viewport rect follows panels across DPI and window resize |
| Input | pointer capture, keyboard focus, drag, wheel, shortcuts |
| Overlay | gizmos, selection outlines, panels and menus layer correctly |
| Performance | no blocking readback in frame loop, frame pacing telemetry |
| Fallback | failure cleanly returns to canvas readback |
| Diagnostics | active adapter and capability flags visible to frontend/logs |

## Non-Goals

- Do not promise guaranteed direct scanout for the editor viewport.
- Do not rely on DOM positioning of OS child windows as a final architecture.
- Do not expose Direct3D, Metal, AppKit, WebView2 or DMA-BUF details through React-facing APIs.
- Do not remove canvas readback until all desktop fallback and diagnostics are mature.

## Open Questions

- Can WGPU expose enough platform-specific surface control for DirectComposition composition swapchains, or does Aster need a lower-level Windows presentation adapter?
- Should Wayland use a fully application-owned embedded compositor, a toolkit-provided offload path, or both behind one capability?
- How should selection/gizmo overlays split between native renderer and Web UI to minimize transparent WebView requirements?
- What is the minimal telemetry needed to prove a frame used no CPU readback?
- Should floating native Scene View remain a user-visible diagnostic mode for comparing native surface performance?
