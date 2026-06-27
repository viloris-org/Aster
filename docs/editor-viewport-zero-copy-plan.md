# Editor Viewport Zero-Copy Presentation Plan

Status: Draft  
Target version: phased delivery  
Last updated: 2026-06-23

## Problem Statement

Varg Editor's Scene View needs a low-latency, high-refresh, predictably laid-out presentation path. The current stable fallback is WebView canvas readback: WGPU renders offscreen, reads the result back to the CPU, then sends it to the frontend canvas through Tauri IPC. This path is reliable and easy to align with React layout, but it does not meet the performance target:

- GPU output passes through a CPU staging buffer and IPC, adding extra copies, synchronization waits, and memory bandwidth cost.
- The larger the viewport and the higher the refresh rate, the more visible the readback cost becomes.
- The readback path serializes rendering, transfer, and WebView compositing, making low latency difficult.
- Web UI overlays, dock panels, transparent windows, and platform compositor behavior make "put a native child surface directly at a DOM position" unstable.

This plan supplements ADR 0001. The ADR records the architectural decision; this document defines the cross-platform technical plan, capability semantics, and delivery order.

## Terminology

Do not describe the editor viewport with only a single `zero_copy: bool`. The real presentation path needs to distinguish at least these capabilities:

- `cpu_readback`: whether GPU texture/buffer data is read back to the CPU.
- `gpu_native_surface`: whether Scene View renders directly to a platform-native GPU surface or compositor-imported buffer.
- `gpu_composited`: whether Scene View and Web UI are composited on the GPU by the system compositor / native view tree.
- `direct_scanout_possible`: whether the current frame could theoretically be scanned out directly by the display controller.

Recommended product terminology:

- **No CPU readback**: the editor performance target Varg must guarantee.
- **GPU-native composition**: the normal goal for the cross-platform editor host.
- **Direct scanout**: an opportunistic optimization, not a stable promise when UI overlays are present.

The reason is that the editor usually has Web UI, overlays, selection gizmos, dock panels, and transparent regions. Any overlay, clipping, scaling, rounded corner, alpha, or format mismatch can make the system fall back to GPU composition. No CPU copy does not mean every frame can use direct scanout.

## Target Architecture

The target architecture is native host owns root:

1. The native host window/view owns the top-level editor presentation.
2. Scene View is a host-managed WGPU native surface or compositor-imported render buffer.
3. Web UI is embedded into the host as panels, dock views, overlays, or input layers.
4. React owns layout intent, editor state, and input semantics, not final root-surface ownership.
5. The platform adapter places Scene View and Web UI into the same native composition tree.

This avoids two failure modes:

- WebView root and native child surface belong to different compositors/lifecycles, causing resize, DPI, and DOM-rect tracking races.
- Canvas readback is reliable but expensive, so it cannot be the final performance path.

## Capability Model

Editor viewport presentation capability should expand from the current boolean model to structured state:

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
| `wayland-embedded-compositor` | No | DMA-BUF/imported buffer | Yes | Maybe, diagnostic only |
| `floating-native-scene-view` | No | Yes | OS-dependent | Yes when unobscured |

`zero_copy` may remain as a compatibility field temporarily, but it should mean `!cpu_readback`, not guaranteed direct scanout.

## Platform Strategy

### Linux X11/Xwayland

Use a Linux GTK host root around a `DrawingArea`/X11 native window:

- Create or reuse a host-owned GTK root container.
- Create a host-owned native drawing surface inside that root.
- Extract Xlib/XCB raw display/window handles.
- Create a WGPU surface through raw-window-handle.
- Present Scene View directly to that surface.
- Put Web UI in the same GTK host window under host-controlled widget geometry, avoiding a separate floating Scene View and avoiding DOM-driven native-window movement.

This path should be described as no-CPU-readback native presentation. It may still be GPU-composited by the X11 compositor, so direct scanout is only opportunistic.

Important boundary: the current Tauri implementation now uses a GTK `Fixed` host root that owns the Scene View `DrawingArea`, the main workspace WebView, and separate child WebViews for the toolbar, hierarchy, inspector, and statusbar. The central workspace WebView is still retained as a compatibility/control layer for non-panel editor surfaces and Scene View layout reporting, but active dock panels are host-managed child WebViews instead of full-window transparent WebView regions.

When the desktop session is Wayland, the editor runs this path through Xwayland. Startup sets GTK/Winit to prefer X11 so the parent editor window and embedded Scene View share X11 handles.

Risks:

- X11 compositor and window manager behavior varies.
- DPI and fractional scaling must be tested.
- Input hit testing must stay owned by the host, not by visual overlap accidents.

### Linux Wayland

Wayland does not provide an X11-style stable foreign child-window embedding model. To avoid spending implementation time on that limitation, native Wayland Scene View embedding is out of scope for now. Linux no-CPU-readback presentation runs under X11/Xwayland instead.

The `wayland-embedded-compositor` boundary may remain in code for compatibility and future research, but it is not a default production path. Canvas readback remains the fallback when X11/Xwayland native host presentation is explicitly disabled or unavailable.

Risks:

- Users without a working `DISPLAY`/Xwayland environment cannot use native-host-window presentation.
- Xwayland compositor behavior and fractional scaling still need validation.
- Future native Wayland work would require a separate architecture decision.

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
- Add tests for capability selection on Linux X11/Xwayland, native Wayland refusal, Windows and macOS.

### Phase 1: Stabilize Linux X11 Native Host

- Treat the GTK/X11 host-root path as the first usable no-readback path.
- Verify resize, DPI, panel overlay, focus, input and scene restart behavior.
- Add telemetry showing active adapter, surface size, viewport rect and CPU readback status.
- Keep canvas readback as fallback when native presentation is explicitly disabled with `VARG_EDITOR_COMPOSITOR=0` or native host setup fails.

### Phase 1.5: Split Web UI Panels

- Split the current single React/WebView shell into hosted panel WebViews or native-hosted panel surfaces.
- Route Web UI as explicit hosted panels/overlays in the GTK root instead of depending on full-window WebView transparency.
- Keep the main workspace WebView as the compatibility/control layer until all editor surfaces have resize, DPI, input, focus, and restart parity as hosted views.
- Split WebView panels are enabled by default on the X11 native-host-window path. Set `VARG_NATIVE_PANEL_WEBVIEWS=0` to disable them for diagnostics.
- The toolbar, hierarchy, inspector, and statusbar routes render real editor data and editing controls while the main WebView keeps the central Scene View and non-panel workspace surfaces.

### Phase 2: Native Wayland Deferral

- Do not select `wayland-embedded-compositor` by default.
- Keep native Wayland presentation refusal semantics explicit.
- Document that Linux no-CPU-readback presentation requires X11/Xwayland.
- Revisit native Wayland only behind a new ADR if Xwayland becomes insufficient.

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

- Can WGPU expose enough platform-specific surface control for DirectComposition composition swapchains, or does Varg need a lower-level Windows presentation adapter?
- What user-facing check should report missing Xwayland/`DISPLAY` before native presentation is requested?
- How should selection/gizmo overlays split between native renderer and Web UI to minimize transparent WebView requirements?
- What is the minimal telemetry needed to prove a frame used no CPU readback?
- Should floating native Scene View remain a user-visible diagnostic mode for comparing native surface performance?
