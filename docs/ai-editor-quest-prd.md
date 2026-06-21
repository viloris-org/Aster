# Aster Quest Detailed Specification

Status: Draft detailed sub-spec
Parent: [`docs/ai-agent-unified-spec.md`](./ai-agent-unified-spec.md)
Last updated: 2026-06-21

## Purpose

This document defines the detailed behavior for **Quest**, Aster's persistent AI task system for durable autonomous game-making work.

Quest is the primary AI-native product direction. It exists for work that needs persistent intent, autonomous execution, artifacts, validation, review evidence, recovery, and apply decisions. Local temporary assistance belongs in Editor AI, defined in [`docs/ai-editor-copilot-prd.md`](./ai-editor-copilot-prd.md).

This document follows the safety commitments in [`docs/ai-agent-unified-spec.md`](./ai-agent-unified-spec.md). It must not claim stronger isolation or authorization than the currently implemented Quest execution path provides.

## Goals

Quest should let a user:

- turn a broad game-making intent into a durable task;
- preserve goal, constraints, spec, timeline, artifacts, and decisions;
- let AI inspect, plan, generate, validate, repair, and prepare or apply reviewable results;
- review changed files, generated assets, diagnostics, validation, risks, and unresolved issues;
- open Quest artifacts in the Editor for inspection or manual correction;
- approve, reject, revise, partially accept, archive, reopen, continue, or policy-apply the task.

Quest succeeds when broad AI work feels autonomous, recoverable, and inspectable rather than like a long chat transcript.

## Non-Goals

Quest does not currently promise:

- full enterprise zero-trust automation;
- OS-level sandboxing;
- signed grant enforcement;
- no active-project mutation unless the implemented execution layer enforces draft workspace apply;
- arbitrary unattended production changes without implemented policy, validation evidence, and rollback;
- cloud synchronization;
- multi-user collaborative review;
- marketplace governance.

Quest is allowed to have future architecture hooks for these, but product copy must reflect only implemented guarantees.

## Quest Versus Editor AI

Use Quest when work is:

- durable;
- broad;
- multi-file;
- multi-artifact;
- risky;
- likely to need validation or repair loops;
- likely to need policy apply, review, or rollback before active-project mutation;
- useful to resume after restart;
- useful to archive as a task record.

Use Editor AI when work is:

- local;
- temporary;
- tied to current selection or file;
- easy to preview;
- easy to undo;
- answerable as explanation.

Editor AI can promote to Quest. Quest can open artifacts in Editor AI or the editor workspace for inspection and manual correction.

## Quest Record

Each Quest should have a durable record.

Required fields:

- `id`;
- `title`;
- `status`;
- `project_path`;
- `created_at`;
- `updated_at`;
- `intent_path` or embedded intent;
- `event_log_path`;
- `artifacts`;
- `review_state`;
- `execution_config`;
- `knowledge_context`;
- `final_decision`.

Optional fields:

- `spec_path`;
- `workspace_id`;
- `workspace_path`;
- `base_revision`;
- `snapshot_id`;
- `branch_name`;
- `parent_quest_id`;
- `model_config`;
- `validation_summary`;
- `risk_summary`.

Quest metadata should live in the editor profile by default, not inside the project, unless explicitly exported.

## Intent And Spec

Every Quest starts with intent.

Intent should capture:

- user's goal;
- selected project context;
- constraints;
- non-goals;
- relevant diagnostics;
- desired output;
- acceptance hints;
- source, if promoted from Editor AI.

A spec is optional for simple or investigative Quests, but required before broad write execution when the task is ambiguous, risky, or multi-artifact.

Spec should include:

- goal;
- scope;
- non-goals;
- affected files/artifacts if known;
- expected behavior;
- validation plan;
- review criteria;
- unresolved decisions.

The user can edit intent or spec before execution. During execution, edits should create a timeline event and may require replanning.

## Status Model

Canonical statuses:

- `draft`: intent exists but execution has not started.
- `clarifying`: Quest needs user answers.
- `specified`: enough intent/spec exists to proceed.
- `planning`: AI is preparing steps or execution approach.
- `prepared`: context, examples, and execution configuration are ready.
- `running`: AI is reading, editing, generating, or inspecting.
- `waiting_for_user`: Quest needs a decision, credentials, manual edit, approval, or policy exception.
- `validating`: deterministic checks are running.
- `repairing`: AI is addressing validation, review, or policy findings.
- `ready_for_review`: result or report is ready for human review or policy apply.
- `applying`: accepted work is entering the active project through the implemented apply path.
- `completed`: final decision recorded.
- `blocked`: cannot proceed under current constraints.
- `archived`: hidden from active work but retained.

Quest status is not a fixed wizard. The orchestrator may skip or repeat states based on task needs.

## Timeline Events

Timeline events should be append-only.

Event types:

- `intent_created`;
- `intent_updated`;
- `spec_created`;
- `spec_updated`;
- `clarification_requested`;
- `clarification_answered`;
- `plan_created`;
- `context_attached`;
- `example_retrieved`;
- `file_read`;
- `file_changed`;
- `scene_changed`;
- `asset_generated`;
- `command_run`;
- `validation_started`;
- `validation_passed`;
- `validation_failed`;
- `repair_started`;
- `repair_finished`;
- `manual_intervention_requested`;
- `manual_intervention_completed`;
- `review_ready`;
- `issue_reported`;
- `quick_fix_requested`;
- `decision_recorded`;
- `apply_started`;
- `apply_finished`;
- `blocked`;
- `archived`;
- `reopened`.

Event fields:

- `id`;
- `quest_id`;
- `type`;
- `timestamp`;
- `summary`;
- `details`;
- `artifact_refs`;
- `diagnostic_refs`;
- `actor`: user, system, model, validator, editor;
- `trust_label`.

Timeline should show meaningful progress, not raw token-by-token model output.

## Artifacts

Quest artifacts are reviewable outputs or evidence.

Artifact types:

- intent;
- spec;
- plan;
- changed file;
- generated file;
- scene preview;
- asset preview;
- validation log;
- diagnostic report;
- diff;
- review report;
- blocked report;
- unresolved issue;
- quick-fix result;
- transaction group;
- final decision.

Artifact fields:

- `id`;
- `type`;
- `label`;
- `path` or storage reference;
- `summary`;
- `created_at`;
- `source_event_id`;
- `trust_label`;
- `open_in_editor_target`;
- `validation_state`.

Artifacts should be openable in the appropriate editor surface when possible.

## Execution Styles And Profiles

Quest may use different execution styles and profiles as implementation matures. Quest remains the product surface. Solo and Extra are Quest execution styles that represent how much agent capacity is assigned to the Quest.

### Controlled Profile

MVP compatibility profile. Uses the same execution gate as Editor AI, with Quest timeline and review wrapping the result.

Appropriate for:

- simple file generation;
- small multi-step tasks;
- investigation reports;
- controlled validation runs.

Limitations:

- may still modify active project if the current execution path does;
- must not claim draft workspace isolation.

### Solo Profile

Primary autonomous Quest profile. One agent owns the full loop:

- inspect project context and Quest artifacts;
- plan the task;
- edit or generate artifacts in the allowed workspace;
- run deterministic validation where available;
- repair local failures within configured limits;
- produce review evidence and unresolved issues;
- invoke apply policy to auto-apply, request review, block, or revise.

Solo should feel like an agent doing the job, not a wizard asking the user to drive every step.

Appropriate for:

- most single-feature or single-system Quests;
- scene and behavior generation;
- bounded bug fixes;
- investigation followed by a scoped fix;
- refactors that fit within one agent's context and validation loop.

Limitations:

- Solo is only as autonomous as the implemented tools, validators, model, and policy allow;
- active-project apply still requires the implemented apply path;
- policy may still route medium/high-risk results to human review.

### Draft Workspace Profile

Target profile for broad Quest writes.

Behavior:

- create or reuse a task workspace or staging area;
- perform broad edits away from active project;
- collect diff, diagnostics, validation, and review artifacts;
- let apply policy discard, auto-apply, request review, or block results.

Commitment only after implemented:

- broad Quest writes do not directly touch active project before policy apply.

### Extra Profile

Agent cluster Quest profile. Extra uses more agent capacity than Solo for broader, riskier, or more parallelizable work.

Behavior:

- orchestrator decomposes work;
- workers handle scoped tasks;
- reviewers inspect artifacts and challenge integration quality;
- integration combines selected worker outputs in the draft workspace;
- deterministic validation and review evidence are attached;
- the same apply policy used by Solo decides auto-apply, human review, block, or revise;
- grants may constrain tool access if enforced by code.

Extra may be exposed as a Quest execution style because it affects cost, latency, capacity, and commercial packaging. Internal worker routing, prompt choreography, and permission mechanics should remain collapsible by default.

## Execution Flow

### Create

1. User enters a goal or promotes from Editor AI.
2. System creates Quest record.
3. Intent artifact is created.
4. Initial timeline event is recorded.
5. Orchestrator selects next state: clarify, specify, plan, inspect, run Solo, run Extra, or wait for policy/user input.

### Clarify

Clarify only when the answer affects:

- product intent;
- scope;
- cost;
- reversibility;
- risk;
- final review expectations.

Do not ask the user to decide internal worker type, tool choice, or implementation detail.

### Specify

Generate or update a spec when:

- task is broad;
- multiple approaches are reasonable;
- expected outcome is ambiguous;
- validation or review needs acceptance criteria;
- the task may affect many artifacts.

### Run

1. Gather context and relevant examples.
2. Normalize planned operations or task steps.
3. Execute through the selected or safest implemented style: usually Solo, optionally Extra.
4. Record timeline events.
5. Attach artifacts and diagnostics.
6. Stop on unsupported or unsafe operations.
7. Invoke apply policy when validation and review evidence are available.

### Command Authorization And Audit

Quest execution may request external commands for validation, builds, diagnostics, asset processing, or dependency inspection. Commands are authorized by execution zone.

#### Sandbox Commands

Sandbox commands may run without per-command user approval when:

- cwd is inside the Quest workspace or another explicitly sandboxed task workspace;
- the command is represented as structured argv, not a shell string;
- arguments do not escape the sandbox through absolute paths, parent-directory traversal, output paths, config paths, or manifest paths;
- network use is allowed by the sandbox policy;
- writes are limited to the sandbox workspace and approved build/cache outputs;
- the command is not destructive, privileged, or arbitrary-code execution.

Sandbox commands must still produce command evidence:

- argv;
- cwd;
- sandbox root;
- network flag;
- write scope;
- start and finish timestamps;
- exit code;
- stdout/stderr summary;
- full log artifact path when useful;
- policy decision.

Sandbox execution must not be described as OS-level isolation unless OS-level isolation is actually implemented.

#### Outside-Sandbox Commands

Commands outside the sandbox require an allowlist rule or explicit user/organization approval before execution.

The MVP allowlist shape should follow Codex-style prefix rules over argv tokens:

```text
prefix_rule(pattern=["cargo", "check"], decision="allow")
prefix_rule(pattern=["flutter", "analyze"], decision="allow")
```

Aster rules should additionally record:

- scope: once, session, or permanent;
- cwd scope;
- network permission;
- write scope;
- risk level;
- creator and source;
- reason;
- last-used audit metadata.

Permanent rules should be visible, revocable, and auditable. Broad rules such as `["docker"]`, shell interpreters, arbitrary script execution, recursive deletion, reset, clean, prune, publish, deploy, and credential-related commands require stronger policy handling than normal validation commands.

Deletion is denied by default, including inside the sandbox. Future deletion support must be scoped to generated build/cache outputs or reviewed transaction groups.

#### Elevation Requests

When Solo or Extra cannot continue under current command policy, the agent may request elevation. The request must be structured and reviewable.

Fields:

- requested command or operation;
- requested capability: outside-sandbox execution, network, write scope expansion, deletion, dependency install, container execution, credential access, or active-project apply;
- requested scope: once, session, Quest, project, or permanent;
- cwd and affected paths;
- reason current policy blocks progress;
- expected output;
- rollback or recovery plan;
- risk level;
- audit artifacts that will be produced if approved.

Elevation outcomes:

- approved once;
- approved for session;
- approved for Quest;
- approved permanently by user or organization policy;
- denied with fallback;
- denied and blocked.

If approval is needed, Quest should enter `waiting_for_user` with the elevation request attached as evidence. If denied, Quest should continue with a lower-privilege fallback when possible or produce a blocked report. Model text must not grant elevation by itself.

### Validate

Validators should run when available:

- language syntax;
- schema;
- asset references;
- scene load;
- script diagnostics;
- targeted tests;
- command registry checks.

Validation output becomes artifacts.

### Repair

Repair is allowed when:

- failure is local and understandable;
- retry limit is not exhausted;
- repair remains in scope;
- repair does not require new risky authority.

Repair should create timeline events and preserve failed evidence.

### Apply Policy

Apply policy decides what happens after execution and validation.

Possible decisions:

- `auto_apply`: result is low-risk, policy-approved, validated, and rollback-capable.
- `needs_review`: user or organization review is required before active-project apply.
- `needs_revision`: result is close but needs a repair or spec adjustment.
- `blocked`: execution cannot proceed under current constraints.
- `reject`: result should not be applied.

Inputs:

- changed paths and artifact kinds;
- risk classification;
- validation status;
- unresolved issues;
- rollback availability;
- user or organization autonomy settings;
- active project freshness;
- command and tool evidence.

Policy is trusted implementation. Model output may recommend a decision but must not authorize apply by itself.

### Review

Quest enters `ready_for_review` when it has:

- generated output or investigation report;
- changed artifact list;
- validation state;
- unresolved issues;
- decision options.

### Apply

Apply behavior depends on implemented safety layer.

Layer 1:

- apply may mean controlled active-project operations.
- UI must not imply draft workspace isolation.

Layer 2:

- apply promotes policy-approved draft workspace changes into active project.
- UI should show transaction groups, diffs, validation, policy decision, and rollback hints.

Apply may be explicit human approval or automatic policy approval. In all layers, apply must be recorded as a durable Quest decision with evidence and recovery information.

## Review Surface

Quest review must answer:

- What was requested?
- What changed?
- Which files, scenes, assets, or entities are affected?
- What validation ran?
- What passed?
- What failed?
- What issues remain?
- What risks remain?
- What policy decision was made?
- What can be auto-applied, manually applied, partially applied, revised, quick-fixed, discarded, archived, or reopened?

Review should compress internal AI activity by default. Expandable details may show logs, traces, events, and intermediate artifacts.

## Unresolved Issues And Quick Fixes

Unresolved issue fields:

- `id`;
- `severity`: blocking, non-blocking, advisory;
- `summary`;
- `affected_artifacts`;
- `evidence_refs`;
- `recommended_action`;
- `requires_user_input`;
- `quick_fix_available`;
- `quick_fix_scope`.

Quick fixes should be scoped to the issue. They should not rerun unrelated work unless necessary.

## Partial Acceptance

Partial acceptance is allowed only when changes can be grouped cleanly.

Transaction group fields:

- `id`;
- `summary`;
- `artifact_refs`;
- `dependencies`;
- `validation_state`;
- `risk_hint`;
- `apply_state`.

If partial acceptance cannot be implemented safely, the UI should offer full accept, reject, revise, or manual open-in-editor instead.

## Open In Editor

Quest artifacts should open in the editor when useful.

Mappings:

- changed code file -> script/behavior editor;
- scene artifact -> Scene View and Hierarchy;
- entity change -> Inspector selection;
- asset artifact -> Project/Assets panel;
- diagnostic -> Console and relevant file;
- spec/intent -> text artifact editor;
- diff -> review surface.

Manual edits made in Editor while Quest is active should be recorded as manual intervention evidence where possible.

## Knowledge

Quest may propose Knowledge updates after completion, blocking, or investigation.

Knowledge proposals must include:

- proposed fact or preference;
- source artifacts;
- confidence;
- whether user approval is required;
- suggested scope: project or user.

Quest-local assumptions must not silently become Knowledge.

## UI Requirements

Quest UI should include:

- Quest registry;
- create/new prompt;
- title and status;
- project identity;
- intent/spec tabs;
- timeline;
- artifact list;
- validation section;
- unresolved issue section;
- review/decision controls;
- open-in-editor controls;
- archive/reopen controls;
- execution configuration where useful.

The user should always know whether work is:

- only intent;
- running;
- draft;
- validated;
- ready for review;
- applied;
- blocked;
- archived.

## Execution Configuration

Quest may expose limited configuration:

- model/provider selection or inherit editor default;
- thinking effort where supported;
- execution style: Solo or Extra;
- validation level;
- whether to generate spec first.
- apply policy/autonomy level where implemented.

Do not expose low-level worker routing or internal permission mechanics as default controls.

## Acceptance Criteria

Quest shell MVP is acceptable when:

- user can create, rename, archive, reopen, and delete a Quest;
- Quest persists across editor restart;
- intent artifact is stored and editable;
- optional spec artifact is stored and editable;
- timeline records major events;
- artifacts can be listed and opened;
- review surface can show changed files, diagnostics, issues, and decisions;
- Editor AI conversation can promote into a Quest intent;
- Quest can open relevant artifacts in Editor.

Quest execution MVP is acceptable when:

- a bounded task can run through the shared execution gate;
- execution events appear in timeline;
- generated or changed artifacts are attached;
- validator output is attached when available;
- blocked outcomes preserve evidence and next actions;
- apply is explicit or policy-approved and recorded;
- UI does not claim stronger isolation than implemented.

Solo milestone is acceptable when:

- one agent can run inspect, plan, workspace edit, validate, and bounded repair without user step-driving;
- sandbox commands can run without approval while producing audit evidence;
- outside-sandbox commands require allowlist or explicit approval;
- destructive commands, including deletion, are denied by default;
- low-risk successful results can reach policy apply;
- medium/high-risk or failed results route to review or blocked states with evidence;
- active-project mutation happens only through the implemented apply path.

Draft workspace milestone is acceptable when:

- broad Quest write work happens outside active project;
- diffs are available before apply;
- discard leaves active project unchanged;
- apply uses a policy-approved path;
- failed apply has recovery behavior.

Extra milestone is acceptable when:

- a Manager can decompose a Quest into worker slices;
- Workers produce bounded outputs in draft workspace attempts;
- Reviewers produce integration findings;
- selected integrated output becomes the Quest review bundle;
- Extra uses the same validation and apply policy as Solo.

## Test Requirements

Required tests:

- Quest record persistence;
- status transitions;
- append-only event writing;
- intent/spec edit persistence;
- promotion from Editor AI;
- artifact open-in-editor routing;
- review decision persistence;
- blocked report preservation;
- execution profile labels do not overclaim safety;
- draft workspace discard/apply behavior once implemented;
- Solo autonomy loop behavior;
- sandbox command audit behavior;
- outside-sandbox command allowlist behavior;
- elevation request approval, denial, and fallback behavior;
- destructive command denial behavior;
- policy auto-apply, needs-review, and blocked routing;
- Extra decomposition, worker output, reviewer evidence, and integration result behavior once implemented.

## Migration From Earlier PRDs

Legacy names:

- "Copilot Mode" maps to Editor AI.
- "Auto Mode" maps to Quest using Solo by default or Extra for cluster execution.
- "SOLO" maps to the Solo Quest execution style: one autonomous agent.
- "Extra" maps to the Extra Quest execution style: Manager, Workers, Reviewers, and integration.
- "Manager/Worker/Reviewer" are Extra implementation roles and should be hidden unless the UI is explaining evidence, cost, or progress.

Old zero-trust language is historical architecture context only. Current Quest promises are defined by this document and the unified spec.

## Open Questions

- What storage location and retention policy should Quest use in the editor profile?
- What is the first durable `quest.json` schema?
- Which timeline events are required for MVP versus later?
- Which broad writes must wait for draft workspace support?
- How should partial acceptance be grouped?
- What validation level is required before ready-for-review?
- How much execution configuration should users see?
