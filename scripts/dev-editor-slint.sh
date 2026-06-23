#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

echo "Starting Aster Editor (Slint) dev mode..."
cargo run -p aster-editor-slint -- "$@"
