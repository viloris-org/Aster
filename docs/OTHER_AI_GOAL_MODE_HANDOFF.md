# Other AI Goal Mode Handoff

This repository contains several goal-mode document packs. Use this file as the
handoff entry point when running another AI agent on another device.

## Branch Rule

Do not create a new branch by default. Use the current existing branch the user
has checked out. There are two directions and they must stay separate:

- AI-native Quest/Editor implementation: real code work for the current Aster
  branch.
- Strong Commander/KeyPool architecture: long-term architecture reference unless
  the current branch is explicitly that direction.

If documents disagree with code, inspect the latest code and `origin/main`.
Existing implementation wins over old notes.

## Required Reading

Read these first for the active AI-native Quest/Editor run:

```text
docs/ai-native-engine-loop-target/START_HERE.md
docs/ai-native-engine-loop-target/FULL_TARGET_PROMPT.md
docs/ai-native-engine-loop-target/QUEST_REAL_EXECUTION_PROMPT.md
docs/quest-real-execution-hardening-target/START_HERE.md
docs/quest-real-execution-hardening-target/GOAL_PROMPT.txt
docs/quest-real-execution-hardening-target/CHECKLIST.md
docs/quest-real-execution-hardening-target/HANDOFF_RULES.md
```

Historical context notes, useful but not authoritative. Verify every claim
against the current branch code before relying on it:

```text
docs/ai-native-engine-loop-progress.md
docs/ai-native-engine-loop-comparison.md
```

Optional reference only:

```text
docs/strong-commander-keypool-branch/START_HERE.md
docs/strong-commander-keypool-branch/RESEARCH_SYNTHESIS.md
docs/strong-commander-keypool-branch/PRODUCT_ARCHITECTURE.md
docs/strong-commander-keypool-branch/KEYPOOL_ROUTER_SPEC.md
```

Do not implement Commander/KeyPool inside the Quest/Editor branch unless the
user explicitly changes the branch goal.

## Goal Prompt To Paste

```text
/goal 不要新建分支，使用当前已有分支继续工作。先不要急着改代码，也不要写空泛规划；先完整阅读 docs/ai-native-engine-loop-target/、docs/quest-real-execution-hardening-target/，并只把 docs/strong-commander-keypool-branch/ 当作长期架构参考。你必须先自己理解这个项目现在到底在做什么：Aster 的 AI-native Quest/Editor 主链路、现有代码已经完成了什么、还缺什么、当前分支和另一个 Commander/KeyPool 方向的边界在哪里。
理解规则：如果文档和代码冲突，以当前代码和最新 main 为准；如果别人已经做了某块，不重复造轮子，而是接上、补强、修边界问题。两个方向必须分清：当前 AI-native Quest/Editor 分支负责真实代码推进；Commander/KeyPool 是另一条长期架构方向，除非当前就在那个分支，否则不要把它强塞进来。
你接下来要自己判断最高价值工作，不要等我逐项指挥。范围包括 Quest real execution、任务分解、workspace/execution/diff/validation/review/apply/rollback/discard、stale check、失败证据、ECS/SceneCommand、物理/渲染/音频验证诊断、Quest/Editor 前端体验、测试和构建。不是全部乱改，而是读懂后按依赖关系和风险优先级推进。
工作方式：先建立上下文，再选一小块真正重要的地方动手；改之前知道为什么改，改之后用测试、构建或代码检查确认。发现 bug 就修，发现缺测试就补，发现前端看不懂后端状态就改善展示，发现主链路不真实就补真实链路。不要只写建议，不要停在规划，不要为了显得忙而大改。每完成一块就重新审视当前状态，继续选择下一块。
质量要求：最终这个已有分支要更接近可合并状态，主链路更真实、更稳、更少 bug。不要留下明显 broken 状态；验证失败要优先处理。只有遇到真实阻塞、环境权限问题，或者当前分支核心目标已经明显完成，才停下来说明。
```

## First Commands For The Agent

On another device, get the current branch first:

```bash
git fetch origin --prune
git checkout feat/ai-native-engine-loop
git pull --ff-only origin feat/ai-native-engine-loop
```

Then inspect state before editing:

```bash
git fetch origin --prune
git status --short --branch
git log --oneline --decorate -n 8
git log --oneline --decorate origin/main..HEAD
```

Then inspect the code before editing. The important code areas are likely:

```text
editor/src-tauri/src/lib.rs
editor/src/renderer/pages/QuestPage.tsx
editor/src/renderer/pages/EditorPage.tsx
editor/src/renderer/quest.ts
crates/engine-ai
crates/engine-ecs
crates/engine-physics
crates/engine-render*
crates/engine-audio
```

## Expected Behavior

The agent should understand first, then work. It should connect to existing code,
reuse finished parts, avoid duplicate rewrites, fix concrete bugs, add missing
tests, run relevant checks, and keep the current branch closer to mergeable.
