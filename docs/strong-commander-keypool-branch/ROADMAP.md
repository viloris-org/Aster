# Roadmap

## MVP: Router And Ledger Spine

Goal: prove the architecture without building the whole product.

- Define models: GoalState, CommanderState, SubagentTask, ModelProfile,
  ProviderChannel, KeyPool, KeySlot, RouterDecision, ExecutionLedger.
- Implement deterministic route selection.
- Implement KeyPool slot selection.
- Add session affinity.
- Add 429 cooldown and fallback.
- Add usage/health stats.
- Add ledger entries for decisions.
- Add tests.

## V1: Commander Runtime

Goal: make long goal-mode execution visible and recoverable.

- Commander loop skeleton.
- Bounded subagent task contract.
- Structured subagent result format.
- Verification result capture.
- Resume token/checkpoint record.
- Context compaction summary format.
- Goal phase UI or CLI panel.
- Router decision UI/CLI evidence.

## V2: Productized Orchestration

Goal: improve throughput and UX.

- Multi-channel provider support for the same model.
- Role-based model tier policy.
- Budget guard.
- Optional stream racing for high-latency tasks.
- Skills/plugins activation graph.
- MCP integration policy.
- KeyPool dashboard.
- Subagent activity dashboard.
- Advanced health statistics.

## V3: Long-Run Automation

Goal: make 8-hour tasks reliable.

- Drift detection.
- Automatic recovery strategy.
- Failure tournament compaction.
- Replayable ledger.
- PR/apply/review integration.
- End-to-end benchmark suite.

