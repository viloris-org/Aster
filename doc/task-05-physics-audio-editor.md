# Task 05: Physics, Audio, And Editor Native Services

## Goal

Add optional physics, audio, and native editor service capabilities while preserving runtime feature trimming.

## Physics Requirements

- Pluggable physics backend abstraction.
- Rigidbody.
- Box, sphere, capsule, and mesh colliders.
- Friction, bounciness, trigger state.
- Layer matrix and query mask.
- FixedUpdate synchronization.
- Contact callbacks.
- Raycast, overlap, and sweep queries.

## Physics Backend Work

- Benchmark Rapier.
- Benchmark Jolt through `cxx`, `bindgen`, or C ABI.
- Decide first release backend after measuring behavior, performance, packaging, and maintenance cost.
- Ensure backend replacement does not break scene, resources, or renderer crates.

## Audio Requirements

- AudioClip loading.
- AudioSource play, pause, stop, loop, volume.
- AudioListener.
- Scene component lifecycle integration.
- Backend isolated in an audio backend crate when FFI or platform-specific code is needed.

## Editor Native Requirements

- Editor UI frame lifecycle.
- Panel registration system.
- Dock tab selection.
- Scene View and Game View texture display.
- Inspector, Hierarchy, Project, and Console native services.
- Picking, Gizmo operation, outline highlight.
- Resource, material, and mesh previews.

## Deliverables

- Physics abstraction and selected first backend.
- Audio module and initial backend.
- Editor core services.
- Editor UI integration.
- Native editor helpers for preview, picking, Gizmo, and Console.

## Acceptance

- Physics, audio, and editor panel tests pass.
- Resource/material/mesh previews are usable.
- Disabling `editor` removes editor dependencies from `runtime-game`.
- Disabling physics or audio keeps those dependencies out of trimmed builds.

