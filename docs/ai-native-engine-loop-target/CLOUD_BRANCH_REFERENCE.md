# Cloud Branch Reference

Claude must compare cloud branches before starting implementation. The goal is
not to copy another branch. The goal is to understand the available approaches,
borrow good ideas, avoid known mistakes, and build a stronger branch.

## Branches To Inspect

```text
origin/main
origin/takeover/upstream-integrated
origin/fix/render-pipeline-wgpu
```

Use commands like:

```bash
git fetch origin --prune
git log --oneline --decorate origin/main..origin/takeover/upstream-integrated
git log --oneline --decorate origin/main..origin/fix/render-pipeline-wgpu
git diff --stat origin/main..origin/takeover/upstream-integrated
git diff --stat origin/main..origin/fix/render-pipeline-wgpu
git diff origin/main..origin/takeover/upstream-integrated -- editor/src/renderer/pages/QuestPage.tsx editor/src/renderer/pages/EditorPage.tsx editor/src-tauri/src/lib.rs editor/src-tauri/src/quest.rs
git diff origin/main..origin/fix/render-pipeline-wgpu -- crates/engine-render-wgpu crates/engine-render
```

## Current Known Remote Shape

At the time this pack was written:

```text
origin/main
- baseline engine/editor branch
- includes Quest UI improvements, physics work, render visibility/GPU particles,
  and distance activation work

origin/takeover/upstream-integrated
- AI-first editor/takeover showcase direction
- broad frontend/editor/backend/docs changes
- useful as product and UI reference
- risky because it is very wide and may flatten subsystem boundaries

origin/fix/render-pipeline-wgpu
- render pipeline hardening reference
- useful for wgpu/render validation and regression prevention
- narrower than takeover, likely safer to inspect for render fixes
```

Claude must refresh this with live `git fetch` and real diffs before acting.

## How To Use The Reference Branches

Use `origin/takeover/upstream-integrated` to learn:

- what a stronger AI-first workbench might look like;
- how QuestPage/EditorPage can be reframed around AI workflow;
- what UI flows, diagnostics, and editor surfaces were attempted;
- what not to do if the implementation is too broad or too decorative.

Use `origin/fix/render-pipeline-wgpu` to learn:

- render hardening patterns;
- wgpu pipeline failure handling;
- render validation and smoke test targets;
- places where main may still be fragile.

Do not copy large diffs blindly. For each borrowed idea, first answer:

```text
What user problem does this solve?
What files must change in this branch?
What smaller implementation preserves Aster's current architecture?
What test/build evidence will prove it works?
```

## Comparison Target

The final branch should be compared against both:

```text
origin/main
origin/takeover/upstream-integrated
```

The comparison should be written to:

```text
docs/ai-native-engine-loop-comparison.md
```

It should cover:

- Quest/Agent execution reality;
- frontend maintainability and product feel;
- editor surfaces and diagnostics;
- ECS/SceneCommand semantics;
- physics/render/audio validation;
- security/apply/rollback guarantees;
- actual verification results.

