# Task 02: Scene, ECS, And Project Model

## Goal

Implement the Rust-native scene, ECS, transform hierarchy, object lifecycle, and project data model. The system must be script-optional and use stable handles rather than exposed memory addresses.

## Requirements

- `Scene`, `GameObject`, `Transform`, and native `Component` trait or equivalent.
- Parent/child hierarchy, sibling index, root object list.
- Object name, tag, layer, main camera, and game camera queries.
- Scene structure versioning.
- Stable object IDs for one runtime session.
- Deferred destroy processed at frame-safe points.
- Lifecycle ordering for `Start`, `Update`, `FixedUpdate`, `LateUpdate`, and `EditorUpdate`.
- Play mode enter/exit must not pollute edit-time scene state.
- Object clone, prefab instantiate, and serialization support.
- Script component proxy and pending component recovery if a script backend is enabled.
- Transform dirty propagation.

## Data Formats

Define independent formats for:

- Project manifest.
- Scene files.
- Prefab files.
- Editor preferences.
- Build configuration.

Each format must include:

- Explicit version field.
- Schema evolution policy.
- Forward-compatible read strategy.
- Failure diagnostics.
- Optional migration framework.

## Deliverables

- Scene/ECS implementation.
- Atomic component storage.
- Project manifest and scene/prefab schemas.
- Rust-native scene API.
- Example project and example scene.
- Optional script component proxy design.

## Acceptance

- Scene, component, serialization, and undo-related tests pass.
- A new scene can be saved, loaded, and saved again.
- Disabling script backend does not break scene tests.
- Play mode preserves edit-time data.

