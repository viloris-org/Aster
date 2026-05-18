# Task 07: Build, Packaging, And CI

## Goal

Make Cargo the primary build entry and establish platform CI, feature/profile validation, and native packaging flows.

## Build Requirements

- `cargo build --workspace`
- `cargo test --workspace`
- `cargo run -p engine-cli`
- `cargo xtask build-editor`
- `cargo xtask package --profile runtime-game`
- `cargo xtask package --profile editor`

CMake must not be the long-term primary build entry. If C/C++ third-party code requires CMake, isolate it in `*-sys` or backend crates.

## Profiles

- `runtime-min`: core, ECS, platform, base assets.
- `runtime-game`: runtime-min, render backend, audio, optional physics.
- `editor`: runtime-game, editor, importers, previews, debug tools.
- `agent-tools`: editor service, agent bridge, sandbox policy, worktree manager, transaction, trace, tool metadata.
- `script-python`: Python binding and script runtime bridge.
- `dev-full`: all stable modules.

## Python Packaging

Only required if Python scripting or tools are enabled:

- Editable install or equivalent.
- Wheel includes Rust extension module and native runtime libraries.
- Python package can locate platform-native dependencies.
- Consider `maturin` or `setuptools-rust`.
- Native editor and runtime releases must not depend on Python packaging.

## Native Packaging

- Native CLI package.
- Native editor app package.
- Game runtime package.
- Windows `.zip` / `.msi`.
- macOS `.app` / `.dmg`.
- Linux AppImage / tarball.

## CI Requirements

Matrix:

- Windows x64.
- macOS arm64.
- macOS x64 if resources allow.
- Linux x64.

Required jobs:

- `cargo fmt --check`
- `cargo clippy --workspace`
- `cargo test --workspace`
- Minimal profile build.
- Editor profile build.
- Agent-tools profile build.
- Agent sandbox/worktree smoke tests.
- Python script backend smoke test if `script-python` is enabled.

## Acceptance

- Workspace builds through Cargo.
- Feature/profile trimming is verified in CI.
- Native package commands produce platform-specific artifacts.
- CI covers Windows, macOS, and Linux.
