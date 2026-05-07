#!/usr/bin/env bash
# Replay tracked Android overrides over the freshly-generated tauri files.
# Run after `tauri android init` (or any time `src-tauri/gen/` was wiped).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$REPO_ROOT/android-overrides"
APP_DST="$REPO_ROOT/src-tauri/gen/android/app"
MAIN_DST="$APP_DST/src/main"

if [[ ! -d "$MAIN_DST" ]]; then
  echo "ERROR: $MAIN_DST not found. Run 'tauri android init' first." >&2
  exit 1
fi

# Manifest + network security config — restricts cleartext traffic to
# loopback / RFC 1918 LAN ranges so a typo in Cloud Profile doesn't
# accidentally allow plain-HTTP to the internet.
cp "$SRC/AndroidManifest.xml" "$MAIN_DST/AndroidManifest.xml"
mkdir -p "$MAIN_DST/res/xml"
cp "$SRC/res/xml/network_security_config.xml" "$MAIN_DST/res/xml/network_security_config.xml"

# Sprint B — release signing config. The override reads a sibling
# keystore.properties (gitignored, points to ~/.config/cornell-diary/)
# so release APKs are signed with the production key. Without the
# properties file it falls back to debug signing.
cp "$SRC/app-build.gradle.kts" "$APP_DST/build.gradle.kts"

echo "✓ Android overrides applied to $MAIN_DST + $APP_DST/build.gradle.kts"
