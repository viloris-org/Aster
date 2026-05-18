# Task 00: Project Decisions And Scope

## Goal

Establish the independent Rust engine project scope before implementation starts. The project references Infernux only for capability boundaries and workflow lessons; it must not inherit Infernux code, module structure, Python production layer, resource formats, naming, or release flow.

## Required Decisions

- Project name, package name, crate prefix, repository strategy, and version strategy.
- Target users and first usable product shape.
- License and third-party dependency policy.
- First supported platforms: Windows x64, macOS Apple Silicon, Linux x64; macOS Intel as best effort.
- First graphics backend: recommended default is Vulkan via `ash`.
- macOS backend strategy: MoltenVK for first usable release, native Metal as later evaluation.
- Physics backend strategy: benchmark Rapier and Jolt FFI before final selection.
- Editor UI direction: egui, imgui-rs, custom UI, or ImGui transition.
- Script backend policy: recommended first release is Rust-only; Python/PyO3 as optional P2 target.
- Infernux importer decision: optional, not core; assign P2 or P3 if required.
- Agent tools strategy: design in P0, read-only tools in P1, writes/transactions/custom tools in P2.
- AI write policy: read-only by default, explicit project configuration and permission gates for writes.
- Agent isolation policy: default to sandboxed readonly mode; file edits use isolated worktrees unless direct writes are explicitly enabled.
- Agent command policy: external commands require allowlisted patterns, sandbox limits, and audit logging.

## Deliverables

- Project naming and versioning note.
- Infernux reference capability matrix.
- Competitive/reference project research note.
- Target platform matrix.
- Initial feature/profile list.
- Initial performance and package-size targets.
- Third-party dependency and license inventory.

## Acceptance

- Clear statement of what is referenced from Infernux and what is redesigned.
- Clear first-release boundary and deferred scope.
- Written confirmation that the project is independent and not a compatibility fork.
