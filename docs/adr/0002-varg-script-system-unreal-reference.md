# ADR 0002: Use Unreal Engine's scripting architecture as a reference boundary

Date: 2026-06-29

## Status

Proposed

## Context

Varg has two script-related surfaces today:

- `engine-script-varg` parses `.varg` logic files, extracts `@export` properties, compiles lifecycle hooks, and runs them through the MVP interpreter.
- `engine-script-declarative` contains JSON/YAML-oriented behavior, scene, UI, systems, asset, and project schemas intended to be easier for AI agents to generate reliably.

Runtime execution in `runtime-min` is component driven. `ScriptComponent` stores a script source path, exported property overrides, and serialized runtime state. The runtime indexes script invocations from scene objects and `ComponentData::Script`, resolves and caches compiled scripts, then dispatches lifecycle hooks such as `start`, `fixed_update`, `update`, and `late_update`.

This is enough for playable prototypes, but it leaves several system boundaries under-specified:

- how editor-facing script properties become stable inspector metadata;
- how script lifecycle events map onto entity/component ownership;
- how scripts should call engine functionality without depending on internal ECS details;
- how visual authoring, text authoring, and AI-generated behavior should converge;
- how diagnostics, hot reload, and serialized state survive script changes.

Unreal Engine is a useful reference point because its gameplay scripting stack is mature, but Varg should borrow the architecture lessons rather than clone Blueprint, UObject, Kismet, or the C++ macro/reflection implementation.

## Decision

Use Unreal Engine's scripting architecture as a reference boundary for Varg, with these translated concepts:

| Unreal concept | Varg equivalent |
| --- | --- |
| Reflected classes, functions, and properties | Engine-owned script metadata registry generated from Rust component/API descriptors and `.varg` declarations |
| Blueprint variables exposed to the editor | `@export var` properties with type, default, category, range, resource kind, and validation metadata |
| Actor/component lifecycle events | Explicit script hooks on entity-attached `ScriptComponent` instances |
| Blueprint callable function libraries | Stable engine API modules such as `Input`, `Audio`, `render`, `scene`, and `entity` |
| Blueprint assets compiling to generated classes | `.varg` script assets compiling to a versioned script IR or bytecode package |
| Simple Construction Script/component templates | `.vscene` prefab/entity composition plus script component defaults |
| Kismet/graph nodes | Future visual script nodes backed by the same engine API registry as text `.varg` |
| Gameplay Tags and categorized metadata | Typed tags, categories, and resource references in scene and script metadata |

The first-class Varg scripting model remains text-first `.varg`. Visual scripting can be added later only if it compiles to the same script IR and uses the same reflected API registry.

Do not copy these Unreal details into Varg:

- C++ macro-style reflection as the user-facing model.
- UObject inheritance as the core gameplay abstraction.
- Blueprint graph storage as the primary source format.
- Global mutable engine state hidden behind script calls.
- A large standard node library before the script API surface is stable.

## Consequences

Positive:

- Script metadata becomes a reusable contract across parser diagnostics, inspector UI, AI tooling, hot reload, and future visual scripting.
- `@export` can grow from parsed declarations into a real reflected property system without changing the public authoring style.
- Runtime lifecycle dispatch stays explicit and component-based, matching Varg's ECS internals while remaining familiar to users coming from actor/component engines.
- Engine APIs become curated capabilities instead of ad hoc interpreter intrinsics.
- Visual scripting, if added, will not fork the gameplay model.

Negative:

- A metadata registry adds up-front design work before more script features can be added cleanly.
- Existing MVP interpreter behavior may need migration to a versioned IR boundary.
- Some Unreal patterns are too heavyweight for Varg and must be actively rejected during implementation.

## Implementation plan

1. Define a script metadata model in `engine-script-varg`:
   - script name and source identity;
   - exported properties with type, default value, category, optional range, resource kind, and doc text;
   - lifecycle hooks with stable names and signatures;
   - engine API calls used by the script, when cheaply discoverable.
2. Move `VargExport` toward this richer metadata shape while preserving current `@export var` syntax.
3. Add a registry of engine script APIs. Start with the APIs already used by scripts: `Input`, `Audio`, `render`, `scene`, `entity`, transforms, UI drawing, spawn, and destroy.
4. Make runtime lifecycle dispatch consume compiled script metadata rather than blindly queueing every script for every hook. If a script has no `fixed_update`, it should not appear in the fixed-update invocation list.
5. Version the compiled script representation so hot reload and serialized script state can detect incompatible changes.
6. Teach the editor inspector to use exported metadata for script component fields instead of treating `exported_values` as an untyped map.
7. Keep declarative behavior trees as the AI-friendly behavior surface, but allow them to share the same action/condition registry as `.varg` script APIs.
8. Defer visual scripting until the metadata registry, lifecycle model, diagnostics, and IR are stable.

## Rejected alternatives

### Clone Blueprint as the primary scripting model

Rejected because Varg's near-term needs are text authoring, AI patchability, deterministic source files, and a smaller runtime. Blueprint is useful as a reference for capability boundaries, not as a storage or UI target.

### Keep growing interpreter intrinsics ad hoc

Rejected because it makes editor UI, AI tooling, diagnostics, and future visual authoring depend on undocumented parser behavior.

### Expose ECS directly to scripts

Rejected because ECS is an internal runtime representation. Scripts should work through stable entity/component APIs and reflected engine capabilities.
