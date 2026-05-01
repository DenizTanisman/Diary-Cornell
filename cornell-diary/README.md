# Cornell Diary

> **Status: Faz 1 in progress.** Core REST + WS + CRDT path is shipping; the
> [Yol Haritası](../YOL_HARITASI.md) tracks the hardening work (CRDT-aware merge,
> backups, observability, Android APK). Default behaviour is unchanged — every
> new path lands behind a feature flag (see [.env.example](.env.example) +
> "Feature flags" below).

Cross-device personal diary built on **Tauri 2 + React 19 + Postgres** (desktop) /
**SQLite** (Android, iOS), with optional real-time multi-user editing via a char-level
CRDT over WebSockets. Classic Cornell layout — dynamic cue sections on the left, a
spacious main notes area on the right, summary + quote bar at the bottom.

## Feature flags (Faz 1+ rollout)

| Flag                   | Default   | What it controls                              | Faz |
| ---------------------- | --------- | --------------------------------------------- | --- |
| `SYNC_MERGE_STRATEGY`  | `lmw`     | Server merge: last-write-wins vs CRDT replay  | 1.1 |
| `PROMETHEUS_ENABLED`   | `false`   | Cloud `/metrics` endpoint                     | 1.3 |
| `SENTRY_DSN`           | (empty)   | Error reporting (no-op if empty)              | 1.3 |
| `DIARY_CLOUD_URL`      | (empty)   | Build-time LAN URL baked into Android APK     | 1.4 |
| `ENABLE_CRDT_GC`       | `false`   | Tombstone GC in snapshot loop                 | 2.2 |
| `BROADCAST_BACKEND`    | `memory`  | WS bus backend (memory \| redis)              | 3.1 |

Defaults preserve the v1.0 behaviour. Opt-in flips one path at a time so a regression in
the new code never blocks the old.

## Highlights

- **Postgres-backed** local store. Single backend across desktop and mobile, no SQLite fallback.
  Schema in [`src-tauri/postgres_migrations/`](src-tauri/postgres_migrations).
- **Optional Cloud sync (FAZ 2)** — REST round-trip (`/auth/login`, `/journals`, `/pull`, `/push`).
  Hourly + on-network-recovery + manual triggers. Last-write-wins on `version` with an
  `updated_at` tie-break and a per-row `is_dirty` flag.
- **Live multi-user edit (FAZ 3)** — RGA-style char-level CRDT in Rust
  (`src-tauri/src/crdt/`), driven by a single shared WebSocket. Off-line keystrokes land in
  `pending_ops` and drain on reconnect. Convergence is property-tested with 200 random ops in 3
  orderings.
- **Cross-platform ready** — same React + Rust codebase for macOS today; iOS / Android share the
  Tauri runtime.
- **TypeScript strict mode**, Vitest, Postgres-gated Rust integration tests, Turkish + English
  localization.

## Tech stack

Tauri 2 · React 19 · TypeScript · Vite · Zustand · Zod · Postgres 16 (sqlx) · tokio-tungstenite ·
reqwest (rustls) · jsonwebtoken · tokio-cron-scheduler · Vitest.

## Architecture

```
React (UI, hooks: useDiary, useCrdtChannel, useSyncStatus)
  │  invoke() / listen()
Tauri commands (commands/{entries,sync,crdt}.rs)
  │
Repository (EntryRepository) ── Sync engine (REST) ── WS client (live CRDT)
  │                              │                    │
  └───────── Postgres (sqlx pool) ───────────────────  └── pending_ops queue
                                                          + CrdtDocument map
```

Cloud sync and CRDT are **opt-in** — Diary works fully offline against just a local Postgres.

## Requirements

- macOS for Phase A (iOS + Android land via the same Tauri runtime).
- Node.js ≥ 20, pnpm ≥ 9.
- Rust toolchain ≥ 1.80.
- Postgres 16, reachable on the URL in `DATABASE_URL` (a local Docker container is the standard
  dev setup — see `docker-compose.yml`).
- Cloud server (optional, only for FAZ 2/3) — see `journal_ai_reporter` repo.

## Getting started

```bash
# 1. Postgres (via Docker compose or your own instance)
docker compose up -d diary_postgres

# 2. Env
cp .env.example .env   # then edit DATABASE_URL / CLOUD_URL as needed

# 3. Run
pnpm install
pnpm tauri dev         # full app (Tauri window + Vite dev server)
pnpm dev               # frontend-only, no Rust / DB

# 4. Tests
DATABASE_URL=... cargo test --manifest-path src-tauri/Cargo.toml --lib   # 47 tests
pnpm test                                                                # 58 tests
pnpm typecheck
```

The Rust setup hook reads `DATABASE_URL` directly from the environment (no automatic `.env`
loading). Either export the vars in your shell or use a wrapper script — see
[OPERATIONS.md](OPERATIONS.md#environment).

## Keyboard shortcuts

| Shortcut       | Action            |
| -------------- | ----------------- |
| `⌘/Ctrl + S`   | Save immediately  |
| `⌘/Ctrl + ←`   | Previous day      |
| `⌘/Ctrl + →`   | Next day          |
| `⌘/Ctrl + T`   | Go to today       |

## Folder layout

```
src/
  components/cornell/   CornellLayout, MainNotesArea, PresenceBadge, …
  components/sync/      CloudSyncPanel, SyncIndicator, ExportDialog, …
  hooks/                useDiary, useCrdtChannel, useSyncStatus, …
  db/                   IDiaryRepository (TS contract), TauriRepository (invoke wrapper)
  sync/                 exporter / importer / qrChunker (manual JSON / QR sync)
  types/                diary, cloudSync, crdt
  pages/                DiaryPage, ArchivePage, SyncPage, SettingsPage
src-tauri/
  postgres_migrations/  0001_initial, 0002_sync_metadata, 0003_pending_ops
  src/db/               EntryRepository trait + Postgres impl
  src/sync/             CloudClient, SyncEngine, AuthManager, network monitor
  src/crdt/             CharNode + CrdtDocument + WsClient + pending_ops
  src/commands/         Tauri command handlers (entries, sync, crdt)
tests/
  unit/                 frontend Vitest unit tests
  integration/          useDiary integration tests
docs:
  OPERATIONS.md         migration / rollback / day-2 ops
  SYNC_BEHAVIOR.md      sync / CRDT semantics, pending_ops, conflicts
  THREAT_MODEL.md       OWASP review, accepted risks, capabilities
```

## Sync data format

Manual JSON exports keep the v1.0 envelope (Zod-validated, SHA-256 checksum):

```json
{
  "$schema": "https://cornell-diary.local/schema/v1.json",
  "format": "cornell-diary-export",
  "version": "1.0.0",
  "exportedAt": "...",
  "deviceId": "...",
  "entryCount": 42,
  "checksum": "sha256:...",
  "entries": [ { "date": "YYYY-MM-DD", "diary": "...", "cueItems": [...], … } ]
}
```

Cloud REST sync uses Cloud's `PushRequest` / `PullResponse` shapes — see
[`src-tauri/src/sync/models.rs`](src-tauri/src/sync/models.rs).

CRDT ops match Cloud's `CRDTOpDTO` (`op_type`, `char_id`, `char_value`, `prev_id`, `peer_id`,
`lamport`, `seq`) so the same JSON works in both directions without translation.

## Security & threat model

See [THREAT_MODEL.md](THREAT_MODEL.md). Highlights:

- All SQL goes through sqlx parameterized queries.
- Tauri capability scope locks filesystem access to `$APPDATA`, `$DOCUMENT`, `$DOWNLOAD`, `$HOME`.
- The user's Cloud password is only on the wire during `connect_cloud`; it never lands in the DB
  or logs. JWT tokens are stored in `sync_metadata` (encrypted-at-rest at Postgres level only —
  full-disk encryption assumed, not in-app secret management).
- Two known accepted risks from `cargo audit` (RUSTSEC-2023-0071 in `rsa` via transitive
  `sqlx-mysql`, RUSTSEC-2024-0413 in `atk` via gtk3) — both deep transitives we can't drop, both
  documented.

## License

MIT
