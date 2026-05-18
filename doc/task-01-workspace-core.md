# Task 01: Rust Workspace And Atomic Core

## Goal

Create the Rust Cargo workspace and minimal atomic runtime foundation. The minimal runtime must build without editor, Python, model importers, physics, audio, or concrete render backend dependencies.

## Crates

- `engine-core`: logging, errors, config, IDs, handles, time, frame counters, math public types.
- `engine-ecs`: atomic ECS, scene storage base, transform hierarchy base.
- `engine-platform`: window/input/filesystem/dynamic-library/platform capability abstraction.
- `engine-assets`: base asset ID/path/manifest subset.
- `engine-render`: render abstraction only; no concrete backend in `runtime-min`.
- `xtask`: unified development and packaging commands.

## Requirements

- Use Cargo workspace as the primary build entry.
- Feature/profile system must support `runtime-min`, `runtime-game`, `editor`, `agent-tools`, `script-python`, and `dev-full`.
- Core crates must not depend on editor, Python, concrete render backend, or concrete physics backend.
- Platform-specific behavior must be isolated in `engine-platform` or backend crates.
- Runtime errors must use structured errors and map cleanly to CLI, editor, log, and script boundaries.
- No hidden global mutable state; global services must be explicit runtime services.
- All cross-thread callbacks must define ownership and legal execution thread.

## Implementation Notes

- Prefer `thiserror` for errors.
- Prefer `tracing` or `log` as the logging facade.
- Evaluate `slotmap`, `generational-arena`, or a custom generational handle system for stable handles.
- Use `PathBuf` and explicit UTF-8 boundary handling for paths.
- Keep FFI code isolated in `*-sys` or `ffi` modules.

## Deliverables

- Workspace skeleton under `crates/`.
- Minimal core, ECS, platform, assets, and render abstraction crates.
- Feature/profile definitions.
- `runtime-min` build path.
- Rust native smoke test.
- Basic CI job for core build and tests.

## Acceptance

- `cargo test --workspace` covers the core crates.
- `runtime-min` builds with no Python, editor, physics, audio, importers, or render backend.
- Core CI runs on Windows, macOS, and Linux.

