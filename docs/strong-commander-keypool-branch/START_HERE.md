# Strong Commander KeyPool Branch Pack

This folder collects the research for a future branch/product direction:

```text
strong-commander-keypool
```

It is based on three inputs:

- Claude Code local deep research:
  `sources/claude-code-deep-research.md`
- ChatGPT web deep research text export:
  `sources/chatgpt-web-deep-research.txt`
- prior SokachCode local design docs referenced by both reports

## Use This Pack For

- designing a strong commander agent architecture;
- adding lower-tier subagents without making them equal decision makers;
- building a multi-channel KeyPool/router for 429/RPM/latency resilience;
- productizing goal mode, skills/plugins/MCP, and long-running coding sessions;
- creating an implementation branch in Aster or SokachCode.

## Recommended Read Order

```text
RESEARCH_SYNTHESIS.md
PRODUCT_ARCHITECTURE.md
KEYPOOL_ROUTER_SPEC.md
IMPLEMENTATION_PROMPT.md
GOAL_PROMPT.txt
sources/claude-code-deep-research.md
sources/chatgpt-web-deep-research.txt
```

## Short Goal Prompt

```text
/goal Build the strong-commander-keypool direction from docs/strong-commander-keypool-branch/: GLM-5.2-class commander, GLM-5.1 multi-channel workers, cheap helper models, KeyPool/router resilience, long goal-mode ledger/recovery, UI panels, skills/plugins activation, tests, and docs. Do not make subagents equal architects. Start from the smallest product-grade slice, verify it, document evidence, then continue.
```

