#!/usr/bin/env bash
set -e
cd "$(dirname "$0")/../editor"

# Build frontend then Tauri app bundle
echo "Building frontend..."
bun run build

echo "Building Tauri app..."
bun run tauri build

echo "Done: editor built to src-tauri/target/release/"
