# Varg Super Resolution and Frame Generation Rendering Capability PRD

Status: Draft  
Target version: phased delivery  
Last updated: 2026-06-20

## Problem Statement

Varg already has the `engine-render` abstraction, the `engine-render-wgpu` backend, dynamic resolution configuration, and 4K/120 Hz runtime performance targets. The current rendering output still mostly assumes that internal render resolution is close to final output resolution. That is not enough for high-refresh desktop, handheld, mobile, future XR, or cloud gaming:

- High resolution and high refresh rates quickly consume GPU, bandwidth, power, and thermal budgets.
- Mobile devices are usually more constrained by tile memory, bandwidth, thermals, driver capabilities, and battery.
- Desktop players already expect mainstream super-resolution capabilities such as DLSS, FSR, XeSS, DirectSR, and MetalFX.
- New vendor solutions increasingly bundle super resolution, anti-aliasing, frame generation, multi-frame generation, low latency, and neural rendering, but hardware, platform, SDK, and licensing constraints differ widely.
- Frame generation can improve perceived smoothness, but it amplifies input latency, UI composition, present pacing, motion vector, occlusion, and post-processing-ordering problems.
- If Varg directly integrates one vendor SDK into `engine-render-wgpu`, it will bind too early to one platform and vendor while marginalizing mobile.

Varg first needs a platform-independent super-resolution and frame-generation capability model. Game content and editor settings should target capabilities, quality tiers, and budgets rather than a specific vendor SDK. Mobile must be a core target from the first phase, not something added after desktop is complete.

## Solution

Varg will establish a cross-platform Render Scaling and Frame Generation capability layer. This layer is responsible for:

- Decoupling internal render resolution, display output resolution, UI resolution, and screenshot/recording resolution.
- Defining unified super-resolution input data: color, depth, motion vectors, exposure, jitter, near/far, render size, display size, reactive/transparency masks, and frame history.
- Defining unified frame-generation input data: consecutive-frame color, depth, motion vectors, UI composition policy, present timing, latency markers, and frame-generation multiplier.
- Providing runtime capability negotiation that selects available solutions by platform, backend, GPU, driver, SDK, power, and temperature.
- Providing stable editor/project settings so games can declare target quality and mobile strategy.
- Providing conservative fallbacks: native, dynamic resolution, built-in spatial scaler, and built-in temporal upscaler.

Technical support is divided into three layers:

1. **Varg built-in common layer**
   - Native render scale.
   - Dynamic Resolution Scaling.
   - Simple spatial upscaler.
   - Temporal upscaler/TAA-ready path.
   - Optional lightweight mobile upscaler.

2. **Open or relatively cross-platform integrations**
   - AMD FSR 1/2/3.
   - AMD FSR Upscaling / FSR 4 / Redstone, enabled according to SDK and hardware capability.
   - Intel XeSS SR/FG/MFG.
   - Apple MetalFX Upscaling / Frame Interpolation for iOS, iPadOS, macOS, tvOS, and visionOS.
   - Qualcomm Snapdragon Game Super Resolution / GSR as a candidate for Android/Windows on Arm mobile and handheld targets.
   - Mobile super-resolution solutions from Arm or GPU vendors as backend-specific adapters that do not contaminate the public API.

3. **Platform- or vendor-specific integrations**
   - NVIDIA DLSS Super Resolution, DLAA, Frame Generation, Multi Frame Generation, and Ray Reconstruction.
   - NVIDIA Streamline as a candidate Windows desktop aggregation entry point.
   - Microsoft DirectSR as a candidate D3D12/Windows multi-vendor super-resolution entry point.
   - Platform driver-level Auto SR / driver override only as detection and compatibility information, not as a core Varg rendering path.

## Goals

- Let Varg output high-quality images at lower internal rendering cost across desktop, handheld, and mobile.
- Treat mobile as a P0 design constraint covering Android, iOS/iPadOS, Windows on Arm, and mobile GPU thermal budgets.
- Reserve stable integration boundaries for DLSS, FSR, XeSS, MetalFX, GSR, DirectSR, and Streamline.
- Deliver reliable super resolution before frame generation; frame generation must not break input responsiveness, UI readability, or frame pacing.
- Let project authors configure quality tiers, target FPS, minimum render scale, power policy, and fallback per platform.
- Let the editor show the current active upscaler, render scale, output resolution, GPU time, frame-generation multiplier, and latency state.
- Keep public configuration, capability selection, and fallback behavior verifiable in headless tests and GPU-less CI.

## Non-Goals

- The first phase does not promise integration with every vendor SDK.
- The first phase does not implement full production integrations for DLSS/FSR/XeSS/MetalFX/GSR.
- The first phase does not promise multi-frame generation or frame-generation quality.
- Do not bind Varg scenes, materials, or UI to a specific vendor SDK.
- Do not treat external override features in driver control panels as built-in engine support.
- Do not force high-cost ML upscalers on mobile; mobile must respect power and temperature.

## Supported Technology Matrix

| Technology | Category | Main platforms | Support strategy |
| --- | --- | --- | --- |
| Native + Dynamic Resolution | Built-in foundation | All platforms | P0 |
| Built-in Spatial Upscaler | Built-in fallback | All platforms | P0 |
| Built-in Temporal Upscaler / TAA path | Built-in temporal capability | All platforms | P1 |
| AMD FSR 1 | Spatial super resolution | PC, handheld, some mobile/console-like environments | P1 candidate, low-barrier fallback |
| AMD FSR 2/3 | Temporal super resolution / frame generation | PC, handheld, supported Vulkan/DX environments | P1/P2 candidate |
| AMD FSR Upscaling / FSR 4 / Redstone | ML super resolution / neural rendering suite | Mainly PC/handheld, best on RDNA 4 | P2+, hardware and SDK capability detection |
| NVIDIA DLSS SR/DLAA | AI super resolution / anti-aliasing | RTX desktop/laptop | P2+, Windows first |
| NVIDIA DLSS FG/MFG | Frame generation / multi-frame generation | RTX, MFG depends on newer hardware | P3+, requires mature low-latency and present pacing |
| NVIDIA Streamline | Multi-vendor integration framework | Windows desktop | P2+ candidate |
| Intel XeSS SR | AI/DP4a super resolution | Intel Arc/iGPU, some cross-vendor GPUs | P2 candidate |
| Intel XeSS FG/MFG | Frame generation / multi-frame generation | Mainly Windows/DX12, capability depends on SDK | P3+ candidate |
| Microsoft DirectSR | D3D12 multi-vendor SR API | Windows/D3D12 | P2+, D3D12 backend only |
| Apple MetalFX Upscaling | Platform super resolution | iOS, iPadOS, macOS, tvOS, visionOS | Mobile P1 |
| Apple MetalFX Frame Interpolation | Platform frame interpolation | Apple Metal 4 supported devices | Mobile P3 |
| Qualcomm Snapdragon GSR | Mobile/Arm super resolution | Snapdragon Android, Windows on Arm | Mobile P1/P2 candidate |
| Arm/MediaTek/Samsung GPU vendor SR | Mobile vendor capability | Android SoC | Mobile P2+ research and adaptation |
| OS/Driver Auto SR | External override | Windows/driver-supported devices | Detection/explanation only, not a core path |

## Mobile-First Requirements

- Android and iOS/iPadOS must have explicit render scale, dynamic resolution, thermal policy, and battery policy.
- Mobile defaults prioritize stable frame time and thermal control, not short-lived peak image quality.
- Mobile must support 30/40/45/60/90/120 FPS target tiers, with concrete availability negotiated against the device refresh rate.
- Mobile must support more aggressive internal resolution lower bounds, with project configuration deciding whether values below 50% are allowed.
- Mobile UI, text, HUD, and touch feedback must not be damaged by low-quality frame generation or super resolution.
- Mobile should prefer native platform capabilities: MetalFX on Apple, Snapdragon GSR on Qualcomm, and Vulkan/vendor extensions on Android.
- Android backends must assume GPU, driver, and extension fragmentation. Public APIs must not depend on a single SoC.
- iOS/iPadOS backends must use Metal platform capabilities while keeping them isolated from the public `wgpu` rendering abstraction.
- Mobile must expose thermal throttling, GPU time, render scale, upscaler mode, and dropped-frame telemetry.
- Frame generation is disabled by default on mobile and can only be enabled when latency, UI composition, and power policy conditions are satisfied.

## User Stories

1. As a mobile player, I want the game to maintain stable frame pacing under thermal pressure, so that the game remains playable during long sessions.
2. As a mobile player, I want the game to lower internal resolution instead of stuttering, so that controls remain responsive.
3. As a mobile player, I want UI and text to stay sharp, so that touch controls and HUD information remain readable.
4. As an iPhone player, I want the game to use MetalFX when available, so that Apple GPUs can render efficiently.
5. As an Android player, I want the game to use Snapdragon GSR or another available mobile upscaler when supported, so that my phone can trade resolution for performance.
6. As a handheld player, I want balanced upscaling presets, so that battery life and image quality can be tuned.
7. As a PC player, I want DLSS, FSR, or XeSS options when my GPU supports them, so that I can choose the best image/performance tradeoff.
8. As a PC player, I want DirectSR or Streamline-backed options to appear only when valid, so that settings do not expose broken choices.
9. As a competitive player, I want frame generation to be optional and clearly separated from super resolution, so that I can prioritize input latency.
10. As a casual player, I want frame generation to improve perceived smoothness when appropriate, so that high-refresh displays look better.
11. As a graphics programmer, I want one capability negotiation layer, so that vendor SDK checks do not spread through the renderer.
12. As a graphics programmer, I want the renderer to produce motion vectors and depth consistently, so that temporal upscalers receive valid inputs.
13. As a graphics programmer, I want UI composition policy to be explicit, so that generated frames do not smear HUD or editor overlays.
14. As a graphics programmer, I want per-platform adapters, so that MetalFX, DirectSR, Streamline and GSR can coexist without contaminating the core API.
15. As an engine developer, I want headless tests for capability selection, so that CI can verify fallback behavior without GPU SDKs.
16. As an engine developer, I want render targets to know internal and output size separately, so that dynamic resolution and upscalers are first-class.
17. As an engine developer, I want quality presets expressed in normalized project settings, so that platforms can map them to vendor-specific modes.
18. As an engine developer, I want latency telemetry around frame generation, so that performance gains do not hide responsiveness regressions.
19. As an editor user, I want the viewport to show current render scale and upscaler, so that visual changes are understandable.
20. As a project owner, I want per-platform defaults, so that mobile builds can use conservative settings while desktop builds use high-end features.
21. As a QA engineer, I want golden scenes for motion, particles, alpha, UI and disocclusion, so that upscaler artifacts can be compared.
22. As a release engineer, I want proprietary SDKs behind feature flags and platform gates, so that licensing and package size remain controlled.

## Implementation Decisions

- Add a render scaling capability model to `engine-render`, independent from `wgpu`, D3D12, Metal or Vulkan.
- Represent super resolution and frame generation as separate capabilities. A backend may support one without the other.
- Keep render resolution, display resolution and UI composition resolution separate in public configuration.
- Add explicit frame data contracts for temporal rendering: camera jitter, previous view/projection, motion vectors, depth, exposure and history invalidation.
- Add render graph concepts for pre-upscale, upscale, post-upscale and UI composition stages.
- Add an `UpscalerBackend`-style deep module interface that can be implemented by built-in, FSR, DLSS, XeSS, DirectSR, Streamline, MetalFX and mobile vendor adapters.
- Add a `FrameGenerationBackend`-style interface later, after upscaling and frame pacing are stable.
- Store user-facing quality modes as engine modes: Native, UltraQuality, Quality, Balanced, Performance, UltraPerformance and Auto.
- Map engine modes to vendor modes inside adapters, not in project content.
- Treat mobile thermal and battery policy as inputs to automatic mode selection.
- Require every adapter to provide capability reason strings, so editor UI can explain unavailable options.
- Require all proprietary SDK integrations to be optional features and excluded from default open-source builds unless licensing permits bundling.

## Required Render Data Contract

Super resolution backends must be able to request:

- Low-resolution color input.
- Output color target.
- Depth buffer.
- Per-pixel motion vectors.
- Exposure or luminance metadata.
- Camera jitter and frame index.
- Previous frame matrices.
- Reactive/transparency mask where supported.
- Reset/history invalidation flag.
- Render size and output size.

Frame generation backends must additionally request:

- Current and previous resolved frames.
- Optical-flow or generated motion input where required by the vendor.
- UI/HUD composition policy.
- Present timing and display refresh metadata.
- Low-latency markers where supported.
- Generated-frame multiplier.
- Screenshot/recording policy for generated frames.

## Platform Strategy

### Desktop Windows

- `wgpu` remains the current practical backend for Varg.
- DirectSR requires a D3D12 path or native handle integration; do not assume it works through portable `wgpu` APIs.
- Streamline/DLSS/XeSS/FSR integrations must be isolated behind backend-specific adapters.
- Frame generation should not ship until the runtime can measure latency, queue depth and present pacing.

### macOS and iOS/iPadOS

- MetalFX is the primary Apple-platform candidate.
- Apple platforms need a Metal-capable backend boundary. If `wgpu` cannot expose required MetalFX integration points cleanly, the adapter should live in a native Metal backend.
- Mobile Apple support is not a desktop afterthought; iPhone and iPad quality presets must be designed alongside macOS.

### Android

- Vulkan is the likely long-term graphics path for advanced vendor upscalers.
- Snapdragon GSR is a first mobile candidate, but the public API must also support non-Qualcomm devices.
- Thermal policy, memory bandwidth and UI readability are P0 Android concerns.
- Built-in dynamic resolution and spatial/temporal upscaling must work even when no vendor SDK is available.

### Windows on Arm and Handhelds

- Treat Windows on Arm as both desktop and mobile-adjacent.
- Snapdragon X/G devices may expose driver or platform upscaling controls; Varg should detect and document interactions but not rely on driver overrides for correctness.
- Handheld presets should bias toward battery, thermals and stable frame pacing.

## Testing Decisions

- Test public configuration serialization and defaults without a GPU.
- Test capability negotiation with fake adapters for supported, unsupported and partially supported devices.
- Test quality-mode mapping without invoking vendor SDKs.
- Test render scale bounds, automatic mode selection and fallback behavior.
- Add golden/render fixture scenes for camera motion, skinned/animated objects, particles, alpha materials, UI overlays and disocclusion.
- Add benchmark scenes that report internal resolution, output resolution, GPU frame time and upscaler mode.
- Add mobile profile simulations for thermal throttling and battery saver mode.
- Add frame-generation-specific tests only after frame generation backend work begins.

## Rollout Plan

### Phase 0: PRD and Data Contract

- Agree on terminology, supported technology matrix and mobile-first requirements.
- Define public render scaling settings and capability structures.
- Document vendor SDK licensing and platform constraints.

### Phase 1: Built-In Scaling Foundation

- Split internal render size from output size across render targets and runtime metrics.
- Add built-in spatial upscale fallback.
- Extend dynamic resolution to mobile-oriented policies.
- Expose editor/runtime telemetry.

### Phase 2: Temporal Inputs

- Generate motion vectors and camera jitter.
- Add history invalidation and previous-frame metadata.
- Build or integrate a basic temporal upscaler/TAA path.

### Phase 3: Mobile Vendor Path

- Prototype MetalFX on Apple platforms where backend access permits.
- Prototype Snapdragon GSR or equivalent Android mobile upscaler when SDK access is confirmed.
- Validate thermal and power behavior on real devices.

### Phase 4: Desktop Vendor Path

- Prototype FSR 2/3 or FSR Upscaling depending on SDK maturity and backend compatibility.
- Investigate DirectSR for a D3D12 backend path.
- Investigate Streamline/DLSS and XeSS behind optional features.

### Phase 5: Frame Generation

- Add frame generation adapter boundary.
- Implement latency and frame pacing telemetry.
- Prototype FSR/XeSS/DLSS/MetalFX frame generation only after UI composition and input latency policies are stable.

## Out of Scope

- Shipping a production DLSS, FSR 4, XeSS, MetalFX or GSR implementation in the first PRD milestone.
- Making claims about vendor certification before legal/licensing review.
- Supporting frame generation in editor viewport before game runtime.
- Supporting generated frames in deterministic offline render tests.
- Replacing the current render backend choice solely to chase one vendor SDK.

## Further Notes

- NVIDIA documents DLSS as a neural rendering suite that includes Super Resolution, DLAA, Frame Generation and Multi Frame Generation, with Streamline positioned as a cross-IHV integration route.
- AMD documents current FSR Upscaling as the ML-powered successor to the former FSR 4 naming, with SDK support that also includes FSR 2/3 era paths and Redstone technologies.
- Microsoft DirectSR standardizes super resolution for D3D12 and exposes DLSS Super Resolution, FSR and XeSS through a common code path where drivers support it.
- Apple documents MetalFX Upscaling, Frame Interpolation and Denoising as Apple-platform performance technologies, with Metal 4 support on recent Apple devices.
- Intel's XeSS SDK releases include SR, FG and MFG capabilities, but platform/API support varies by version and GPU.
- Mobile support should be validated on physical devices early; desktop GPU benchmarks do not predict mobile thermals.

## References

- NVIDIA DLSS: https://developer.nvidia.com/rtx/dlss
- AMD FSR Upscaling: https://gpuopen.com/amd-fsr-upscaling/
- Microsoft DirectSR preview: https://devblogs.microsoft.com/directx/directsr-preview/
- Apple Metal: https://developer.apple.com/metal/
- Intel XeSS SDK releases: https://github.com/intel/xess/releases
