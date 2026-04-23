# Diary Cornell

Offline-first, cross-platform personal diary built with **Tauri 2** and **React 19**. The classic Cornell note-taking method, adapted for daily journaling — dynamic cue sections on the left, a spacious main notes area on the right, and a summary + quote bar at the bottom.

The app runs natively on **macOS** and **Android** today, shares a single TypeScript + Rust codebase, and stores every entry in a local SQLite database. No cloud, no telemetry.

---

## Downloads

Ready-to-install builds are published on the [**Releases**](https://github.com/DenizTanisman/Diary-Cornell/releases) page:

| Platform | Artifact | Notes |
| -------- | -------- | ----- |
| macOS (Apple Silicon) | `Cornell Diary_<version>_aarch64.dmg` | Unsigned dev build — right-click → Open the first time. |
| Android (arm64) | `cornell-diary-<version>-arm64.apk` | Enable "Install unknown apps" for your browser / file manager. |

> **Note:** builds are unsigned development artifacts. For production distribution (Apple notarization, Play Store) additional signing steps are required — see [Building from source](#building-from-source).

---

## Highlights

- **Small & fast** — Tauri binaries are orders of magnitude smaller than an Electron equivalent.
- **Local-first & private** — every entry stays in a SQLite database on-device. No cloud sync unless you export.
- **Cornell layout** — dynamic cue list, main notes, summary, and daily quote, all in one view.
- **Manual sync** — export/import as checksummed JSON, or transfer between devices via chunked animated QR codes.
- **Repository pattern** — the data layer is behind an `IDiaryRepository` interface. SQLite today, a remote Jarvis API later — the UI doesn't change.
- **TypeScript strict mode**, 44+ Vitest tests, Turkish + English localization.

## Screenshots

> _Add screenshots to `docs/screenshots/` and reference them here once captured._

---

## Project layout

```
Diary-Cornell/
├── README.md                 ← you are here
└── cornell-diary/            ← the Tauri + React app
    ├── src/                  ← React frontend
    ├── src-tauri/            ← Rust backend, capabilities, migrations
    ├── tests/                ← Vitest unit + integration
    └── README.md             ← detailed developer docs
```

All `pnpm` / `cargo` / `tauri` commands below assume you are inside `cornell-diary/`.

---

## Building from source

### Requirements

- Node.js ≥ 20, pnpm ≥ 9
- Rust toolchain (`rustup`) ≥ 1.80
- macOS build: Xcode Command Line Tools
- Android build: Android Studio (SDK + NDK), `$ANDROID_HOME` and `$NDK_HOME` exported, plus the Rust targets:
  ```bash
  rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
  ```

### Install dependencies

```bash
cd cornell-diary
pnpm install
```

### Desktop dev (macOS)

```bash
pnpm tauri dev          # hot-reload dev
pnpm tauri build        # release .dmg + .app in src-tauri/target/release/bundle/
```

### Android APK

```bash
# one-time on a fresh checkout (scaffolds src-tauri/gen/android/):
pnpm tauri android init

# dev build on a connected device or emulator:
pnpm tauri android dev

# release APK (unsigned):
pnpm tauri android build --apk

# output:
# src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
```

For a signed APK, generate a keystore and wire it into `src-tauri/gen/android/app/build.gradle.kts` — see the [Tauri Android signing guide](https://v2.tauri.app/distribute/sign/android/).

### Tests & typecheck

```bash
pnpm test              # vitest run
pnpm test:coverage
pnpm typecheck
```

---

## Sync format (v1.0)

Exports are plain JSON with a SHA-256 checksum over a canonicalized entries array. The importer validates with Zod and rejects checksum mismatches unless the user explicitly overrides.

```json
{
  "$schema": "https://cornell-diary.local/schema/v1.json",
  "format": "cornell-diary-export",
  "version": "1.0.0",
  "exportedAt": "...",
  "deviceId": "host-abc12345",
  "entryCount": 42,
  "checksum": "sha256:...",
  "entries": [ /* { date, diary, cueItems, summary, quote, createdAt, updatedAt, version } */ ]
}
```

On Android the export flow uses the system **Storage Access Framework** picker, so files land directly in Downloads, Drive, or any storage provider the user chooses — no extra permissions required.

---

## Security notes

- All SQL uses parameterized queries.
- Tauri capabilities restrict filesystem access to `$APPDATA`, `$APPLOCALDATA`, `$APPCACHE`, `$DOCUMENT`, `$DOWNLOAD`, `$HOME`.
- Diary content is never written to logs.
- No `dangerouslySetInnerHTML`; React escapes all user content.

---

## Roadmap

- iOS build (share codebase with Android)
- Signed Play Store and Apple-notarized releases
- Jarvis remote repository (optional cloud sync while preserving local-first default)
- Rich-text Cornell cells, tag/search across the archive

---

## License

[MIT](cornell-diary/LICENSE)
