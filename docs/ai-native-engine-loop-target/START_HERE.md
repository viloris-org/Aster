# Aster Goal Pack

Use this folder as the Claude Code goal context:

```text
docs/ai-native-engine-loop-target/
```

This pack is for a long-running VSCode Claude Code goal-mode run against:

```text
D:\Aster-v3-audit
```

## Short Goal Prompt

Paste this into Claude Code:

```text
/goal Work continuously on feat/ai-native-engine-loop; do not stop at planning. First read docs/ai-native-engine-loop-target/ and compare origin/main, origin/takeover/upstream-integrated, and origin/fix/render-pipeline-wgpu. Borrow good ideas without blindly copying. Implement and keep iterating: real Quest execution loop, product-grade QuestPage/EditorPage, SceneCommand/ScenePatch, physics/render/audio validation, safe apply/rollback, and evidence docs. Every loop must edit files, run focused verification, fix failures, update progress/comparison, then choose the next target. Do not summarize until the branch has a comparable working loop. Ask only for credentials, destructive operations, or severe conflicts.
```

## Read Order

Claude should read:

```text
docs/ai-native-engine-loop-target/CLOUD_BRANCH_REFERENCE.md
docs/ai-native-engine-loop-target/FULL_TARGET_PROMPT.md
docs/ai-native-engine-loop-target/QUEST_REAL_EXECUTION_PROMPT.md
docs/ai-native-engine-loop-target/CLAUDE_CODE_RUN_STYLE.md
AGENTS.md
docs/quest-workflow-ui-reference.md
docs/ai-editor-quest-prd.md
```

## Expected Run Style

This is not a one-shot planning task.

Claude must:

- compare remote branches before deciding what to implement;
- use the other branch as a reference, not as source to blindly copy;
- make real code changes;
- run focused verification;
- repair failures;
- update evidence docs;
- continue to the next highest-value change.

Do not let the run end after "the plan looks good".
