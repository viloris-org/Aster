# Task 08: Testing, Performance, And Acceptance

## Goal

Establish unit, integration, performance, lightweight, atomicity, and agent acceptance coverage for the first publishable release.

## Unit Test Coverage

- Handle and registry lifecycle.
- Scene hierarchy operations.
- Transform dirty propagation.
- Asset path and GUID mapping.
- RenderGraph resource declarations and topological sort.
- Physics layer masks.
- Script callback error wrapping if script backend is enabled.
- Agent tool schema, permissions, sandbox policy, worktree lifecycle, transaction rollback, and trace recording.

## Integration Coverage

- Rust API creates, saves, and loads scenes.
- Rust API adds built-in components and updates runtime.
- Rust API drives RenderGraph.
- Physics backend emits callbacks to runtime event bus.
- Logs appear in editor Console.
- Play mode enter/exit preserves objects, components, and serialized fields.
- Script-to-runtime tests if script backend is enabled.
- Agent bridge read-only tools, sandbox rejection, worktree diff/apply/discard, write tools, rollback, and permission denial if agent tools are enabled.

## Performance Baselines

Track repeatable baselines for:

- Empty scene startup.
- Editor first frame.
- 1,000 and 10,000 GameObject update.
- 1,000 and 10,000 mesh draw CPU frame time.
- Scene View plus Game View dual render target.
- Resource import time.
- Shader hot reload time.
- Script API overhead if enabled.
- Physics 1,024 cubes benchmark.
- Minimal runtime startup and memory.
- Editor and game runtime package size.
- Cold resource scan.
- Multi-thread resource import throughput.
- Agent catalog generation.
- Agent main-thread round trip.
- Agent sandbox command overhead.
- Agent worktree create/diff/apply time.

## Acceptance Rules

- First usable version must establish repeatable baselines; exact parity with Infernux is not required.
- Future key paths must not regress by more than 10% without justification.
- High-frequency script APIs must not suffer order-of-magnitude regressions.
- `runtime-min` cold startup must be clearly smaller than full editor startup.
- Per-frame heap allocations on hot paths must be observable and have reduction targets.

## Lightweight Acceptance

- `runtime-min` builds without editor, Python, physics, audio, or importers.
- `runtime-game` can include only selected render backend and resource formats.
- Disabled features keep their dependencies out of final dependency graph.
- Packages include only profile-required libraries and assets.
- CLI emits dependency and feature trimming reports.
- Disabling `agent-tools` removes MCP/HTTP/sandbox/worktree/trace/tool registry dependencies.

## Atomicity Acceptance

- Each crate has README or module documentation.
- Each crate has tests.
- Core runtime tests pass with editor disabled.
- Rust-native API tests pass with Python disabled.
- Physics backend replacement does not affect scene/assets/render compilation.
- Render backend replacement does not affect ECS/resource manifest/script abstraction compilation.
- Editor and runtime core tests pass with agent tools disabled.

## Agent Isolation Acceptance

- Read-only mode cannot write files, mutate runtime state, run commands, or access network.
- File edits default to an isolated worktree or copy-on-write workspace.
- Worktree status, diff, test result, merge/apply, discard, and conflict reporting work.
- Direct writes are disabled by default and require explicit project policy plus confirmation.
- Sandbox denies filesystem access outside allowed roots.
- Sandbox denies network and process execution unless a tool declares the capability and policy grants it.
- Command execution uses allowlisted command patterns and records stdout/stderr summaries.
- Environment variables are filtered and secrets are not exposed by default.
- Trace links each write to a transaction ID, worktree ID, changed files, and rollback or merge decision.

## Release Acceptance

- Rust-native runtime starts.
- Default project can be created, opened, saved, and reopened.
- Scene View and Game View render correctly.
- Scene/system lifecycle works from Rust APIs.
- Built-in components can be added, edited, and serialized.
- RenderGraph can be driven from Rust APIs.
- Basic physics scene runs.
- Resource import, material, texture, and shader hot reload paths work.
- Windows, macOS, and Linux native packages build.
- `runtime-min`, `runtime-game`, `editor`, and `agent-tools` profiles build.
