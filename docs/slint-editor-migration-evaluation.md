# Slint Editor Migration Evaluation

状态：Migration Started  
日期：2026-06-23  
结论：建议先做 Slint shell spike，不建议立刻全量迁移。

## Migration Log

- 2026-06-23: Added `editor-slint` as the first parallel Slint editor shell. It builds as `aster-editor-slint`, accepts `--project <path>`, reuses `engine-editor::EditorShell`, and shows native Slint shell panels for Hierarchy, Scene View placeholder, Inspector, Project, and Console. The WGPU Scene View adapter is intentionally still pending.
- 2026-06-23: Wired the Slint shell to real `EditorShell` scene editing operations for the first backend bridge pass: create object, delete selected object, quick rename, nudge transform on X/Y/Z, add/remove Camera/Light/MeshRenderer, save, undo, redo, hierarchy selection, asset selection, and console feedback. This proves Slint callbacks can mutate the same scene/project state as the old editor, but it is still not a full replacement for the Tauri backend.
- 2026-06-23: Added a second parity pass for Slint asset behavior: create script/material/prefab/scene assets, rename selected asset, delete selected asset, reimport selected asset, reimport all assets, rescan project assets, preserve `.meta` cleanup/rename behavior, and show the real scene path / render status instead of hard-coded shell text. This narrows day-to-day Project panel drift from Tauri, but the implementation still duplicates Tauri backend helper logic until a shared editor host exists.
- 2026-06-23: Corrected a structural parity issue: the Tauri app has three top-level screens (`hub`, `editor`, `quest`), while the first Slint shell incorrectly treated Quest as an editor rail. Slint now has top-level `Hub`, `Editor`, and `Quest` screen selection, with Quest rendered as its own workspace screen. This is still not feature parity with the React Quest page, but it prevents the migration from baking in the wrong product model.
- 2026-06-23: Corrected the Slint editor UI model to match the Tauri/Calm editor screenshots more closely. The rail now changes the left side panel content while the center Scene View remains stable, instead of switching the center workspace away from Scene View. The shell chrome was also tightened toward the Tauri visual system: 48px top bar, 56px rail, 276px left panel, 280px inspector, compact status bar, transparent toolbar buttons, green selected rail state, and denser inspector rows. `cargo check -p aster-editor-slint` passes after this pass.

## Background

Aster Editor 当前是 Tauri 2 + React 19 + Vite/Bun 前端，Rust 后端在 `editor/src-tauri`，编辑器领域能力主要在 `engine-editor`、`runtime-min` 和渲染相关 crates。现有前端不是一个薄壳：`EditorPage.tsx`、`QuestPage.tsx`、`AiPanel.tsx`、`CalmEditorPrototype.tsx` 已经承载大量复杂 UI、Markdown/代码预览、AI 流式输出、资产面板、脚本编辑、构建面板和 Scene View overlay。

现有架构的核心问题不是 React 本身，而是 Scene View presentation：

- 稳定 fallback 是 WebView canvas readback，会经过 GPU -> CPU -> IPC -> WebView canvas。
- 实验性 native child surface 可以减少 readback，但 WebView root 与 native child surface 分属不同 compositor/lifecycle，resize、DPI、Wayland/X11 和 DOM rect 跟踪存在竞态。
- ADR 0001 已经把目标定为 native host owns root：原生 host 拥有顶层窗口和 Scene View surface，UI 作为 panel/overlay/dock view 进入同一个 native composition tree。

Slint 的吸引力正好落在这个问题上：它是 Rust-first/native UI toolkit，可以让编辑器 UI 更靠近 Rust event loop、winit/WGPU 和平台窗口，而不是继续让 WebView 拥有 editor root。

## External Facts Checked

- Slint 最新 GitHub release 当前为 `1.16.1`，发布时间是 2026-04-23。
- Slint docs 列出 `backend-winit`，支持 Windows、macOS、web browser、X11 和 Wayland；也提供 `backend-winit-x11` 与 `backend-winit-wayland` 分拆 feature。
- Slint docs 列出 `renderer-femtovg-wgpu`，即 FemtoVG + WGPU renderer；也有 Skia renderer 和 software renderer。
- Slint 1.12 release 明确加入了 WGPU integration，目标之一是把 WGPU/Bevy 这类渲染库嵌入 Slint app。
- Slint blog 在 2026-03-31 有一篇 “Changing the Default Style in Slint — Deprecating Native-Looking Styles”，这意味着“更原生”需要谨慎定义：Slint 更像 native-code UI/低 WebView 依赖，不一定等于系统控件外观 1:1。

Sources:

- https://github.com/slint-ui/slint/releases
- https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/
- https://docs.slint.dev/latest/docs/rust/slint/docs/cargo_features/
- https://slint.dev/blog/slint-1.12-released
- https://slint.dev/blog/

## Fit Assessment

### Strong Fits

1. Native host root direction

Slint aligns better with ADR 0001 than the current WebView-owned root. A Slint shell can own the top-level window through winit, host dock panels and toolbars in native-code UI, and let the engine Scene View become a first-class WGPU/native region instead of a DOM-measured rectangle.

2. Rust integration

Aster already has Rust-first editor/backend state. Moving shell-level UI to Slint would reduce IPC translation for common editor state changes and could let `engine-editor` expose strongly typed state models directly to UI bindings.

3. Lower presentation risk than WebView + child surface

If Slint owns the layout tree and the renderer integration, viewport sizing becomes toolkit/native layout state rather than DOM rect observation. That directly attacks the current compositor drift problem.

4. Smaller dependency surface for core editor

A Slint editor could eventually remove React/Vite/WebView from the primary shell, reducing JS dependency churn and making packaging more Rust-centric.

### Weak Fits / Risks

1. Complex editor UI rewrite cost is high

The current React side is substantial. Rewriting the Quest workspace, AI chat/Markdown rendering, script editor, command palette, dialogs, project browser, inspector, overlays, and build UI into Slint is a product rewrite, not a mechanical migration.

2. Web ecosystem affordances are currently valuable

React gives easy Markdown rendering, syntax highlighting, textareas, selection behavior, browser-like layout, and rapid iteration. Slint can build polished tooling UI, but the existing AI/Quest flows use web-style rich text and long-form interaction patterns that will be expensive to recreate well.

3. WGPU version mismatch risk

Aster currently uses workspace `wgpu = 29.0.3`. Slint cargo features currently expose `unstable-wgpu-27` in docs. That does not automatically block a shell spike, but shared-device or deep WGPU interop may need adapter boundaries, version isolation, or waiting for Slint's WGPU API to catch up.

4. “Native effect” may not mean native widgets

Slint is native code and can avoid WebView composition costs, but its controls are not necessarily platform-native widgets. If the desired effect is OS-authentic menus, text fields, accessibility semantics, IME, dock/window integration, and platform conventions, the spike must test those explicitly.

5. Licensing/product constraints need review

Slint has open-source and commercial licensing paths. Aster is MPL-2.0. Before committing to Slint as editor foundation, verify that the chosen Slint license and any commercial obligations fit the distribution model.

## Backend Parity Map

The current blocker is not only UI coverage. The Tauri editor backend is still the production host for many capabilities, and a large amount of that host logic lives in `editor/src-tauri/src/lib.rs` rather than a reusable UI-agnostic crate.

### Wired in `editor-slint`

- Project open through `--project <path>` using `EditorShell::open_project`.
- Top-level shell structure matching Tauri's three screens at a coarse level: Hub, Editor, and Quest.
- Scene hierarchy and asset list snapshots from `ProjectContext`.
- Selection mirrored into `EditorShell::select_entity_id`.
- Save/undo/redo through `EditorShell`.
- Basic scene mutations: create, delete, rename, transform nudge, and add/remove Camera/Light/MeshRenderer components.
- Basic project asset actions: create script/material/prefab/scene, rename/delete/reimport selected asset, reimport all assets, and rescan the `ProjectContext` asset database.
- Rail-driven left-panel switching for Scene, Assets, Scripts, Build, and Diagnostics while keeping the center Scene View visible, matching the Tauri editor's primary layout behavior.
- Console feedback using `ConsoleService`.

### Not Wired Yet

- Hub project picker, create project, delete project, recent projects, installs page, settings page, durable app state.
- File dialogs and platform open-folder actions.
- Scene open/save-as dialogs.
- Full object operations: duplicate, reparent, drag/drop hierarchy, precise transform editing, component field editing.
- Project file and asset actions still missing beyond the basic parity pass: import external asset via file dialog, open asset in file manager, read/write text assets, full script editing, asset references, AMDL/script diagnostics, package execution, search/filter/context menus, and user-entered names for created/renamed assets.
- Viewport presentation: CPU readback, native host window, Wayland embedded compositor, editor compositor, or Slint/WGPU adapter.
- Play runtime start/stop backed by `runtime-min`.
- Quest feature parity: create/rename/delete/export/branch quests, prompt rewrite, voice transcription, intent/spec editing and preview, model settings, project file suggestions, clarification cards, live AI streaming, timeline grouping, artifact pane, knowledge approval/attachment, review metrics, transaction groups, apply/discard/rollback/revision/quick-fix flows, and opening artifacts back into Editor.
- i18n and theme preferences.

### Required Refactor Before Full Migration

Do not keep porting Tauri RPC handlers directly into `editor-slint`. The next backend step should be a shared editor host crate or module that exposes typed services used by both frontends:

- `EditorProjectHost`: hub state, durable preferences, project open/create/delete.
- `EditorSceneHost`: scene tree, object/component/transform commands, undo/redo, dirty tracking.
- `EditorAssetHost`: project file and asset operations.
- `EditorViewportHost`: presentation capability selection and viewport lifecycle.
- `EditorQuestHost`: Quest/Copilot/knowledge state and streaming events.

The Slint frontend should bind to these typed host APIs directly. The Tauri frontend can keep its JSON-RPC command names as an adapter over the same APIs until it is retired.

## Migration Options

### Option A: Keep Tauri/React, continue native-host-window seam

Best when the near-term priority is feature delivery and preserving existing UI investment.

Pros:

- Lowest rewrite cost.
- Keeps Quest/AI rich text flows intact.
- Current docs and code already define the presentation seam.

Cons:

- Still depends on difficult WebView + native host integration.
- More platform-specific work around WebView2/WKWebView/WebKitGTK.
- The root ownership problem remains unless the native host architecture is fully implemented.

### Option B: Slint shell for editor frame + keep React for rich panels

Best compromise for evaluation.

Pros:

- Tests whether Slint can own the root window and Scene View presentation.
- Preserves Quest/AI panels as WebView or external panel during transition.
- Lets high-risk viewport work be evaluated without rewriting every panel.

Cons:

- Hybrid app complexity.
- Need a clear boundary for focus, shortcuts, drag/drop, theme, and panel lifecycle.
- Could temporarily duplicate shell concepts.

### Option C: Full Slint editor

Best only if spike proves Slint solves presentation and the team accepts a UI rewrite.

Pros:

- Cleanest Rust-native editor stack.
- Removes WebView from the core shell.
- Strongest alignment with `engine-editor` and typed Rust state.

Cons:

- Highest rewrite cost.
- Need replacements for Markdown, syntax highlighting, advanced text editing, AI streaming UI, web-style selection, and possibly accessibility/IME polish.
- Slows feature development during migration.

## Recommended Spike

Build a separate workspace member, for example `editor-slint-spike`, without deleting the Tauri editor.

Scope:

1. Slint window shell

- Top menu/toolbar.
- Left hierarchy panel.
- Center Scene View region.
- Right inspector placeholder.
- Bottom console placeholder.
- Theme tokens matching current calm editor style.

2. Scene View integration

- Render a simple `WgpuRenderDevice` frame into the Slint-owned viewport.
- Resize viewport through Slint layout state, not DOM measurement.
- Report `cpu_readback`, `gpu_native_surface`, `gpu_composited`, and frame time diagnostics.

3. Input and overlay

- Pointer drag orbit/pan/zoom.
- Entity selection stub.
- One transform gizmo or overlay marker.
- Keyboard shortcuts for save, undo, redo, command palette.

4. Rich panel pressure test

- Implement one compact inspector in Slint.
- Implement one AI/Quest-like streaming text panel, enough to test long text, Markdown-ish content, selection/copy, scrolling, and performance.

5. Packaging and platform checks

- Linux X11 and Wayland behavior.
- Windows/macOS build feasibility if runners are available.
- Binary size, startup time, input latency, font rendering, IME, accessibility smoke.

Success criteria:

- Scene View can resize repeatedly without compositor drift.
- Frame path avoids CPU readback for the Scene View.
- WGPU interop does not force a second incompatible rendering architecture.
- Text-heavy AI/Quest panel is acceptable or can be cleanly delegated to a web panel.
- Slint shell code feels maintainable after implementing real editor state bindings.

Failure criteria:

- Shared WGPU/device integration is blocked by version/API mismatch.
- Wayland behavior still requires a separate embedded compositor of similar complexity.
- Rich text/editor workflows are significantly worse than the current React implementation.
- Accessibility, IME, keyboard focus, or platform menu behavior is not shippable without large custom work.

## Proposed Path

1. Do the spike first, time-boxed to 3-5 engineering days.
2. Keep current Tauri/React editor as the production path during the spike.
3. Use the spike to update ADR 0001 with a concrete adapter choice:
   - `native-host-window-tauri`
   - `native-host-window-slint`
   - or `hybrid-slint-shell-web-panels`
4. If successful, migrate shell-level UI first:
   - menu bar
   - viewport host
   - hierarchy
   - inspector basics
   - console
5. Delay Quest/AI panel migration until the shell proves better. Those panels are the least obvious fit for a first Slint rewrite.

## Recommendation

Slint is worth evaluating seriously because it matches Aster's root presentation problem better than a WebView-owned shell. But the right move is a measured Slint shell spike, not an immediate editor rewrite.

The strategic question is:

Can Slint make Scene View presentation boring while preserving enough UI expressiveness for an editor?

If yes, migrate the shell and viewport first. If no, keep Slint out of the critical path and continue the existing native host-window seam in Tauri.
