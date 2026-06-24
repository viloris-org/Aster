# Quest Real Execution Hardening Target

This folder is a Claude goal-mode support pack for a high-value lane inside the
larger AI-native Quest/Editor run:

```text
Quest real execution validation, hardening, tests, and bug repair.
```

This is not a separate product direction by itself. Use it together with
`docs/ai-native-engine-loop-target/` when continuing the current implementation
branch. Do not create a new branch unless the user explicitly asks. The agent
should understand the existing code first, then decide how to continue the main
Quest/Editor work while using this checklist to keep the execution chain real
and low-bug.

## Read Order

```text
GOAL_PROMPT.txt
FULL_TARGET_PROMPT.md
CHECKLIST.md
HANDOFF_RULES.md
RUN_REPORT_TEMPLATE.md
../ai-native-engine-loop-target/QUEST_REAL_EXECUTION_PROMPT.md
../ai-native-engine-loop-progress.md
../ai-native-engine-loop-comparison.md
```

## Core Shape

```text
quest request
-> workspace
-> execution
-> diff
-> validation
-> review
-> stale/apply guard
-> apply/rollback/discard evidence
```

The output should be evidence: tests, bug fixes, command results, and a written
report of what is still risky.
