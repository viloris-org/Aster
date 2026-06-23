# Quest Real Execution Spine

This document defines the backend spine that the rest of the AI-native editor
should connect to. If the long goal run becomes unfocused, return here and make
this path more real.

## Product Standard

Quest should behave like a reviewable local PR pipeline:

```text
intent -> workspace -> execution -> diff -> validation -> review -> apply
```

A good Quest run shows:

- what was requested;
- what context was used;
- what files or scene objects changed;
- what validations ran;
- what failed;
- what is safe to apply;
- how to roll back or recover.

## Minimum Real Path

Implement or strengthen the smallest path that can be tested without an
external model:

```text
deterministic runner
-> writes a small controlled change in a safe workspace
-> collects changed files
-> generates diff/review data
-> runs deterministic validation
-> returns a review bundle
```

This runner is allowed to be simple. It is not allowed to skip the same evidence
pipeline that real model execution would use.

## Things To Avoid

- Do not call a fake success result "real execution".
- Do not let AI writes go directly into the active project.
- Do not hide validation failures.
- Do not mix all new backend logic into a giant `lib.rs` block if a small module
  can make the boundary clearer.
- Do not create `quest.rs` and `quest/mod.rs` at the same time.
- Do not rely on a frontend label to prove a backend guarantee.

## Review Bundle Shape

The exact Rust types should follow the codebase, but the frontend needs these
concepts:

```text
quest_id
workspace_id
changed_files
diff_summary
validation_items
risk_or_warnings
unresolved_items
apply_readiness
rollback_reference
failure_evidence
```

Keep the model/provider layer separate from this evidence shape.

## Apply Guard

Before applying changes to the active project, check:

- the workspace belongs to the current project;
- paths stay inside the allowed root;
- the project has not changed unexpectedly since review;
- validation status is known;
- rejected or unresolved items are not silently applied;
- credentials/secrets are not being introduced.

If the apply path cannot be completed safely in one run, implement the
guardrails and write the remaining apply/rollback gap into the progress doc.

## Frontend Connection

QuestPage should not display opaque backend text. It should expose:

- running state;
- validation state;
- changed file list;
- review decision;
- blocked/failure evidence;
- apply readiness;
- follow-up repair action.

Small backend improvements should be surfaced through the existing UI when
reasonable. If the UI is too tangled, extract the smallest component/hook needed
to make the evidence readable.

## Tests

Prefer targeted tests:

```bash
cargo test -p <editor-or-quest-related-crate>
cargo check -p <editor-or-quest-related-crate>
cd editor && bun run build
```

Discover actual package names first.

