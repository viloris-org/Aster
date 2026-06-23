# Claude Code Run Style

Use plain VSCode Claude Code goal mode. Do not depend on CCW, workflow
dashboards, subagents, or parallel agent orchestration for this task.

The goal is sustained direct engineering:

```text
read -> edit -> verify -> fix -> document -> continue
```

## Why Direct Mode

This repository is large enough that orchestration overhead can become fake
progress. The useful work is in:

- reading current code accurately;
- making small but meaningful edits;
- running local checks;
- repairing compile/build failures;
- improving UI structure and backend guarantees;
- recording evidence.

## Rate Limit Behavior

If the model hits a rate limit or unstable provider response:

- do not stop immediately;
- reduce prompt size;
- avoid launching extra agents;
- continue with local work: `rg`, file reads, cargo metadata, focused tests,
  TypeScript build fixes, progress/comparison updates;
- retry later with a smaller, concrete next step.

## Anti-Planning Rule

A good plan is not deliverable. A deliverable is:

- changed files;
- passing or honestly failing commands;
- evidence of improvement;
- a next concrete implementation target.

If Claude writes a plan, the next action must be to execute the first slice.

## When To Ask The User

Ask only for:

- credentials, accounts, tokens, or external service access;
- destructive operations;
- serious git conflicts;
- mutually exclusive product direction.

Do not ask for:

- permission to continue after a slice;
- obvious refactors required by the goal;
- choosing between small implementation details that can be tested;
- UI micro-polish that can be judged from the product brief.

