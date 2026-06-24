# Product Architecture

## System Shape

```text
User Goal
-> Goal Runtime
-> Strong Commander
-> Router
-> Subagents / Helpers / Tools
-> Verification
-> Ledger
-> UI Evidence
```

## Major Modules

### Goal Runtime

Owns lifecycle:

```text
created
scouting
planning
executing
verifying
repairing
compacting
retrying
blocked
completed
rolled_back
```

It persists:

- current phase;
- approved scope;
- active tasks;
- verification results;
- recovery checkpoints;
- resume token;
- stop reason.

### Strong Commander

The commander is the only component allowed to make hard product and
architecture decisions.

Responsibilities:

- interpret the user goal;
- maintain the canonical task state;
- choose slices;
- decide which work must stay with the commander;
- assign subagent tasks;
- inspect subagent results;
- merge or reject results;
- trigger verification;
- decide next action after failures;
- manage compaction and resume.

Commander should use a top-tier coding model, such as a GLM-5.2-class model or
another best-available coding model.

### Lower-Tier Subagents

Subagents are bounded workers.

Good tasks:

- codebase scouting;
- file map summaries;
- test log analysis;
- UI review;
- security checklist review;
- documentation extraction;
- candidate patch suggestions;
- repeated local checks.

Bad tasks:

- final architecture decisions;
- broad refactors across shared files;
- git reset/cleanup;
- secret handling;
- final merge;
- deciding stop/completion.

### Cheap Helper Models

Cheap/fast models are for low-risk throughput:

- classify errors;
- summarize logs;
- rank files;
- draft docs;
- detect duplicate TODOs;
- produce quick UI critique;
- compare two outputs.

They should not write final code without commander review.

### Router And KeyPool

The router chooses:

- role model;
- provider channel;
- key slot;
- fallback chain;
- race or no-race mode;
- cooldown behavior.

The KeyPool handles:

- multi-key pooling;
- session affinity;
- RPM/TPM accounting;
- 429 cooldown;
- health score;
- budget guard;
- stream race slots.

### Ledger

The ledger is the durable truth source.

It records:

- commander decisions;
- subagent assignments;
- model/channel/key choices;
- tool calls;
- file changes;
- tests and build results;
- 429/fallback/cooldown events;
- compaction summaries;
- open risks.

This allows long runs to resume after context loss.

## UI Information Architecture

### Input Strip

Keep this minimal:

- goal active / paused / blocked / completed;
- current phase;
- elapsed time;
- active agents count;
- last verification result;
- open panel button;
- stop/pause control.

Do not show raw model lists or debugging noise here.

### Commander Panel

Primary right-side panel:

- current commander intent;
- active slice;
- next decision;
- scope;
- blockers;
- verification status;
- compaction/resume state.

### Subagent Activity

Shows:

- task title;
- assigned role;
- model tier;
- status;
- output confidence;
- files inspected or touched;
- commander verdict.

### Router/KeyPool Dashboard

Shows:

- provider/channel health;
- key slot health;
- RPM usage;
- 429 count;
- cooldown;
- cache affinity bindings;
- fallback events;
- stream race win/loss;
- budget burn.

This is advanced but essential for users who run domestic/multi-channel models.

### Skills/Plugins Page

Shows:

- installed skills;
- activation rules;
- last activation reason;
- input/output contract;
- verification command;
- risk level.

Skills should be discoverable but not always injected.

## Product Success Criteria

A successful implementation lets the user answer:

- Who is in charge?
- What is currently being done?
- What model/key/provider was used?
- Why did the router choose it?
- What failed?
- What recovered?
- What evidence proves progress?
- What can continue safely?

