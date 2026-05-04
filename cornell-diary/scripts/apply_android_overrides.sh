#!/usr/bin/env bash
# Replay tracked Android overrides over the freshly-generated tauri files.
# Run after `tauri android init` (or any time `src-tauri/gen/` was wiped).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$REPO_ROOT/android-overrides"
DST="$REPO_ROOT/src-tauri/gen/android/app/src/main"

if [[ ! -d "$DST" ]]; then
  echo "ERROR: $DST not found. Run 'tauri android init' first." >&2
  exit 1
fi

cp "$SRC/AndroidManifest.xml" "$DST/AndroidManifest.xml"
mkdir -p "$DST/res/xml"
cp "$SRC/res/xml/network_security_config.xml" "$DST/res/xml/network_security_config.xml"

echo "✓ Android overrides applied to $DST"
