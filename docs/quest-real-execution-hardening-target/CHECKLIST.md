# Quest Hardening Checklist

Use this as a concrete audit checklist. Mark items as proven, fixed, blocked, or
out of scope in the final run report.

## Baseline

- Fetch remote branches.
- Record current branch and baseline commit.
- Check dirty worktree before editing.
- Identify whether work starts from `main`, a feature branch, or an agreed
  integration branch.

## Execution Path

- Locate Quest creation, start, finish, cancel, apply, discard, rollback, export,
  and validation handlers.
- Trace how a model or deterministic provider result becomes workspace changes.
- Verify real execution and deterministic test execution use the same evidence
  pipeline where possible.
- Confirm no direct active-project writes happen before review/apply.

## Workspace And Diff

- Workspace path is contained under the expected Quest/project root.
- Reviewed paths cannot escape with `..`, absolute paths, odd separators, or
  symlink-like behavior.
- New, modified, deleted, unchanged, binary, and large files are represented
  safely.
- Large/binary files do not require loading unbounded bytes into memory.
- No-change execution becomes a useful blocked/no-op review state.

## Validation And Review

- Validation items are recorded for success and failure paths.
- Failed validation cannot be silently converted to success.
- Review bundle includes changed files, findings, metrics, warnings, unresolved
  items, and apply readiness.
- Provider or credential failure is clear before or during execution.
- Stub/deterministic provider allows tests without live API keys.

## Apply, Discard, Rollback

- Apply checks project freshness against the review snapshot.
- Discard also rejects stale reviews where appropriate.
- Selected apply/discard only accepts files or transaction groups present in the
  review bundle.
- Rollback restores active project content after apply.
- Discard does not mutate active project content.
- Cleanup behavior is explicit or tested.

## Request Lifecycle

- Start/finish request state cannot leak completed requests forever.
- Cancellation records a truthful state.
- Failure records an actionable error, not a fake success.
- Concurrent or repeated request IDs cannot corrupt state.

## Verification

- Add tests for gaps before or with fixes.
- Run targeted tests.
- Run formatting/check commands for touched crates.
- Record commands and exact outcomes in the run report.

