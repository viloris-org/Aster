# Varg Critical Game-Making Requirements

Updated: 2026-05-19

## Goal

Move the current Varg project from an engine-module skeleton to a state where contributors can create, run, and debug a small playable game. This document only captures the items that most affect game creation in the current project, ordered by how blocking they are.

## Current Assessment

The project already has fairly clear crate boundaries, scene/Prefab data formats, ECS lifecycle support, an asset registry, rendering abstractions, an editor UI shell, physics/audio abstractions, and CLI smoke paths. The core runtime flow is still mostly placeholder or headless:

- `runtime-min` only ticks the headless renderer. It has no real window, input, asset loading, scripting, physics, audio, or integrated game loop.
- `engine-render` has RenderDevice/RenderGraph abstractions, but defaults to `HeadlessRenderDevice` and cannot render a scene visibly.
- The Tauri editor already has Hub/Shell UI, but Scene View/Game View, Hierarchy, and Project still need to bind to real project and scene capabilities.
- `engine-assets` has database, manifest, import queue, and CPU/GPU cache concepts, but lacks real importers, file watching, runtime loading, and closed reference loops for rendering/audio/scenes.
- `engine-physics` and `engine-audio` currently provide complete traits plus null backends. The interface direction is sound, but game functionality is not usable yet.
- The example project only has manifest, scene, prefab, preferences, and build configuration. It has no playable assets, script entry point, or build artifact.

## P0: Required Before Varg Can Make Games

### 1. Runnable Game Runtime

**Problem**

`runtime-min::RuntimeServices::tick` currently only runs the render graph and frame counter. Scene lifecycle, input, fixed timestep, asset loading, physics, audio, and window events have not been composed into a game loop.

**Requirements**

- Provide a `runtime-game` runner that can load project configuration, load the default scene, and enter a continuous frame loop.
- Define a clear per-frame order: input collection -> fixed update accumulation -> physics fixed update -> scene fixed lifecycle -> scene update/late update -> audio update -> render submit -> deferred destroy.
- Support pause, single-step, quit, window resize, and basic error reporting.
- Add an `varg` command similar to `run <project>` that can directly run `examples/project`.

**Acceptance**

- `cargo run -p varg -- run examples/project` opens a window and continuously runs the default scene.
- Components/scripts on the Player object receive start/update/fixed_update.
- Closing the window or pressing Escape exits reliably.

### 2. Real Rendering Path and Game View

**Problem**

The rendering layer is currently mostly abstraction plus headless/stub behavior. There is no entity extraction from Scene to visible frames, no camera, no mesh/material/texture binding, no swapchain presentation, and no editor viewport texture.

**Requirements**

- Choose an initial backend. Prefer `wgpu`, because the Tauri editor viewport readback path already uses it.
- Define minimal rendering components: Camera, MeshRenderer, Light, MaterialRef.
- Connect the submission flow from Scene -> RenderWorld/RenderQueue -> RenderDevice.
- Make Game View and Scene View use real offscreen targets instead of placeholders.
- Provide a built-in debug material, basic mesh, and default shader so test objects can render even without assets.

**Acceptance**

- Opening the sample scene shows a geometric object from the camera view.
- Scene View and Game View both show real rendering output.
- Resize does not crash, and the deferred resource-destruction queue can be verified.

### 3. Input System and Player Control

**Problem**

`engine-platform::input` only has a small set of event enums. It has no input state, axis mapping, mouse buttons, wheel, gamepad support, or runtime update integration.

**Requirements**

- Create `InputState`: pressed/released/down, mouse delta, wheel, and cursor position.
- Support action mappings, such as `MoveForward = W/Up/GamepadLeftY`.
- Convert winit events to engine input events and reset transient state every frame.
- Runtime and scripts/components can query input.

**Acceptance**

- The sample Player can move with WASD or arrow keys.
- Input state updates correctly at frame boundaries, and press/release only trigger for one frame.

### 4. Scene Component Serialization and Inspector Editing

**Problem**

Scene files currently save GameObject metadata, Transform, and script proxies, but regular components, rendering components, physics components, and audio components do not have a unified serialization/deserialization mechanism. The editor Inspector also does not provide real property editing.

**Requirements**

- Define a component schema/registry: component type ID, field metadata, default values, serialization format, and migration policy.
- Scene/Prefab files support component lists and can instantiate them into ECS.
- Inspector displays and edits Transform, Camera, MeshRenderer, Rigidbody, Collider, AudioSource, and Script based on schema.
- Play Mode uses an edit-state copy and does not contaminate the edit state after exit.

**Acceptance**

- MeshRenderer/Camera/Rigidbody declared in a JSON scene are restored at runtime.
- Inspector changes to Transform are reflected immediately in Scene View.
- Prefab instantiation preserves component data.

### 5. Asset Import, Loading, and Hot-Reload Loop

**Problem**

The asset layer data structures already exist, but the project lacks real importers and runtime loading flow. Game creation requires images, models, materials, audio, and shaders to become usable resources from disk.

**Requirements**

- Minimal support: PNG/JPEG texture, glTF model, material JSON/TOML, WGSL or GLSL shader, and WAV/OGG audio.
- Project scanning generates and maintains `.meta` files and the manifest.
- ImportQueue actually reads files, produces CPU resources, and queues GPU uploads.
- The editor Project panel shows the asset tree, import state, error diagnostics, and thumbnails.
- File changes mark assets stale and trigger reimport/reupload.

**Acceptance**

- Dropping an image into project assets makes it visible and previewable in the Project panel.
- After a material references a texture, the sample mesh renders with that material.
- Changing the image file refreshes without restarting.

## P1: Determines Whether Games Are Usable and Debuggable

### 6. Physics Backend and ECS Synchronization

**Problem**

Physics already has traits, Collider/Rigidbody descriptions, and a null backend, but it cannot simulate, collide, or query.

**Requirements**

- Integrate a real backend. Rapier is the recommended first implementation.
- Establish create, destroy, sync, and event-dispatch flows between Rigidbody/Collider components and PhysicsWorld.
- Support raycast, overlap, trigger enter/exit, and collision enter/exit.

**Acceptance**

- Player can move on the ground and be blocked by colliders.
- Trigger regions can emit events.

### 7. Scripting or Game-Logic Extension

**Problem**

Scene already has a Varg-first `ScriptComponent`, but scripting capabilities are still thin. Relying only on native Rust components creates a high barrier for users and does not fit editor workflows well.

**Requirements**

- Deepen the Varg scripting approach without exposing Python, Rhai, or other backends as user-selectable options.
- Script components can receive lifecycle events, input, Transform access, spawn/destroy, resource access, and physics queries.
- Script errors appear in Console and can point to file/line locations.

**Acceptance**

- The sample `player_controller` can drive Player.
- Script errors do not crash the editor, and Console shows diagnostics.

### 8. Editor Project Opening and Scene Saving

**Problem**

Hub and Shell already have state/UI, but `open` currently only shows Hub. The LaunchEditor action does not really switch to or load a project, and Project/Hierarchy/Inspector are mostly empty states.

**Requirements**

- Hub enters the Editor screen after creating or opening a project.
- EditorShell owns the current ProjectContext, Scene, and AssetDatabase.
- Hierarchy binds to the Scene object tree; Project binds to assets; Console binds to diagnostics; Toolbar Play/Pause/Stop calls Scene play mode.
- Save/save-as scenes and save editor preferences.

**Acceptance**

- Opening `examples/project` from Hub shows Main Camera and Player in Hierarchy.
- Renaming an object or editing Transform persists after saving and reopening.

### 9. Build and Package a Minimal Game

**Problem**

The CLI has smoke/profiles and an `xtask` entry point, but there is no project-oriented build/package flow yet.

**Requirements**

- Upgrade `build.runtime-min.toml` into an executable build config.
- Produce a target directory containing runtime binary, assets manifest, import cache, and default scene.
- Support debug/release, target platform, resource copying, and basic version information.

**Acceptance**

- One command can package `examples/project` into a runnable directory.
- The packaged game starts successfully without relying on source-tree paths.

## P2: Improves Creation Efficiency and Stability

### 10. Debugging and Diagnostics

- Frame time, draw call, resource count, entity count, and physics step time.
- Console supports source, filtering, jump-to-source, clear, and copy.
- Render/asset/script/physics errors all flow into diagnostics.

### 11. Editor Interaction

- Scene picking, outlines, and transform gizmo are connected to the real scene and renderer.
- Undo/Redo command stack.
- Prefab create/apply/revert.
- Multi-select, duplicate, delete, and drag assets into the scene.

### 12. Automated Verification

- Runtime smoke expands from headless to windowless scene simulation.
- Scene save/load golden tests cover components.
- Importer fixture tests cover texture/material/model/audio.
- Editor state tests cover open project, save scene, and play mode.

## Recommended Milestones

### M1: Visible, Controllable, Exitable

- Continuous `runtime-game` frame loop.
- winit window and input state.
- A real rendering backend can draw a debug mesh.
- CLI `run examples/project`.

### M2: Editable, Saveable, Reopenable

- Editor opens projects.
- Hierarchy/Inspector/Project bind to real data.
- Scene component serialization.
- Save scenes and preferences.

### M3: Import Assets and Build a Small Demo

- texture/material/model import.
- MeshRenderer/Camera/Light.
- Project panel asset tree and thumbnails.
- Sample scene shows a textured mesh.

### M4: Gameplay Foundations

- Input action mappings.
- Scripting or game-logic backend.
- Physics backend and collision events.
- Sample Player controller.

### M5: Distributable

- Project build/package.
- Asset manifest loads from the package.
- Basic performance and diagnostics panel.

## Priority Conclusion

The biggest blocker to making games is not one missing module, but the lack of an end-to-end loop. Focus first on M1: `runtime-game + window input + real rendering + CLI run`. Once that loop works, the editor, assets, physics, and scripting can all iterate around the same verifiable path instead of adding more abstraction without playable results.
