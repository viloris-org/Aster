# Aster

[![CI](https://github.com/viloris-org/Aster/actions/workflows/core.yml/badge.svg)](https://github.com/viloris-org/Aster/actions/workflows/core.yml)
[![Nightly](https://github.com/viloris-org/Aster/actions/workflows/nightly.yml/badge.svg)](https://github.com/viloris-org/Aster/actions/workflows/nightly.yml)
[![License: MPL-2.0](https://img.shields.io/badge/License-MPL%202.0-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-1.78+-orange.svg)

English | [简体中文](README.zh-CN.md) | [日本語](README.ja.md)

Aster is a Rust game engine with a native editor — make scenes, tweak physics, write
scripts, and see results in real time.

![Aster Editor](docs/screenshots/editor.png)

> **Screenshot placeholder** — replace `docs/screenshots/editor.png` with an actual
> editor screenshot once the UI stabilises.

## Getting Started

```sh
git clone https://github.com/viloris-org/Aster
cd Aster

# Launch the editor
cd editor
bun install
bun tauri dev
```

> **Prerequisites:** [Rust ≥ 1.78](https://rustup.rs/), [Bun ≥ 1.0](https://bun.sh/),
> [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/).
> Linux users: `sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev
> libayatana-appindicator3-dev librsvg2-dev`

## Features

- **Scene editor** — place objects, tweak transforms, add components, all through a
  visual interface. No hand-editing JSON.
- **Live play mode** — hit Play, see physics and scripts run, hit Stop with zero
  cleanup. Your edit scene is never touched.
- **AI-assisted editing** — describe what you want in natural language; the agent
  plans and executes scene changes under a sandboxed review workflow.
- **Asset pipeline** — drop glTF/PNG into the project panel. File watcher triggers
  import, hot reload pushes updates live.
- **Pluggable rendering** — swap backends without touching engine code. Ships with
  WGPU; Vulkan in progress.
- **Headless runtime** — the same engine runs servers, CI tests, or automated
  builds. No window required.
- **No unsafe code** — every crate uses `#![forbid(unsafe_code)]`.

## Project Structure

```
Aster/
├── editor/                  # Tauri desktop app (React + Rust)
├── crates/
│   ├── engine-editor/       # Editor workflow, services, agent tooling
│   ├── engine-editor-ui/    # egui panels, widgets, viewport rendering
│   ├── engine-ecs/          # Scene, entity, transform, world
│   ├── engine-assets/       # Database, importers, hot reload
│   ├── engine-render/       # Render graph, device trait
│   ├── engine-render-wgpu/  # WGPU backend
│   ├── engine-render-vulkan/# Vulkan backend (WIP)
│   ├── engine-physics/      # Physics (rapier3d)
│   ├── engine-audio/        # Audio pipeline
│   ├── engine-core/         # IDs, errors, math, config
│   ├── engine-platform/     # Window, input, filesystem
│   ├── engine-script-rhai/  # Rhai scripting
│   ├── engine-animation/    # Animation system
│   ├── engine-ai/           # AI planner & system prompts
│   ├── engine-agent-cluster/# Agent orchestration
│   ├── runtime-min/         # Composition root
│   └── …                    # i18n, shader, policy, skeleton, etc.
├── xtask/                   # Build & automation tasks
├── examples/                # Sample project & scenes
└── docs/                    # Design notes
```

## Editing a Scene

1. Launch the editor → **Hub** screen
2. Create or open a project
3. **Hierarchy** panel lists every object in the scene
4. **Inspector** shows the selected object's transform and components
5. **Scene View** renders the 3D viewport — orbit, pan, zoom
6. Click **Play** to run physics and scripts in **Game View**
7. Add components (Camera, Light, MeshRenderer, Rigidbody, Collider, …) or write a
   Rhai script

## Build Profiles

Profiles select which subsystems are linked at compile time:

| Profile | What you get |
|---|---|
| `editor` | Full editor with egui panels, wgpu viewports, agent tools |
| `runtime-min` | Headless — CI smoke tests, servers, automated builds |
| `runtime-game` | Headless + windowing |
| `dev-full` | Everything: editor, physics, audio, script, agent, render |

```sh
cargo build -p runtime-min --no-default-features --features editor
cargo build -p runtime-min --no-default-features --features runtime-min
```

## Building the Editor

```sh
cd editor
bun install

# Development (hot-reload frontend + Rust backend)
bun tauri dev

# Distribution bundle
bun tauri build
# → editor/src-tauri/target/release/bundle/
```

## Testing

```sh
# Full engine test suite
cargo test --workspace

# Headless runtime only (fast)
cargo test -p runtime-min --no-default-features --features runtime-min

# Editor services
cargo test -p engine-editor --no-default-features --features agent-tools

# WGPU backend
cargo test -p engine-render-wgpu
```

## License

Mozilla Public License 2.0. See [LICENSE](LICENSE).
