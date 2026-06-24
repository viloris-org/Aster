# Research Synthesis

## Core Thesis

The target architecture is not "many weak models pretending to be a strong
agent." It is a hierarchy:

```text
Strong Commander
-> lower-tier subagents
-> cheap/fast helper models
-> provider/channel/keypool router
```

The commander owns architecture, final decisions, context strategy, merge,
verification policy, and recovery. Subagents are controlled workers and
reviewers. Cheap models handle rote work.

This matters because domestic and OpenAI-compatible model ecosystems often have
uneven behavior:

- high first-token latency;
- RPM/TPM constraints;
- 429 bursts;
- provider-specific streaming quirks;
- weaker tool-call reliability;
- long-context drift;
- inconsistent JSON/schema adherence.

A product-grade system should not hide these issues. It should route around
them.

## Agreement Between The Two Reports

Both research sources agree on these points:

- goal mode fails when it stops after planning;
- long coding sessions need a ledger, checkpoints, and resume tokens;
- subagents are useful only with strict task contracts and structured results;
- router decisions must be based on role, health, latency, budget, context, and
  failure history;
- KeyPool needs session affinity, RPM pooling, cooldown, fallback, and optional
  stream racing;
- skills/plugins/MCP should be activated by the commander, not dumped into every
  prompt;
- the UI must expose current phase, active agents, verification, fallback, and
  KeyPool health without overwhelming the input box.

## Useful Differences

The Claude Code research is stronger on:

- concrete data models;
- commander lifecycle;
- context compaction and recovery;
- pseudocode;
- implementation acceptance criteria.

The ChatGPT web research is stronger on:

- competitor comparison;
- product framing;
- UI information layers;
- anti-stopping goal mode rules;
- short product-friendly implementation phases.

The combined branch should use Claude Code's concrete architecture and
ChatGPT's product packaging.

## Product Positioning

The product should be positioned as:

```text
An AI coding tool for long autonomous engineering runs where one strong model
commands cheaper workers through a resilient router and visible execution
ledger.
```

The differentiator is not "more agents." The differentiator is:

- commander-first judgment;
- bounded subagent delegation;
- model/channel/key-level routing;
- long-run recovery;
- evidence-based progress;
- UI that makes the invisible orchestration understandable.

## Non-Negotiable Design Rules

1. Subagents are not peers of the commander.
2. The commander must never blindly trust subagent summaries.
3. API keys and provider secrets must stay outside model context.
4. Stream racing is a targeted latency tool, not a default mode.
5. Session affinity should be the default for cache-sensitive sessions.
6. 429 should trigger cooldown and fallback, not stop the goal.
7. A goal is not complete because a plan exists.
8. Skills/plugins are activated by need and evidence, not by being installed.
9. The UI must show commander state first, debugging details second.
10. Every long run needs a ledger that survives compaction.

## Target User Value

The user should be able to start a large coding goal, leave it running, and come
back to:

- what the commander decided;
- which subagents worked on what;
- which model/key/provider was used;
- what failed and recovered;
- what tests/builds ran;
- what remains blocked;
- what can be safely applied or continued.

