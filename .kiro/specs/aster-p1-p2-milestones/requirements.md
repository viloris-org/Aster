# Requirements Document

## Introduction

This document specifies the P1 and P2 milestone requirements for the Aster game engine. P1 covers the features that determine whether a game is usable and debuggable: physics simulation, game logic scripting, a functional editor project workflow, and a build/package pipeline. P2 covers features that improve production efficiency and stability: diagnostics, editor interaction capabilities, and automated validation.

The requirements are grounded in the existing codebase. Aster already has a `PhysicsBackend` trait with a null backend, `ScriptComponentProxy` in the scene model, `HubState`/`EditorShell` UI shells, `BuildConfiguration` schema, `ConsoleService` with filtering, `GizmoService`/`PickingService`/`OutlineService` stubs, and a `RuntimeServices` game loop. These requirements describe what must be built on top of that foundation.

## Glossary

- **PhysicsWorld**: The `engine_physics::PhysicsWorld` struct that owns a `PhysicsBackend` and the layer collision matrix.
- **PhysicsBackend**: The `engine_physics::PhysicsBackend` trait implemented by real physics libraries (e.g., Rapier) and the existing `NullPhysicsBackend`.
- **RigidbodyComponent**: The `engine_ecs::physics::RigidbodyComponent` ECS component that stores a `RigidbodyDesc` and a live `BodyHandle`.
- **ColliderComponent**: The `engine_ecs::physics::ColliderComponent` ECS component that stores a `ColliderDesc` and a live `ColliderHandle`.
- **PhysicsSync**: The system responsible for creating, updating, and destroying physics bodies/colliders in the `PhysicsWorld` to match the ECS scene state.
- **ScriptBackend**: A pluggable scripting runtime (Rhai selected as the first implementation) that executes game logic scripts attached to scene objects.
- **ScriptComponentProxy**: The `engine_ecs::scene::ScriptComponentProxy` struct that stores the backend name, script path, and serialized state for a script component.
- **ScriptContext**: The API surface exposed to scripts, providing access to lifecycle hooks, input, transform, spawn/destroy, resource handles, and physics queries.
- **EditorShell**: The `engine_editor_ui::EditorShell` struct that owns the panel registry, command registry, selection service, and console service.
- **ProjectContext**: A new struct that `EditorShell` will hold, containing the loaded `ProjectManifest`, `Scene`, and `AssetDatabase` for the currently open project.
- **HubState**: The `engine_editor_ui::HubState` struct managing the project list, installs, and new-project dialog.
- **Hierarchy**: The editor panel that displays the scene object tree bound to the active `Scene`.
- **Inspector**: The editor panel that displays and edits component data for the selected scene object.
- **Project**: The editor panel that displays the asset tree from the `AssetDatabase`.
- **Console**: The editor panel backed by `ConsoleService` that displays filtered diagnostic messages.
- **Toolbar**: The editor top bar with Play/Pause/Stop buttons that control `Scene` play mode.
- **BuildConfig**: The `engine_ecs::schema::BuildConfiguration` TOML file (`build.runtime-min.toml`) that drives the build pipeline.
- **BuildPipeline**: The xtask or CLI subsystem that reads a `BuildConfig`, compiles the runtime binary, copies assets, and writes the output directory.
- **DiagnosticsOverlay**: A runtime/editor panel that displays frame time, draw calls, resource counts, entity counts, and physics step time.
- **GizmoService**: The `engine_editor::native::GizmoService` stub that will be wired to the scene view for transform manipulation.
- **PickingService**: The `engine_editor::native::PickingService` stub that will be wired to scene-view click events.
- **OutlineService**: The `engine_editor::native::OutlineService` stub that will be wired to the selected entity highlight pass.
- **UndoStack**: A new command-stack service that records reversible editor operations.
- **PrefabFile**: The `engine_ecs::schema::PrefabFile` format used for prefab create/apply/revert workflows.
- **RuntimeServices**: The `runtime_min::RuntimeServices` struct that owns the scene, renderer, input, and game loop.
- **ConsoleService**: The `engine_editor::ConsoleService` struct with push/clear/filter operations.
- **ConsoleFilter**: The `engine_editor::ConsoleFilter` struct used to filter console entries by level, source, and message text.
- **ImportQueue**: The `engine_assets::ImportQueue` struct that manages CPU import and GPU upload tasks.
- **AssetDatabase**: The `engine_assets::AssetDatabase` struct that maps GUIDs to paths and stores resource metadata.
- **Rhai**: The Rhai embedded scripting language (https://rhai.rs), selected as the first `ScriptBackend` implementation.

---

## Requirements

### Requirement 6: Physics Backend and ECS Synchronization

**User Story:** As a game developer, I want physics simulation to run in the game loop and stay synchronized with the scene, so that objects collide, rigidbodies move under gravity, and I can query the physics world from scripts and components.

#### Acceptance Criteria

1. WHEN the `runtime-game` feature is enabled, THE `PhysicsWorld` SHALL be initialized with the Rapier backend before the first game frame.
2. WHEN a `RigidbodyComponent` is added to a scene entity, THE `PhysicsSync` SHALL create a corresponding body in the `PhysicsWorld` and store the returned `BodyHandle` in `RigidbodyComponent.handle`.
3. WHEN a `ColliderComponent` is added to a scene entity that also has a `RigidbodyComponent`, THE `PhysicsSync` SHALL attach the collider to the body identified by `RigidbodyComponent.handle` and store the returned `ColliderHandle` in `ColliderComponent.handle`.
4. WHEN a scene entity with a `RigidbodyComponent` is destroyed, THE `PhysicsSync` SHALL call `PhysicsBackend::destroy_body` for the associated `BodyHandle`.
5. WHEN a scene entity with a `ColliderComponent` is destroyed without its parent body, THE `PhysicsSync` SHALL call `PhysicsBackend::remove_collider` for the associated `ColliderHandle`.
6. WHILE the game loop is running, THE `PhysicsWorld` SHALL be stepped once per fixed-timestep tick at a rate of 60 Hz by default.
7. AFTER each `PhysicsWorld` fixed step, THE `PhysicsSync` SHALL read the world-space transform from each live `BodyHandle` and write it back to the corresponding entity's `TransformHierarchy` entry.
8. WHEN `PhysicsBackend::raycast` is called with a valid origin, direction, and max distance, THE `PhysicsWorld` SHALL return the closest `RayHit` or `None` if no body intersects the ray within the given distance.
9. WHEN `PhysicsBackend::overlap_sphere` is called with a valid center and radius, THE `PhysicsWorld` SHALL return all `OverlapResult` entries for bodies whose colliders intersect the sphere.
10. WHEN two non-trigger colliders make first contact, THE `PhysicsBackend` SHALL emit a `ContactEvent` with `entered = true`; WHEN they separate, THE `PhysicsBackend` SHALL emit a `ContactEvent` with `entered = false`.
11. WHEN a collider with `is_trigger = true` overlaps another collider, THE `PhysicsBackend` SHALL emit a trigger-enter event; WHEN the overlap ends, THE `PhysicsBackend` SHALL emit a trigger-exit event.
12. WHEN `PhysicsBackend::drain_contacts` is called, THE `PhysicsWorld` SHALL return all contact and trigger events accumulated since the previous call and clear the internal event buffer.
13. THE `LayerMatrix` SHALL control which layer pairs generate contacts, and THE `PhysicsSync` SHALL apply the matrix when creating colliders so that colliders on non-colliding layer pairs do not generate contact events.
14. IF the Rapier backend fails to initialize, THEN THE `RuntimeServices` SHALL log an error to `ConsoleService` and fall back to `NullPhysicsBackend` without crashing.

---

### Requirement 7: Script and Game Logic Extension

**User Story:** As a game developer, I want to attach Rhai scripts to scene objects and have them receive lifecycle callbacks, read input, manipulate transforms, spawn/destroy objects, and query physics, so that I can implement game logic without recompiling the engine.

#### Acceptance Criteria

1. THE `ScriptBackend` SHALL be Rhai as the first implementation, compiled under the `script-rhai` Cargo feature flag.
2. WHEN a scene entity has a `ScriptComponentProxy` with `backend = "rhai"`, THE `ScriptBackend` SHALL load the script file identified by `ScriptComponentProxy.script` from the project asset root.
3. WHEN the scene runs the `Start` lifecycle stage, THE `ScriptBackend` SHALL call the `on_start` function defined in each loaded Rhai script, if present.
4. WHEN the scene runs the `Update` lifecycle stage, THE `ScriptBackend` SHALL call the `on_update` function defined in each loaded Rhai script, if present.
5. WHEN the scene runs the `FixedUpdate` lifecycle stage, THE `ScriptBackend` SHALL call the `on_fixed_update` function defined in each loaded Rhai script, if present.
6. WHILE a Rhai script is executing, THE `ScriptContext` SHALL expose `input.is_pressed(key)`, `input.axis(name)`, and `input.mouse_delta()` functions that read from the current frame's `InputState`.
7. WHILE a Rhai script is executing, THE `ScriptContext` SHALL expose `transform.get_position()`, `transform.set_position(x, y, z)`, `transform.get_rotation()`, and `transform.set_rotation(x, y, z, w)` functions that read and write the entity's `TransformHierarchy` entry.
8. WHILE a Rhai script is executing, THE `ScriptContext` SHALL expose a `spawn(name)` function that creates a new scene entity and returns its stable ID.
9. WHILE a Rhai script is executing, THE `ScriptContext` SHALL expose a `destroy(id)` function that defers destruction of the entity with the given stable ID.
10. WHILE a Rhai script is executing, THE `ScriptContext` SHALL expose `physics.raycast(ox, oy, oz, dx, dy, dz, max_dist)` that delegates to `PhysicsBackend::raycast` and returns hit distance or `nil`.
11. WHEN a Rhai script raises a runtime error, THE `ScriptBackend` SHALL push a `ConsoleEntry` with `level = Error`, `source.subsystem = "script"`, `source.file` set to the script path, and `source.line` set to the Rhai error line number.
12. IF a Rhai script file is not found at the path stored in `ScriptComponentProxy.script`, THEN THE `ScriptBackend` SHALL push a `ConsoleEntry` with `level = Error` and `source.file` set to the missing path, and SHALL set `ScriptComponentProxy.pending_recovery = true`.
13. WHEN `ScriptComponentProxy.pending_recovery` is `true`, THE `ScriptBackend` SHALL skip lifecycle calls for that proxy without crashing the game loop.
14. THE `ScriptBackend` SHALL expose a `get_resource(path)` function that resolves a `project:/` or `builtin:/` reference through `AssetDatabase` and returns the asset GUID as a string.

---

### Requirement 8: Editor Opens Project and Saves Scene

**User Story:** As a game developer, I want to open a project from the Hub, see my scene objects in the Hierarchy, edit them in the Inspector, and save the scene, so that my work persists across editor sessions.

#### Acceptance Criteria

1. WHEN the user selects a project in `HubState` and clicks Launch, THE `EditorShell` SHALL load the `ProjectManifest` from the project root, construct a `ProjectContext` containing the manifest, a `Scene` loaded from `ProjectManifest.default_scene`, and an `AssetDatabase` scanned from `ProjectManifest.asset_root`.
2. WHEN `ProjectContext` is loaded, THE `Hierarchy` panel SHALL display one row per `GameObject` in the active `Scene`, ordered by parent-child relationship and sibling index.
3. WHEN the user selects a row in the `Hierarchy`, THE `EditorShell` SHALL update `SelectionService` with a `Selection::Entity` containing the selected entity's stable ID string.
4. WHEN `SelectionService` holds a `Selection::Entity`, THE `Inspector` panel SHALL display the `Transform` fields (position, rotation, scale) and all `ComponentData` entries for that entity using the `ComponentSchemaRegistry`.
5. WHEN the user edits a numeric or string field in the `Inspector`, THE `EditorShell` SHALL update the corresponding `ComponentData` in the `Scene` and bump the scene structure version.
6. WHEN `ProjectContext` is loaded, THE `Project` panel SHALL display the asset tree rooted at `ProjectManifest.asset_root`, showing each registered asset's path, kind, and import state from the `AssetDatabase`.
7. WHEN the user clicks Save (Ctrl+S or toolbar), THE `EditorShell` SHALL serialize the active `Scene` to JSON using `Scene::to_json` and write it to the path stored in `ProjectManifest.default_scene`.
8. WHEN the user clicks Save, THE `EditorShell` SHALL also write the current `EditorPreferences` to `editor.preferences.toml` in the project root using `engine_editor::write_preferences_toml`.
9. WHEN the user clicks Play in the `Toolbar`, THE `EditorShell` SHALL call `Scene::enter_play_mode` and update `ShellUiState.playing = true`.
10. WHEN the user clicks Stop in the `Toolbar` while `ShellUiState.playing` is `true`, THE `EditorShell` SHALL call `Scene::exit_play_mode` and update `ShellUiState.playing = false`.
11. WHEN the user clicks Pause in the `Toolbar` while `ShellUiState.playing` is `true`, THE `EditorShell` SHALL set `RuntimeServices.paused = true`.
12. THE `Console` panel SHALL display all entries from `ConsoleService` and SHALL apply the active `ConsoleFilter` before rendering rows.
13. IF loading the `ProjectManifest` fails, THEN THE `EditorShell` SHALL push a `ConsoleEntry` with `level = Error` and the parse error message, and SHALL remain on the Hub screen.
14. IF loading the default scene file fails, THEN THE `EditorShell` SHALL push a `ConsoleEntry` with `level = Error`, create an empty `Scene`, and continue opening the editor with an empty scene.

---

### Requirement 9: Build and Package Minimal Game

**User Story:** As a game developer, I want a single command to compile and package my project into a self-contained output directory, so that I can distribute or test the game without the source tree.

#### Acceptance Criteria

1. THE `BuildPipeline` SHALL read a `BuildConfiguration` from the path specified by the `--config` argument, defaulting to `build.runtime-min.toml` in the project root.
2. WHEN `BuildConfiguration.release` is `false`, THE `BuildPipeline` SHALL invoke `cargo build` with the `--profile dev` flag; WHEN `BuildConfiguration.release` is `true`, THE `BuildPipeline` SHALL invoke `cargo build --release`.
3. WHEN `BuildConfiguration.target` is not `"native"`, THE `BuildPipeline` SHALL pass `--target <value>` to the `cargo build` invocation.
4. AFTER a successful `cargo build`, THE `BuildPipeline` SHALL copy the compiled runtime binary to `<output_dir>/bin/`.
5. THE `BuildPipeline` SHALL scan the project asset root, generate a `ResourceManifestFormat` using `AssetDatabase::manifest`, and write it as `assets_manifest.json` to `<output_dir>/`.
6. THE `BuildPipeline` SHALL copy the import cache file to `<output_dir>/import_cache.json` if it exists in the project root.
7. THE `BuildPipeline` SHALL copy the default scene file identified by `ProjectManifest.default_scene` to `<output_dir>/scenes/`.
8. THE `BuildPipeline` SHALL write a `build_info.json` file to `<output_dir>/` containing the build timestamp, target platform, release flag, and engine version from `Cargo.toml`.
9. WHEN the `engine-cli` `build` subcommand is invoked with a project path, THE `BuildPipeline` SHALL execute steps 1–8 and report progress to stdout.
10. IF `cargo build` exits with a non-zero status, THEN THE `BuildPipeline` SHALL print the captured stderr output and exit with a non-zero status code.
11. WHEN the packaged binary is launched from `<output_dir>` without the source tree present, THE `RuntimeServices` SHALL locate `assets_manifest.json` and the default scene relative to the binary path and start successfully.
12. THE `BuildPipeline` SHALL validate the `BuildConfiguration` using `BuildConfiguration::diagnostics` before invoking `cargo build`, and IF diagnostics are non-empty, THEN THE `BuildPipeline` SHALL print each diagnostic and exit with a non-zero status code.

---

### Requirement 10: Debug and Diagnostics Experience

**User Story:** As a game developer, I want to see frame time, draw calls, resource counts, entity counts, and physics step time in a diagnostics overlay, and I want the Console to let me filter, copy, and jump to source, so that I can identify and fix performance and correctness problems quickly.

#### Acceptance Criteria

1. THE `DiagnosticsOverlay` SHALL display the following metrics updated once per frame: frame time in milliseconds (2 decimal places), draw call count, loaded resource count from `AssetRegistry`, live entity count from `World`, and physics step time in milliseconds.
2. WHEN the frame time exceeds 33.3 ms (below 30 fps), THE `DiagnosticsOverlay` SHALL render the frame time value in a warning color distinct from the normal text color.
3. THE `DiagnosticsOverlay` SHALL be toggled on and off by a keyboard shortcut (F2 by default) without restarting the runtime or editor.
4. WHEN a render error, asset error, script error, or physics error occurs, THE responsible subsystem SHALL push a `ConsoleEntry` to `ConsoleService` with the appropriate `ConsoleLevel` and a `ConsoleSource` identifying the subsystem, file, and line where available.
5. THE `Console` panel SHALL render a filter bar that accepts a minimum level selector (Trace/Debug/Info/Warn/Error) and a free-text search field, and SHALL pass these as a `ConsoleFilter` to `ConsoleService::filtered` before rendering rows.
6. WHEN the user clicks a `ConsoleEntry` row that has a non-`None` `source.file`, THE `Console` panel SHALL emit a `Selection::Asset` event with the file path so that the editor can open or highlight the source file.
7. THE `Console` panel SHALL provide a Clear button that calls `ConsoleService::clear`.
8. THE `Console` panel SHALL provide a Copy button that writes all currently visible (filtered) console entries as plain text to the system clipboard, one entry per line in the format `[LEVEL] [SOURCE] MESSAGE`.
9. WHEN the `Console` panel receives a new entry while scrolled to the bottom, THE `Console` panel SHALL auto-scroll to keep the newest entry visible.
10. THE `DiagnosticsOverlay` SHALL expose its metrics as a Rust struct so that automated tests can assert on metric values without rendering.

---

### Requirement 11: Editor Interaction Capabilities

**User Story:** As a game developer, I want to click objects in the Scene View to select them, use a transform gizmo to move/rotate/scale them, undo and redo my changes, create and apply prefabs, and multi-select/copy/delete/drag assets to the scene, so that I can build scenes efficiently.

#### Acceptance Criteria

1. WHEN the user clicks a pixel in the Scene View, THE `PickingService` SHALL perform a ray-cast from the camera through that pixel and return the `PickResult` containing the entity ID of the closest intersecting object, or `None` if no object is hit.
2. WHEN `PickingService` returns a non-`None` `PickResult`, THE `EditorShell` SHALL update `SelectionService` with the corresponding `Selection::Entity`.
3. WHEN `SelectionService` holds a `Selection::Entity`, THE `OutlineService` SHALL add the entity to the outline render pass so that it is visually highlighted in the Scene View.
4. WHEN `SelectionService` holds a `Selection::Entity`, THE `GizmoService` SHALL render a transform gizmo at the entity's world-space position in the Scene View.
5. WHEN the user drags the translate handle of the `GizmoService` gizmo, THE `GizmoService` SHALL compute the new world-space position and update the entity's `TransformHierarchy` entry.
6. WHEN the user drags the rotate handle of the `GizmoService` gizmo, THE `GizmoService` SHALL compute the new world-space rotation and update the entity's `TransformHierarchy` entry.
7. WHEN the user drags the scale handle of the `GizmoService` gizmo, THE `GizmoService` SHALL compute the new local scale and update the entity's `TransformHierarchy` entry.
8. WHEN the user performs any transform edit via the gizmo or Inspector, THE `UndoStack` SHALL record the operation as a reversible command.
9. WHEN the user invokes Undo (Ctrl+Z), THE `UndoStack` SHALL reverse the most recent command and update the `Scene` and `SelectionService` accordingly.
10. WHEN the user invokes Redo (Ctrl+Y or Ctrl+Shift+Z), THE `UndoStack` SHALL re-apply the most recently undone command.
11. THE `UndoStack` SHALL retain at most 100 commands; WHEN the limit is exceeded, THE `UndoStack` SHALL discard the oldest command.
12. WHEN the user right-clicks a `GameObject` in the `Hierarchy` and selects "Create Prefab", THE `EditorShell` SHALL serialize the object and its children into a `PrefabFile` and write it to the project prefabs directory.
13. WHEN the user right-clicks a prefab asset in the `Project` panel and selects "Apply to Scene", THE `EditorShell` SHALL update all scene instances of that prefab to match the current prefab file content.
14. WHEN the user right-clicks a prefab instance in the `Hierarchy` and selects "Revert", THE `EditorShell` SHALL restore the instance's component data to match the source `PrefabFile`.
15. WHEN the user holds Ctrl and clicks multiple rows in the `Hierarchy`, THE `SelectionService` SHALL accumulate a multi-selection of `Selection::Entity` values.
16. WHEN the user presses Delete with one or more entities selected, THE `EditorShell` SHALL call `Scene::destroy_deferred` for each selected entity and record the operation in the `UndoStack`.
17. WHEN the user presses Ctrl+D with one or more entities selected, THE `EditorShell` SHALL call `Scene::clone_object` for each selected entity and record the operation in the `UndoStack`.
18. WHEN the user drags an asset from the `Project` panel onto the Scene View, THE `EditorShell` SHALL create a new `GameObject` at the drop position with the appropriate component (e.g., `MeshRenderer` for a model asset) and record the operation in the `UndoStack`.

---

### Requirement 12: Automated Validation

**User Story:** As an engine developer, I want automated tests that cover windowless scene simulation, scene save/load round-trips, importer correctness, and editor state transitions, so that regressions are caught before they reach users.

#### Acceptance Criteria

1. THE `runtime-min` test suite SHALL include a windowless scene simulation test that creates a `RuntimeServices` with a headless renderer, populates the scene with at least one `GameObject` carrying a `RigidbodyComponent` and a `ColliderComponent`, runs 60 fixed-timestep ticks, and asserts that the entity's transform has changed from its initial value.
2. THE `engine-ecs` test suite SHALL include a scene save/load golden test that serializes a scene containing one `Camera`, one `MeshRenderer`, one `Rigidbody`, one `Collider`, and one `AudioSource` component to JSON, deserializes it, and asserts that the deserialized scene is equal to the original.
3. THE scene save/load golden test SHALL also assert that serializing the deserialized scene produces byte-identical JSON to the first serialization (round-trip property).
4. THE `engine-assets` test suite SHALL include importer fixture tests for each supported asset type: PNG texture, glTF model, material JSON, WGSL shader, and WAV audio. WHEN a valid fixture file is passed to the corresponding importer, THE importer SHALL produce an `ImportOutcome` with an empty `diagnostics` list.
5. WHEN an invalid or malformed fixture file is passed to an importer, THE importer SHALL produce an `ImportOutcome` with at least one `AssetDiagnostic` entry and SHALL NOT panic.
6. THE `engine-editor-ui` test suite SHALL include an editor state test that constructs an `EditorShell`, calls the open-project flow with the `examples/project` path, and asserts that the `Hierarchy` contains entries for "Main Camera" and "Player".
7. THE `engine-editor-ui` test suite SHALL include an editor state test that opens a project, modifies a `GameObject` name, saves the scene to a temporary path, reloads the scene from that path, and asserts that the modified name is preserved.
8. THE `engine-editor-ui` test suite SHALL include a play-mode test that opens a project, enters play mode, asserts `ShellUiState.playing == true`, exits play mode, and asserts that the edit-time scene is unchanged.
9. THE `runtime-min` test suite SHALL include a smoke test that runs `smoke_runtime_min` and asserts the returned frame index equals 1, confirming the headless path still compiles and executes after P1 changes.
10. FOR ALL valid `SceneFile` values produced by `Scene::to_json`, parsing with `Scene::from_json` and re-serializing SHALL produce JSON equal to the original (round-trip property).
11. THE `engine-physics` test suite SHALL include a property test that creates a `RigidbodyDesc` with arbitrary `linear_damping` and `angular_damping` values in the range [0.0, 10.0], creates a body in the Rapier backend, steps the simulation for 10 ticks, and asserts that the body transform is finite (no NaN or infinity in translation or rotation components).
