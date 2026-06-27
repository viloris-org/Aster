# Varg

[![CI](https://github.com/viloris-org/Varg/actions/workflows/core.yml/badge.svg)](https://github.com/viloris-org/Varg/actions/workflows/core.yml)
[![Nightly](https://github.com/viloris-org/Varg/actions/workflows/nightly.yml/badge.svg)](https://github.com/viloris-org/Varg/actions/workflows/nightly.yml)
[![License: MPL-2.0](https://img.shields.io/badge/License-MPL%202.0-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-1.96+-orange.svg)

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-Hant.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | Español

Varg es un motor de juegos nativo de IA. Describe tu juego en lenguaje natural y un clúster de agentes autónomos construye la escena, la lógica y la UI. También incluye un editor visual completo para ajustar, pulir y tomar el control cuando quieras.

![Varg Editor](docs/screenshots/editor.png)

> **Captura provisional**: reemplaza `docs/screenshots/editor.png` por una captura real del editor cuando la UI se estabilice.

## Primeros Pasos

```sh
git clone https://github.com/viloris-org/Varg
cd Varg

# Iniciar el editor
cd editor
bun install
bun run dev:tauri
```

> **Requisitos:** [Rust ≥ 1.96](https://rustup.rs/), [Bun ≥ 1.3.14](https://bun.sh/),
> [dependencias de sistema de Tauri](https://v2.tauri.app/start/prerequisites/).
> Linux: `sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev
> libayatana-appindicator3-dev librsvg2-dev`

## Funciones

- **Núcleo nativo de IA**: no es un asistente añadido encima. Un clúster multiagente planifica, construye y revisa tu juego de forma autónoma.
- **Descripción declarativa del juego**: sistemas declarativos para comportamiento, escena, UI, configuración, assets y estructura de proyecto permiten generar datos estructurados en vez de código frágil.
- **Editor visual de escenas**: coloca objetos, ajusta transformaciones y añade componentes desde una interfaz pulida.
- **Modo Play en vivo**: pulsa Play para ejecutar física y scripts; pulsa Stop para volver sin ensuciar la escena de edición.
- **Pipeline de assets**: arrastra glTF/PNG al panel del proyecto. El watcher importa y el hot reload actualiza en vivo.
- **Renderizado enchufable**: cambia backend sin tocar el código del motor. Incluye WGPU.
- **Runtime headless**: el mismo motor funciona en servidores, CI y builds automatizados.
- **Cero código unsafe**: todos los crates usan `#![forbid(unsafe_code)]`.

## Estructura del Proyecto

```text
Varg/
├── editor/                  # App de escritorio Tauri (React + Rust)
├── crates/                  # ECS, assets, render, física, audio, IA, etc.
├── xtask/                   # Tareas de build y automatización
├── examples/                # Proyecto y escenas de ejemplo
└── docs/                    # Notas de diseño
```

## Editar una Escena

1. Inicia el editor y abre **Hub**
2. Crea o abre un proyecto
3. **Hierarchy** lista todos los objetos de la escena
4. **Inspector** muestra transformaciones y componentes del objeto seleccionado
5. **Scene View** renderiza el viewport 3D con orbit, pan y zoom
6. Pulsa **Play** para ejecutar física y scripts en **Game View**
7. Añade Camera, Light, MeshRenderer, Rigidbody, Collider, etc., o escribe scripts Varg

## Perfiles de Build

| Profile | Qué incluye |
|---|---|
| `editor` | Servicios del editor, viewports wgpu y herramientas de Agent para Tauri |
| `runtime-min` | Headless: smoke tests de CI, servidores y builds automatizados |
| `runtime-game` | Headless + ventanas |
| `dev-full` | Todo: editor, física, audio, scripts, Agent y render |

```sh
cargo build -p runtime-min --no-default-features --features editor
cargo build -p runtime-min --no-default-features --features runtime-min
```

## Empaquetar un Juego

```sh
cargo xtask package --project examples/project --target native --format folder --debug
cargo xtask package --project examples/project --target native --format folder --release
```

El paquete se escribe en `exports/<project>/<target>/<channel>/` e incluye el binario runtime, script de lanzamiento, manifiesto del proyecto, escena por defecto, assets copiados y manifiestos de paquete.

## Construir el Editor

```sh
cd editor
bun install
bun run dev:tauri
bun run tauri build
```

## Tests

```sh
cargo test --workspace
cargo test -p runtime-min --no-default-features --features runtime-min
cargo test -p engine-editor --no-default-features --features agent-tools
cargo test -p engine-render-wgpu
```

## Licencia

Mozilla Public License 2.0. Consulta [LICENSE](LICENSE).
