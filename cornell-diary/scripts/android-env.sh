#!/usr/bin/env bash
# Source this before running `pnpm tauri android dev/build`.
#
#   . scripts/android-env.sh
#   pnpm tauri android dev
#
# Detects the NDK directory under $ANDROID_HOME and points cc-rs / cargo
# at the right cross-compilers (cc-rs looks for `aarch64-linux-android-clang`
# without an API-level suffix, but the NDK only ships API-suffixed binaries
# — these env vars bridge that gap).

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export ANDROID_SDK_ROOT="$ANDROID_HOME"

# Pick the highest installed NDK; works whether the user has 25, 27, 30, …
NDK_DIR="$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1)"
if [ -z "$NDK_DIR" ]; then
    echo "android-env: no NDK found under $ANDROID_HOME/ndk/ — install one via Android Studio → SDK Manager → SDK Tools → NDK." >&2
    return 1 2>/dev/null || exit 1
fi
export NDK_HOME="$NDK_DIR"
export ANDROID_NDK_HOME="$NDK_DIR"

# minSdk = 24 (Android 7); matches gen/android/app/build.gradle.kts.
API="${ANDROID_API_LEVEL:-24}"
TOOLCHAIN="$NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin"

for triple in aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android; do
    case "$triple" in
        aarch64-linux-android)    suffix="aarch64-linux-android${API}" ;;
        armv7-linux-androideabi)  suffix="armv7a-linux-androideabi${API}" ;;
        i686-linux-android)       suffix="i686-linux-android${API}" ;;
        x86_64-linux-android)     suffix="x86_64-linux-android${API}" ;;
    esac
    var="$(echo "$triple" | tr '-' '_')"
    export "CC_${var}=${TOOLCHAIN}/${suffix}-clang"
    export "CXX_${var}=${TOOLCHAIN}/${suffix}-clang++"
    export "AR_${var}=${TOOLCHAIN}/llvm-ar"
    upper="$(echo "$var" | tr '[:lower:]' '[:upper:]')"
    export "CARGO_TARGET_${upper}_LINKER=${TOOLCHAIN}/${suffix}-clang"
done

echo "android-env: NDK_HOME=$NDK_HOME, API=$API"
