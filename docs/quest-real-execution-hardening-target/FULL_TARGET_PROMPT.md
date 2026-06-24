# Full Target Prompt

Use this as supporting context for Claude Code goal mode. The shorter
`GOAL_PROMPT.txt` is the current entry prompt.

```text
You are working in the Aster/Varg editor repository. This document describes the
Quest real execution hardening lane inside the larger AI-native Quest/Editor
implementation run.

Current collaboration context:
- Another person may be implementing the main Quest execution and physics work.
- Do not race them by rewriting the same large feature surfaces.
- Use the current designated branch unless the user explicitly asks for a new
  branch.
- Your value here is to combine implementation with verification, hardening,
  missing tests, bug repair, and evidence-backed review.

First actions:
1. Fetch and inspect remote state.
2. Confirm the current branch and dirty worktree before editing.
3. Read these docs:
   - docs/quest-real-execution-hardening-target/START_HERE.md
   - docs/quest-real-execution-hardening-target/CHECKLIST.md
   - docs/quest-real-execution-hardening-target/HANDOFF_RULES.md
   - docs/ai-native-engine-loop-target/QUEST_REAL_EXECUTION_PROMPT.md
   - docs/ai-native-engine-loop-progress.md
   - docs/ai-native-engine-loop-comparison.md
4. Map the real Quest execution path in code before changing it.

Primary mission:
Prove and harden this chain:

quest request
-> isolated workspace
-> real or deterministic execution
-> changed file detection
-> diff/review bundle
-> validation results
-> stale project guard
-> selected apply/discard gate
-> rollback or cleanup evidence
-> clear failure state

Do not accept a green UI label as proof. Backend guarantees must be backed by
tests or explicit command evidence.

High-value audit areas:
- start_quest_execution / finish_quest_execution request lifecycle;
- cancellation and completed request cleanup;
- workspace creation, workspace identity, and project-root containment;
- diff generation for text, binary, large files, new files, deleted files, and
  unchanged files;
- validation entries when execution succeeds, fails, or produces no changes;
- review bundle completeness: changed files, summary, findings, metrics,
  warnings, unresolved items, and apply readiness;
- stale active-project detection before apply and discard;
- selected apply/discard validation against reviewed transaction groups;
- rollback correctness after apply;
- discard behavior and workspace cleanup;
- deterministic stub/provider path for testability without API keys;
- credential/provider errors before execution starts;
- path traversal and suspicious path handling;
- dangerous command or process execution boundaries;
- failure reporting that does not pretend success.

Implementation discipline:
- Add targeted tests first when the risk is clear.
- Fix the smallest concrete bug that makes a test pass.
- Do not rewrite QuestPage or EditorPage unless needed to expose real evidence.
- Do not refactor giant files for aesthetics during this run.
- If a large extraction is truly necessary, stage it in tiny commits and keep
  behavior unchanged.
- Do not introduce new provider secrets or write secrets into logs, prompts,
  review bundles, ledger entries, or screenshots.
- Keep deterministic tests independent of live API keys.

Suggested verification commands:
- Discover package names first with cargo metadata or Cargo.toml inspection.
- Run targeted Rust tests around Quest, editor Tauri, engine-ai, and policy.
- Run cargo fmt --check after code edits.
- Run cargo check for the touched crate.
- Run cd editor && bun run build only if frontend or shared TS surfaces changed.
- If a command cannot run, record the exact command, failure, and likely cause.

Loop rule:
Do not stop after a plan. Each loop must produce at least one of:
- a failing test that exposes a real gap;
- a passing test that locks existing behavior;
- a bug fix;
- a verification command result;
- an evidence-backed finding with file/function references.

If the first pass finds no major bug, go deeper instead of stopping:
- add edge-case tests;
- test cancellation/failure/no-change branches;
- test large/binary behavior;
- test rollback/discard invariants;
- test stale review behavior after project mutation;
- inspect whether frontend states represent backend truth.

Stop condition:
Stop only when you have produced a run report with:
- branch and baseline commit;
- code path map;
- tests added or verified;
- bugs fixed;
- commands run and results;
- remaining risks with concrete file/function references;
- clear handoff notes for the feature implementer.
```
