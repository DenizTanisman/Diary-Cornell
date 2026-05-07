# Android overrides

`tauri android init` regenerates `src-tauri/gen/android/` from scratch and
overwrites every file in there. Things we change by hand (network security
policy, manifest entries) need to be reapplied each time.

This directory holds the tracked source-of-truth versions. After a
`tauri android init` (or after `cargo clean` blows away `gen/`), run:

```bash
bash scripts/apply_android_overrides.sh
```

That script copies each file in this directory to the matching path under
`src-tauri/gen/android/app/src/main/`. Idempotent — safe to re-run.

## Why this exists

MD 03 / Faz 3.3 added a **narrow** cleartext allow-list for Cloud + Bridge
LAN reach. The default `usesCleartextTraffic="true"` was too permissive
(a typo in Cloud Profile editor would silently work). The override config
restricts cleartext to localhost / 127.0.0.1 / 10.0.2.2 / RFC 1918 LAN
ranges; production URLs must be HTTPS.

Sprint B added `app-build.gradle.kts` — release signing config that
reads `keystore.properties` (gitignored sibling file) pointing at the
production keystore in `~/.config/cornell-diary/`. Falls back to debug
signing when the properties file is missing, so a fresh checkout still
produces an installable APK without any secrets.
