# Varg

[![CI](https://github.com/viloris-org/Varg/actions/workflows/core.yml/badge.svg)](https://github.com/viloris-org/Varg/actions/workflows/core.yml)
[![Nightly](https://github.com/viloris-org/Varg/actions/workflows/nightly.yml/badge.svg)](https://github.com/viloris-org/Varg/actions/workflows/nightly.yml)
[![License: MPL-2.0](https://img.shields.io/badge/License-MPL%202.0-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-1.96+-orange.svg)

[English](README.md) | [简体中文](README.zh-CN.md) | 繁體中文 | [日本語](README.ja.md) | [한국어](README.ko.md) | [Español](README.es.md)

Varg 是 AI 原生遊戲引擎。用自然語言描述你想做的遊戲，自主 Agent 叢集就會協助構建場景、邏輯與 UI。完整的視覺化編輯器也在那裡，讓你隨時微調、打磨並完全掌控。

![Varg Editor](docs/screenshots/editor.png)

> **截圖佔位**：UI 穩定後請替換為實際編輯器截圖。

## 快速開始

```sh
git clone https://github.com/viloris-org/Varg
cd Varg

# 啟動編輯器
cd editor
bun install
bun run dev:tauri
```

> **前置需求：** [Rust ≥ 1.96](https://rustup.rs/)、[Bun ≥ 1.3.14](https://bun.sh/)、
> [Tauri 系統依賴](https://v2.tauri.app/start/prerequisites/)。
> Linux 使用者：`sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev
> libayatana-appindicator3-dev librsvg2-dev`

## 功能特色

- **AI 原生核心**：不是外掛式 AI 助手，而是由多 Agent 叢集自主規劃、構建並審查你的遊戲。自然語言輸入，可遊玩場景輸出。沙盒審查讓專案保持安全。
- **宣告式遊戲描述**：六套完整宣告式系統（行為樹、場景圖、UI 版面、系統設定、資源清單、專案結構）讓 Agent 生成結構化 JSON，而不是直接寫程式碼。
- **視覺化場景編輯器**：用精緻介面放置物件、調整 transform、加入元件。讓 AI 做重活，再由你手動打磨細節。
- **即時播放模式**：按下 Play，物理與腳本開始執行；按下 Stop，零清理。編輯場景不會被污染。
- **資源管線**：把 glTF/PNG 放進專案面板。檔案監看器會觸發匯入，熱重載即時推送更新。
- **可插拔渲染**：不碰引擎程式碼也能切換後端。內建 WGPU。
- **無頭執行時**：同一套引擎可跑在伺服器、CI 管線或自動化構建中，不需要視窗。
- **零 unsafe 程式碼**：每個 crate 都使用 `#![forbid(unsafe_code)]`，預設安全。

## 專案結構

```text
Varg/
├── editor/                  # Tauri 桌面應用（React + Rust）
├── crates/                  # 引擎 crate：ECS、資源、渲染、物理、音訊、AI 等
├── xtask/                   # 構建與自動化任務
├── examples/                # 範例專案與場景
└── docs/                    # 設計筆記
```

## 編輯場景

1. 啟動編輯器，進入 **Hub**
2. 建立或開啟專案
3. **Hierarchy** 面板列出場景內所有物件
4. **Inspector** 顯示選取物件的 transform 與元件
5. **Scene View** 渲染 3D 視埠，可 orbit、pan、zoom
6. 點擊 **Play**，在 **Game View** 執行物理與腳本
7. 加入 Camera、Light、MeshRenderer、Rigidbody、Collider 等元件，或撰寫 Varg 腳本

## 構建 Profile

| Profile | 內容 |
|---|---|
| `editor` | Tauri 前端所需的編輯器服務、wgpu 視埠與 Agent 工具 |
| `runtime-min` | 無頭模式：CI smoke tests、伺服器、自動化構建 |
| `runtime-game` | 無頭 + 視窗支援 |
| `dev-full` | 全部：編輯器、物理、音訊、腳本、Agent、渲染 |

```sh
cargo build -p runtime-min --no-default-features --features editor
cargo build -p runtime-min --no-default-features --features runtime-min
```

## 打包遊戲專案

```sh
cargo xtask package --project examples/project --target native --format folder --debug
cargo xtask package --project examples/project --target native --format folder --release
```

輸出位於 `exports/<project>/<target>/<channel>/`，包含執行時二進位、啟動腳本、專案清單、預設場景、資源與 package manifest。

## 建置編輯器

```sh
cd editor
bun install
bun run dev:tauri
bun run tauri build
```

## 測試

```sh
cargo test --workspace
cargo test -p runtime-min --no-default-features --features runtime-min
cargo test -p engine-editor --no-default-features --features agent-tools
cargo test -p engine-render-wgpu
```

## 授權

Mozilla Public License 2.0。詳見 [LICENSE](LICENSE)。
