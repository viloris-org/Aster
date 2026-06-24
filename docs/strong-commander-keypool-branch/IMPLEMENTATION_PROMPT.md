# Implementation Prompt

Use this prompt when asking an AI coding agent to implement the branch.

```text
You are working on the strong-commander-keypool branch direction. Read docs/strong-commander-keypool-branch/ first, then implement a product-grade slice of the architecture.

Core rule: do not build a peer-to-peer multi-agent toy. Build a commander-first AI coding runtime.

Architecture target:
- one strong commander model owns hard decisions, context, final merge, recovery, and stop conditions;
- lower-tier subagents execute bounded tasks under commander assignment;
- cheap/fast helper models handle logs, tests, summaries, scouting, and repeated checks;
- a KeyPool/router handles model role selection, provider channel selection, key slot selection, 429 cooldown, RPM pooling, session affinity, fallback, budget guard, and optional stream racing;
- a durable ledger records decisions, model/key routing, subagent work, verification, failures, compaction, and recovery;
- UI exposes commander state, subagent activity, KeyPool health, verification, and goal progress without overwhelming the input box.

Start with the smallest useful product slice:
1. Define core data models: GoalState, CommanderState, SubagentTask, ModelProfile, ProviderChannel, KeyPool, KeySlot, RouterDecision, ExecutionLedger, VerificationResult.
2. Implement router/keypool decision logic with deterministic tests for session affinity, RPM pooling, 429 cooldown, fallback chain, and budget guard.
3. Add a commander loop skeleton that can assign bounded subagent tasks and record results in a ledger.
4. Add UI or CLI evidence surfaces for goal phase, commander action, active subagents, router decision, 429/fallback events, and verification state.
5. Add docs and verification commands.

Do not expose API keys to model context. Do not make stream racing default. Do not let subagents modify shared files without commander merge. Do not stop after planning; implement, test, document, and continue.
```

## Acceptance Criteria

- Router has deterministic tests.
- KeyPool supports at least session affinity, cooldown, fallback, and usage stats.
- Commander/subagent contract is represented in code.
- Ledger records routing and task evidence.
- UI/CLI shows enough state to debug a long run.
- Docs explain how this improves long AI coding sessions.

