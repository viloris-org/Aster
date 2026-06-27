# Varg

[![CI](https://github.com/viloris-org/Varg/actions/workflows/core.yml/badge.svg)](https://github.com/viloris-org/Varg/actions/workflows/core.yml)
[![Nightly](https://github.com/viloris-org/Varg/actions/workflows/nightly.yml/badge.svg)](https://github.com/viloris-org/Varg/actions/workflows/nightly.yml)
[![License: MPL-2.0](https://img.shields.io/badge/License-MPL%202.0-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-1.96+-orange.svg)

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-Hant.md) | [日本語](README.ja.md) | 한국어 | [Español](README.es.md)

Varg는 AI 네이티브 게임 엔진입니다. 만들고 싶은 게임을 자연어로 설명하면 자율 Agent 클러스터가 씬, 로직, UI를 구축합니다. 전체 비주얼 에디터도 제공되어 언제든 세부 조정과 마무리, 직접 제어가 가능합니다.

![Varg Editor](docs/screenshots/editor.png)

> **스크린샷 자리표시자**: UI가 안정되면 `docs/screenshots/editor.png`를 실제 에디터 스크린샷으로 교체하세요.

## 빠른 시작

```sh
git clone https://github.com/viloris-org/Varg
cd Varg

# 에디터 실행
cd editor
bun install
bun run dev:tauri
```

> **필수 조건:** [Rust ≥ 1.96](https://rustup.rs/), [Bun ≥ 1.3.14](https://bun.sh/),
> [Tauri 시스템 의존성](https://v2.tauri.app/start/prerequisites/).
> Linux 사용자: `sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev
> libayatana-appindicator3-dev librsvg2-dev`

## 기능

- **AI 네이티브 코어**: 덧붙인 AI 도우미가 아니라, 여러 Agent가 계획, 구축, 리뷰를 자율적으로 수행합니다. 자연어 입력에서 플레이 가능한 씬까지 이어집니다.
- **선언형 게임 설명**: 행동 트리, 씬 그래프, UI 레이아웃, 시스템 설정, 에셋 매니페스트, 프로젝트 구조를 구조화된 데이터로 생성합니다.
- **비주얼 씬 에디터**: 오브젝트 배치, transform 조정, 컴포넌트 추가를 정돈된 인터페이스에서 처리합니다.
- **라이브 플레이 모드**: Play로 물리와 스크립트를 실행하고 Stop으로 즉시 복귀합니다. 편집 씬은 건드리지 않습니다.
- **에셋 파이프라인**: glTF/PNG를 프로젝트 패널에 넣으면 파일 감시, 임포트, 핫 리로드가 이어집니다.
- **플러그형 렌더링**: 엔진 코드를 바꾸지 않고 백엔드를 교체할 수 있으며 WGPU가 포함됩니다.
- **헤드리스 런타임**: 서버, CI, 자동 빌드에서도 같은 엔진을 사용할 수 있습니다.
- **unsafe 코드 없음**: 모든 crate가 `#![forbid(unsafe_code)]`를 사용합니다.

## 프로젝트 구조

```text
Varg/
├── editor/                  # Tauri 데스크톱 앱 (React + Rust)
├── crates/                  # ECS, 에셋, 렌더링, 물리, 오디오, AI 등
├── xtask/                   # 빌드 및 자동화 작업
├── examples/                # 샘플 프로젝트와 씬
└── docs/                    # 설계 노트
```

## 씬 편집

1. 에디터를 실행해 **Hub** 화면으로 이동
2. 프로젝트를 만들거나 열기
3. **Hierarchy** 패널에서 씬의 모든 오브젝트 확인
4. **Inspector**에서 선택한 오브젝트의 transform과 컴포넌트 확인
5. **Scene View**에서 3D 뷰포트를 orbit, pan, zoom
6. **Play**를 눌러 **Game View**에서 물리와 스크립트 실행
7. Camera, Light, MeshRenderer, Rigidbody, Collider 등을 추가하거나 Varg 스크립트 작성

## 빌드 Profile

| Profile | 내용 |
|---|---|
| `editor` | Tauri 프런트엔드를 위한 에디터 서비스, wgpu 뷰포트, Agent 도구 |
| `runtime-min` | 헤드리스: CI smoke tests, 서버, 자동 빌드 |
| `runtime-game` | 헤드리스 + 윈도우 지원 |
| `dev-full` | 에디터, 물리, 오디오, 스크립트, Agent, 렌더링 전체 |

```sh
cargo build -p runtime-min --no-default-features --features editor
cargo build -p runtime-min --no-default-features --features runtime-min
```

## 게임 프로젝트 패키징

```sh
cargo xtask package --project examples/project --target native --format folder --debug
cargo xtask package --project examples/project --target native --format folder --release
```

패키지는 `exports/<project>/<target>/<channel>/`에 작성되며 런타임 바이너리, 런처 스크립트, 프로젝트 매니페스트, 기본 씬, 복사된 에셋과 package manifest를 포함합니다.

## 에디터 빌드

```sh
cd editor
bun install
bun run dev:tauri
bun run tauri build
```

## 테스트

```sh
cargo test --workspace
cargo test -p runtime-min --no-default-features --features runtime-min
cargo test -p engine-editor --no-default-features --features agent-tools
cargo test -p engine-render-wgpu
```

## 라이선스

Mozilla Public License 2.0. 자세한 내용은 [LICENSE](LICENSE)를 참고하세요.
