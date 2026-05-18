# Task 03: Assets And Resources

## Goal

Build the asset database, registry, manifest, dependency graph, import pipeline, and reload behavior needed by runtime and editor workflows.

## Requirements

- GUID-to-path mapping.
- AssetDatabase and AssetRegistry semantics.
- Texture, material, shader, audio, model, skinned model, and animation resource records.
- Resource dependency graph.
- Resource hot reload.
- Built-in and project resource resolution.
- Project panel thumbnail/preview data.
- CPU resource cache and GPU resource cache with separate lifetimes.
- Resource loading tasks separated from GPU upload tasks.
- Stable Rust-native resource handles, with script-specific mappings only in script backend layers.
- Importers must be feature-gated and removable from minimal runtime builds.

## Data Formats

Define independent formats for:

- Resource meta / GUID.
- Material files.
- Shader configuration.
- Import cache.
- Resource manifest.

Required properties:

- Versioned schema.
- Clear diagnostics on failed load/import.
- Migration path for future schema changes.

## Deliverables

- AssetDatabase.
- AssetRegistry.
- Resource manifest.
- GUID and dependency graph implementation.
- Parallel import queue.
- Initial material, texture, shader, model, audio resource types.
- Feature flags for heavy importers such as FBX/assimp.

## Acceptance

- Project resources can be created, imported, cached, reloaded, and resolved by GUID.
- Project and Inspector resource workflows are usable.
- Hot reload works for supported resource types.
- Disabling FBX/assimp importers keeps heavy import dependencies out of `runtime-min`.

