# KeyPool Router Spec

## Routing Principle

Model routing should serve the commander hierarchy.

```text
role -> tier -> model -> provider channel -> key slot -> fallback/race policy
```

Do not let the router choose a weak model for high-risk commander work just
because it is cheap or available.

## Role Tiers

### Tier 1: Commander

Use the strongest coding model available.

Examples:

- GLM-5.2-class model;
- Claude/GPT top coding model;
- Qwen-Coder top model.

Responsibilities:

- architecture decisions;
- core code edits;
- merge decisions;
- recovery;
- final review.

### Tier 2: Lower-Tier Workers

Use strong but cheaper/lower-tier models.

Examples:

- GLM-5.1 through multiple channels;
- Qwen-Coder mid-tier;
- other reliable OpenAI-compatible coding models.

Responsibilities:

- scouting;
- candidate patch planning;
- focused file analysis;
- frontend review;
- security checklist;
- test failure attribution.

### Tier 3: Cheap Helpers

Use fast or cheap models.

Examples:

- Flash-class models;
- MiniMax;
- cheap Qwen/GLM variants;
- local rules where possible.

Responsibilities:

- log summaries;
- classification;
- doc cleanup;
- repeated checks;
- simple comparison tasks.

## KeyPool Model

```typescript
type KeyPool = {
  providerId: string
  modelId: string
  channels: ProviderChannel[]
  slots: KeySlot[]
  policy: KeyPoolPolicy
  health: PoolHealth
}

type ProviderChannel = {
  id: string
  providerKind: "official" | "openrouter" | "siliconflow" | "gateway" | "custom"
  baseUrl: string
  supportsStreaming: boolean
  supportsToolCalls: boolean
  contextLimit: number
  costProfile?: CostProfile
}

type KeySlot = {
  id: string
  channelId: string
  keyRef: string
  rpmLimit?: number
  tpmLimit?: number
  monthlyBudget?: number
  cooldownUntil?: number
  sessionAffinity: Record<string, string>
  stats: {
    requests: number
    successes: number
    failures: number
    rateLimited: number
    avgFirstTokenMs?: number
    avgTotalMs?: number
    cacheHitRate?: number
  }
}
```

## Selection Modes

### Session Affinity

Default for long coding sessions.

Use the same slot for the same goal/session where possible to protect prompt
cache and reduce drift.

### RPM Pooling

Use for parallel subagent/helper tasks.

Distribute independent tasks across slots while respecting per-slot cooldown and
budget.

### Fallback Chain

Use after:

- 429;
- timeout;
- provider error;
- stream failure;
- malformed tool call;
- degraded health.

Preferred chain:

```text
same model same channel different key
-> same model different channel
-> equivalent model same role tier
-> higher tier if needed
-> pause/escalate
```

### Stream Racing

Use sparingly for high first-token latency and user-visible tasks.

Do not race every request. Race only when:

- first-token latency is a real bottleneck;
- task is valuable enough;
- budget guard allows it;
- duplicate side effects are impossible;
- loser requests can be cancelled.

## 429 Handling

```typescript
function handle429(slot: KeySlot, pool: KeyPool, task: RoutedTask): RouterDecision {
  markRateLimited(slot)
  slot.cooldownUntil = now() + backoff(slot.stats.rateLimited)

  const fallback = findFallbackSlot(pool, task, { exclude: slot.id })
  if (fallback) {
    return {
      action: "retry",
      slotId: fallback.id,
      reason: "429 fallback to healthy slot",
    }
  }

  const channelFallback = findFallbackChannel(pool, task)
  if (channelFallback) {
    return {
      action: "retry",
      channelId: channelFallback.id,
      reason: "429 fallback to alternate channel",
    }
  }

  return {
    action: "defer",
    reason: "all slots cooling down",
    retryAfterMs: soonestCooldown(pool),
  }
}
```

## Health Score

Score each channel/slot using:

- recent success rate;
- 429 rate;
- average first-token latency;
- timeout rate;
- tool-call validity;
- schema adherence;
- cost;
- remaining budget;
- cache affinity match.

Router should expose the decision reason to the ledger and UI.

## Secret Boundary

The model must never see raw API keys.

The commander can know abstract channel names and health, but key values remain
in credential storage or environment-backed secret references.

