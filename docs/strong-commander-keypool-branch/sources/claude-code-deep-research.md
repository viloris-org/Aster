# Strong Commander KeyPool — Deep Research Architecture Document

> **Version:** 1.0 — Research & Architecture
> **Date:** 2026-06-24
> **Status:** Design document — research only, no implementation
> **Scope:** Practical AI coding architecture with GLM-5.2-class commander, GLM-5.1 multi-channel subagents, cheap/fast helpers, and KeyPool/router load balancing

---

## 1. Executive Summary

SokachCode already has a sophisticated routing core (16 hard filters, 8-dimension scoring, 7-error fallback matrix), a 14-phase goal state machine, a KeyPool multi-key system (4 modes, 39 tests passing), and a full frontend UI (32px status bar, 7-tab goal panel, 10-tab router workspace). What it does not have is a strong commander architecture — an explicit hierarchy where one top-tier model owns all hard decisions, lower-tier models execute sub-tasks under commander assignment, and cheap models handle rote work like scouting, testing, and summarization across multiple channels with a multi-key, multi-provider KeyPool that doubles throughput and eliminates 429 bottlenecks.

### Core Thesis

The architecture should not rely on many weak models acting as peers. The desired hierarchy is:

**Strong commander model → lower-tier subagents → cheap/fast helper models → provider/channel/keypool router**

### Key Innovations

1. **Commander-First Hierarchy**: One model makes all hard decisions; subagents are instruments of commander will
2. **Multi-Channel KeyPool**: Stream racing, cache affinity, RPM pooling, 429 fallback across N keys
3. **Role-Tier Mapping**: Dynamic model selection per role with cost/latency tradeoffs
4. **429/RPM Resilience**: Exponential backoff, slot fallback, budget guards
5. **Cache Affinity**: Same session binds to same key slot for prompt caching accumulation
6. **Stream Racing**: N keys compete; first token wins; others cancelled
7. **Fallback Chain**: Primary → secondary slot → different model → pause_user

### Copilot-Grade Quality Bar

- Concrete. Do not produce a generic agent report.
- Do not make weak models equal decision-makers.
- Do not ignore RPM, 429, first-token latency, cache affinity, provider differences, tool-call instability, cost, long-context drift, or UI/product design.

## 2. SokachCode Reference Findings

SokachCode V6.4 provides the foundational layers this architecture builds upon. Understanding what exists is critical to defining what needs to change.

### 2.1 Existing Architecture Strengths

**Router Core Engine (Layer 0-3)**:
- 16 hard filters with optimized execution order — boolean checks first, expensive token estimation last
- 8-dimension weighted scoring (roleFit 0.24, toolReliability 0.18, contextMargin 0.14, recentSuccessRate 0.14, latencyScore 0.10, costScore 0.10, userPreference 0.07, providerHealth 0.03)
- Per-role weight overrides (planner weights reasoning higher; executor weights tool reliability higher)
- Fallback action matrix: 8 error scenarios → 8 deterministic recovery actions
- LLM Router as optional enhancement layer — rules-first, AI-augmented
- Pure functional core: route() is deterministic, no Math.random, zero side effects
- Zero-second-copy data: RouteCandidate[] is the single data structure for the routing layer

**KeyPool Multi-Key System (V6.4)**:
- 4 scheduling modes: cache_affinity (default), stream_race, round_robin, cost_optimal
- Per-slot statistics (RPM, monthly spend, success rate, latency, 429 count)
- 429 handling with exponential backoff (2s/4s/8s... max 10 min)
- Session-to-slot binding with LRU eviction for cache affinity
- Pool-level budget and RPM aggregation
- 39 unit tests passing

**Goal State Machine**:
- 14-phase execution: created → scouting → planning → plan_ready → awaiting_approval → executing → verifying → completed/failed/paused/retrying/cancelled/rolling_back/rolled_back
- 3 execution modes: guided (AI auto-executes, pauses at checkpoints), confirm (user approves plan before execution), auto (fully automatic)
- Structured steps per goal with planVersion tracking and approvedPlanVersion locking
- Polling + SSE dual-channel synchronization (12s active / 45s background, real-time SSE events)
- mergeGoalSnapshot: 5 conflict-resolution rules for concurrent updates
- Terminal freeze: once a goal reaches completed/failed/cancelled/rolled_back, the elapsed time is frozen and polling stops

**Watchdog & Stall Detection**:
- 5 stall types with distinct thresholds: stream_idle (45s), tool_idle (90s), shell_idle (120s), step_timeout (900s), no_heartbeat (300s)
- Heartbeat tracker consuming real events: LLM stream tokens, tool start/result/error, shell stdout/stderr, router decisions, compaction events
- 8-action escalation matrix: retry_same, fallback_model, compact_then_retry, pause_user, downgrade_model, upgrade_model, kill+summarize, fail_goal

**Statistics & Ledger**:
- RouterLedger stores up to 1,000 RouterDecision + 10,000 UsageEvent records
- 1-minute bucket granularity heatmap from UsageEvent ledger
- ComputeStats runs O(events) single traversal — no caching, always fresh
- Rankings: most-used models, most-failed models, most-429 models, cost per model

**Frontend UI**:
- 32px fixed goal status bar with contextual text (not model lists, not debug logs)
- 7-tab goal activity panel: Overview | Plan | Activity | Scope | Verification | Routing | Next
- 10-tab router workspace: Overview | Candidates | Decisions | Filters | Scoring | Scenarios | Budgets | Recovery | Statistics | Simulator
- Domestic Gateway page with quick-add for 9 Chinese model providers
- Auto Agent settings with per-role model assignment (scout/planner/executor/verifier/summarizer/router)
- No Math.random anywhere — deterministic IDs, round-robin via counter

**API Key Safety**:
- Keys never enter the routing layer — RouteCandidate only carries providerID/modelID/variant
- LLM Router input is sanitized: only providerID/modelID/healthTier/budgetTier transmitted
- Ledger records only providerID/modelID, never auth info
- Frontend displays masked keys (sk-12ab****cdef), type="password" inputs
- Zero data copying between layers — single source of truth from opencode auth store

### 2.2 What SokachCode Does NOT Have (Gaps This Research Fills)

| Capability | Current State | Needed for Strong Commander |
|------------|--------------|------------------------------|
| Commander model | No dedicated commander role | GLM-5.2-class model with supreme planning/delegation authority |
| Subagent hierarchy | All roles are peers (6 equal roles) | Commander assigns bounded tasks to subagents; subagents report back |
| Helper tier | No separate cheap tier | Flash/MiniMax-Mini for rote work (tests, logs, summaries) |
| Multi-channel dispatch | KeyPool expands key, but stream_race is per-provider | Commander can dispatch same task across N channels concurrently |
| Long-run context strategy | Compaction snapshots are generic | Commander controls when to compact, what to keep, and structured resume tokens |
| Commander state tracking | No separate state for "who is the commander" | CommanderState tracks decisions, subagent assignments, risk assessments |
| Result merging | Verifier handles merge for execution results | Commander merges subagent results across potentially conflicting outputs |
| Escalation stop conditions | Watchdog triggers pause/fail | Commander decides when to escalate to user — but also when to force-stop |
| Risk assessment | No dedicated risk model | Risk assessor role computes per-step risk before commander delegates |
| Skills/plugins/MCP | No external tool ecosystem | Commander decides which skills to activate; MCP integration for tools |

## 3. Product Positioning

### 3.1 The Gap in Current Coding Agents

Current coding agents (Codex, Claude Code, Cursor, Cline, Roo Code, Devin) all use flat peer-to-peer architectures. No product currently offers:

- Hierarchical commander → subagent model selection
- Multi-key stream racing for first-token latency optimization
- Explicit role-to-tier mapping with dynamic cost/latency tradeoffs
- 429-aware routing with slot-level cooldown and automatic fallback
- Session-level cache affinity binding for prompt caching accumulation
- Skills/plugins/MCP as first-class commander-activatable capabilities

### 3.2 Competitive Landscape

| Product | Architecture | Model Tiering | Multi-Key | 429 Handling | Cache Affinity | MCP/Skills |
|---------|-------------|---------------|-----------|-------------|----------------|------------|
| Codex CLI | Flat peer | Manual | No | No | No | Limited |
| Claude Code | Flat peer | Manual | No | Basic retry | No | Partial |
| Cursor Agent | Flat peer | Auto-select | No | No | No | No |
| Cline/Roo Code | Flat peer | Manual roles | No | No | No | Partial |
| Devin | Opaque | Unknown | No | Unknown | Unknown | Custom |
| **SokachCode + Commander** | **Hierarchical** | **4 tiers dynamic** | **KeyPool 2-keys+** | **Per-slot cooldown + fallback** | **Session binding** | **First-class** |

### 3.3 Target Market

Developers running 30min+ coding sessions who need:

- Cost-efficient tiered model usage (not $15/hr for everything)
- Completable long sessions (8-hour runs that don't drift)
- Multi-key throughput (2+ API keys for parallel execution)
- Observable decision chains (debugging what the AI decided and why)

## 4. Strong Commander Architecture

### 4.1 Commander State Model

The commander maintains CommanderState:

- **currentGoal**: GoalState — the active goal the commander is driving
- **assignedSubagents**: SubagentTask[] — tasks delegated to lower-tier workers
- **modelProfile**: ModelProfile — GLM-5.2-class model configuration
- **contextStrategy**: "compacted" | "full" | "selective" — what context the commander holds
- **decisionLog**: RouterDecision[] — all routing decisions the commander made
- **roleAssignments**: Map<string, ModelRole> — which role uses which tier
- **activeKeySlots**: KeySlot[] — which slots are currently racing/affinity-bound
- **failCount**: number — consecutive fallback count for escalation

### 4.2 Commander Decision Hierarchy

The commander never asks "should I use model A or B?" — it asks:

1. "What type of task is this?" → selects role from ModelRole enum
2. "Which tier handles this role?" → consults ModelRole tier assignment
3. "Which key slot should I use?" → consults KeyPool for this session
4. "What if this fails?" → builds fallback chain before dispatching

The commander does NOT:

- Try multiple models in parallel for the same decision (wasteful)
- Let subagents decide their own role assignments
- Decay its own context across multi-hour runs (it compacts strategically)

### 4.3 Commander Context Strategy

During an 8-hour run, the commander must hold:

- GoalState snapshot (what was approved, what changed, verification summary)
- CommanderDecision log (last N routing decisions with rationale)
- CurrentPlan (the approved plan, not the full conversation)
- RiskAssessment (current risk tier and thresholds)

The commander does NOT hold:

- Full tool call chains from every subagent (those go to the ledger)
- Raw API responses from subordinate model calls
- Intermediate reasoning drafts from subagent planning phases

### 4.4 Commander Lifecycle

```
commanderLoop(goal) {
  while (!goal.isTerminal) {
    assessRisk(goal) → risk
    if (risk.critical) escalateToUser(goal, risk)
    delegate = selectSubagentTask(goal, risk)
    assignSubagent(delegate) → subagentId
    result = await subagentComplete(subagentId, timeout)
    if (result.verification.failed) {
      retryCount++
      if (retryCount > maxRetries) {
        escalateToUser(goal, result.failureReason)
      } else {
        delegate = adjustTaskForRetry(delegate, result)
        continue
      }
    }
    mergeIntoGoalState(goal, result)
    recordDecision(goal, delegate.modelUsed, result)
  }
}
```

### 4.5 Commander Escalation Criteria

The commander escalates to user (pauses the run) when:

1. Scope drift detected: implementation diverges >30% from approved plan
2. Consecutive fallbacks exceed budget: more than 3 slot/model fallbacks in one step
3. Critical risk: security vulnerability, data loss risk, or breaking API change
4. Stalled beyond threshold: no progress for >stepTimeout despite retries
5. Budget exhaustion: monthly spend within 5% of limit on an expensive model

## 5. Lower-Tier Subagent Architecture

### 5.1 Subagent Task Model

Each subagent receives a SubagentTask:

```
number: number — identity marker
role: AgentRole — what kind of task this is
description: string — bounded, specific task description
context: {
  allowedFiles: string[],
  recentChanges: string[],
  verificationCriteria: string[]
}
modelTier: ModelTier — recommended tier (not enforced — commander decides)
expectedOutputFormat: OutputFormat — structured or freeform
timeoutMs: number — how long before the commander abandons this task
fallbackPolicy: FallbackPolicy — what to do on failure
parentDelegationId?: string — if this task was spawned from another
createdAt: number — timestamp
```

### 5.2 Subagent Execution Model

Subagents operate in a strict execute-and-report pattern:

1. Receive SubagentTask with bounded scope
2. Execute within timeoutMs and context budget
3. Report result with structured OutputFormat
4. Do NOT: modify GoalState directly, create new subgoals, override scope boundaries
5. Do NOT: select their own routing model (commander selected, KeyPool dispatches)

### 5.3 GLM 5.1 as Lower-Tier Worker

GLM 5.1 (and equivalents: DeepSeek V3, Qwen2.5) are assigned to implementation roles:

- **PLANNER_ASSISTANT**: Decomposes the approved plan into implementation steps
- **EXECUTOR_CODE**: Writes and edits code based on step descriptions
- **FRONTEND_REVIEWER**: Checks UI consistency and UX patterns
- **SECURITY_REVIEWER**: Scans for vulnerabilities and insecure patterns
- **FUNCTIONAL_VERIFIER**: Runs tests and validates behavior matches spec

GLM 5.1-class models should NOT be assigned to:

- **COMMANDER**: Needs stronger reasoning and context tracking
- **RISK_ASSESSOR**: Needs deeper analysis of implications
- **ROUTER**: Needs consistent decision-making across many candidates

### 5.4 Output Contamination Prevention

Subagent outputs must be structured to avoid polluting commander context:

- Use OutputFormat.structured for machine-parseable results (JSON)
- Use OutputFormat.summary for natural language summaries (max 200 tokens)
- Raw tool outputs go to ExecutionLedger, NOT to commander context
- Only consensus results, failure reports, and risk deltas reach the commander

### 5.5 Subagent Communication Protocol

```
subagentReport(taskId, result) {
  // 1. Validate result against verification criteria
  validationResult = verifyAgainstCriteria(result, task.verificationCriteria)

  // 2. Check for scope boundary violation
  if (result.scopeDiff > task.allowedScopeDiff) {
    result.scopeViolation = true
  }

  // 3. Compute risk delta
  riskDelta = assessRiskDelta(task, result)

  // 4. Trim output for commander context
  commanderPayload = {
    taskId: result.taskId,
    status: result.status,
    summary: result.summary, // 200 tokens max
    verification: validationResult,
    riskDelta: riskDelta,
    filesChanged: result.filesChanged,
    recommendedNextAction: result.recommendedNextAction
  }

  // 5. Full result goes to ExecutionLedger
  ledger.recordExecution(taskId, result.fullOutput)

  // 6. Commander receives commanderPayload only
  sendToCommander(taskId, commanderPayload)
}
```

## 6. Model Role Matrix

### 6.1 Role Definitions

```
type AgentRole =
  | "commander"        // Supreme orchestrator — owns plan, delegates everything
  | "planner"          // Creates structured implementation plans
  | "executor"         // Writes/edits code, runs tools, tests
  | "verifier"         // Validates code against spec
  | "summarizer"       // Compresses long outputs, generates reports
  | "router_classifier" // Selects which model to use for which role
  | "frontend_reviewer" // UI consistency, UX, design patterns
  | "security_reviewer" // Vulnerability scanning, auth patterns
  | "risk_assessor"     // Computes per-step risk before delegation
  | "helper"            // Cheap/fast: log scanning, test parsing, trivial classification
```

### 6.2 Tier Assignments (Concrete Model Mapping)

| Role | Tier | Model Class | Cost Target | Context | Token Budget |
|------|------|-------------|-------------|---------|-------------|
| commander | TIER_1 | GLM-5.2-class | $3+/M output | 128K+ | 80K per turn |
| planner | TIER_2 | GLM-5.1-class | $0.5-3/M output | 64K+ | 32K per plan |
| executor | TIER_2 | GLM-5.1-class | $0.5-3/M output | 64K+ | 40K per step |
| verifier | TIER_2 | GLM-5.1-class | $0.5-3/M output | 32K+ | 16K per check |
| summarizer | TIER_2 | GLM-5.1-class | $0.5-3/M output | 32K+ | 8K per summary |
| router_classifier | TIER_2 | GLM-5.1-class | $0.5-3/M output | 8K | 2K per decision |
| frontend_reviewer | TIER_2 | GLM-5.1-class | $0.5-3/M output | 32K | 16K per review |
| security_reviewer | TIER_2 | GLM-5.1-class | $0.5-3/M output | 32K | 16K per review |
| risk_assessor | TIER_2 | GLM-5.1-class | $0.5-3/M output | 16K | 4K per assessment |
| helper | TIER_3 | Flash/MiniMax-Mini | <$0.5/M | 8K | 2K per task |

### 6.3 Role-to-Tier Assignment Rules

- **Tier 1 (commander only)**: One model, always the same strong model, never auto-swapped
- **Tier 2 (subagents)**: Can be dynamically swapped within tier based on slot availability and budget
- **Tier 3 (helpers)**: Always use the cheapest available slot; no fallback to higher tiers

### 6.4 Fallback Within Tier

When a Tier 2 model fails:

1. Try a different slot for the same model (KeyPool fallback)
2. Try a different model within the same tier (GLM-5.1 → Qwen2.5 → DeepSeek V3)
3. Only escalate to commander if all Tier 2 options exhausted

When a Tier 3 helper fails:

1. Try a different slot (KeyPool fallback)
2. Skip this helper task — commander handles it or delegates to Tier 2

### 6.5 Dynamic Tier Adjustment

The commander can upgrade/downgrade based on need:

- Risk assessment shows critical risk → upgrade executor from TIER_2 to TIER_1
- Budget nearing limit → downgrade optional reviews from TIER_2 to TIER_3
- Cache affinity slot shows high latency → try different slot within same tier

## 7. Router and KeyPool Design

### 7.1 Router Decision Flow (Commander-Aware)

```
function selectModelForRole(goal, role, taskContext) {
  // Step 1: Commander decides the role tier
  tier = ModelRole[role].tier

  // Step 2: Build candidates for this tier from KeyPool
  candidates = buildCandidatesByTier(tier, goal.poolConfig)

  // Step 3: Apply KeyPool expansion (multi-key → multi-candidate)
  expandedCandidates = KeyPool.expand(candidates, goal.sessionId)

  // Step 4: Apply hard filters
  passed = applyHardFilters(expandedCandidates, taskContext)

  // Step 5: Score candidates within this tier
  scored = scoreCandidates(passed, taskContext, perRoleWeights[role])

  // Step 6: Select best with KeyPool slot recommendation
  best = selectBest(scored)
  slot = KeyPool.selectKeySlot(expandedCandidates, goal.sessionId, goal.poolMode)

  // Step 7: Record decision
  RouterDecision.log({
    goalId: goal.id,
    role: role,
    tier: tier,
    chosenModel: best.ref,
    chosenSlot: slot.index,
    fallbackChain: scored[1..3].map(s => s.ref),
    reason: best.filterReasons.join("; "),
    confidence: best.score
  })

  return { model: best.ref, slot: slot }
}
```

### 7.2 KeyPool V2: Multi-Channel Dispatch

Building on the V6.4 KeyPool (4 scheduling modes, 39 tests), V2 adds:

### 7.3 Cache Affinity Design

- Bind sessionId → slotIndex in a session-to-slot map
- Binding lasts: until session ends, slot becomes unhealthy, or affinity count exceeds max
- Max affinity per slot: configurable (default 5 sessions per slot)
- When binding is exhausted: LRU eviction, bind next available session

### 7.4 Stream Racing Design

- Commander sends same request to N slots concurrently
- First slot to return first token wins; others are cancelled
- Race parallelism: min(config.raceParallelism, availableSlots.length)
- Cancellation: abort other in-flight requests immediately after winner identified
- Cost impact: N slots × request cost, but only one result used
- Benefit: TTFT reduction from 15s → 7.5s with 2 keys, or ~5s with 3 keys

### 7.5 429/RPM Cooldown Design

- Per-slot counter tracks requests per minute
- 429 response: increment counter, set cooldownUntil = now + backoffTime
- Exponential backoff: 2s → 4s → 8s → 16s → max 5 min
- While slot is cooling: KeyPool.expand excludes this slot from candidates
- Commander never waits for cooldown — it gets next available slot automatically
- Cooldown reset: successful response resets counter, clears cooldown

### 7.6 Fallback Chain Design

Before dispatching any subagent task, commander builds a fallback chain:

```
buildFallbackChain(primaryModel, primarySlot) {
  chain = [primaryModel@primarySlot]

  // Same model, different slot (KeyPool co-keys for same provider)
  alternateSlots = KeyPool.alternateSlotsForModel(primaryModel, exclude=primarySlot)
  for slot in alternateSlots:
    chain.push(primaryModel@slot)

  // Same tier, different model
  tierAlternatives = ModelRole[Tier2Models], exclude primaryModel
  for model in tierAlternatives:
    chain.push(model@KeyPool.selectSlotForModel(model))

  // Escalation: commander handles directly (no auto-fallback beyond this)
  chain.push("commander_manual")

  return chain
}
```

### 7.7 Budget Guard Design

- Per-slot monthly spend tracking (independent monthly budgets per key)
- Pool-level aggregate budget (sum of all slot budgets)
- Budget guard triage:
  - Slot exceeds own limit: remove slot from pool, notify user
  - Pool exceeds aggregate limit: escalate to commander, pause new deployments
  - Commander exceeds personal budget: pause all Tier 1 usage, downgrade to Tier 2

### 7.8 Health Score and Circuit Breaker

Each KeySlot has a health score computed from recent history:

- Successful requests: +1 health (capped at 100)
- 429: -10 health
- 401/403: -50 health (auth failure = slot unusable)
- Timeout (>30s no response): -5 health
- Health < 20: slot enters cooldown (excluded from selection)
- Health < 0: circuit breaker opens (slot disabled until manual reset or 30min timeout)

### 7.9 Provider Difference Handling

Different providers have different characteristics:

- **Anthropic**: prompt caching works best with same model variant + same key affinity
- **OpenAI**: tool calls use different schema; streaming SSE format differs
- **DeepSeek/Qwen**: Chinese providers often have stricter 429 limits, cheaper rates
- **OpenRouter**: gateway adds ~100-300ms latency; routing model must account for this
- **Custom providers**: unknown characteristics → conservative scoring, higher health threshold

## 8. Goal Mode Design for 8-Hour Runs

### 8.1 Problem: Why Existing Goal Modes Fail at 8 Hours

A goal running 8 hours experiences:

- 200+ LLM API calls (commander + subagents + verifiers + summarizers)
- 50+ tool execution events (file writes, test runs, shell commands)
- 3+ context compaction events (each losing some state)
- Multiple provider failures and 429 recoveries
- Scope drift (implementation diverges from approved plan)

Existing 14-phase state machines handle this poorly because:

- Plan state is not compacted — commander loses the original plan after compaction
- Verification results are not aggregated — each verify starts fresh
- Router decisions are not trend-analyzed — the same failing slot is retried repeatedly
- No explicit drift detection — scope breaks are only noticed when verification fails

### 8.2 Goal Mode V2 State Machine Extensions

Adding to the existing 14 phases:

- **compacted**: intermediate phase indicating context was compacted
- **reassessing_risk**: after compaction, risk is recomputed before continuing
- **drift_detected**: scope has diverged from approved plan

### 8.3 Structured Recovery Checkpoints

Every N steps (configurable, default every 5 steps), the goal state machine inserts a checkpoint:

```
function insertRecoveryCheckpoint(goal) {
  checkpoint = {
    stepIndex: goal.currentStepIndex,
    planHash: hash(goal.currentPlan),
    changedFiles: goal.changedFiles,
    verificationStatus: goal.verificationSummary,
    lastRouterDecisions: goal.recentRouterDecisions,
    lastSubagentResults: goal.recentSubagentOutputs,
    totalSpend: goal.totalSpend,
    timestamp: now()
  }
  goal.recoveryCheckpoints.push(checkpoint)

  // Keep only last 20 checkpoints (ring buffer)
  if (goal.recoveryCheckpoints.length > 20) {
    goal.recoveryCheckpoints.shift()
  }
}
```

### 8.4 Drift Detection

Commander continuously monitors implementation against approved plan:

```
function detectDrift(goal, currentImplementation) {
  approvedPlan = goal.approvedPlan
  diff = computeScopeDiff(approvedPlan, currentImplementation)

  criticalFiles = diff.files that are in approvedPlan.scope but not in implementation
  unauthorizedFiles = diff.files in implementation but NOT in approvedPlan.scope

  driftScore = (unauthorizedFiles.length * 3 + criticalFiles.length * 1) / approvedPlan.totalFiles

  return {
    driftScore: driftScore,
    driftDetected: driftScore > 0.3,
    unauthorizedFiles: unauthorizedFiles,
    missingFiles: criticalFiles,
    driftSeverity: driftScore > 0.5 ? "critical" : driftScore > 0.3 ? "warning" : "none"
  }
}
```

### 8.5 Escalation Stop Conditions

Commander halts execution and presents to user:

```
function shouldEscalate(goal, assessment) {
  // Critical risk threshold
  if (assessment.riskLevel === "critical") return true

  // Repeated failure after all fallbacks exhausted
  if (assessment.fallbackChainExhausted) return true

  // Budget exhaustion within 5% of monthly limit
  if (SpendTracker.pctRemaining < 5) return true

  // Scope drift exceeding threshold after 3 attempts to correct
  if (assessment.driftAttempts > 3 && assessment.driftScore > 0.5) return true

  // No progress for >2x step timeout
  if (assessment.stallDuration > goal.stepTimeout * 2) return true

  return false
}
```

### 8.6 Resume Token Design

After compaction, the commander needs to resume from a structured state:

```
ResumeToken {
  compactedFrom: timestamp,
  goalId: string,
  currentPlanHash: string,
  planVersion: number,
  currentStepIndex: number,
  verificationSummary: {
    passed: number,
    failed: number,
    pendingSteps: string[]
  },
  changedFiles: string[],
  recentRouterDecisions: RouterDecision[], // last 10
  recentSubagentOutputs: SubagentResult[], // last 5
  lastRiskAssessment: RiskAssessment,
  recoveryCheckpointIndex: number,
  nextActionHint: string // commander's recommended next step
}
```

### 8.7 Cost Trajectory Management

Over 8 hours, the commander manages cost by:

- Starting with Tier 2 for exploration, Tier 1 only for planning/verification
- Tracking cumulative spend per tier
- Adjusting tier assignments based on budget proximity:
  - >80% budget used: downgrade reviews to helper, skip non-critical verifications
  - >95% budget used: even planner downgraded to summary-only mode
  - Budget hit: escalate immediately, do not silently truncate

## 9. Context Compaction and Recovery

### 9.1 The Compaction Problem

In an 8-hour run, commander context fills with:

- Full conversation history with every subagent
- Tool call chains from hundreds of file operations
- Error logs and retry traces from failed 429s
- Intermediate drafts and abandoned planning branches
- Verification reports from every step

Compaction must preserve what matters and discard what doesn't.

### 9.2 Tournament Compaction Strategy

The commander does not discard context — it compacts it into a Tournament state:

```
TournamentState {
  concept: string;        // original user goal (preserved verbatim)
  phase: number;          // current phase index
  round: number;          // which compaction round we're in (1st, 2nd, 3rd...)
  elites: Scorecard[];    // top N candidates from last round
  scorecards: Scorecard[]; // all surviving candidates with full scores
  champion?: Scorecard;   // winner of last round (if completed)
  loser_pile: Scorecard[]; // eliminated candidates (for rollback/debug)
  context: ExecutionContext; // compacted context (crucially NOT full messages)
}

Scorecard {
  id: string;             // unique identifier (used for rollback)
  label: string;          // human-readable label ("planner-v2", "executor-v3-iteration2")
  role: AgentRole;
  tier: ModelTier;
  modelRef: ModelRef;
  modelProvider: string;
  score: number;          // composite score (correctness + cost + latency)
  scores: BreakdownScores; // individual dimension scores
  verdict: "pass" | "fail" | "pending" | "partial";
  rationale: string;      // why this scorecard exists
  artifacts: Artifact[];  // links to generated code/files
  parentId?: string;      // parent scorecard ID (for hierarchy tracking)
  metadata: {
    createdAt: number;
    completedAt?: number;
    tokensIn: number;
    tokensOut: number;
    costUSD: number;
    latencyMs: number;
    toolCalls: ToolCall[];
  };
}
```

### 9.3 Compaction Algorithm

```
function compactCommanderContext(goal, trigger) {
  // 1. Build Tournament state from current conversation
  tournament = {
    concept: goal.originalGoal,
    phase: goal.currentPhase,
    round: goal.compactionCount + 1,
    scorecards: [],
    context: extractExecutionContext(goal)
  }

  // 2. Create scorecards for each subagent result
  for each subagentResult in goal.recentSubagentOutputs {
    scorecard = {
      id: genId(),
      label: `${subagentResult.role}-v${subagentResult.version}`,
      role: subagentResult.role,
      tier: subagentResult.tier,
      modelRef: subagentResult.modelRef,
      score: subagentResult.compositeScore,
      verdict: subagentResult.verdict,
      rationale: subagentResult.summary,
      artifacts: subagentResult.filesChanged,
      parentId: subagentResult.parentTaskId,
      metadata: {
        createdAt: subagentResult.startTime,
        completedAt: subagentResult.endTime,
        tokensIn: subagentResult.tokensIn,
        tokensOut: subagentResult.tokensOut,
        costUSD: subagentResult.costEstimate,
        latencyMs: subagentResult.latencyMs,
        toolCalls: subagentResult.toolCalls
      }
    }
    tournament.scorecards.push(scorecard)
  }

  // 3. Identify elites (top performers by composite score)
  tournament.elites = selectElites(tournament.scorecards, topN=5)

  // 4. Identify failures/pending work
  pendingWork = identifyPendingWork(tournament)

  // 5. Store full conversation state before compacting
  goal.lastFullState = serializeFullGoalState(goal)
  goal.tournamentState = tournament
  goal.compactionCount++

  // 6. Compact: keep only Tournament state + elites summary + pending work
  goal.conversationHistory = tournament.elites.map(e => e.summary) + pendingWork.map(w => w.description)
  goal.toolCallHistory = [] // all gone — artifacts contain file references
  goal.errorHistory = [] // errors summarized in scorecard rationales

  return tournament
}
```

### 9.4 Context Rollback (Tournament Re-ingestion)

If compaction loses critical information:

```
function rollbackCompaction(goal, checkpointIndex) {
  if (checkpointIndex >= goal.lastFullState) {
    // Full rollback: restore entire conversation
    goal = deserializeFullGoalState(goal.lastFullState)
  } else if (checkpointIndex >= goal.lastTournamentState) {
    // Tournament-level rollback: restore from scorecards
    tournament = goal.tournamentStates[checkpointIndex]

    // Reconstruct conversation from scorecards
    for scorecard in tournament.scorecards {
      if (scorecard.verdict === "pass") {
        restoreFromArtifacts(scorecard.artifacts)
      }
    }

    // Re-inject any missing context manually
    goal.conversationHistory = tournament.elites.map(e => reconstructMessage(e))
    goal.rollbackContext = {
      rolledBackFrom: tournament.round,
      revivedScorecards: tournament.scorecards.map(s => s.id),
      reason: "context loss detected"
    }
  }
}
```

### 9.5 Context Budget Management

The commander tracks a strict context budget:

```
ContextBudget {
  maxTotalTokens: 128K,    // for commander context window
  reservedForPlan: 16K,    // always keep plan in context
  reservedForVerification: 8K, // keep current verification state
  reservedForCommander: 32K,   // commander's own working memory

  // Compaction triggers:
  softTrigger: 80% of max,  // begin tournament compaction
  hardTrigger: 95% of max,  // force compaction
  emergencyTrigger: 100%,   // emergency: truncate oldest non-essential messages
}
```

## 10. Skills/Plugins/MCP Productization

### 10.1 Commander as Skill Activator

The commander decides which skills, plugins, and MCP tools to activate for each task. This is NOT delegated to subagents — the commander evaluates:

- Does this task require a specific skill domain (e.g., React, database migration)?
- Are there MCP tools that could accelerate this task (e.g., SWE-bench for validation)?
- Which plugins provide verified, tested capabilities vs. experimental ones?

### 10.2 Skill Graph Design

Skills form a directed graph of dependencies:

```
Skill {
  id: string;
  name: string;
  version: string;
  description: string;
  category: SkillCategory;    // "frontend", "backend", "database", etc.
  tier: SkillReliabilityTier; // "verified", "tested", "experimental"
  entryPoints: EntryPoint[];
  requiredContext: ContextRequirement;
  produces: OutputSpec;
  sideEffects: string[];      // what it modifies (for commander awareness)
  dependencies: string[];     // other skills it requires
  author: string;
  verifiedAt: number;         // when it passed verification
}

EntryPoint {
  name: string;
  inputSchema: JSONSchema;
  outputSchema: JSONSchema;
  costEstimate: CostEstimate;
  timeoutMs: number;
  fallbackBehavior: "retry" | "skip" | "alternative";
}
```

### 10.3 Commander Activation Protocol

```
function commanderActivateSkill(commanderState, task) {
  // 1. Commander evaluates task against skill graph
  relevantSkills = SkillGraph.query(task.description, task.role, task.scope)

  // 2. Filter by tier reliability
  verifiedSkills = relevantSkills.filter(s => s.tier === "verified")
  testedSkills = relevantSkills.filter(s => s.tier === "tested")

  // 3. Select primary (verified) and backup (tested) skills
  preferredSkill = verifiedSkills[0] || testedSkills[0] || null
  if (!preferredSkill) {
    commanderState.log("No matching skill — Commander will handle directly")
    return { delegated: false }
  }

  // 4. Check prerequisite context availability
  if (!commanderState.hasContext(preferredSkill.requiredContext)) {
    contextTask = { role: "helper", task: "gather " + preferredSkill.requiredContext }
    return { delegated: false, preTask: contextTask }
  }

  // 5. Activate skill — attach entry point to subagent task
  subagentTask = {
    role: task.role,
    description: task.description,
    skillEntryPoint: preferredSkill.entryPoints[0],
    expectedOutput: preferredSkill.produces,
    timeoutMs: preferredSkill.timeoutMs,
    fallback: "skip" // if skill fails, skip it, don't retry
  }

  return { delegated: true, subagentTask: subagentTask }
}
```

### 10.4 Plugin Architecture

Plugins extend the core system with new capabilities:

- **Router plugins**: Custom routing strategies beyond the 8-dimension scoring
- **Provider plugins**: New provider integrations (extend KeyPool)
- **Verification plugins**: Custom verification logic per project type
- **Compaction plugins**: Custom compaction strategies per goal type
- **UI plugins**: Custom dashboard panels for specific workflows

### 10.5 MCP Integration

MCP (Model Context Protocol) provides standardized tool interfaces:

- Commander loads MCP server list at goal start
- MCP tools are categorized and matched to subagent tasks
- MCP servers must pass a capability verification before commander delegates to them
- MCP tool failures trigger specific fallback behaviors (not generic retry)

### 10.6 Skill Verification

Skills entering the verified tier must pass:

1. Automated test suite (at least 3 test cases per entry point)
2. Cost benchmark (doesn't exceed estimated cost by >50%)
3. Reliability test (succeeds >90% of expected task types)
4. Security audit (no unauthorized file access, no data exfiltration)

## 11. UI Information Architecture

### 11.1 Session Layer (What the user sees during execution)

```
[Goal Status Bar — 32px, fixed above composer]
Status: executing | Phase: implementing | Mode: guided | Timer: 02:31

[Open Panel]
| User input area with Commander suggestions |
| Session Messages |
| Subagent results, verification outputs, commander decisions |
```

### 11.2 Commander Panel (Right sidebar)

The commander panel replaces the generic goal panel with commander-specific views:

- **Commander View**: Show current plan, risk assessment, delegation queue
- **Subagent View**: Active delegations with model, tier, status, progress
- **KeyPool View**: Live slot status (which keys racing, health scores, 429 counts)
- **Router View**: Recent routing decisions with rationale (not just "chosen model X")
- **Ledger View**: Execution ledger with filterable task/role/model/slot columns
- **Budget View**: Per-tier spending, per-slot monthly usage, pool aggregate
- **Drift View**: Scope comparison visualization (approved plan vs. current implementation)
- **Action View**: Available actions from commander (escalate, re-plan, compact, revert)

### 11.3 KeyPool Dashboard Design

```
[Slot Overview]
┌─────────────────────────────────────────────────────────┐
│ Slot 0 (GLM-5.2 bind) │ Slot 1 (GLM-5.1 race) │ Slot 2 │
│ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │ ─ ─ ─ │
│ Health: 85 │ Health: 72 │ Health: 0 │ ← circuit open │
│ Requests/min: 12/60 │ Requests/min: 8/60 │ Requests/min: N/A │
│ 429 cooldown: none │ 429 cooldown: 2s left │ 429 cooldown: 30m │
│ Cache affinity: Goal#7│ Cache affinity: none │ Cache affinity: N/A │
│ Monthly spend: $2.40 │ Monthly spend: $1.85 │ Monthly spend: N/A │
│ Last success: 2s ago │ Last success: 5s ago │ Last success: N/A │
│ Availability: active │ Availability: racing │ Availability: disabled │
└─────────────────────────────────────────────────────────┘

[Pool Summary]
Total RPM capacity: 120 (60 per slot × 2 active slots)
Total monthly budget: $50.00 (remaining $45.75)
Fallback chain: GLM-5.2@slot0 → GLM-5.2@slot1 → GLM-5.1@slot0 → commander_manual

[RPM Usage Chart]
Minute-by-minute chart showing requests per slot, total capacity line, and 429 events
```

### 11.4 Settings Page Architecture

KeyPool-specific settings (beyond existing domestic-gateway):

- **Pool configuration**: Race parallelism, max affinity per slot, cooldown backoff max
- **Budget allocation**: Per-slot monthly limits, pool aggregate limit, controller budget
- **Affinity strategy**: Cache affinity binding duration, LRU eviction threshold
- **Health thresholds**: Circuit breaker health floor, recovery health minimum
- **Cost rules**: When to downgrade tiers, when to pause, daily/weekly cost caps

### 11.5 Commander Settings

- **Default commander model**: Selectable from ModelRef pool (GLM-5.2-class requirement)
- **Escalation preferences**: When to pause for user, when to force-stop
- **Subagent assignment rules**: Per-role tier defaults, custom overrides
- **Verification intensity**: How thorough should reviews be (affects token budget)
- **Drift tolerance**: How much scope deviation to allow before escalating

## 12. Data Models

### 12.1 Core Types

```
GoalState {
  id: string;
  description: string;
  sessionId: string;
  phase: GoalPhase;
  mode: GoalMode;
  currentStepIndex: number;
  steps: GoalStep[];
  approvedPlan: Plan;
  changedFiles: string[];
  verificationSummary: VerificationSummary;
  totalSpend: number;
  totalTokensIn: number;
  totalTokensOut: number;
  driftStatus: DriftStatus;
  riskLevel: RiskLevel;
  compactionCount: number;
  recoveryCheckpoints: RecoveryCheckpoint[];
  lastFullState?: string;           // serialized state before compaction
  tournamentState?: TournamentState;
  rollbackContext?: RollbackContext;
  commanderLog: CommanderDecision[];
  subagentLog: SubagentResult[];
  stallDetections: StallDetection[];
  createdAt: number;
  updatedAt: number;
  terminalAt?: number;
}

CommanderState {
  goalId: string;
  modelProfile: ModelProfile;
  activeDelegations: SubagentTask[];
  completedDelegations: SubagentTaskResult[];
  delegationBudget: { total: number; used: number; remaining: number };
  lastRiskAssessment: RiskAssessment;
  riskTrend: "stable" | "rising" | "critical";
  contextStrategy: "full" | "compacted" | "tournament";
  activeKeySlots: KeySlot[];
  failCount: number;
  escalationReason?: string;
  commanderModel: ModelRef;
  roleAssignments: Map<AgentRole, ModelTier>;
}

SubagentTask {
  id: string;
  parentGoalId: string;
  parentDelegationId?: string;
  role: AgentRole;
  tier: ModelTier;
  description: string;
  context: {
    allowedFiles: string[];
    scopeBoundary: string[];
    verificationCriteria: string[]
  };
  expectedOutputFormat: OutputFormat;
  timeoutMs: number;
  fallbackPolicy: FallbackPolicy;
  skillEntryPoint?: EntryPoint;
  modelOverride?: ModelRef;
  createdAt: number;
  completedAt?: number;
}

SubagentTaskResult {
  taskId: string;
  status: "success" | "failure" | "partial" | "timeout";
  summary: string;          // 200 tokens max — what commander sees
  fullOutput: string;       // goes to ExecutionLedger
  verification: VerificationResult;
  riskDelta: RiskDelta;
  scopeViolation: boolean;
  filesChanged: string[];
  tokensIn: number;
  tokensOut: number;
  costEstimate: number;
  latencyMs: number;
  modelUsed: ModelRef;
  slotUsed: KeySlotIndex;
  toolCalls: ToolCallSummary[];
  recommendedNextAction: string;
  failureReason?: string;
  completedAt: number;
}

ModelRole {
  role: AgentRole;
  tier: ModelTier;
  defaultModel: ModelRef;
  fallbackModels: ModelRef[];
  tokenBudget: number;
  contextRequirement: number;
  toolSupport: boolean;
  verificationDepth: "minimal" | "standard" | "thorough";
  costTarget: CostTarget;
}

ModelTier = "tier_1_commander" | "tier_2_subagent" | "tier_3_helper";

ModelProfile {
  ref: ModelRef;
  tier: ModelTier;
  capabilities: Capabilities;
  costInfo: CostInfo;
  health: ProviderHealth;
  avgFirstTokenMs: number;
  avgTotalLatencyMs: number;
  supportedRoles: AgentRole[];
  channelCapacity: number;  // how many concurrent requests this channel can handle
}

ProviderChannel {
  providerId: string;
  channelId: string;        // "api.deepseek.com", "openrouter", "opencode-go"
  health: ProviderHealth;
  avgLatencyMs: number;
  authenticated: boolean;
}

KeyPool {
  providerId: string;
  slots: KeySlot[];
  mode: KeyPoolMode;
  config: {
    raceParallelism: number;
    maxAffinityPerSlot: number;
    cooldownMaxMs: number;
  };
  poolStats: PoolStats;
}

KeyPoolMode = "cache_affinity" | "stream_race" | "round_robin" | "cost_optimal";

KeySlot {
  index: number;
  keyRef: string;
  health: number;           // 0-100 score
  rpmUsed: number;
  rpmLimit: number;
  cooldownUntil?: number;
  monthlySpend: number;
  monthlyLimit: number;
  affinitySessions: Map<string, number>;
  stats: SlotStats;
  circuitOpen: boolean;
}

KeySlotIndex = number;

RateLimitState {
  slotIndex: number;
  cooldownUntil: number;
  failCount: number;
  backoffMs: number;
  consecutive429: number;
  max429BeforeCircuit: number;
}

ExecutionLedger {
  entries: ExecutionEntry[];
  maxEntries: number;       // ring buffer
}

ExecutionEntry {
  id: string;
  taskId: string;
  goalId: string;
  role: AgentRole;
  tier: ModelTier;
  modelUsed: ModelRef;
  slotUsed: number;
  status: ExecutionStatus;
  tokensIn: number;
  tokensOut: number;
  costUSD: number;
  latencyMs: number;
  firstTokenMs: number;
  errorCode?: string;
  errorMessage?: string;
  toolCalls: ToolCallID[];
  fallbackUsed: boolean;
  fallbackReason?: string;
  verificationResult?: VerificationResult;
  startedAt: number;
  completedAt: number;
}

VerificationResult {
  passed: boolean;
  score: number;           // 0-1
  checks: VerificationCheck[];
  summary: string;
  criticalIssues: string[];
  warningIssues: string[];
  completedAt: number;
}

VerificationCheck {
  type: VerificationCheckType;
  passed: boolean;
  detail: string;
}

VerificationCheckType = "syntax" | "types" | "tests" | "security" | "scope" | "standards";

RouterDecision {
  id: string;
  goalId: string;
  taskId: string;
  role: AgentRole;
  chosenModel: ModelRef;
  chosenSlot: KeySlotIndex;
  tier: ModelTier;
  fallbackChain: ModelRef[];
  reason: string;
  confidence: number;
  alternatives: AlternativeModel[];
  alternativesRejected: RejectedAlternative[];
  decidedAt: number;
}

RiskAssessment {
  score: number;           // 0-1, higher = riskier
  level: RiskLevel;
  factors: RiskFactor[];
  mitigation: string[];
  escalated: boolean;
  assessedAt: number;
}

RiskFactor = "scope_expansion" | "security" | "breaking_change" | "test_coverage" | "cost" | "complexity";
RiskLevel = "low" | "medium" | "high" | "critical";
```

### 12.2 State Relationship Diagram

```
GoalState
├── CommanderState (per-goal commander state)
│   ├── activeDelegations → SubagentTask[]
│   ├── completedDelegations → SubagentTaskResult[]
│   └── commanderModel → ModelRef
├── KeyPool (attached to goal via sessionId)
│   ├── slots → KeySlot[]
│   └── slot.index → RateLimitState
└── ExecutionLedger (shared across goals, scoped by sessionId)
    ├── entries → ExecutionEntry[]
    └── entry.taskId → SubagentTaskResult

RouterDecision (emitted per subagent dispatch, linked to goalId)
├── chosenModel → ModelRef
├── chosenSlot → KeySlotIndex
└── fallbackChain → ModelRef[]

VerificationResult (attached to each SubagentTaskResult)
├── checks → VerificationCheck[]
├── criticalIssues → string[]
└── warningIssues → string[]
```

## 13. Algorithm Pseudocode

### 13.1 commanderLoop

```
function commanderLoop(goal: GoalState, commanderState: CommanderState) {
  while (!isTerminal(goal.phase)) {
    // 1. Risk assessment before any delegation
    risk = riskAssessor.evaluate(goal, commanderState.roleAssignments)
    commanderState.lastRiskAssessment = risk
    if (risk.level === "critical") {
      escalateToUser(goal, risk)
      goal.phase = "paused"
      break
    }

    // 2. Select next task based on plan and risk
    nextTask = commanderState.approvedPlan.steps[goal.currentStepIndex]

    // 3. Select subagent model and slot
    modelSlot = selectModelForRole(goal, nextTask.role, nextTask.context)

    // 4. Build fallback chain BEFORE dispatching
    fallbackChain = buildFallbackChain(modelSlot.model, modelSlot.slot)

    // 5. Create delegation
    delegation = SubagentTask {
      id: genId(),
      parentGoalId: goal.id,
      role: nextTask.role,
      tier: ModelRole[nextTask.role].tier,
      description: nextTask.description,
      context: nextTask.context,
      fallbackPolicy: FallbackPolicy {
        chain: fallbackChain,
        maxRetries: 3
      },
      modelOverride: modelSlot.model,
      slotOverride: modelSlot.slot,
      timeoutMs: nextTask.timeoutMs
    }
    commanderState.activeDelegations.push(delegation)
    recordRouterDecision(goal, delegation, modelSlot, fallbackChain)

    // 6. Execute with fallback
    result = executeWithFallback(delegation, fallbackChain)

    // 7. Remove from active, add to completed
    commanderState.activeDelegations = filterOut(commanderState.activeDelegations, delegation.id)
    commanderState.completedDelegations.push(result)

    // 8. Process result
    if (result.status === "success") {
      mergeIntoGoalState(goal, result)
      goal.currentStepIndex++
    } else if (result.status === "partial") {
      handled = handlePartialResult(goal, result)
      if (!handled) escalateToUser(goal, risk)
    } else {
      if (fallbackChain.hasMore && fallbackChain.retriesRemaining > 0) {
        nextTask = adjustTaskForFallback(nextTask, result)
        goal.currentStepIndex-- // retry same step with fallback model
      } else {
        escalateToUser(goal, risk)
      }
    }
  }
}
```

### 13.2 assignSubagent

```
function assignSubagent(commanderState: CommanderState, task: SubagentTask) {
  // 1. Validate task against commander context budget
  contextBudget = estimateContextBudget(task)
  if (commanderState.contextRemaining < contextBudget) {
    compactContext(commanderState)
  }

  // 2. Select model for role using router
  routerDecision = selectModelForRole(
    goal = commanderState.currentGoal,
    role = task.role,
    taskContext = task.context
  )

  // 3. Select slot from KeyPool
  slot = KeyPool.selectKeySlot(
    candidates = routerDecision.candidates,
    sessionId = commanderState.currentGoal.sessionId,
    mode = commanderState.currentGoal.poolConfig.mode
  )

  // 4. Build the actual LLM call configuration
  callConfig = {
    model: routerDecision.model,
    slot: slot,
    timeout: task.timeoutMs,
    maxTokens: ModelRole[task.role].tokenBudget,
    systemPrompt: buildSystemPrompt(task.role, task.tier),
    context: buildSubagentContext(task),  // only what subagent needs
    tools: ModelRole[task.role].toolSupport ? task.tools : [],
    stream: true  // subagents always stream for first-token latency
  }

  // 5. For stream_race mode: prepare parallel calls
  if (commanderState.currentGoal.poolConfig.mode === "stream_race") {
    raceSlots = KeyPool.selectRaceSlots(slot, parallelism = 2)
    callConfig.raceSlots = raceSlots
    callConfig.stream = true
  }

  // 6. Record and dispatch
  dispatchRecord = {
    taskId: task.id,
    model: callConfig.model,
    slot: callConfig.slot,
    fallbackChain: routerDecision.fallbackChain,
    dispatchedAt: now()
  }
  executeLLMCall(callConfig)

  return dispatchRecord
}
```

### 13.3 selectModelForRole

```
function selectModelForRole(goal: GoalState, role: AgentRole, context: TaskContext) {
  // Step 1: Get role's tier and model assignments
  roleConfig = ModelRole[role]
  tier = roleConfig.tier

  // Step 2: Query available models for this tier from KeyPool
  candidates = queryModelsByTier(tier, goal.sessionId)

  // Step 3: Expand via KeyPool (multi-key → multi-candidate with slot variants)
  expandedCandidates = KeyPool.expand(candidates, goal.sessionId)

  // Step 4: Apply hard filters (16 conditions)
  filtered = applyHardFilters(expandedCandidates, context, roleConfig)

  // Step 5: If above threshold, apply LLM router for nuanced selection
  if (filtered.passed.length > 1 && shouldUseLLMRouter(role, context.complexity)) {
    llmRouterResult = callLLMRouter(role, context, filtered.passed)
    chosen = filterRemaining(llmRouterResult)
    confidence = llmRouterResult.confidence
  } else {
    // Step 6: Score and select (deterministic, no LLM call)
    scored = scoreCandidates(filtered.passed, context, perRoleWeights[role])
    chosen = selectBest(scored)
    confidence = chosen.score
  }

  // Step 7: Select KeySlot (with cache affinity / stream race / round robin)
  slot = KeyPool.selectKeySlot(expandedCandidates, goal.sessionId, goal.poolConfig.mode)

  // Step 8: Record decision
  decision = RouterDecision {
    goalId = goal.id,
    role = role,
    tier = tier,
    chosenModel = chosen.ref,
    chosenSlot = slot.index,
    fallbackChain = scored[1..3].map(s => s.ref),
    reason = chosen.filterReasons,
    confidence = confidence,
    decidedAt = now()
  }

  // Step 9: Update pool stats
  KeyPool.recordSlotUsage(slot.index, decision)

  return { model: chosen.ref, slot: slot, decision: decision, fallbackChain: fallbackChain }
}
```

### 13.4 selectKeySlot

```
function selectKeySlot(candidates: RouteCandidate[], sessionId: string, mode: KeyPoolMode) {
  availableSlots = candidates.filter(c => c.health.circuitOpen !== true)

  if (mode === "cache_affinity") {
    // Default mode: bind session to slot
    existingBinding = affinityMap.get(sessionId)
    if (existingBinding) {
      slot = existingBinding
      if (!checkSlotHealth(slot) || slot.affinitySessions.size > config.maxAffinityPerSlot) {
        // Slot unhealthy or full — rebind
        slot = selectNewSlotForAffinity(availableSlots)
        affinityMap.set(sessionId, slot)
      }
    } else {
      slot = selectNewSlotForAffinity(availableSlots)
      affinityMap.set(sessionId, slot)
    }
    return { primarySlot: slot, raceSlots: [] }
  }

  else if (mode === "stream_race") {
    parallelism = min(config.raceParallelism, availableSlots.length)
    selected = selectRaceCandidates(availableSlots, parallelism)
    return { primarySlot: selected[0], raceSlots: selected[1..parallelism-1] }
  }

  else if (mode === "round_robin") {
    globalCounter++
    index = globalCounter % availableSlots.length
    return { primarySlot: availableSlots[index], raceSlots: [] }
  }

  else { // cost_optimal
    existingBinding = affinityMap.get(sessionId)
    if (existingBinding && checkSlotHealth(existingBinding)) {
      return { primarySlot: existingBinding, raceSlots: [] }
    }
    sorted = availableSlots.sort(s => s.monthlySpend)
    return { primarySlot: sorted[0], raceSlots: [] }
  }
}
```

### 13.5 handle429

```
function handle429(slot: KeySlot, request: Request) {
  // 1. Record the 429
  slot.failCount429++
  slot.consecutive429++
  slot.health = max(0, slot.health - 10)

  // 2. Calculate exponential cooldown
  backoff = 2000 * (2 ^ (slot.consecutive429 - 1))  // 2s, 4s, 8s, 16s...
  slot.cooldownUntil = now() + min(backoff, config.cooldownMaxMs) // max 5 min

  // 3. If consecutive 429s exceed threshold, open circuit breaker
  if (slot.consecutive429 >= config.max429BeforeCircuit) {
    slot.circuitOpen = true
    slot.circuitOpenUntil = now() + 1800000 // 30 min cooldown before retry
    notifyCommander("Circuit breaker opened for slot " + slot.index, "warning")
  }

  // 4. Immediately fallback to alternate slot for the current request
  fallbackSlot = KeyPool.findBestFallbackSlot(slot.index)
  if (fallbackSlot) {
    retryRequest(request, fallbackSlot)
    recordRetry("slot_fallback", slot.index, fallbackSlot.index)
  } else {
    // No fallback available — escalate to commander
    notifyCommander("No available slot for role " + request.role + " after 429", "critical")
    escalateToUser("No model slot available — all keys in cooldown or circuit open")
  }
}

function recordSlotSuccess(slot: KeySlot) {
  slot.consecutive429 = 0
  slot.health = min(100, slot.health + 1)

  // Remove cooldown if health recovers
  if (slot.health >= config.healthRecoveryThreshold && slot.cooldownUntil) {
    slot.cooldownUntil = undefined
  }

  // Circuit breaker recovery
  if (slot.circuitOpen && now() > slot.circuitOpenUntil) {
    slot.circuitOpen = false
    slot.circuitOpenUntil = undefined
    slot.consecutive429 = 0
    slot.health = 50  // Start conservative
  }

  // Update affinity if applicable
  if (slot.affinityMap) {
    for session in slot.affinityMap {
      slot.affinityMap[session] = now()
    }
  }
}
```

### 13.6 buildFallbackChain

```
function buildFallbackChain(primaryModel: ModelRef, primarySlot: KeySlotIndex): FallbackChain {
  chain = []

  // Tier 1: Same model, different slot (KeyPool co-keys for same provider)
  sameModelSlots = KeyPool.findSlotsForModel(primaryModel, exclude = [primarySlot])
  for slot in sameModelSlots {
    if (slot.health >= config.healthFloor && !slot.circuitOpen) {
      chain.push({ model: primaryModel, slot: slot.index, reason: "same model, alternate slot" })
    }
  }

  // Tier 2: Same tier, different model
  roleConfig = ModelRole[currentRole]
  for fallbackModel in roleConfig.fallbackModels {
    if (fallbackModel !== primaryModel) {
      slot = KeyPool.selectSlotForModel(fallbackModel)
      if (slot && !slot.circuitOpen && slot.health >= config.healthFloor) {
        chain.push({ model: fallbackModel, slot: slot.index, reason: "tier alternate model" })
      }
    }
  }

  // Tier 3: Commander handles directly (last resort)
  chain.push({ model: "commander_direct", slot: undefined, reason: "commander escalation" })

  return {
    chain: chain,
    currentIndex: 0,
    maxRetries: 3,
    shouldEscalateToUser: false,
    escalationReason: undefined
  }
}
```

### 13.7 streamRace

```
function streamRace(request: LLMRequest, slots: KeySlot[], parallelism: number) {
  // 1. Prepare identical requests for each slot
  raceRequests = slots.map(slot => {
    request.slot = slot.index
    request.modelSlot = slot.index
    return { request: request, slot: slot, state: "pending" }
  })

  // 2. Send all requests concurrently (streaming mode)
  for raceReq in raceRequests {
    raceReq.state = "running"
    raceReq.stream = openStream(raceReq.request)
    raceReq.firstTokenReceived = false
    raceReq.firstTokenTime = undefined
    raceReq.dispatchedAt = now()
  }

  // 3. Wait for first token arrival
  winner = undefined
  while (!winner) {
    for raceReq in raceRequests {
      if (raceReq.state === "running" && !raceReq.firstTokenReceived) {
        token = raceReq.stream.nextToken()
        if (token) {
          raceReq.firstTokenReceived = true
          raceReq.firstTokenTime = now() - raceReq.dispatchedAt
          winner = raceReq
          break
        }
      }
    }

    // Check for timeout
    if (now() - raceRequests[0].dispatchedAt > request.timeoutMs) {
      winner = raceRequests.find(r => r.state !== "failed")
      if (!winner) throw Error("All race slots timed out")
      break
    }
  }

  // 4. Cancel all other in-flight requests immediately
  for raceReq in raceRequests {
    if (raceReq !== winner) {
      raceReq.state = "cancelled"
      raceReq.stream.abort()
    }
  }

  // 5. Continue with winner's stream
  winner.state = "winner"
  winnerResult = collectRemainingTokens(winner.stream)

  // 6. Record all slot results for stats
  for raceReq in raceRequests {
    KeyPool.recordSlotResult(raceReq.slot.index, raceReq.state, raceReq.firstTokenTime)
  }

  return {
    model: winner.request.model,
    slot: winner.slot.index,
    tokens: winnerResult.tokens,
    latencyMs: winner.firstTokenTime,
    fullLatencyMs: now() - winner.dispatchedAt,
    raceParticipants: raceRequests.length,
    losers: raceRequests.filter(r => r !== winner).map(r => r.slot.index)
  }
}
```

### 13.8 mergeSubagentResults

```
function mergeSubagentResults(commanderState: CommanderState, results: SubagentTaskResult[]) {
  // 1. Group results by role and tier
  groupedResults = groupBy(results, r => (r.role, r.tier))

  // 2. For each group, apply role-specific merge strategy
  merged = {}
  for (role, group in groupedResults) {
    if (role === "verifier" || role === "security_reviewer") {
      // Review roles: take the more conservative verdict
      merged[role] = conservativeMerge(group)
    } else if (role === "summarizer") {
      // Summarizer: take the most detailed summary that doesn't exceed budget
      merged[role] = largestSummaryWithinBudget(group, maxTokens = 200)
    } else if (role === "executor") {
      // Executor: merge file changes, prefer latest version
      merged[role] = mergeExecutions(group)
    } else {
      // Default: take the highest-scored result
      merged[role] = highestScore(group)
    }
  }

  // 3. Detect conflicts between merged results
  conflicts = detectConflicts(merged)
  if (conflicts.length > 0) {
    // Commander resolves conflicts
    for conflict in conflicts {
      resolution = commanderResolveConflict(conflict, commanderState.lastRiskAssessment)
      merged[conflict.domain] = resolution.resolvedValue
      commanderState.conflictLog.push({
        conflict: conflict,
        resolution: resolution,
        resolvedAt: now()
      })
    }
  }

  // 4. Return merged result for goal state update
  return merged
}

function conservativeMerge(verificationResults: VerificationResult[]): VerificationResult {
  mostStrict = verificationResults.reduce((a, b) => {
    if (a.verdict === "fail" || b.verdict === "fail") return { verdict: "fail", issues: [...a.issues, ...b.issues] }
    if (a.score < b.score) return a
    return b
  })

  return VerificationResult {
    passed: mostStrict.verdict === "pass",
    score: min(...verificationResults.map(r => r.score)),
    checks: mergeChecks(verificationResults),
    criticalIssues: unique([...verificationResults.flatMap(r => r.criticalIssues)]),
    warningIssues: unique([...verificationResults.flatMap(r => r.warningIssues)]),
    summary: "Merged from " + verificationResults.length + " verifications"
  }
}

function mergeExecutions(executionResults: SubagentTaskResult[]): ExecutorMerge {
  fileChanges = {}
  for result in executionResults {
    for change in result.filesChanged {
      if (!fileChanges[change.path] || change.timestamp > fileChanges[change.path].timestamp) {
        fileChanges[change.path] = change
      }
    }
  }
  return ExecutorMerge {
    filesChanged: Object.values(fileChanges),
    conflicts: detectFileConflicts(executionResults),
    recommendedResolution: recommendResolution(conflicts)
  }
}
```

### 13.9 compactAndResume

```
function compactAndResume(goal: GoalState, commanderState: CommanderState) {
  // 1. Build Tournament state
  tournament = buildTournamentState(goal, commanderState)
  elites = selectElites(tournament.scorecards, topN = 5)
  pendingWork = identifyPendingWork(tournament)

  // 2. Serialize full state for potential rollback
  fullState = serializeGoalState(goal)
  commanderState.lastFullState = fullState
  goal.tournamentState = tournament
  goal.compactionCount++

  // 3. Compact goal state
  goal.conversationHistory = constructCompactContext(elites, pendingWork)
  goal.toolCallHistory = extractArtifactReferences(tournament)
  goal.errorHistory = summarizeErrors(tournament)

  // 4. Build resume token for commander
  resumeToken = ResumeToken {
    compactedFrom: now(),
    goalId: goal.id,
    currentPlanHash: hash(goal.approvedPlan),
    planVersion: goal.approvedPlan.version,
    currentStepIndex: goal.currentStepIndex,
    verificationSummary: goal.verificationSummary,
    changedFiles: goal.changedFiles,
    lastRouterDecisions: goal.commanderLog.slice(-10),
    lastSubagentOutputs: goal.subagentLog.slice(-5),
    lastRiskAssessment: commanderState.lastRiskAssessment,
    recoveryCheckpointIndex: goal.recoveryCheckpoints.length - 1,
    nextActionHint: suggestNextAction(goal, elites, pendingWork)
  }

  // 5. Rebuild commander context from resume token
  rebuildCommanderContext(commanderState, resumeToken)

  return commanderLoop(goal, commanderState)
}

function compactContext(commanderState: CommanderState) {
  if (commanderState.contextStrategy === "full") {
    tournament = buildTournamentState(commanderState.currentGoal, commanderState)
    commanderState.contextStrategy = "tournament"
    return tournament
  } else if (commanderState.contextStrategy === "tournament") {
    tournament = commanderState.currentGoal.tournamentState
    tournament.round++
    tournament.elites = tournament.scorecards
      .filter(s => s.verdict === "pass")
      .sort(s => -s.score)
      .slice(0, 3) // keep only top 3
    tournament.conversationHistory = tournament.elites.map(e => e.summary)
    return tournament
  }
}
```

### 13.10 Resume Token Reconstruction

```
function reconstructFromResumeToken(resumeToken: ResumeToken, goal: GoalState) {
  tournament = goal.tournamentState

  // Re-play elite scorecards as conversation summary
  conversation = tournament.elites.map(elite => reconstructMessage(elite))

  // Add pending work descriptions
  pendingWork = resumeToken.pendingWork || []
  conversation.push(...pendingWork.map(w => "PENDING: " + w.description))

  // Restore key structural state
  goal.currentStepIndex = resumeToken.currentStepIndex
  goal.approvedPlan = restorePlan(resumeToken.currentPlanHash, resumeToken.planVersion)
  goal.verificationSummary = resumeToken.verificationSummary
  goal.changedFiles = resumeToken.changedFiles
  goal.commanderLog = resumeToken.lastRouterDecisions
  goal.subagentLog = resumeToken.lastSubagentOutputs
  goal.stallDetections = []

  // Set risk from resume token
  commanderState.lastRiskAssessment = resumeToken.lastRiskAssessment

  return { goal: goal, commanderState: commanderState }
}
```

## 14. MVP / V1 / V2 Roadmap

### 14.1 MVP (4-6 weeks)

**Core: Commander dispatches one subagent at a time via existing router**

- Commander model selection: hardcoded to GLM-5.2-class, with per-role tier override
- Single subagent dispatch (no parallel): commander assigns task → waits for result → processes
- KeyPool integration: reuse existing V6.4 KeyPool (4 modes, 39 tests) with minor commander-state additions
- Commander state model: CommanderState tracking active delegations and decision log
- Existing UI reuse: extend goal-status-bar and goal-activity-panel with commander tab
- Decision logging: CommanderDecision records in existing RouterLedger format
- Single fallback chain: key-level fallback only (no model-tier fallback yet)
- No MCP/skills integration (deferred to V1)

**MVP Scope Boundaries**

- **What IS included**: Single-step delegation, KeyPool ref dispatch, commander decision log, UI commander tab, result merging
- **What IS NOT included**: Multi-channel stream racing (cache_affinity only for MVP), risk assessment automation, drift detection, skills/plugins, MCP integration

### 14.2 V1 (8-12 weeks)

**Core: Full hierarchical dispatch with risk assessment and drift detection**

- Commander risk assessment: automated per-step risk scoring with escalation thresholds
- Drift detection: scope diff computation during execution with commander intervention
- KeyPool V2 additions: stream racing mode implementation, health score circuit breaker, budget guard
- Fallback chain: full tier-model-slot escalation chain with commander override
- Compaction integration: Tournament compaction strategy for commander context
- Recovery checkpoints: structured RecoveryCheckpoint insertion every N steps
- Skills/plugins framework: basic skill graph with verified/tested tier system
- Commander UI: full commander panel with delegation view, KeyPool dashboard, drift visualization
- Multi-step delegation: commander can chain tasks (delegate → wait → delegate → merge → continue)
- Resume token: structured ResumeToken for post-compaction continuation

**V1 Scope Boundaries**

- **What IS included**: Risk assessment, drift detection, stream racing, health scores, compaction, recovery checkpoints, skill graph, commander UI
- **What IS NOT included**: MCP deep integration, advanced verification plugins, cost trajectory simulation, cross-session learning

### 14.3 V2 (16-20 weeks)

**Core: Production-grade with MCP, cross-session learning, and advanced analytics**

- MCP integration: commander-activatable MCP tool graph with capability verification
- Advanced verification plugins: per-project verification strategies (React, Rust, Python etc.)
- Cost trajectory modeling: predictive cost tracking with proactive budget management
- Cross-session learning: pattern analysis across multiple goal executions for better delegation
- Advanced drift prevention: plan-lock mechanism that prevents implementation from diverging during execution
- Multi-commander support: allow switching commander model mid-execution (for cost/budget reasons)
- Skills marketplace: external skill sharing with versioning, verification, and rating system
- Full observability dashboard: leaderboards, trend analysis, failure pattern detection
- Provider channel abstraction: unified channel interface that handles Anthropic/OpenAI/Chinese APIs transparently
- Stream racing at scale: N-key concurrent execution with intelligent cancellation and token budget sharing

**V2 Scope Boundaries**

- **What IS included**: MCP, verification plugins, cost modeling, learning, plan-lock, skill marketplace, full observability
- **What IS NOT included**: Distributed multi-machine execution, self-improving commander algorithms, autonomous debugging beyond scope

## 15. Risks and Anti-Patterns

### 15.1 Architectural Risks

**Weak Commander Model Risk** — Using a sub-tier model as commander destroys the entire hierarchy. The commander must be genuinely stronger than any subagent it delegates to. Mitigation: enforce tier_1 assignment for commander role; block runtime downgrade.

**Subagent Context Drift** — Even bounded tasks can drift if the subagent's context window is too large. Mitigation: strict context budgets per subagent task; commander does NOT pass full parent context.

**KeyPool Health Spiral** — One unhealthy slot can drag down the entire pool if it keeps receiving requests. Mitigation: health-score circuit breaker; circuit-open slots excluded from all dispatch for 30 min; commander notified immediately.

**Compaction Information Loss** — Aggressive compaction loses critical state. Mitigation: ring buffer of full states (last 5 full states available for rollback); ResumeToken always contains verification summary and changed files.

**Commander Context Overflow** — The commander itself can overflow during 8-hour runs. Mitigation: Tournament compaction for commander context; commander never holds raw subagent outputs, only structured scorecards.

**429 Cascade Failure** — One 429 on a primary slot in stream_race can cause wrong slot selection. Mitigation: 429 detection at slot level, not model level; KeyPool.handle429 implements immediate fallback.

**Cost Unpredictability** — Parallel subagent execution can exceed budget unexpectedly. Mitigation: Budget guard triage at slot, pool, and commander levels; escalation at 80% spend threshold.

### 15.2 Anti-Patterns (What NOT to Do)

**Anti-Pattern 1: Subagent Peer Negotiation**
- WRONG: Two executor subagents independently decide to run tests — duplicate work, wasted budget
- CORRECT: Commander decides "run integration tests" → delegates to ONE executor subagent → receives result → decides next action

**Anti-Pattern 2: Same Model, All Tiers**
- WRONG: Using GLM-5.2 for everything because "it's the best"
- CORRECT: Tier 3 helpers for mundane tasks (log scanning, test parsing). Tier 2 for implementation. Tier 1 only for planning/verification/commander decisions.

**Anti-Pattern 3: No Failover Before Dispatch**
- WRONG: Dispatch to primary slot only; on 429, retry the same slot (wastes 2s+)
- CORRECT: Build fallback chain BEFORE dispatch. On 429, immediately switch to fallback slot.

**Anti-Pattern 4: Full Context Pass-through**
- WRONG: Commander passes its entire context window to subagent
- CORRECT: Commander builds bounded SubagentTask.context (allowedFiles, scopeBoundary). Subagent receives 8K-32K tokens, not 128K.

**Anti-Pattern 5: Ignoring Cache Affinity**
- WRONG: Random slot selection on every request → prompt cache hits drop to 0% → 2 keys slower than 1 key
- CORRECT: cache_affinity default mode; same session → same key slot; deep cache accumulation reduces TTFT

**Anti-Pattern 6: No Escalation Threshold**
- WRONG: Commander retries indefinitely on failure → infinite budget burn
- CORRECT: 3 retry limit per step; after exhaustion, escalate to user with evidence package

**Anti-Pattern 7: Commander Trusts Without Verification**
- WRONG: Commander accepts subagent output at face value
- CORRECT: Every subagent result goes through verification before merging; VerificationResult is mandatory

**Anti-Pattern 8: Stream Racing Without Cancellation**
- WRONG: Send N requests, wait for all to complete, then pick the "best" response
- CORRECT: Stream race = first token wins => cancel remaining requests immediately => avoid wasted compute and cost

**Anti-Pattern 9: Cost Bloat from Parallel Execution**
- WRONG: Launch 5 subagents simultaneously for "efficiency" → 5× token cost
- CORRECT: Sequential delegation (commander dispatches one → collects → decides next); parallel only for explicitly race-mode actions

**Anti-Pattern 10: No Health Score Degradation**
- WRONG: Slot keeps getting requests even after 10 consecutive failures
- CORRECT: Health score drops per failure; circuit breaker at health < 0; recovery only after manual reset or timeout

## 16. Implementation Prompt

Below is the definitive implementation prompt for building the Strong Commander + KeyPool architecture.

---

### Implementation Prompt: SokachCode Strong Commander Architecture

**Project**: D:\sokachcode (built on opencode framework)
**Output file**: D:\sokachcode\STRONG_COMMANDER_KEYPOOL_IMPLEMENTATION.md
**Constraint**: Research/documentation only — DO NOT implement code. Produce a comprehensive implementation specification.

#### Phase 1: Commander Core (Estimated 3-4 weeks)

Build the Commander dispatch engine that wraps the existing SokachCode V6 router core:

1. **Commander State Manager**
   - Create CommanderState type (extends existing goal-state.ts)
   - Track active delegations, completed delegations, decision log
   - Implement delegation lifecycle: create → dispatch → wait → collect → merge → cleanup

2. **Commander Loop Engine**
   - Implement commanderLoop() — the main execution cycle
   - Risk assessment integration: computeRisk() before each delegation
   - Escalation decision: shouldEscalate() with 5 threshold conditions
   - Result processing: success → merge and continue; failure → fallback chain or escalate

3. **Subagent Task Model**
   - Create SubagentTask type with bounded scope definition
   - Context budget enforcement: estimateContextBudget() per task
   - Output format: structured (JSON summary ≤200 tokens) vs full (goes to ledger)
   - scopeViolation detection: compare subagent output against scope boundary

4. **Result Merging Engine**
   - mergeSubagentResults() with role-specific strategies
   - Conservative merge for verification results (most strict verdict wins)
   - File change merge for execution results (most recent edit per file)
   - Conflict detection and commander resolution

5. **Fallback Chain Builder**
   - buildFallbackChain(): same-model alternate slots → same-tier alternate models → commander escalation
   - executeWithFallback(): iterates through chain with retry budget
   - Escalation to user when chain exhausted with evidence package

#### Phase 2: KeyPool V2 Extensions (Estimated 2-3 weeks)

Extend existing KeyPool (V6.4, 39 tests) with commander-specific features:

1. **Stream Race Implementation**
   - streamRace(): parallel dispatch to N slots, first token wins, others cancelled
   - Token budget sharing: race slots share a common token budget
   - Cost accounting: track N-slot cost even when only one result used
   - Integration with executeWithFallback(): stream_race mode as primary dispatch strategy

2. **Health Score Circuit Breaker**
   - Compute health score (0-100) from recent request history
   - Circuit breaker: health < 0 → disable slot for 30 min
   - Recovery: automatic reset after timeout + successful request
   - Commander notification on circuit open/close events

3. **Budget Guard System**
   - Per-slot monthly budget tracking (independent per API key)
   - Pool-level aggregate budget (sum of all slots)
   - Commander-level budget (total for this goal execution)
   - Tri-level escalation: slot exceeds → disable slot; pool exceeds → pause new delegations; commander exceeds → escalate to user

4. **Cache Affinity Optimization**
   - Binding lifecycle: session → slot until session ends or slot unhealthy
   - LRU eviction when affinity count exceeds max per slot
   - Commander notification when affinity rebind occurs (performance impact)
   - Affinity statistics: cache hit rate per slot, per session

5. **Provider Difference Handling**
   - Per-provider characteristics: first-token latency profile, tool-call schema differences, error response formats
   - Cost estimation refinement: actual cost tracking per provider
   - Health threshold customization: stricter for unstable providers, laxer for reliable ones

#### Phase 3: Risk Assessment & Drift Detection (Estimated 2 weeks)

1. **Risk Assessor Module**
   - RiskAssessment type with RiskLevel enum (low/medium/high/critical)
   - perStep risk computation: scope expansion, security implications, breaking changes, test coverage gaps
   - riskTrend tracking: stable/rising/critical based on consecutive step assessments

2. **Drift Detection System**
   - computeScopeDiff(): compare implemented files against approved plan scope
   - driftScore calculation: weighted metric (unauthorized files × 3, missing files × 1)
   - Drift severity: critical (>0.5), warning (>0.3), none (<0.3)
   - Drift correction loop: commander proposes fix → subagent implements → re-verify

3. **Escalation Decision Engine**
   - shouldEscalate(): 5 conditions → boolean decision
   - Evidence package assembly: drift events, failure history, risk changes
   - User-facing escalation message: structured with options (approve, modify plan, cancel)

#### Phase 4: Compaction & Recovery (Estimated 2 weeks)

1. **Tournament Compaction**
   - TournamentState and Scorecard types
   - compactCommanderContext(): build tournament from conversation
   - Elite selection: top-N scorecards by composite score
   - Context reconstruction: elites summary + pending work + resume token

2. **Recovery Checkpoint System**
   - RecoveryCheckpoint type with step-index, plan-hash, verification-summary
   - Ring buffer: last 20 checkpoints, oldest evicted
   - insertRecoveryCheckpoint(): inserts at configurable step interval

3. **Resume Token Design**
   - ResumeToken struct with all required state for continuation
   - reconstructFromResumeToken(): rebuild goal and commander state from token
   - Rollback support: restore from lastFullState or tournament checkpoint

#### Phase 5: Skills/Plugins/MCP (Estimated 2-3 weeks)

1. **Skill Graph**
   - Skill type with id, name, tier (verified/tested/experimental), entryPoints
   - EntryPoint with input/output schemas, cost estimate, timeout, fallback behavior
   - SkillGraph query: find relevant skills by task description, role, scope
   - Commander activation protocol: filtered by tier, checked for prerequisite context

2. **Plugin Architecture**
   - Plugin types: RouterPlugin, ProviderPlugin, VerificationPlugin, CompactionPlugin, UIPlugin
   - Plugin registration and lifecycle
   - Commander-plugin communication: plugin reports capabilities, commander activates

3. **MCP Integration**
   - MCP server discovery and capability verification
   - MCP tool categorization matched to subagent tasks
   - MCP failure handling: specific fallback behaviors per tool (not generic retry)

#### Phase 6: UI & Observability (Estimated 3-4 weeks)

1. **Commander Panel**
   - Current plan display with step progress
   - Active delegations list with model, tier, status, progress bars
   - Risk assessment gauge (visual scorecard)
   - Commander decision log (expandable, each entry with rationale)

2. **KeyPool Dashboard**
   - Slot overview cards (health score, RPM, cooldown status, cache affinity)
   - Pool summary (total RPM capacity, monthly budget remaining, fallback chain)
   - RPM usage chart (minute-by-minute, per slot)
   - Circuit breaker events (timeline of open/close events)

3. **Drift Visualization**
   - Side-by-side: approved plan scope vs. current implementation
   - Color-coded diff: green (approved), red (unauthorized), yellow (missing)
   - Drift score trend over time

4. **Budget & Cost Dashboard**
   - Per-tier spending bar chart
   - Per-slot monthly usage with percentage of limit
   - Pool aggregate view
   - Trajectory projection: will we hit budget within this goal's remaining steps?

5. **Skills Marketplace Page**
   - Skill catalog with tier badges (verified/tested/experimental)
   - Installation/removal with commander notification
   - Skill rating and review system

#### Phase 7: Testing & Integration (Estimated 2 weeks)

1. **Unit Tests (per module)**
   - Commander loop logic: test delegation lifecycle, result merging, fallback chain
   - KeyPool V2: stream race behavior, health score computation, circuit breaker
   - Risk assessment: test risk scoring for various task scenarios
   - Drift detection: test scope diff computation for various implementation states
   - Compaction: test tournament construction, elite selection, resume token reconstruction

2. **Integration Tests**
   - Commander → Router → KeyPool → subagent dispatch flow
   - Fallback chain: trigger 429, verify slot fallback, verify model fallback
   - Compaction → resume: compact mid-goal, verify goal continues correctly
   - Drift → escalation: implement unauthorized change, verify commander interrupts

3. **E2E Simulation Tests**
   - Simulate 8-hour goal with realistic task distribution across tiers
   - Verify budget guard triggers at correct thresholds
   - Verify circuit breaker recovers after timeout
   - Verify compaction preserves critical state
   - Verify cost stays within budget projection

#### Integration Requirements

- Reuse ALL existing SokachCode V6 components: RouterCore, KeyPool V6.4, GoalState, Ledger, Watchdog, Stats
- Zero second-copy data: CommanderState flows through existing type system with extensions
- No Math.random: deterministic everything
- All existing 39 KeyPool tests must pass + new V2 tests (stream race, health, circuit breaker)
- No new provider/model store; everything flows through RouteCandidate[]
- API key safety preserved: keys never enter routing layer, LLM prompts, or ledger

#### Acceptance Criteria

1. Commander model is tier_1 only — cannot be downgraded at runtime
2. Subagent receives bounded context (max 32K tokens per task), not full parent context
3. KeyPool slot selection respects cache affinity by default (same session → same slot)
4. 429 handling: slot cooldown + immediate fallback, no waiting
5. Fallback chain built BEFORE dispatch, not after failure
6. Stream race: first token wins, other requests cancelled immediately
7. Health score circuit breaker: slot disabled at health < 0 for 30 min
8. Drift detection: scope diff computed every 5 steps, escalation at driftScore > 0.3
9. Compaction: Tournament state preserves elites + pending work; ResumeToken enables restart
10. Commander decision log: every delegation recorded with rationale, model, slot, confidence, fallback chain
11. Budget triage: slot, pool, commander levels with escalating actions
12. All 39 existing KeyPool tests pass + 15+ new V2 tests pass
13. No Math.random used anywhere in new code
14. No API key or auth info in commander context, LLM prompts, router decisions, or ledger entries
