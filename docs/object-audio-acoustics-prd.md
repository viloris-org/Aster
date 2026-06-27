# Varg Object Audio and Environmental Acoustics System PRD

Status: Phase 0 / Phase 1 Implemented  
Target version: phased delivery  
Last updated: 2026-06-19

## Implementation Status

- **Phase 0 — Architecture Contracts and Baselines:** completed on 2026-06-19.
- **Phase 1 — Production PCM Audio:** completed on 2026-06-19.
- **Phase 2 and later:** deferred until explicitly scheduled.

Phase 0/1 implementation includes the device-independent block renderer, bounded command/event queues, object-audio schema foundations, deterministic Memory/Null paths, CPAL default-device output, Symphonia WAV/OGG decoding, resident and bounded background-streamed clips, linear resampling, voice prioritization and virtualization, gain/pitch/seek/fade/scheduled playback, final limiting, device capability and health diagnostics, device-error recovery, ECS schema migration, runtime source/listener transform synchronization, and editor Inspector exposure.

Automated behavior is covered by subsystem, ECS serialization, runtime integration and workspace compilation checks. Physical-device listening and latency validation must be performed on a desktop with an available sound device; the implementation automatically falls back to the Memory backend when no device is available.

## Problem Statement

Varg already has foundational structures for audio resources, source descriptions, Listener, Bus, effects, ECS components, and pluggable backends. It still does not form a real-time audio system suitable for game production:

- The default runtime uses the memory backend, does not connect to the operating system audio device, and cannot output real sound.
- Spatialization only includes distance attenuation and left/right panning, so it cannot reliably express front/back, up/down, distance, or externalized space.
- Full world transforms, velocity, and lifecycle for sources and Listeners are not continuously synchronized.
- There is no streaming decode, resampling, real-time mixing, voice management, or deterministic audio-thread model.
- Scene geometry, materials, and spatial regions do not affect sound. Occlusion, obstruction, reflection, environmental reverb, and room connectivity are missing.
- The current data model does not fully express the directivity, source size, priority, propagation parameters, and output strategy required by object audio.
- Output still implicitly depends on traditional channel-layout thinking and cannot uniformly adapt to stereo, headphones, surround devices, or platform spatial-audio APIs.
- There is no Dolby Atmos platform output path, capability negotiation, dynamic-object budget, or fallback strategy.
- The editor has no workflow for spatial-audio authoring, preview, debugging, or performance analysis.

Adding a specific channel layout or Dolby-specific path directly on top of the current structure would bind the internal model to one output format and cause stereo, headphones, Atmos, and future platform backends to grow separate logic. Varg needs an output-device-independent object audio model first, then explicit layering for environmental propagation and endpoint rendering.

## Solution

Varg will evolve the existing `engine-audio` into an object-audio core and add an independent, optional environmental acoustics subsystem. Game content is expressed through sound objects, Listener, Bus, and acoustic environments, not through fixed channel layouts such as 2.0, 5.1, or 7.1.4 as the primary authoring model.

The system uses a three-layer architecture:

1. **Object audio core**
   - Manages audio resources, voices, real-time mixing, Bus, DSP, playback state, and object metadata.
   - Provides stable, device-independent, testable control interfaces.
   - Distinguishes spatialized objects, non-spatialized direct content, and environmental sound fields.

2. **Environmental acoustics system**
   - Extracts simplified acoustic geometry, acoustic materials, Room, Portal, and Zone data from scenes.
   - Computes direct-sound occlusion, obstruction, transmission, early reflections, and reverb-send parameters.
   - Supports real-time, baked, and hybrid computation modes, degrading by quality tier.

3. **Output rendering and platform adaptation**
   - Stereo Speaker: stable downmix for ordinary two-speaker setups.
   - Binaural HRTF: 3D spatial rendering for any stereo headphones.
   - Surround Bed: compatible output for traditional multichannel devices.
   - Platform Spatial Audio: submits dynamic spatial objects to platforms such as Windows/Xbox, where the system outputs Dolby Atmos, Windows Sonic, or another user-selected spatial format.
   - Null/Memory/Offline: used for device-less environments, tests, and offline rendering.

Dolby Atmos is defined as a platform output capability, not as Varg's internal scene format or object model. When Atmos is disabled, unsupported by the device, or constrained by object budget, content must remain functionally correct and automatically fall back to HRTF, surround bed, or stereo output.

End users should receive consistent spatial intent from the same scene and the same set of audio objects:

- Ordinary stereo headphones receive front/back, up/down, distance, and environmental space cues through HRTF.
- Ordinary stereo speakers receive stable stereo positioning, distance, and environmental response.
- Multichannel devices receive output matched to their actual layout.
- Supported platforms can submit high-priority objects as Dolby Atmos dynamic objects.

## Goals

- Establish a cross-platform real-time audio foundation that can output real sound.
- Decouple the spatial-audio content model from fixed channel layouts into an object model.
- Make stereo headphones a first-class spatial-audio output target.
- Support extensible environmental acoustic propagation without blocking basic game runtime.
- Support quality tiers configured by project, platform, and hardware capability.
- Establish a legal, optional, fallback-capable platform adaptation path for Dolby Atmos.
- Provide stable schemas usable by AI Agents and declarative scene generation.
- Keep headless tests, server runtime, and environments without audio devices usable.

## Success Metrics

- On supported desktop platforms, the sample project can output audio stably for 30 continuous minutes without underruns, crashes, or significant memory growth.
- A benchmark scene with 128 active voices and 32 spatialized voices stays within a 15% single-CPU-core audio budget on the target development machine.
- Default interactive audio targets no more than 50 ms end-to-end latency; backend buffering is configurable.
- Listener and dynamic source transform updates take effect within the next audio block without obvious zipper noise.
- HRTF mode can distinguish left/right, front/back, and up/down in standard subjective test scenes, and direction changes do not pop.
- When HRTF is absent, Atmos is absent, or the spatial-object budget is zero, the system falls back automatically without losing critical sounds.
- Occlusion state changes are smooth, with no per-frame switching noise; ray-query budgets are controllable.
- The same scene can switch between Stereo, Binaural, Surround, and Platform Spatial output without modifying scene content.
- The audio thread does not perform blocking file I/O, acquire locks that may be held by the game thread for long periods, or perform non-preallocated heap allocations.
- Headless CI can deterministically verify voice lifecycles, object selection, propagation parameters, and routing behavior.

## User Stories

1. As a game player, I want sounds to originate from their world positions, so that I can understand events without looking directly at them.
2. As a headphone player, I want front, back, above, and below cues from ordinary stereo headphones, so that spatial audio does not require a surround speaker system.
3. As a stereo speaker player, I want a stable and intelligible mix, so that object audio still improves the experience on common hardware.
4. As a home theater player, I want the game to use my configured spatial output, so that supported sounds can be rendered over my actual speaker layout.
5. As a player, I want dialogue, UI, music, and accessibility cues to remain clear, so that spatial processing does not reduce usability.
6. As a player, I want audio settings to expose output mode, dynamic range, HRTF, quality, and latency, so that I can match my hardware and preferences.
7. As a player, I want the game to recover when the output device changes or disappears, so that unplugging headphones does not crash the game.
8. As a player, I want important sounds to remain audible during heavy scenes, so that voice limits do not hide gameplay-critical information.
9. As a player, I want walls and closed spaces to affect sound smoothly, so that audio agrees with the visible environment.
10. As a player, I want rooms to sound different according to size and material, so that environments feel physically coherent.
11. As a player, I want doors and openings to transmit sound between rooms, so that nearby activity remains believable.
12. As a player, I want moving sources to exhibit plausible Doppler behavior, so that fast vehicles and projectiles sound dynamic.
13. As a sound designer, I want to author a sound as an object rather than targeting fixed channels, so that one asset works across output devices.
14. As a sound designer, I want to choose point, cone, line, area, or ambient source behavior, so that different emitters have appropriate spatial characteristics.
15. As a sound designer, I want to configure source directivity, distance curves, spread, Doppler, and priority, so that localization matches the intended source.
16. As a sound designer, I want to route sounds through named buses, sends, snapshots, and effects, so that I can control the mix by game state.
17. As a sound designer, I want music and UI to bypass world spatialization when required, so that direct-to-listener content remains natural.
18. As a sound designer, I want to preview binaural and stereo output in the editor, so that I can author without specialized speakers.
19. As a sound designer, I want to preview a selected source from the Listener position, so that spatial settings can be tuned quickly.
20. As a sound designer, I want a visual overlay for audible range, directivity, Rooms, Portals, and acoustic rays, so that scene behavior is inspectable.
21. As a sound designer, I want meters for object count, virtual voices, bus levels, clipping, and underruns, so that mix and performance problems are visible.
22. As a sound designer, I want to mark sounds as critical, normal, ambience, or disposable, so that object and voice budgeting preserves intent.
23. As a sound designer, I want deterministic random containers, playlists, and variation, so that repeated sounds avoid obvious repetition.
24. As a sound designer, I want loop points and gapless streaming, so that ambience and music do not click or pause.
25. As a sound designer, I want per-source reverb sends and occlusion responses, so that different content reacts appropriately to the environment.
26. As a level designer, I want to assign acoustic materials to scene surfaces, so that concrete, glass, wood, and fabric affect propagation differently.
27. As a level designer, I want Rooms and Portals to be generated or authored from scene structure, so that indoor propagation is efficient.
28. As a level designer, I want acoustic Zones to override ambience and reverb, so that artistic control can supplement physical simulation.
29. As a level designer, I want acoustic geometry to use simplified meshes, so that rendering detail does not automatically become audio cost.
30. As a level designer, I want static acoustic data to be baked, so that large environments can run on modest hardware.
31. As a level designer, I want dynamic doors and blockers to update propagation at runtime, so that gameplay changes are reflected in audio.
32. As a gameplay programmer, I want stable handles and commands for play, pause, stop, seek, parameter changes, and fades, so that audio behavior is predictable.
33. As a gameplay programmer, I want audio commands to be safe from any game thread, so that gameplay systems do not manipulate the real-time mixer directly.
34. As a gameplay programmer, I want sound events to return instance handles and completion notifications, so that logic can coordinate with playback.
35. As a gameplay programmer, I want declarative AudioSource and AudioListener components, so that scenes and prefabs serialize cleanly.
36. As a gameplay programmer, I want runtime parameters and snapshots, so that combat, underwater, pause, and slow-motion states can alter the mix.
37. As a gameplay programmer, I want queries for estimated audibility and playback state, so that game logic can respond without depending on backend details.
38. As a gameplay programmer, I want listener-relative and world-relative emitters, so that first-person equipment and world events can use different spaces.
39. As a gameplay programmer, I want one logical sound event to own layered voices, so that complex sounds remain manageable.
40. As an AI Agent, I want explicit audio object schemas and valid ranges, so that generated scenes produce valid spatial audio configurations.
41. As an AI Agent, I want capability-aware defaults, so that generated content works without knowing the player's output hardware.
42. As an engine developer, I want the real-time mixer isolated behind a deep module, so that platform APIs do not leak into ECS and gameplay code.
43. As an engine developer, I want the acoustic solver isolated from the audio device backend, so that propagation can be tested and replaced independently.
44. As an engine developer, I want bounded lock-free command and event queues, so that game state can communicate safely with the audio thread.
45. As an engine developer, I want fixed-size or pooled real-time allocations, so that audio callbacks do not unpredictably allocate.
46. As an engine developer, I want deterministic memory and offline backends, so that CI can test behavior without sound hardware.
47. As an engine developer, I want output capability negotiation, so that platform object limits and layouts are handled centrally.
48. As an engine developer, I want a voice allocator with priority, distance, gain, age, and category policies, so that overload behavior is deterministic.
49. As an engine developer, I want virtualization for inaudible voices, so that playback timelines continue without full DSP cost.
50. As an engine developer, I want decoded sample caching and streaming budgets, so that memory and I/O remain bounded.
51. As an engine developer, I want metrics for callback duration and buffer health, so that regressions can be diagnosed.
52. As an engine developer, I want acoustic quality tiers, so that low-end and high-end configurations share the same content.
53. As an engine developer, I want a platform-neutral object scene, so that Windows Spatial Audio and future APIs are adapters rather than forks.
54. As a Windows player, I want the game to honor my selected Windows spatial sound provider, so that Dolby Atmos for Headphones, home theater, or Windows Sonic can be chosen at system level.
55. As a Windows engine developer, I want high-priority sounds submitted as dynamic spatial objects, so that supported platform renderers retain their object metadata.
56. As a Windows engine developer, I want object allocation to respond to the platform's current dynamic-object count, so that device capability changes do not break playback.
57. As a Windows engine developer, I want excess objects to fold into a bed or internal binaural/stereo mix, so that object limits degrade gracefully.
58. As a release engineer, I want Dolby-branded support behind a separately enabled and validated integration, so that licensing and certification claims remain accurate.
59. As a QA engineer, I want repeatable reference scenes and captured outputs, so that spatial, routing, and acoustic regressions can be compared.
60. As a QA engineer, I want device-loss, sample-rate-change, underrun, and object-exhaustion tests, so that runtime recovery is verified.
61. As an accessibility user, I want a reduced-spatialization or mono-compatible mode, so that important information remains understandable.
62. As an accessibility user, I want dialogue and critical cues protected from aggressive virtualization, so that gameplay remains accessible.
63. As a project owner, I want advanced acoustics to be optional, so that small games do not pay build size or runtime cost for unused features.
64. As a project owner, I want project-level audio quality defaults and platform overrides, so that shipping configurations are reproducible.
65. As a project owner, I want the implementation to be incremental, so that basic working audio ships before expensive physical simulation.

## Functional Requirements

### Playback and Asset Pipeline

- Decode at least WAV and OGG/Vorbis in the first production milestone.
- Support resident clips for short sounds and streamed clips for music and long ambience.
- Normalize all mixer input to an internal floating-point format and a device-selected mix sample rate.
- Provide resampling when asset and device sample rates differ.
- Support play, pause, resume, stop, seek, looping, loop regions, fades and scheduled start.
- Preserve gapless playback where the source format and decoder permit it.
- Support mono assets as the preferred source format for positional objects.
- Define explicit handling for stereo and multichannel source assets.
- Keep asset decode and disk I/O outside the real-time audio callback.

### Object Audio

- Each spatial object must support transform, velocity, gain, pitch, source shape, directivity, spread, attenuation, Doppler scale, priority, bus, sends and virtualization policy.
- Transform updates must be interpolated or smoothed between game frames and audio blocks.
- Listener must include transform, velocity, output preferences and optional head-tracking transform.
- Multiple listeners are not required for the first production milestone, but the public model must not make future support impossible.
- Direct content must bypass world HRTF and room positioning unless explicitly routed otherwise.
- Ambient fields must support non-localized or partially localized rendering.

### Mixing and DSP

- Provide a hierarchical Bus graph with gain, mute, solo, effects, sends and snapshots.
- Use click-free parameter smoothing for gain, filters, sends and routing changes.
- Provide limiter protection on the final output.
- Preserve effect state across normal frame updates.
- Support sidechain input and ducking for dialogue or accessibility use cases.
- Distinguish per-object DSP from shared Bus DSP to control CPU cost.

### Voice Management

- Distinguish logical sound instances, physical voices and platform spatial objects.
- Apply configurable limits per project and per category.
- Use deterministic scoring based on explicit priority, estimated gain, distance, category, age and critical status.
- Virtualize voices instead of stopping them when timeline continuity matters.
- Promote and demote voices without resetting playback position.
- Reserve capacity for dialogue, UI and gameplay-critical cues.

### Binaural and Stereo Rendering

- Provide built-in stereo panning as a universal fallback.
- Provide an HRTF renderer that accepts object-relative direction and distance and outputs stereo.
- Interpolate HRTF filters during object motion to avoid discontinuities.
- Support configurable HRTF quality and maximum binaural object count.
- Render lower-priority objects through a cheaper shared spatial bed when the HRTF budget is exceeded.
- Allow speaker stereo and headphone binaural modes to use different processing.
- Do not force direct music or UI through headphone virtualization.

### Environmental Acoustics

- Represent acoustic materials using frequency-dependent absorption, transmission and scattering bands.
- Extract or import simplified acoustic geometry independently from render geometry.
- Support direct-path visibility tests and multiple-ray coverage tests for large sources.
- Produce smoothed occlusion gain and low-pass parameters.
- Support obstruction as partial path interference distinct from total occlusion.
- Support Room and Portal graphs for efficient indoor propagation.
- Support artist-authored Zones and reverb overrides.
- Support algorithmic reverb in the initial milestone and convolution reverb as a later option.
- Support early reflection approximation and late-reverb parameter generation.
- Support static bake data with versioning and invalidation when geometry or material inputs change.
- Permit dynamic blockers and Portal openness to modify baked/static results.

### Platform Spatial Output and Dolby Atmos

- Treat platform spatial APIs as output sinks consuming Varg audio objects.
- Query the active output mode, supported static layout and dynamic object count.
- Maintain a platform-independent object priority policy.
- Submit selected hero objects as dynamic platform objects.
- Render or fold remaining objects into a compatible static bed or internal fallback mix.
- Handle a dynamic object count of zero without content failure.
- On Windows, initially target Microsoft Spatial Sound rather than implementing a Dolby bitstream encoder.
- Allow the operating system and user-selected provider to render Windows Sonic, Dolby Atmos or other supported formats.
- Keep Dolby-specific packaging, branding, certification and licensing outside the open core unless explicitly approved.
- Do not claim Dolby Atmos support until the relevant integration and release requirements are met.

### Editor and Tooling

- Add AudioSource, AudioListener, AcousticMaterial, AcousticGeometry, AcousticRoom, AcousticPortal and AudioZone authoring support.
- Display source range, direction cone, spread, Room boundaries, Portals and acoustic debug paths.
- Provide preview modes for stereo speakers, binaural headphones and platform spatial output when available.
- Provide real-time meters and diagnostics for voices, virtual voices, spatial objects, buses, CPU, buffer health and clipping.
- Provide an acoustic bake command with progress, cancellation, cache status and diagnostics.
- Store all authoring data in versioned declarative schemas suitable for scene serialization and AI operations.

### Runtime Reliability

- Recover from output device loss and default-device changes.
- Support device sample-rate and channel-layout changes through controlled backend restart.
- Maintain a Null backend and deterministic Memory backend.
- Report underruns, device errors and fallback decisions through runtime diagnostics.
- Never allow an audio backend failure to crash headless or server profiles.

## Implementation Decisions

### 1. Extend the Existing Audio Subsystem

- `engine-audio` remains the public audio subsystem; a second competing playback engine will not be introduced.
- Existing clip, source, Listener, Bus and backend concepts will be migrated rather than duplicated.
- Compatibility constructors and schema defaults will preserve existing scenes during the migration.
- The current backend trait is considered a control-plane prototype, not the final real-time rendering boundary.

### 2. Extract a Deep Real-Time Renderer Module

- Introduce an internal renderer module that owns decoded sample access, voice state, resampling, spatialization, Bus processing and final mixing.
- Its primary boundary is block-based: consume immutable audio data and bounded commands, then render a fixed number of frames into an output buffer.
- Operating-system backends only manage devices, callback timing and format conversion. They do not own gameplay semantics.
- Gameplay and ECS code communicate through command and event queues rather than calling mutable backend objects from arbitrary threads.
- Real-time processing uses preallocated pools and bounded queues; callback-time file I/O, logging, blocking locks and unbounded allocation are prohibited.

### 3. Separate Control Plane and Render Plane

- The control plane owns assets, logical instances, ECS bindings, serialization and high-level commands.
- The render plane owns physical voices, DSP state, sample cursors and output buffers.
- Commands carry monotonic sequence numbers or timestamps so ordering is deterministic.
- Events report completion, errors, marker crossings, device changes and voice state transitions back to the control plane.
- Stale handles use generation counters to prevent accidental reuse.

### 4. Define Device-Independent Audio Objects

- Replace the current optional-position model with an explicit spatial mode:
  - Direct
  - Object
  - Ambient field
- A source can blend between direct and spatial rendering, but the blend is an authored property rather than an implicit output-channel decision.
- Objects use world coordinates converted into Listener-relative coordinates at the renderer boundary.
- Source shape and directivity are part of object metadata.
- Output channels are selected only by the output renderer.

### 5. Use Mono Positional Sources by Default

- Mono assets are the canonical format for point-like spatial objects.
- Stereo sources default to direct or ambient-field playback.
- Explicit policies define whether stereo content remains stereo, is downmixed to mono, or is interpreted as a spatial bed.
- Arbitrary multichannel source ingestion is deferred until the internal channel semantics are formally defined.

### 6. Built-In Binaural Renderer Before Atmos

- HRTF rendering is the first advanced spatial output because it benefits the largest hardware base and validates the object model.
- The HRTF implementation is hidden behind a renderer interface so datasets and convolution implementations can change.
- The first version may use a compact public-domain or appropriately licensed HRTF dataset.
- Partitioned convolution, filter interpolation and shared fallback beds are implementation targets.
- Personalized HRTF and ear scanning are out of scope.

### 7. Add `engine-acoustics` as an Optional Deep Module

- Environment propagation becomes a separate crate or feature-isolated subsystem with no dependency on operating-system audio APIs.
- Input is an `AcousticSceneSnapshot` containing simplified geometry, materials, Rooms, Portals, dynamic blockers, source samples and Listener state.
- Output is a compact `PropagationFrame` per relevant source containing direct gain, filter bands, delay, direction, early-reflection taps and late-reverb sends.
- The audio renderer consumes propagation results but remains functional when no acoustic solver is present.
- The acoustic solver can therefore be replaced, benchmarked, baked or disabled independently.

### 8. Reuse Physics Queries Selectively

- Basic occlusion may reuse the physics world's ray-query interface when collision geometry sufficiently matches acoustic geometry.
- Advanced propagation will not depend directly on Rapier internals.
- Acoustic geometry has its own layers and simplified representation because render and collision meshes may be unsuitable for sound.
- Cross-subsystem exchange occurs through snapshots or narrow query interfaces, avoiding circular crate dependencies.

### 9. Quality Tiers

- **Off:** distance and basic stereo only.
- **Low:** basic panning/HRTF, one occlusion ray, algorithmic reverb zones.
- **Medium:** multi-ray occlusion, Room/Portal propagation, limited early reflections.
- **High:** more reflection paths, higher HRTF quality, convolution reverb or denser acoustic updates.
- **Bake-focused:** static propagation data plus limited dynamic correction.
- Each tier has explicit budgets for voices, HRTF objects, rays, update rate, reflection paths and memory.

### 10. Hybrid Real-Time and Baked Acoustics

- Full wave simulation is not a runtime target.
- Geometric acoustics and perceptual approximations are the primary approach.
- Static levels can bake visibility, Room coupling, reflection probes or impulse responses.
- Dynamic sources and Listeners consume baked data with runtime interpolation.
- Moving doors, blockers and objects add bounded dynamic corrections.
- Acoustic calculations update at a lower configurable rate than the audio callback and are smoothed by the renderer.

### 11. Platform Backend Strategy

- A cross-platform PCM device backend is required first. The implementation may use a focused device library, but its public types must not leak into engine APIs.
- Windows Spatial Audio is a separate output backend using the same object selection and routing model.
- Platform backends advertise capabilities rather than being selected by hard-coded platform checks throughout the engine.
- Capability data includes sample rate, block size, channel layout, spatial support, static-object mask and dynamic-object count.
- Unsupported features return structured capability diagnostics and trigger deterministic fallback.

### 12. Atmos Integration Strategy

- Varg will not implement a proprietary Dolby encoder as part of this PRD.
- On Windows and Xbox-compatible targets, Varg will submit spatial objects through the platform spatial API.
- The operating system handles output to Dolby Atmos for Headphones or home theater when enabled by the user and supported by the device.
- Platform object scarcity is handled through:
  1. reserved critical objects;
  2. score-based hero object selection;
  3. co-location or grouping where perceptually safe;
  4. folding remaining content into a static bed or internal mix.
- Offline Dolby Atmos master creation for film/music deliverables is a separate product workflow and not part of game-runtime Atmos support.

### 13. ECS and Schema Evolution

- Existing AudioSource data migrates to an expanded, versioned object-audio component.
- Listener orientation comes from the actual world transform, not a fixed default direction.
- Runtime synchronizes source and Listener transform plus velocity every frame using batched updates.
- New schema types include source spatial mode, shape, directivity, attenuation curve, priority, Doppler, bus, sends and acoustic response.
- Acoustic components remain data-only in ECS; backend handles are runtime state and are never serialized.
- Schema migration fills conservative defaults so old scenes continue to load.

### 14. Bus and Effect Graph Evolution

- Bus processing becomes channel-layout aware instead of treating buffers as untyped interleaved samples.
- Routing direction and mix semantics are explicitly defined and covered by tests.
- Effects declare supported layouts, latency and real-time safety.
- Unsupported effect/layout combinations either adapt through a defined conversion or emit diagnostics.
- Reverb receives environment sends rather than being applied indiscriminately to the entire mix.

### 15. Asset Decode and Streaming

- Decode support is implemented through an isolated decoder abstraction.
- The initial production formats are WAV and OGG/Vorbis; additional codecs require licensing and platform analysis.
- Short clips are decoded into an immutable sample store.
- Long clips use background decode workers and bounded ring buffers.
- Stream starvation emits diagnostics and degrades without blocking the audio callback.
- Cache keys include asset identity, import settings, channel policy and target sample format.

### 16. Scheduling and Time

- Audio time is based on sample frames, not variable game-frame delta.
- Game commands may request immediate, delayed or sample-clock scheduled execution.
- Renderer state continues during temporary game-frame stalls as long as buffers are available.
- Pause behavior distinguishes game pause, Bus pause and device suspension.

### 17. Diagnostics and Profiling

- Expose active logical instances, physical voices, virtual voices, platform objects, callback time, DSP time, decode time, buffer fill, underruns and peak levels.
- Diagnostics use lock-free counters or snapshots safe for the audio thread.
- Debug visualization consumes copied state and never reads mutable renderer internals.
- Platform fallback reasons are visible in the editor and runtime logs.

### 18. Feature and Build Boundaries

- Base audio types and Null/Memory behavior remain lightweight.
- Device output, HRTF, advanced acoustics and platform spatial output are separately feature-gated.
- Headless and server profiles compile without native audio libraries.
- Advanced features must not force Dolby SDKs or platform-specific dependencies into all builds.
- Any third-party native dependency requires supported-platform, license, build and CI analysis before adoption.

## Technical Delivery Plan

### Phase 0 — Architecture Contracts and Baselines

Deliverables:

- Finalize renderer/control-plane boundary and audio thread invariants.
- Define versioned object, Listener, Bus, capability and command schemas.
- Add deterministic benchmark scenes and golden behavioral tests.
- Record baseline latency, voice count and current API compatibility.
- Define third-party dependency acceptance criteria and license review.

Exit criteria:

- Public architecture review completed.
- Old AudioSource scenes migrate without data loss.
- Memory backend tests cover new command and handle semantics.

### Phase 1 — Production PCM Audio

Deliverables:

- Real operating-system output backend.
- Block mixer, voice pool, sample clock and command/event queues.
- WAV and OGG/Vorbis decoding.
- Resident and streaming playback.
- Resampling, gain, pitch, looping, fades and final limiter.
- Device discovery, device loss recovery and diagnostics.
- Runtime transform synchronization and editor playback preview.

Exit criteria:

- A sample game produces stable audio on supported desktop targets.
- Streaming music and looping ambience run without audible gaps under normal load.
- Audio callback satisfies real-time safety checks.
- Null and Memory backends remain available.

### Phase 2 — Object Audio and Binaural Rendering

Deliverables:

- Expanded AudioObject and Listener schemas.
- Direction, velocity, attenuation, directivity, spread and Doppler.
- Stereo speaker renderer.
- HRTF binaural renderer with smooth motion.
- Voice priority, virtualization and HRTF object budgeting.
- Direct-content path for UI, music and dialogue.
- Spatial debug visualization and metrics.

Exit criteria:

- Same authored scene works in stereo and binaural modes.
- Object overload preserves critical cues and degrades deterministically.
- Direction changes and renderer switching do not click or reset unrelated voices.

### Phase 3 — Practical Environmental Acoustics

Deliverables:

- Acoustic materials and simplified acoustic geometry.
- Single- and multi-ray occlusion.
- Obstruction, transmission filtering and parameter smoothing.
- AudioZone, Room and Portal data models.
- Algorithmic room reverb and environment sends.
- Dynamic doors and Portal openness.
- Acoustic debug overlays.

Exit criteria:

- Walls, doorways and rooms produce stable, tunable audible differences.
- Acoustic queries remain inside configured CPU and ray budgets.
- Disabling acoustics preserves object playback and spatialization.

### Phase 4 — Baked and Higher-Quality Propagation

Deliverables:

- Acoustic bake pipeline and cache invalidation.
- Static Room coupling and reflection data.
- Early reflection approximation.
- Optional convolution reverb and impulse-response assets.
- Hybrid baked/runtime propagation.
- Quality presets and platform defaults.

Exit criteria:

- Large static reference level meets target CPU budget.
- Bake output is deterministic for the same inputs and version.
- Missing or stale bake data falls back safely.

### Phase 5 — Platform Spatial Audio and Atmos Path

Deliverables:

- Platform capability negotiation.
- Windows Spatial Audio object output.
- Dynamic object allocator and object/bed fallback.
- Runtime output-mode changes.
- Atmos-capable hardware/headphone validation matrix.
- Licensing, branding and release documentation.

Exit criteria:

- With Windows spatial sound enabled, hero objects are submitted as dynamic objects.
- With dynamic object count zero, the scene continues through the fallback renderer.
- Supported Dolby Atmos configurations are validated before public claims are enabled.

### Phase 6 — Advanced Research Track

Candidate work:

- Diffraction approximation.
- GPU-assisted acoustic ray tracing.
- Higher-order Ambisonics for ambient fields.
- Head tracking.
- Multiple Listeners and split-screen policy.
- Authoring integration for external audio middleware.

This phase is not committed by this PRD and requires independent cost/benefit approval.

## Testing Decisions

Good tests verify externally observable audio behavior, timing contracts, fallback policy and serialized data. Tests must not assert private mixer structure, specific thread scheduling or incidental DSP implementation details.

### Unit and Property Tests

- Handle generation rejects stale clip, instance and voice handles.
- Commands retain ordering and scheduled sample-frame semantics.
- Attenuation curves are bounded, monotonic where required and stable at edge cases.
- Voice scoring preserves critical reservations and deterministic tie-breaking.
- Virtualized voices preserve timeline position.
- Parameter smoothing reaches target values without discontinuity.
- Bus routing conserves expected gain and honors mute, solo, sends and snapshots.
- Schema serialization and migration preserve authored values.
- Capability negotiation selects the documented fallback.
- Room/Portal graph propagation handles disconnected rooms, cycles and dynamic openness.
- Acoustic material interpolation remains bounded across frequency bands.

### Renderer Tests

- Render known mono inputs at canonical directions and verify finite, bounded stereo output.
- Verify left/right energy relationships rather than exact implementation coefficients.
- Verify HRTF transitions do not create sample discontinuities beyond defined thresholds.
- Verify direct sources remain unaffected by world rotation.
- Verify Doppler changes pitch in the expected direction and remains bounded.
- Verify limiter prevents output above the configured ceiling.
- Verify object demotion to fallback beds does not stop playback.

### Acoustic Solver Tests

- Unobstructed paths produce unity or configured direct gain.
- Fully blocking geometry reduces direct gain and applies the expected filtering range.
- Partial obstruction produces intermediate results.
- Open and closed Portals alter Room coupling predictably.
- Dynamic blockers update propagation without invalidating unrelated static data.
- Bake output is deterministic and versioned.
- Solver time and ray count remain within configured budgets for reference scenes.

### Integration Tests

- Scene loading creates logical instances and synchronizes source transforms.
- Moving the main camera updates Listener position, orientation and velocity.
- Destroying an entity releases its logical instance without stale playback.
- Device restart preserves or intentionally restarts voices according to documented policy.
- Stream decode starvation reports an underrun without deadlocking.
- Editor changes to source properties reach the running preview.
- Headless runtime uses Memory or Null output without native device access.

### Platform Tests

- Enumerate output capabilities and handle unsupported spatial APIs.
- Validate dynamic object count changes while running.
- Validate zero-object fallback.
- Validate sample-rate and device changes.
- Validate Windows spatial output using platform-provided test tooling where available.
- Keep platform tests feature-gated and skippable when required hardware or APIs are unavailable.

### Performance and Soak Tests

- Benchmark 32, 64, 128 and 256 logical voices with configurable physical voice limits.
- Benchmark HRTF object counts and fallback-bed behavior.
- Benchmark acoustic ray and reflection budgets.
- Run 30-minute playback and streaming soak tests.
- Detect callback deadline misses, allocations, lock contention and memory growth.
- Track benchmark baselines in CI where runners are stable; otherwise publish local reference commands and thresholds.

### Subjective Listening Tests

- Maintain reference scenes for azimuth, elevation, distance, front/back, room transitions, occlusion and fast motion.
- Use structured listening checklists rather than unrecorded informal judgment.
- Compare stereo speaker, binaural and platform spatial modes.
- Subjective tests supplement but do not replace automated correctness and performance tests.

### Prior Art in the Repository

- Keep deterministic backend tests alongside the audio implementation.
- Use integration tests under subsystem and runtime test directories.
- Name tests by observable behavior.
- Feature-gate native-device and platform tests.
- Permit hardware-dependent tests to skip with an explicit reason while keeping pure renderer tests headless.

## Rollout and Compatibility

- Existing AudioSource scenes load through schema migration.
- Initial defaults preserve current behavior: direct or simple spatial blend, full volume, no advanced acoustics.
- New advanced features are opt-in by project quality profile until production stability is established.
- Runtime diagnostics report when a requested feature is unavailable and identify the selected fallback.
- Project manifests record audio feature requirements and recommended defaults.
- Saved scenes contain semantic object properties, never platform object IDs or fixed device channels.

## Security, Licensing, and Legal Constraints

- Dolby and Dolby Atmos are trademarks and licensed technologies; public branding requires separate approval.
- No Dolby SDK, encoder or proprietary asset may be committed without license review.
- HRTF datasets, codec libraries and impulse responses require compatible licenses and attribution review.
- Audio decoders must be assessed for malformed-input handling and resource exhaustion.
- Untrusted audio assets are decoded with bounded memory and duration checks.
- Platform-specific integrations remain optional so the open core can build without proprietary dependencies.

## Risks and Mitigations

- **Risk: Real-time audio introduces hard-to-reproduce timing defects.**  
  Mitigation: isolate the renderer, prohibit callback-time blocking, add counters, deterministic offline rendering and soak tests.

- **Risk: Building playback, HRTF and acoustics simultaneously delays all usable audio.**  
  Mitigation: phase production PCM output first and require exit criteria before advanced acoustics.

- **Risk: Physical simulation consumes excessive CPU.**  
  Mitigation: use perceptual approximations, quality tiers, lower update rates, baking and bounded source selection.

- **Risk: Platform object limits vary at runtime.**  
  Mitigation: capability negotiation, reserved priorities, virtualization and bed/internal-renderer fallback.

- **Risk: HRTF localization varies by listener.**  
  Mitigation: provide dataset choice where practical, front/back tuning, stereo fallback and accessibility controls.

- **Risk: Existing Bus/effect implementation is not channel-layout aware.**  
  Mitigation: formalize buffer layouts and effect capabilities before expanding DSP.

- **Risk: Render or physics geometry is too detailed or semantically wrong for acoustics.**  
  Mitigation: dedicated simplified acoustic geometry with optional import from existing meshes.

- **Risk: Dolby scope is mistaken for general spatial audio architecture.**  
  Mitigation: keep Atmos as one platform adapter and use neutral terminology throughout schemas.

- **Risk: Native dependencies compromise portability or safe-code policy.**  
  Mitigation: isolate native backends, review licenses/build support, retain Null/Memory implementations and avoid exposing foreign types.

- **Risk: Advanced authoring overwhelms small projects.**  
  Mitigation: strong defaults, automatic Room/Zone suggestions, quality presets and optional advanced components.

## Out of Scope

- Implementing a proprietary Dolby Atmos encoder.
- Generating Dolby Atmos cinema, music or streaming-service master files.
- Claiming Dolby certification before completing the applicable commercial process.
- Full finite-element, boundary-element or real-time wave-equation simulation.
- Guaranteed physically exact diffraction and interference.
- Personalized HRTF measurement or ear scanning.
- Automatic conversion of arbitrary stereo music into isolated spatial objects.
- Shipping external middleware authoring tools such as Wwise or FMOD as part of Varg.
- Voice chat, echo cancellation, speech recognition and network audio transport.
- Procedural sound synthesis redesign, except integration with the new mixer.
- Multiple simultaneous Listeners in the first production release.
- Mobile, console and Web platform parity in the first PCM milestone.
- Advanced research items listed in Phase 6 without a separate approval.

## Dependencies

- Stable runtime window and device lifecycle.
- Asset import and streaming infrastructure.
- ECS transform propagation and lifecycle hooks.
- Physics or dedicated spatial query support for basic occlusion.
- Editor component schema and Inspector extensibility.
- Platform-specific testing environments for native output and spatial APIs.
- Legal review for codecs, datasets, third-party DSP and Dolby branding.

## Open Questions

These questions do not block this PRD but must be resolved at the indicated phase:

- Which cross-platform PCM device library best satisfies safe-code, latency, maintenance and platform requirements?
- Which decoder stack will provide WAV and OGG/Vorbis while keeping dependencies and licenses acceptable?
- Which HRTF dataset and convolution implementation will be used initially?
- Should advanced acoustic geometry live in scene files, imported assets or a generated cache?
- What are the initial target platforms beyond Windows and Linux desktop?
- What latency, CPU and memory budgets define low-, medium- and high-tier reference hardware?
- Which exact Windows/Xbox release paths require direct Dolby commercial engagement?
- Should external middleware integration remain an adapter or become a supported alternative backend?

## Further Notes

- Object audio is an authoring and runtime abstraction, not a promise that every logical source receives a dedicated hardware or Atmos object.
- Buses remain useful for mix organization even when content is object-based; removing fixed output channels does not remove category routing.
- The strongest near-term user benefit comes from reliable PCM output, HRTF binaural rendering and practical occlusion—not from deep physical simulation.
- Environmental acoustics should produce perceptually stable results under strict budgets rather than maximize physical completeness.
- The architecture intentionally allows a future third-party or proprietary acoustic solver without changing scene semantics or gameplay APIs.
- Follow-up implementation issues should be cut as vertical slices that each produce an audible or inspectable result, rather than by creating every low-level subsystem in isolation.
