# Task 09: Release And Documentation

## Goal

Prepare the first cross-platform native release and keep the project documentation complete enough for users, contributors, and automation tools.

## Release Deliverables

- Windows, macOS, and Linux release packages.
- Native CLI.
- Native editor application.
- Game runtime package.
- Optional external project importer.
- Optional Python script package if Python scripting is enabled.
- Optional agent-tools profile package if AI/Agent target is enabled.
- Example project that can start, edit, save, and run on the three desktop platforms.

## Required Documentation

- Rust architecture overview.
- Script API documentation.
- File format documentation.
- Build guide.
- Packaging guide.
- Performance baseline report.
- Development status board.
- Reference project capability matrix.
- AI/Agent tool protocol.
- Agent permission, transaction, and audit model.
- Agent sandbox and worktree isolation model.
- Feature/profile trimming guide.
- Platform support matrix.
- Atomic crate boundary documentation.

## Suggested Documents

- `doc/rust-engine-new-project-requirements.md`
- `doc/rust-reference-capability-matrix.md`
- `doc/rust-roadmap.md`
- `doc/rust-performance-baseline.md`
- `doc/rust-build-and-packaging.md`
- `doc/rust-platform-support-matrix.md`
- `doc/rust-feature-profiles.md`
- `doc/rust-agent-tools.md`
- `doc/rust-agent-security-model.md`
- `doc/rust-agent-sandbox-and-worktree.md`

## Development Standards

- Run `cargo fmt`.
- Run `cargo clippy`.
- Avoid panic/unwrap abuse.
- Localize every `unsafe` block and document its invariants.
- Keep FFI types in `*-sys` or `ffi` modules.
- Do not expose internal mutable Rust references over script boundaries.
- Long-lived resources need explicit release paths.
- Logs include module, context, and error cause.

## Acceptance

- CI passes on the supported platform matrix.
- Windows, macOS, and Linux packages install and start.
- Example project works on all supported desktop platforms.
- Documentation covers architecture, build, packaging, data formats, platform support, feature trimming, agent security, sandboxing, and worktree isolation.
