# Handoff Rules

This target is designed for collaboration with other people or agents.

## Do

- Use the current designated branch unless the user explicitly tells you to
  create another branch.
- Pull or fetch the latest agreed baseline before starting.
- Keep edits focused on Quest execution validation, hardening, tests, and bug
  fixes.
- Prefer tests and evidence over speculative rewrites.
- Leave clear notes for the person building the main Quest feature.
- Check `git status --short --branch` before and after edits.

## Avoid

- Do not duplicate broad Quest execution or physics feature work already owned by
  another person.
- Do not rewrite large frontend pages merely to improve style.
- Do not implement Commander/KeyPool architecture in this branch.
- Do not mix unrelated ECS, render, audio, or UI polish work into this hardening
  pass unless it directly blocks Quest verification.
- Do not commit secrets, generated build output, `target/`, or machine-local
  configuration.

## Conflict Rule

If another branch already changed the same function, do not bulldoze it. Read the
new code, add tests around the intended behavior, and make the smallest compatible
fix. If the conflict is architectural, write a concrete finding with file and
function references before touching broad surfaces.
