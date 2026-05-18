# Task 06: AI And Agent Tools

## Goal

Provide an optional AI/Agent operation layer that lets external AI clients observe, modify, verify, and recover project state through stable, permissioned, auditable tools. File and command operations must run inside explicit sandbox and worktree boundaries. This layer must be fully removable from minimal runtime builds.

## Non-Goals

- No built-in cloud LLM is required.
- No model training is required.
- No AI asset generation is required.
- No unconfirmed remote execution is allowed.
- No unrestricted host filesystem access is allowed.
- No direct edits to the user's primary worktree are allowed unless the active policy explicitly permits it.

## Core Services

- `AgentService`: register, query, execute tools, and validate permissions.
- `ToolSchema`: tool name, parameters, return format, risk, side effects, recovery strategy.
- `ToolContext`: controlled access to project, runtime, editor, assets, and scene.
- `PermissionPolicy`: read-only, write, filesystem, runtime control, and experimental tool gates.
- `SandboxPolicy`: filesystem roots, network access, process execution, environment variables, resource limits, and approval requirements.
- `WorktreeManager`: create, inspect, diff, merge, discard, and clean isolated agent worktrees.
- `MainThreadExecutor`: execute runtime/editor/scene operations on the legal main thread with timeout and cancellation.
- `TransactionManager`: begin, status, commit, rollback for scene, asset, file, and worktree changes.
- `TraceRecorder`: audit successful and failed calls, argument summary, result summary, recovery hints.
- `KnowledgeGate`: require guide/describe token before high-risk operations.
- `ProjectToolRegistry`: validated project-defined agent tools.
- `McpBridge`: optional local-only MCP or equivalent transport.

## Isolation Model

Agent writes must use one of these modes:

- `readonly`: observe project, runtime, editor, scene, assets, console, and diagnostics only.
- `transactional`: write through engine services with rollback support for scene, asset, and project metadata changes.
- `worktree`: create an isolated VCS worktree or copy-on-write project workspace, apply file edits there, then produce a diff for review.
- `direct`: write to the active project only when explicitly enabled by project policy and user confirmation.

The default mode is `readonly`. The recommended write mode is `worktree` for file edits and `transactional` for live scene/editor operations.

## Sandbox Requirements

- Filesystem access must be restricted to declared project roots, generated worktrees, temporary directories, and configured cache directories.
- File writes outside allowed roots require explicit approval and must be logged.
- Network access is disabled by default and enabled per tool group or per invocation.
- Process execution is disabled by default. Build, test, format, package, and external tool commands require allowlisted command patterns.
- Environment variables must be filtered; secrets are never exposed to tools unless explicitly granted.
- Tools must declare whether they need filesystem read, filesystem write, network, process execution, runtime mutation, editor mutation, or external device access.
- Sandboxes must enforce CPU, memory, wall-clock timeout, and output-size limits where practical.
- Every sandbox escape request must include reason, target path or command, expected side effects, and rollback plan.

## Worktree Requirements

- Agent code and asset edits should happen in an isolated worktree by default.
- Worktrees must record base revision, parent project, active profile, and creating agent/session.
- Worktree state must be inspectable through status, diff, changed files, generated assets, and test results.
- Worktree merge/apply must be an explicit step and may require user confirmation.
- Conflicts with the main worktree must be reported with file paths and suggested resolution options.
- Generated caches and build outputs must not be merged unless declared as release artifacts.
- Discarding a worktree must remove temporary files and update trace/audit records.
- Non-VCS projects must support a copy-on-write workspace with equivalent diff/apply/discard behavior.

## Tool Groups

- `docs`: self-description, workflow help, tool catalog.
- `api`: subsystem, component, shader, and resource knowledge.
- `project`: project info, settings, build configuration.
- `editor`: play, stop, pause, step, selection.
- `scene`: query, save, open, create, serialize scenes.
- `hierarchy`: object creation, parenting, hierarchy query.
- `asset`: read, write, search, refresh, resolve assets.
- `material`: create and edit material properties.
- `render`: RenderGraph, render settings, post-processing.
- `console`: read logs and filter errors.
- `runtime`: runtime status, assertions, waits, errors.
- `ui`: semantic UI create/edit operations.
- `transactions`: begin, status, commit, rollback.
- `sandbox`: capabilities, allowed roots, command allowlist, approvals, resource limits.
- `worktree`: create, status, diff, test, merge, discard isolated worktrees.
- `research`: tool quality, contract validation, trace-to-tool suggestions.

## Deliverables

- Agent service interface in P0.
- Sandbox and worktree policy interfaces in P0.
- Read-only tool catalog, project state, scene query, console read, and runtime observation in P1.
- Isolated worktree edit flow, diff review, and sandboxed build/test tools in P1/P2.
- Write tools, transactions, tracing, permission denial paths, and project-defined tools in P2.
- Optional MCP/local protocol bridge.
- Agent sandbox, worktree, and security model documentation.

## Acceptance

- Tool catalog lists schema, risk, and side effects.
- Read-only tools do not modify project files, scene, or runtime state.
- Write tools use transactions or explicitly declare non-rollback behavior.
- File-editing tools default to isolated worktree mode.
- Direct write mode is disabled by default and requires project policy plus explicit confirmation.
- Permission-disabled writes, external commands, and file writes are rejected.
- Sandbox rejects filesystem, network, process, or environment access outside declared capability grants.
- Worktree status, diff, merge, discard, and conflict reporting are tested.
- Runtime-touching tools use main-thread execution.
- Trace records success, failure, summaries, and recovery advice.
- Trace records sandbox grants, worktree base revision, changed files, command invocations, and merge/discard decisions.
- High-risk tools reject calls without a knowledge token.
- Custom tools are validated before registration.
- Transport binds to local address by default.
- `agent-tools` builds and tests independently.
- Disabling `agent-tools` removes MCP, HTTP, sandbox/worktree manager, trace, and tool registry dependencies from `runtime-min`.
