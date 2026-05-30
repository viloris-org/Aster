#!/usr/bin/env bash
set -e
cd "$(dirname "$0")/../editor"

# Start Vite dev server in background, then Tauri dev
echo "Starting Aster Editor (Tauri) dev mode..."
bun run tauri dev
