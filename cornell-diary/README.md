# Cornell Diary

Offline-first, cross-platform personal diary app built with **Tauri 2.0** and **React 19**. The classic Cornell note-taking method, adapted for daily journaling — dynamic cue sections on the left, a spacious main notes area on the right, and a summary + quote bar at the bottom.

## Highlights

- **Small & fast** — ships as a lightweight Tauri binary (orders of magnitude smaller than Electron).
- **Local-first & private** — every entry stays in a local SQLite database on your device. No cloud, no telemetry.
- **Cross-platform ready** — macOS today; iOS and Android share the same React + Rust codebase.
- **Repository & Strategy patterns** — the data layer is behind an `IDiaryRepository` interface, so you can swap SQLite for a remote (e.g. future Jarvis API) repository without touching UI.
- **Manual sync** — export/import the archive as JSON, or transfer between devices via chunked, animated QR codes.
- **TypeScript strict mode**, Vitest unit + integration tests, and Turkish + English localization out of the box.

## Tech Stack

Tauri 2 · React 19 · TypeScript · SQLite (`tauri-plugin-sql`) · Vite · Zustand · Zod · date-fns · react-router · qrcode + qr-scanner · Vitest

## Architecture

```
UI (React + TypeScript)
   │
Hooks (useDiary, useDateNavigator, useTheme, useKeyboardShortcuts)
   │
Repository Interface (IDiaryRepository)
   │
SQLiteRepository  ◄─────── future: JarvisAPIRepository
   │
Tauri plugins (sql / fs / dialog / os / clipboard-manager)
   │
SQLite database (local)
```

Sync is a separate module: `exporter` → checksummed JSON, `importer` → schema-validated `bulkUpsert`, with last-write-wins conflict resolution and optional QR chunking for large archives.

## Requirements

- macOS (Phase A) with Xcode Command Line Tools for building
- Node.js ≥ 20
- pnpm ≥ 9
- Rust toolchain (`rustup`) ≥ 1.80

## Getting Started

```bash
pnpm install

# Desktop dev server (opens the Tauri window)
pnpm tauri dev

# Production build (macOS .dmg / .app)
pnpm tauri build

# Frontend-only dev (in-browser, no DB access)
pnpm dev

# Run tests
pnpm test
pnpm test:coverage
pnpm typecheck
```

On first launch the app creates `cornell_diary.db` in the OS application-data directory and seeds `app_settings`.

## Keyboard Shortcuts

| Shortcut      | Action             |
| ------------- | ------------------ |
| `⌘/Ctrl + S`  | Save immediately   |
| `⌘/Ctrl + ←`  | Previous day       |
| `⌘/Ctrl + →`  | Next day           |
| `⌘/Ctrl + T`  | Go to today        |

## Folder Layout

```
src/
  components/
    cornell/      # CornellLayout, DateHeader, CueSection, MainNotesArea, SummaryBar
    sync/         # ExportDialog, ImportDialog, QRGenerator, QRScanner
    common/       # AppToolbar, DateNavigator, SaveIndicator, ErrorBoundary
    ui/           # Modal primitives
  db/             # IDiaryRepository, SQLiteRepository, RepositoryContext
  hooks/          # useDiary, useDateNavigator, useTheme, useKeyboardShortcuts
  sync/           # exporter, importer, conflictResolver, qrChunker, qrAssembler
  stores/         # settingsStore, syncStore (Zustand)
  utils/          # date, crypto, deviceId, sanitize, validation, logger
  locales/        # tr.json, en.json + tiny t() function
  pages/          # DiaryPage, ArchivePage, SettingsPage, SyncPage, NotFoundPage
  styles/         # globals.css, cornell.css, themes.css
src-tauri/
  migrations/     # 001_initial.sql
  capabilities/   # default.json (Tauri 2 permissions)
  src/lib.rs      # Registers all plugins + migrations
tests/
  unit/           # date, crypto, exporter, importer, conflictResolver,
                  # qrChunker, stores, locales, sanitize, repositoryMapping, SaveIndicator
  integration/    # useDiary
```

## Data Format (Sync v1.0)

Exports are plain JSON with a SHA-256 checksum over a canonicalized entries array. Schema validation uses Zod; checksum mismatches require explicit user confirmation to import.

```json
{
  "$schema": "https://cornell-diary.local/schema/v1.json",
  "format": "cornell-diary-export",
  "version": "1.0.0",
  "exportedAt": "...",
  "deviceId": "host-abc12345",
  "entryCount": 42,
  "checksum": "sha256:...",
  "entries": [ { "date": "YYYY-MM-DD", "diary": "...", "cueItems": [...], "summary": "...", "quote": "...", "createdAt": "...", "updatedAt": "...", "version": 1 } ]
}
```

## Security Notes

- All SQL uses parameterized queries (`$1, $2, …`) — no string concatenation.
- Tauri capability scope limits filesystem access to `$APPDATA`, `$DOCUMENT`, `$DOWNLOAD`, `$HOME`.
- Diary content never hits application logs; errors are logged at warn/error only.
- No `dangerouslySetInnerHTML`; React escapes all user content.

## Future: Jarvis Integration

Because the UI talks to `IDiaryRepository`, you can add a `JarvisAPIRepository` that calls a remote HTTP API and swap it in via DI without touching any component.

## License

MIT
