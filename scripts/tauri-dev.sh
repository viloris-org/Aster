#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root/editor"

"$repo_root/scripts/check-linux-inotify-capacity.py" --startup-only

if [[ "$(uname -s)" == "Linux" ]]; then
  export GDK_BACKEND=x11
  export WINIT_UNIX_BACKEND=x11
  # The X11 native host path uses split child WebView panels by default.
  # Set ASTER_NATIVE_PANEL_WEBVIEWS=0 to disable them for diagnostics.
fi

if "$repo_root/scripts/check-linux-inotify-capacity.py" --inotify-only --quiet; then
  exec bunx tauri dev "$@"
fi

"$repo_root/scripts/check-linux-inotify-capacity.py" --inotify-only
exit 1
