# Cornell Diary — System Architecture

> **Status:** Living document, code-driven. Reflects the codebase as of 2026-05-05.
> **Scope:** Diary (Tauri desktop app) + Cloud (FastAPI sync server) + journal_ai_reporter (Gemini-backed reporting microservice).
> **Owner:** Deniz Tanışma. Single source of architectural truth — supersedes every file under [docs/archive/](docs/archive/).

---

## 1. Executive summary

Cornell Diary is a **privacy-first, offline-first journal application** that uses the Cornell note-taking method (one diary column + seven cue title/content pairs + summary + quote slot per day). It runs as a native desktop app on macOS / Windows / Linux via Tauri 2.x, with a planned Android build (next sprint). The user's data lives locally first and syncs to a self-hosted Cloud server only when the user opts in.

The system has three deployable units, each independently versioned:

| Unit                          | Purpose                                               | Stack                                | Default port |
| ----------------------------- | ----------------------------------------------------- | ------------------------------------ | ------------ |
| **Diary** (this repo)         | Local-first editor + sync client + Cloud supervisor   | Tauri 2.11 + React 19 + TypeScript + Rust | UI WebView (1420 dev) |
| **Cloud** (`~/Projects/Cloud`) | Multi-device sync server + CRDT WebSocket + auth     | FastAPI + SQLAlchemy + Postgres 16   | `:5001` (HTTP), `:5434` (Postgres) |
| **journal_ai_reporter**       | Tag-driven AI reports over journal entries (Gemini)   | FastAPI + asyncpg + google-generativeai | `:8002` (Bridge), `:8001` (Sidecar) |

The reporter is **currently inactive**: the on-device LLM panel that consumed it was removed from Diary on 2026-05-05 (commit [`1a6a620`](../../commit/1a6a620)). Its code remains in tree because ImaginingJarvis (Anthropic's chat layer) still uses it, but Diary itself no longer talks to it.

---

## 2. System topology

```
┌──────────────────────────────────────────────────────────────────────┐
│                          User's Machine                               │
│                                                                       │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │  Cornell Diary (Tauri app, com.deniz.cornelldiary)            │    │
│  │                                                                │    │
│  │  WebView (React + Zustand)  ◄──── tauri::invoke ────►  Rust   │    │
│  │  ┌──────────────────────┐                          ┌────────┐ │    │
│  │  │ Pages: Diary/Archive/│                          │ 35 IPC │ │    │
│  │  │ Sync/Settings        │                          │ cmds   │ │    │
│  │  │ + CRDT live editor   │                          └───┬────┘ │    │
│  │  │ + Cloud panel        │                              │      │    │
│  │  └──────────────────────┘                              │      │    │
│  └────────────────────────────────────────────────────────┼──────┘    │
│                                                            │           │
│                          ┌─────────────────────────────────┴──────┐    │
│                          │ SyncEngine • CRDT WsClient • Scheduler │    │
│                          │ • CloudServiceSupervisor               │    │
│                          └────┬───────────────────┬───────────────┘    │
│                               │                   │                    │
│                  ┌────────────▼────────┐ ┌───────▼─────────────────┐   │
│                  │ Local DB (sqlx)     │ │ Spawn child process     │   │
│                  │ • postgres (desktop)│ │ • bash start_postgres.sh│   │
│                  │ • sqlite (mobile)   │ │ • .venv/bin/uvicorn      │   │
│                  └─────────────────────┘ └────────┬────────────────┘   │
│                                                   │                    │
│  ┌────────────────────────────────────────────────▼────────────────┐   │
│  │  Cloud (FastAPI on :5001)                                        │   │
│  │  • /auth/*  • /sync/{pull,push}  • /journals  • /entries         │   │
│  │  • /ws/journal/{id}   ◄── tokio-tungstenite WS over JWT          │   │
│  │  • /health/{live,ready}  • /metrics (Prometheus)                  │   │
│  │                                                                    │   │
│  │  ┌──────────────────────┐    ┌─────────────────────────────┐     │   │
│  │  │ asyncpg / SQLAlchemy │───►│ Postgres 16 (Docker, :5434) │     │   │
│  │  └──────────────────────┘    └─────────────────────────────┘     │   │
│  └─────────────────────────────────────┬────────────────────────────┘   │
│                                         │                                │
│  ┌──────────────────────────────────────▼─────────────────────────┐     │
│  │  journal_ai_reporter (separate repo, currently inactive)        │     │
│  │  • Cornell Sidecar :8001 — read-only HTTP over Postgres        │     │
│  │  • Reporter Bridge :8002 — Gemini-backed /report endpoint      │     │
│  └────────────────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────────────┘
                              ▲
                              │ HTTPS (in production: Caddy reverse proxy + ACME)
                              │
                  ┌───────────▼─────────────┐
                  │ Other devices: phone /  │
                  │ tablet / second laptop  │
                  └─────────────────────────┘
```

**Key invariants of the topology**
- The **same user** can run Diary on N devices; each registers its own `peer_id` with Cloud and is identified by a server-issued UUID.
- The **same Cloud instance** can host multiple users (auth-gated). Diary's `sync_metadata` table is a singleton row, so one Diary install = one Cloud account at a time.
- **CRDT operations** are real-time (WebSocket fan-out), but **REST sync** is eventually-consistent (every 2 minutes on the scheduler, or on network-up transitions, or on user click).
- **Reporter bridge** does not appear in the live data flow today; it is documented here so future contributors can revive or excise it intentionally.

---

## 3. Component: Cornell Diary

### 3.1 Source layout

```
cornell-diary/
├── src/                              # React frontend (TypeScript)
│   ├── App.tsx                       # router config (5 routes)
│   ├── pages/                        # Diary / Archive / Sync / Settings / NotFound
│   ├── components/
│   │   ├── cornell/                  # CornellLayout, MainNotesArea, CueSection, DateHeader, …
│   │   ├── sync/                     # CloudSyncPanel, CloudServicePanel, ExportDialog, QR*
│   │   ├── settings/                 # CloudProfileSelector, AutoSyncToggle
│   │   ├── common/                   # AppToolbar, DateNavigator, SaveIndicator, ErrorBoundary
│   │   └── ui/                       # Modal
│   ├── hooks/                        # useDiary, useDateNavigator, useKeyboardShortcuts, …
│   ├── stores/                       # Zustand: settingsStore, syncStore
│   ├── locales/                      # tr.json + en.json + index.ts (useT hook)
│   ├── db/                           # TauriRepository (typed wrapper over invoke())
│   └── types/                        # diary.ts, sync.ts, cloudSync.ts, crdt.ts, …
├── src-tauri/                        # Rust backend
│   └── src/
│       ├── lib.rs                    # setup hook, command registration (367 lines)
│       ├── commands/                 # 35 #[tauri::command] functions, grouped by domain
│       ├── db/                       # EntryRepository trait + postgres_impl + sqlite_impl
│       ├── sync/                     # SyncEngine, scheduler, network monitor, conflict, auth
│       ├── crdt/                     # WsClient, CrdtDocument, ws_proto, pending_ops
│       └── error.rs                  # DomainError envelope
├── postgres_migrations/              # 6 versioned .sql files
├── sqlite_migrations/                # 6 versioned .sql files (dialect-adapted)
├── tests/                            # vitest (frontend, 62 tests)
├── tauri.conf.json                   # bundle id, window size, capabilities
├── Cargo.toml                        # default-features = ["postgres"], opt = ["sqlite"]
└── package.json                      # pnpm scripts: dev, build, test, typecheck
```

### 3.2 Frontend surface

#### Routes (`src/App.tsx`)

| Path             | Component       | Notes                                                     |
| ---------------- | --------------- | --------------------------------------------------------- |
| `/`              | redirect        | `/diary/{todayISO()}`                                     |
| `/diary/:date`   | `DiaryPage`     | Main editor; renders `CornellLayout`                      |
| `/archive`       | `ArchivePage`   | List of entry dates (blank entries are filtered out)      |
| `/sync`          | `SyncPage`      | Cloud panel + Cloud Service panel + manual export/import  |
| `/settings`      | `SettingsPage`  | Theme, language, cloud profile, auto-sync toggle          |
| `*`              | `NotFoundPage`  | 404                                                       |

#### State management (Zustand)

- **`settingsStore`** — `theme: 'light' | 'dark' | 'auto'`, `language: 'tr' | 'en'`, `autoSaveIntervalMs: number`, `hydrate()`. Persisted to `localStorage` keys `cornell-diary:theme`, `cornell-diary:language`.
- **`syncStore`** — `dialog: 'none' | 'export' | 'import' | 'qr-send' | 'qr-scan'`, `lastResult: SyncResult | null`. Ephemeral UI state only.

Outside these two stores, mutable diary content lives inside `useDiary` (a React hook backed by Tauri commands and an autosave timer; `src/hooks/useDiary.ts`).

#### i18n

- `src/locales/tr.json` (default) and `src/locales/en.json` — JSON dictionaries
- `useT()` hook returns a `t(key, params?)` function bound to the active language
- Switching language persists to `settingsStore` and re-renders subscribed components instantly (no refresh needed)

#### Keyboard shortcuts (`src/hooks/useKeyboardShortcuts.ts`)

| Shortcut       | Action                                |
| -------------- | ------------------------------------- |
| `⌘/Ctrl + S`   | Force-save current entry              |
| `⌘/Ctrl + ←`   | Previous day                          |
| `⌘/Ctrl + →`   | Next day                              |
| `⌘/Ctrl + T`   | Jump to today                         |
| `⌘/Ctrl + G`   | Open date picker (header date click also opens it) |

The header date label itself is a button that triggers a hidden `<input type="date">` via `showPicker()`.

### 3.3 Tauri backend — IPC surface

All 35 commands are registered in `src-tauri/src/lib.rs:330-365`. They serialize errors as `DomainError { code, message }` (never stack traces).

#### Entry CRUD (`commands/entries.rs`, 12 commands)

| Command                  | Purpose                                                       |
| ------------------------ | ------------------------------------------------------------- |
| `diary_get_by_date`      | Fetch one entry by ISO date                                   |
| `diary_upsert`           | Create or update one entry (autosave path)                    |
| `diary_delete`           | Delete one entry                                              |
| `diary_list_dates`       | All entry dates **excluding** rows where every user-visible field is blank (filter added 2026-05-05) |
| `diary_list_range`       | Date-range fetch (used by archive)                            |
| `diary_list_all`         | All entries (export path; expensive)                          |
| `diary_search`           | Full-text search across diary/cue/summary/quote               |
| `diary_entry_count`      | Total entries (settings page stats)                           |
| `diary_last_updated_at`  | Timestamp of most-recently-edited entry                       |
| `diary_bulk_upsert`      | Protective bulk insert (skips existing dates)                 |
| `diary_get_setting` / `diary_set_setting` | Read/write app_settings rows                  |

#### Sync (`commands/sync.rs`, 10 commands)

| Command                       | Purpose                                                     |
| ----------------------------- | ----------------------------------------------------------- |
| `connect_cloud`               | username/password login; picks or creates a journal         |
| `disconnect_cloud`            | Revoke tokens, clear sync_metadata                          |
| `trigger_sync`                | One full pull → conflict-resolve → push cycle               |
| `get_sync_status`             | `{enabled, online, dirtyCount, lastPullAt, lastPushAt}`     |
| `forgot_password_cloud`       | Send reset email via Cloud's `/auth/forgot-password`        |
| `reset_password_cloud`        | Submit token + new password via Cloud's `/auth/reset-password` |
| `get_auto_sync_enabled`       | Read live scheduler flag (falls back to setting if scheduler not booted) |
| `set_auto_sync_enabled`       | Persist + flip live scheduler (every 2 min)                 |
| `get_auto_start_cloud`        | Should Diary spawn local Cloud at launch?                   |
| `set_auto_start_cloud`        | Toggle the above                                            |

#### CRDT (`commands/crdt.rs`, 4 commands)

| Command            | Purpose                                                    |
| ------------------ | ---------------------------------------------------------- |
| `subscribe_crdt`   | Open WS subscription for `(entry_date, field_name)`; mirror entry; return materialised text |
| `apply_local_op`   | Broadcast one keystroke as a `CharOp::Insert` or `CharOp::Delete` |
| `apply_local_text` | Diff full textarea content against last known text and broadcast minimal ops |
| `unsubscribe_crdt` | Stop mirroring one field (socket stays open)               |

#### Cloud Profile (`commands/profile.rs`, 5 commands)

`list_cloud_profiles`, `get_active_cloud_profile`, `set_active_cloud_profile`, `upsert_cloud_profile`, `delete_cloud_profile`. Switching the active profile clears auth and flags `pending_restart=true`; the new URL is read on next launch (deliberately not hot-swapped — see [§9.2](#92-non-obvious-decisions)).

#### Cloud Service (`commands/cloud_service.rs`, 3 commands)

| Command                | Purpose                                                            |
| ---------------------- | ------------------------------------------------------------------ |
| `start_cloud_service`  | Spawn `bash scripts/start_postgres.sh` (if :5434 idle) + uvicorn child on :5001 |
| `stop_cloud_service`   | `child.kill()` + `child.wait()`; tear down postgres container if Diary started it |
| `cloud_service_status` | TCP probe of `127.0.0.1:5001` (500ms timeout). Returns `{state: 'idle' \| 'starting' \| 'running' \| 'error', pid, healthy, lastError}` |

The auto-start hook in `lib.rs:265-314` calls the same `start_cloud_service_internal` function during the setup hook if the toggle is on.

### 3.4 Tauri State

| State                     | Holds                                                  | Where used                              |
| ------------------------- | ------------------------------------------------------ | --------------------------------------- |
| `AppState`                | `Arc<dyn EntryRepository>`, optional pg_pool           | All entry/setting commands              |
| `SyncState`               | `Arc<SyncEngine>`, `NetworkMonitor`                    | Sync + auth + auto-sync commands        |
| `AutoSyncState`           | `Arc<OnceCell<AutoSyncHandle>>`                        | Get/set auto-sync (lazy-init)           |
| `ProfileState`            | `Arc<dyn CloudProfileRepository>`                      | Profile CRUD                            |
| `CrdtState`               | `Arc<WsClient>` (lazy-connect)                         | CRDT subscribe/op commands              |
| `CloudServiceState`       | `Arc<Mutex<CloudInner>>` (child + started_postgres)    | Cloud spawn / stop / status             |

### 3.5 Setup hook order (`lib.rs:133-328`)

1. **Sentry init** (lines 103-117) — opt-in via `SENTRY_DSN` env var; no-op if empty.
2. **Tracing init** (119-125) — `RUST_LOG`-driven; logs to stderr.
3. **Plugins** (128-132) — opener, fs, dialog, os, clipboard-manager.
4. **Database setup** (133-158) — resolve `DATABASE_URL` (Postgres) or app-data path (SQLite), build pool, run migrations, store `AppState`.
5. **Cloud profile resolution** (164-185) — env var > active profile > default `http://127.0.0.1:5001`.
6. **Sync engine** (186-194) — `CloudClient` + `AuthManager` + `SyncEngine`, store `SyncState`.
7. **Network monitor** (196-201) — 30 s probe loop on `/health`, broadcasts state changes; sync triggers on offline→online edges.
8. **Auto-sync scheduler** (205-258) — read setting (default ON), spawn `sync::scheduler::start()` on Tauri's runtime (NOT inside setup hook — see [§9.1](#91-macos-main-thread-rule)).
9. **Cloud service auto-start** (265-314) — read toggle (default OFF), spawn `start_cloud_service_internal` if on.
10. **CRDT WS client** (316-326) — create lazy `WsClient` (socket opens on first `subscribe_crdt`).
11. **Command handler registration** (330-365).

### 3.6 Database

Two backends, **chosen at compile time** via Cargo features:

| Feature              | When                | URL resolution                                    |
| -------------------- | ------------------- | ------------------------------------------------- |
| `postgres` (default) | Desktop / dev       | `DATABASE_URL` env var (e.g. `postgres://diary_user:…@localhost:5435/diary_db`) |
| `sqlite`             | Android (planned)   | `{app_data_dir}/cornell_diary.db` (auto-create)   |

Both backends implement the **same `EntryRepository` trait** (`db/repository.rs`); the rest of the code never knows which is in use.

#### Migrations (6 versions, dialect-adapted in both backends)

| Version | Adds                                                                      |
| ------- | ------------------------------------------------------------------------- |
| 0001    | `diary_entries` (wide schema: title_1..7, content_1..7), `sync_log`, `app_settings` |
| 0002    | `sync_metadata` singleton (CHECK id=1) — peer_id, JWT pair, timestamps, sync_enabled, device_label |
| 0003    | `pending_ops` queue for unpushed CRDT operations (JSONB payload, indexed on (pushed, created_at)) |
| 0004    | `baseline_version` constraint (last server-acked version per entry — used by Faz 1.1 merge strategy) |
| 0005    | `cloud_profiles` table + seeded `local` (active) and `production` (inactive) rows |
| 0006    | `llm_settings` singleton + `ai_summary` / `ai_tags` / `ai_sentiment` / `ai_generated_at` columns on `diary_entries` (now unused after 2026-05-05 LLM removal — left in place to avoid an irreversible migration) |

#### Schema highlights

```text
diary_entries
  date                  TEXT PRIMARY KEY (ISO 8601)
  diary, summary, quote TEXT
  title_1..7            TEXT  ← wide schema; flattened to CueItem[] in TypeScript
  content_1..7          TEXT
  device_id             TEXT  (originating device)
  version               BIGINT (optimistic lock)
  cloud_entry_id        UUID, cloud_journal_id UUID, is_dirty BOOL,
  last_synced_at        TIMESTAMPTZ
  ai_summary, ai_tags, ai_sentiment, ai_generated_at  ← unused since 2026-05-05
  created_at, updated_at
  INDEX (updated_at DESC), INDEX (is_dirty) WHERE is_dirty, INDEX (cloud_entry_id)

sync_metadata        ← SINGLETON (CHECK id = 1) so Diary is single-account
  peer_id, cloud_user_id, cloud_journal_id
  access_token, refresh_token, token_expires_at  ← never logged
  last_pull_at, last_push_at, last_full_sync_at, sync_enabled, device_label

pending_ops
  id BIGSERIAL, entry_date TEXT FK CASCADE, field_name TEXT,
  op_payload JSONB, created_at, pushed BOOL
  INDEX (pushed, created_at) WHERE pushed = false

app_settings
  key TEXT PK, value TEXT, updated_at TIMESTAMPTZ
  Active keys: theme, language, auto_save_interval_ms, first_launch_date,
               auto_sync_enabled, auto_start_cloud_on_launch

cloud_profiles
  id TEXT PK, name, base_url, api_key, is_active BOOL UNIQUE-when-true,
  last_used_at, created_at, updated_at
  Seeded: 'local' (http://localhost:5001, active), 'production' (empty, inactive)
```

### 3.7 Sync engine

```
                         every 2 min                       offline→online
       UI button         (scheduler)                       (network monitor)
            \                |                                  |
             \               v                                  v
              ╰────────►  SyncEngine::run_full_cycle()  ◄──────╯
                          │
                          │  cycle_lock: Mutex<()> serialises
                          ▼
                  ┌───────────────────────┐
                  │ 1. AuthManager refresh │ ← refresh JWT if near expiry
                  ├───────────────────────┤
                  │ 2. PULL                │ → GET /journals/{j}/entries?since=…
                  │    + conflict resolver │
                  ├───────────────────────┤
                  │ 3. PUSH                │ → POST /journals/{j}/entries
                  │    (only is_dirty=true)│
                  ├───────────────────────┤
                  │ 4. update sync_metadata│
                  └───────────────────────┘
```

**Conflict resolution** (`sync/conflict.rs`) — comparing local row with cloud row, considering `version` (primary) and `updated_at` (tie-break):

| Local state       | Cloud version vs. local   | Decision                       |
| ----------------- | ------------------------- | ------------------------------ |
| not present       | —                         | `InsertCloud`                  |
| clean             | cloud > local             | `OverwriteWithCloud`           |
| clean             | cloud ≤ local             | `LocalAlreadyFresher` (no-op)  |
| dirty             | cloud > local AND newer   | `CloudWonOverDirtyLocal` (cloud wins, local loses changes — surfaced in SyncReport) |
| dirty             | cloud ≤ local             | `LocalWon` (push will resolve) |

`SyncReport.conflictsLocalWon` and `conflictsCloudWon` counters surface to the UI so the user sees what happened.

**Auto-sync scheduler** (`sync/scheduler.rs`):

```rust
const AUTO_SYNC_CRON: &str = "0 */2 * * * *"; // every 2 minutes
pub struct AutoSyncHandle {
    active: Arc<AtomicBool>,
    _scheduler: Arc<tokio::sync::Mutex<JobScheduler>>,
}
```

Toggling the UI checkbox flips `active` — the scheduled job checks the flag before running; the scheduler itself is never paused/resumed (no torn state).

**Network monitor** (`sync/network.rs`) — 30 s probe loop on `{cloud_base}/health`, exposes a `tokio::sync::watch` channel that the engine subscribes to. Online → fires one extra `run_full_cycle()` so freshly-back-online clients converge fast.

### 3.8 CRDT real-time editing

```
Diary frontend                  Diary backend (Rust)              Cloud (FastAPI)
─────────────────              ──────────────────────             ──────────────────
keystroke ─► useDiary
            └─► invoke
                 'apply_local_text'
                       │
                       ▼
                 diff old → new ─► CharOp[]
                       │
                       ▼ pending_ops table (write)
                       │
                       ▼ WsClient.send(crdt_op)
                       │
                       └────── WebSocket ──────►  /ws/journal/{id}
                                                  ws_journal::handle_op()
                                                  ├─ apply to crdt_operations table
                                                  ├─ bump entry.version, last_modified_at
                                                  └─ broadcast to room (excl. sender)
                                                                 │
                       ◄────── WebSocket ──────────── crdt_op_broadcast
                       │
                 WsClient.recv() ─► document.apply_remote(op) ─► tauri::emit('crdt-text-updated')
                                                                       │
            useCrdtChannel  ◄─────────────────────────────────────────┘
            (re-renders MainNotesArea / CueItem in CRDT mode)
```

**On reconnect** the WS client replays everything in `pending_ops WHERE pushed = false` in chronological order, then marks them pushed. Duplicate inserts are harmless (RGA convergence guarantees idempotency on `char_id`).

**CRDT does NOT push individual ops to the REST sync surface.** The 2-minute scheduler pushes the materialised text. This is deliberate layering — see [§9.4](#94-crdt-and-rest-sync-are-separate-concerns).

### 3.9 Cloud service supervisor

The `commands/cloud_service.rs` module wraps a `tokio::process::Command` so Diary can manage Cloud as a child process. Goal: a non-technical user clicks "Cloud'u Başlat" in the UI and gets a fully-working Cloud server without touching a terminal.

```
start_impl()
  ├─ if !$HOME/Projects/Cloud exists → DomainError::Validation (Turkish msg)
  ├─ if !.venv/bin/uvicorn exists → same
  ├─ if !TCP-connect(127.0.0.1:5434) → run bash scripts/start_postgres.sh
  │    (records started_postgres = true so stop tears it down)
  ├─ spawn .venv/bin/uvicorn src.main:app --host 0.0.0.0 --port 5001
  │    with kill_on_drop(true), stdout/stderr → /dev/null
  ├─ store child in CloudServiceState
  └─ poll TCP :5001 (500 ms timeout) → return CloudServiceStatus
```

The TCP probe replaces an earlier reqwest `/health/live` probe — uvicorn binds the port only after FastAPI startup events succeed, so a successful TCP connect is equivalent to "Cloud is ready" and is faster + more reliable on cold launch (commit [`d1ade45`](../../commit/d1ade45)).

### 3.10 Build / packaging

| File                         | Notable settings                                                |
| ---------------------------- | --------------------------------------------------------------- |
| `tauri.conf.json`            | identifier `com.deniz.cornelldiary`, window 1200×800 (min 720×520), category `Productivity`, no CSP, all desktop bundles |
| `Cargo.toml`                 | `default = ["postgres"]`, `sqlite` feature gate; key deps: tauri 2.11, sqlx 0.8, reqwest 0.12 (rustls-tls only), tokio-tungstenite 0.23, tokio-cron-scheduler 0.13, sentry 0.34 |
| `package.json`               | scripts: `dev`, `build`, `test`, `typecheck`, `format`; React 19.1, react-router-dom 7.14, Zustand 5.0, date-fns 4.1, qrcode 1.5, qr-scanner 1.4 |
| `.github/workflows/ci.yml`   | Frontend (typecheck + vitest + build) → then Rust (cargo test against Postgres 16 service container) → then bundle artifact upload (Faz 2.3) |

#### Cargo features cheat-sheet

```bash
# Desktop default — Postgres
cargo build

# Mobile / standalone — SQLite
cargo build --features sqlite --no-default-features

# Targeted check (CI uses both)
cargo check && cargo check --features sqlite --no-default-features
```

### 3.11 Tests

| Suite                                  | Count | Runner                |
| -------------------------------------- | ----- | --------------------- |
| Frontend unit + integration            | 62    | vitest (jsdom)        |
| Rust unit + integration                | 57    | `cargo test`          |

Frontend categories: store mutations, repo mocks, conflict resolver, exporter/importer, QR chunker, sanitizer, locale loading, date utils, hooks. Rust categories: conflict decision, CRDT document convergence (500-mutation soak), cue codec round-trip, sync client serialisation, scheduler timing.

CI workflow is at [`.github/workflows/ci.yml`](.github/workflows/ci.yml).

---

## 4. Component: Cloud

### 4.1 Source layout

```
~/Projects/Cloud/
├── src/
│   ├── main.py                     # create_app(), lifespan, routers, CORS, Sentry
│   ├── config.py                   # pydantic-settings (60+ env vars)
│   ├── logger.py                   # structlog (JSON in prod, console in dev)
│   ├── metrics.py                  # Prometheus histograms / gauges / counters
│   ├── exceptions.py               # CloudError hierarchy
│   ├── auth/                       # JWT, password hashing, dependencies
│   ├── api/
│   │   └── routes/                 # auth.py, journals.py, entries.py, sync.py, ws_journal.py, health.py
│   ├── db/                         # session.py, models.py
│   ├── services/                   # auth_service, sync_service, crdt_service, snapshot_service, email_service
│   ├── crdt/                       # CRDTDocument, CharNode, conflict_resolver
│   └── ws/                         # ConnectionManager, protocol, handlers
├── alembic/versions/               # 2 migrations (initial + auth hardening)
├── tests/
│   ├── unit/                       # 6 files
│   ├── integration/                # 13 files
│   └── stress/                     # 2 files
├── scripts/                        # start_postgres.sh, stop_postgres.sh, deploy.sh, backup.sh, …
├── Dockerfile                      # multi-stage, non-root, /health/live healthcheck
├── docker-compose.yml              # dev (Postgres 16-alpine on :5434)
├── docker-compose.production.yml   # prod (migrate one-shot + cloud + caddy)
├── Caddyfile                       # ACME HTTPS reverse proxy
└── pyproject.toml                  # ruff, pytest config
```

### 4.2 App wiring (`src/main.py`)

- **App factory** (`create_app()` line 59) — title `Cloud Sync Server`.
- **Lifespan** (33-56) — async context manager configures logging, starts the snapshot loop, ensures graceful shutdown.
- **CORS** (80-86) — origins from `allowed_origins` env var (CSV, never `*`); credentials, all methods/headers.
- **Sentry** (14-30) — opt-in via `sentry_dsn`; if empty, SDK is never imported (zero overhead). When enabled: 10% perf trace sampling, PII disabled.
- **Routers mounted** (102-107) — `/health`, `/auth`, `/journals`, `/entries`, `/sync`, `/ws`.
- **Prometheus** (93-100) — auto-mounts `/metrics` if `prometheus_enabled=true`. Excludes `/metrics` and `/health.*` from instrumentation.
- **Deployment-mode hardening** (63-77) — when `deployment_mode=prod`: `/docs`, `/redoc`, `/openapi.json` are disabled and `debug=False` is forced.

### 4.3 Auth (`src/api/routes/auth.py` + `src/services/auth_service.py`)

Eight endpoints, all under `/auth`:

| Endpoint              | Method | Body                                    | Response          | Notes                                                |
| --------------------- | ------ | --------------------------------------- | ----------------- | ---------------------------------------------------- |
| `/register`           | POST   | `{username, email, password}`           | `TokenResponse`   | Creates user, issues token pair, sends verification email (soft gate); returns 201 |
| `/login`              | POST   | `{username, password}`                  | `TokenResponse`   | Lockout check before password work; records `LoginAttempt` (success/failure + IP) |
| `/refresh`            | POST   | `{refresh_token}`                       | `TokenResponse`   | Rotates the refresh token; reusing a rotated `jti` revokes ALL of the user's tokens |
| `/logout`             | POST   | `{refresh_token}`                       | `{status:"ok"}`   | Revokes one `jti`; idempotent (no info leak)         |
| `/verify`             | GET    | `?token=…`                              | `{status:"ok"}`   | One-shot email verification token (24-h TTL by default) |
| `/forgot-password`    | POST   | `{email}`                               | `{status:"ok"}`   | Always 200 (no email enumeration); only sends mail if `email_verified=true` |
| `/reset-password`     | POST   | `{token, new_password}`                 | `{status:"ok"}`   | Consumes token, hashes new password, revokes ALL the user's sessions |
| `/me`                 | GET    | (Bearer access token)                   | `MeResponse`      | Touches `last_sync_at` for telemetry                 |

#### Token model

- **Access token**: HS256, default 15-min TTL (`jwt_access_ttl_minutes`). Claims: `sub`, `peer_id`, `exp`, `iat`, `type="access"`.
- **Refresh token**: HS256, default 7-day TTL (`jwt_refresh_ttl_days`). Claims: `sub`, `peer_id`, `jti`, `exp`, `iat`, `type="refresh"`. The `jti` is a 16-byte random hex stored in the `refresh_tokens` table for rotation tracking.
- **Reuse detection**: each rotation marks the old `jti.revoked_at = now` and stores the new `jti` as `replaced_by_jti`. If a client presents a `jti` that's already been rotated, the service revokes every token for that user and forces re-login.

#### Password reset flow

Single-use tokens in `password_reset_tokens` (`token, user_id, issued_at, expires_at, used_at`). Default TTL 60 min. After successful reset, ALL of the user's refresh tokens are revoked so a leaked token can't hijack a still-active session.

#### Login lockout (Faz 2.1)

After `login_lockout_threshold` (10) failed attempts within `login_lockout_window_minutes` (15), the account locks for `login_lockout_duration_minutes` (15) — regardless of source IP, to defeat distributed brute-force. Tracked per (username + IP) in `login_attempts`. The slowapi limiter additionally caps the endpoint at 5/min per IP.

#### Email backends (Faz 2.5)

`email_backend` setting (`auto | console | file | smtp`) — selected per-call in `email_service.py`:

- **console** (default in dev) — logs message to structlog and keeps a 100-element ring buffer for tests.
- **file** — writes `.txt` files to `email_file_dir` (default `/tmp/cloud_emails`). Filename includes timestamp + sanitised recipient.
- **smtp** — `aiosmtplib` with STARTTLS on port 587, 15-s timeout. Imported lazily so dev installs don't pay for it.
- **auto** — smtp if `smtp_host` is set, else console (legacy behaviour).

### 4.4 Sync API (`src/api/routes/sync.py` + `src/services/sync_service.py`)

| Endpoint      | Method | Body / Query                                          | Response                          |
| ------------- | ------ | ----------------------------------------------------- | --------------------------------- |
| `/sync/pull`  | GET    | `journal_id`, optional `since`, optional `peer_id`    | `{entries[], crdt_ops[], server_time}` |
| `/sync/push`  | POST   | `{journal_id, peer_id, device_label, idempotency_key, entries[], crdt_ops[]}` | `{merged_entries[], crdt_ops_applied, crdt_ops_skipped, duplicate, server_time}` |

#### Merge semantics (`merge_field_level()` in `sync_service.py`)

The `sync_merge_strategy` setting selects the algorithm:

- **`lmw`** (default, last-modified-wins) — server's version + `last_modified_at` wins on tie. Simple, used by pre-Faz-1.1 clients.
- **`crdt`** (baseline-aware) — if the client supplies `baseline_version` and the server has moved past it, the server skips overwriting text fields; the on-entry CRDT op log handles char-level merging instead.

Version + `last_modified_at` always bump on a successful merge. Conflicts are counted and exposed via `sync_conflicts_total{strategy}` Prometheus counter.

#### Idempotency

In-memory LRU keyed by `idempotency_key` (10 k entries, 5-min TTL). Replays return `{duplicate: true, ...}` with empty result lists. **Per-process only** — a multi-replica deployment needs Redis (v2 roadmap).

#### Device registration

Implicit. Each `/sync/push` upserts a row in `sync_state` keyed by `peer_id` with `(user_id, last_pull_at, last_push_at, device_label, updated_at)`. Used by the Diary settings page to show "Last synced from MacBook Pro" type telemetry.

### 4.5 CRDT WebSocket

#### Connection

```
ws://{cloud_base}/ws/journal/{journal_id}?token={access_token}
```

- Token is decoded via `decode_token(token, expected_type="access")` (line 40); failure closes with `1008 POLICY_VIOLATION`.
- IDOR check (`_authorize`, lines 100-118) verifies the user is the owner OR has a row in `journal_collaborators`; otherwise close.
- Per-user connection cap: `max_ws_conn_per_user` (5). Exceeded closes with `1013 TRY_AGAIN_LATER`.

#### Room model

- One room per `journal_id` (UUID key).
- `ConnectionManager._rooms: dict[UUID, list[Connection]]`.
- Each `Connection` holds `(peer_id, user_id, websocket)`.
- Broadcast fanout (`manager.broadcast()`) — sends to all peers, optionally excluding the sender; dead sockets are pruned.

#### Message catalogue (`ws/protocol.py` + `ws/handlers.py`)

| Type                  | Direction      | Payload                                                       |
| --------------------- | -------------- | ------------------------------------------------------------- |
| `subscribe`           | client→server  | empty (just confirms presence)                                |
| `presence`            | client→server  | empty (broadcast presence to room)                            |
| `ping`                | client→server  | empty (server replies `pong`)                                 |
| `crdt_op`             | client→server  | `{entry_id, field_name, op: InsertOp \| DeleteOp}`            |
| `crdt_op_broadcast`   | server→client  | `{entry_id, field_name, op, sender_peer_id}`                  |
| `presence_update`     | server→client  | `{peers: [peer_id, …]}`                                       |
| `snapshot_updated`    | server→client  | `{entry_id, field_name, server_time}` — sent when snapshot service writes a fresh materialised text |
| `ack`                 | server→client  | `{op_count_applied, op_count_skipped}`                        |
| `error`               | server→client  | `{code, message}`                                             |

Per-connection rate cap: `rate_limit_ws` ops/sec (default 100).

#### Persistence (`crdt_service.py`)

Each accepted op is appended to `crdt_operations` (id, entry_id, field_name, op_type, char_id, char_value, prev_id, next_id, peer_id, lamport_clock, created_at). Application is idempotent on `char_id`. The entry's `version` and `last_modified_at` are bumped.

#### Snapshot loop

Background task started in `lifespan` (interval `crdt_snapshot_interval_seconds`, default 30). For each `(entry, field)` with new ops since last snapshot: materialise via `CRDTDocument.materialize()` and write to `snapshots` and into the entry's text column. The snapshot row stores `last_op_id` for idempotency. If `enable_crdt_gc=true`, every Nth pass runs tombstone GC (off by default — see [§9.5](#95-cloud-gc-is-opt-in)).

### 4.6 Database (Postgres + Alembic)

Eleven tables across two migrations:

| Table                       | Purpose                                                                       |
| --------------------------- | ----------------------------------------------------------------------------- |
| `users`                     | id, username (unique), email (unique), password_hash, peer_id (unique), email_verified, created_at, last_sync_at |
| `journals`                  | id, owner_id, title, created_at, updated_at                                   |
| `journal_collaborators`     | journal_id + user_id (composite PK), role (default `editor`), added_at        |
| `entries`                   | id, journal_id, entry_date (with UC on (journal_id, entry_date)), 4 text fields, version, last_modified_*, created_at |
| `crdt_operations`           | RGA log: id, entry_id, field_name, op_type, char_id, char_value, prev_id, next_id, peer_id, lamport_clock, created_at |
| `snapshots`                 | id, entry_id, field_name, materialised_text, last_op_id, created_at           |
| `sync_state`                | peer_id (PK), user_id, last_pull_at, last_push_at, device_label, updated_at   |
| `email_verification_tokens` | one-shot email verify (token PK, user_id, issued_at, expires_at, used_at)     |
| `password_reset_tokens`     | same shape as above for resets                                                |
| `refresh_tokens`            | jti (PK), user_id, issued_at, expires_at, revoked_at, replaced_by_jti         |
| `login_attempts`            | id, username, ip, attempted_at, success — for sliding-window lockout          |

Connection pool (`db/session.py`): `pool_size=10, max_overflow=20, pool_pre_ping=True`. Async sessions via SQLAlchemy 2.x with asyncpg driver.

### 4.7 Health, observability, deployment

#### Health endpoints

- `GET /health/live` → `{status:"ok", service, time}`. No DB call. Used by k8s liveness + Caddy upstream check.
- `GET /health/ready` → executes `SELECT 1`. Returns 503 with `{status:"degraded", checks:{db:"fail:..."}}` on failure.
- `GET /health` → alias of `/health/live` for backward compat with `journal_ai_reporter` sidecar.
- `GET /debug/rooms` → authenticated; lists current WS rooms + total connections.

#### Metrics (`metrics.py`)

- **Histograms** (`*_duration_seconds`) — `sync_pull`, `sync_push`, `crdt_op_apply` with sub-second buckets.
- **Gauges** — `ws_active_connections`, `crdt_pending_queue_size{journal_id}`.
- **Counter** — `sync_conflicts_total{strategy}` labeled by lmw/crdt.

#### Logging

structlog 24.x. Dev mode: ANSI-coloured console. Prod mode: JSON lines. Every request logs `method, path, status, elapsed_ms, request_id`. The `RequestContextMiddleware` injects a `request_id` (from `X-Request-Id` header or fresh UUIDv4); responses include `x-request-id` for tracing.

#### Dockerfile

Multi-stage, `python:3.13-slim` base. Build stage installs deps into a venv; runtime stage copies only the venv + source. Runs as non-root `app:app`. Exposes `:5001`. Healthcheck `curl /health/live`. CMD: `uvicorn src.main:app --host 0.0.0.0 --port 5001 --proxy-headers --forwarded-allow-ips "*"`.

#### docker-compose

- **`docker-compose.yml`** (dev) — postgres-16-alpine on `:5434:5432`, named volume.
- **`docker-compose.production.yml`** — postgres bound to `127.0.0.1:5434` only (no public DB), one-shot `migrate` service runs `alembic upgrade head` before `cloud` starts, both with healthcheck-conditional `depends_on`.
- **`Caddyfile`** — domain placeholder, automatic HTTPS via ACME, security headers (HSTS, X-Frame-Options DENY, X-Content-Type-Options nosniff), removes upstream `Server:` header, JSON access logs with rotation, internal cleartext `:8080/health` for upstream LB.

#### Scripts

- `scripts/start_postgres.sh` — sources `.env`, runs `docker compose up -d postgres`, polls `pg_isready` for 30 s.
- `scripts/stop_postgres.sh` — `docker compose stop postgres`.
- `scripts/deploy.sh`, `scripts/backup.sh`, `scripts/restore.sh`, `scripts/dep_audit.sh` — operational utilities.

### 4.8 Tests

| Suite          | Files | Notes                                                         |
| -------------- | ----- | ------------------------------------------------------------- |
| Unit           | 6     | CRDT primitives (CharNode, CRDTDocument, snapshot builder, conflict resolver), JWT, password hashing |
| Integration    | 13    | Auth flow, hardening, email backends, sync flow, WS flow, CRDT GC, snapshot service, collaborators, deployment modes, ORM, observability, backup/restore |
| Stress         | 2     | End-to-end long-running, concurrent writes                    |

Run with `pytest`; markers `unit`, `integration`, `stress` for selective runs.

---

## 5. Component: journal_ai_reporter (currently inactive)

> **Status as of 2026-05-05:** Diary no longer calls this service. The on-device LLM panel that consumed it was removed (commit [`1a6a620`](../../commit/1a6a620)). The code remains because **ImaginingJarvis** (a separate repo) still depends on it for tag-driven journal reports.

### 5.1 What it is

Two FastAPI processes co-located in one repo (`journal_ai_reporter/`):

- **Cornell Sidecar** (`cornell_journal_api/`, port 8001) — read-only Postgres reader. Exposes `GET /api/entries` against the live Diary database, gated by `X-API-Key`.
- **Reporter Bridge** (`src/`, port 8002) — Gemini-backed pipeline. Single endpoint: `POST /report`. Pipeline: **Converter** (calls Sidecar) → **Parser** (regex-based categorisation, optional hybrid LLM) → **Reporter** (tag-specific Gemini prompt + Pydantic-validated response + Turkish markdown render).

### 5.2 Tag commands

| Tag                   | Slice of parsed journal              | Output                           |
| --------------------- | ------------------------------------ | -------------------------------- |
| `/detail`             | full tree                            | Summary, todos, concerns, successes, patterns, recommendation |
| `/todo`               | todos branch                         | Open / completed / deferred + analysis |
| `/concern`            | concerns branch                      | Anxieties / fears / failures + empathic Turkish summary |
| `/success`            | successes branch                     | Achievements / milestones + motivational tone |
| `/date{dd.mm.yyyy}`   | one day's entry                      | Day narrative + highlights + emotional tone |

### 5.3 Pipeline & key files

```
POST /report  ──►  src/api/routes.py
                   ├─ verify_internal_api_key (Bearer)
                   ├─ slowapi 20/min limit
                   │
                   ├─ Phase 1: src/modules/converter/
                   │    └─ HTTP GET cornell_sidecar/api/entries (asyncpg under the hood)
                   ├─ Phase 2: src/modules/parser/
                   │    ├─ legacy: parser/categorizer.py (Turkish + English regex)
                   │    └─ hybrid: parser/hybrid_classifier.py (off by default)
                   └─ Phase 3: src/modules/reporter/
                        ├─ tag_handlers.py picks slice
                        ├─ ai_client.py (Gemini 2.5 Flash, JSON response_mime_type)
                        ├─ Pydantic-validate response (retry once on parse failure)
                        └─ markdown_renderer per handler
```

### 5.4 Hybrid classifier — why it's off

`HYBRID_CLASSIFIER_ENABLED=false` (default). Per-sentence LLM verification of MEDIUM-confidence keyword matches blows the latency budget: a 7-entry report goes from ~14 s (legacy) to ~50 s (hybrid) because of Gemini's 15 RPM rate limit. Three fixes are queued (per-sentence timeout, broader HIGH-keyword catalogue, batched LLM calls) — see `journal_ai_reporter/parser/README.md`.

### 5.5 Security model

User-facing content is wrapped in `<user_journal>...</user_journal>` and the closing tag is rewritten to `[/user_journal]` before being sent to Gemini, preventing prompt-injection escape. No prompts or journal content are logged. PII-safe logs (request_id, endpoint, status, duration only).

### 5.6 Tests

112 tests, 93 % coverage (95 % each in Bridge and Sidecar). Markers: `unit`, `integration`, `security`.

---

## 6. End-to-end data flows

### 6.1 First-time Cloud connect

```
SyncPage   ──► invoke 'connect_cloud'(username, password, deviceLabel)
   │              │
   │              ▼
   │        SyncEngine::connect(user, pass, label)
   │              │
   │              ▼  POST /auth/login {username, password}
   │              │       └────► Cloud validates, returns {access_token, refresh_token, ttl_*}
   │              │
   │              ▼  POST /journals (if user has none)  -or-  GET /journals (pick first)
   │              │       └────► Cloud returns {journal_id, journal_name}
   │              │
   │              ▼  Persist sync_metadata row (peer_id, tokens, journal_id, device_label, sync_enabled=true)
   │              │
   │              ▼ return ConnectReport {userId, peerId, journalId, journalName}
   │
   └──► UI shows "Connected as <username>" + green sync indicator
```

### 6.2 Auto-sync cycle (every 2 min)

```
JobScheduler tick
  └─ if AutoSyncHandle.is_active() == false: skip
  └─ else: SyncEngine::run_full_cycle()
       └─ acquire cycle_lock (mutex against UI-triggered sync)
       └─ AuthManager.refresh_if_needed()
            └─ if access_token within 60 s of expiry: POST /auth/refresh
       └─ PULL phase
            └─ GET /journals/{j}/entries?since={last_pull_at}
            └─ for each entry: ConflictResolver.decide(local, cloud) → apply
       └─ PUSH phase
            └─ collect rows WHERE is_dirty = true
            └─ POST /journals/{j}/entries (batch, baseline_version per row)
            └─ on success: clear is_dirty, bump last_synced_at
       └─ update sync_metadata (last_pull_at, last_push_at)
       └─ release cycle_lock
       └─ return SyncReport (UI updates indicator)
```

### 6.3 CRDT live-edit fan-out

See [§3.8](#38-crdt-real-time-editing). Round-trip from keystroke to remote peer's screen typically ≤ 80 ms over LAN.

### 6.4 Cloud spawn from Diary launch

```
Tauri setup hook
  └─ read app_settings.auto_start_cloud_on_launch (default false)
  └─ if true: tauri::async_runtime::spawn(start_cloud_service_internal)
       └─ check $HOME/Projects/Cloud exists + .venv/bin/uvicorn exists
       └─ if !TCP(127.0.0.1:5434): bash scripts/start_postgres.sh
       └─ spawn .venv/bin/uvicorn (kill_on_drop)
       └─ TCP-poll :5001 until accept (500 ms timeout per probe)
       └─ CloudServicePanel sees "running" within ~5–10 s of launch
```

---

## 7. Configuration reference

### 7.1 Diary environment

| Var                  | Default                                                                 | Effect                                                  |
| -------------------- | ----------------------------------------------------------------------- | ------------------------------------------------------- |
| `DATABASE_URL`       | (Postgres feature) required; (SQLite feature) auto-resolves to app data | Connection string                                       |
| `CLOUD_URL`          | active profile's `base_url` else `http://127.0.0.1:5001`                | Overrides cloud profile selection at boot               |
| `SENTRY_DSN`         | empty                                                                   | Empty = Sentry not initialised (zero overhead)          |
| `RUST_LOG`           | `info,cornell_diary=debug`                                              | Tracing filter                                          |

### 7.2 Cloud environment (subset of ~60)

| Var                              | Default              | Effect                                                  |
| -------------------------------- | -------------------- | ------------------------------------------------------- |
| `JWT_SECRET`                     | `replace_me`         | **MUST override in prod** — HS256 signing key           |
| `JWT_ACCESS_TTL_MINUTES`         | 15                   | Access token lifetime                                   |
| `JWT_REFRESH_TTL_DAYS`           | 7                    | Refresh token lifetime                                  |
| `DB_HOST/PORT/NAME/USER/PASSWORD` | localhost / 5434 / cloud_db / cloud_user / change_me_in_dev | Postgres connection |
| `ALLOWED_ORIGINS`                | `http://localhost:1420` | CORS allowlist (CSV)                                  |
| `DEPLOYMENT_MODE`                | `dev`                | `prod` hides `/docs`, forces `debug=False`              |
| `SYNC_MERGE_STRATEGY`            | `lmw`                | `crdt` activates baseline-aware merge                   |
| `LOGIN_LOCKOUT_*`                | 10 / 15 min / 15 min | Sliding-window lockout                                  |
| `EMAIL_BACKEND`                  | `auto`               | `console` / `file` / `smtp`                             |
| `SMTP_HOST/PORT/USER/PASSWORD`   | empty / 587 / / /    | Used when `email_backend=smtp`                          |
| `CRDT_SNAPSHOT_INTERVAL_SECONDS` | 30                   | Background materialisation cadence (0 = disabled)       |
| `ENABLE_CRDT_GC`                 | false                | Opt-in tombstone GC                                     |
| `PROMETHEUS_ENABLED`             | false                | Mounts `/metrics`                                       |
| `SENTRY_DSN`                     | empty                | Opt-in error tracking                                   |

Full list: `~/Projects/Cloud/.env.example` and `.env.prod.example`.

### 7.3 Cloud profiles in Diary

The `cloud_profiles` table holds N profiles; exactly one is active (UNIQUE-when-true index on `is_active`). Two are seeded at install:

- `local` — `http://localhost:5001`, active, protected (cannot be deleted)
- `production` — empty `base_url`, inactive, protected

Users can add custom profiles via Settings → Cloud Profile. Switching the active profile **clears auth and sets `pending_restart=true`**; the new URL takes effect on next launch (deliberate — see [§9.2](#92-non-obvious-decisions)).

---

## 8. Roadmap

### 8.1 Next sprint — Android build

Diary is already SQLite-ready (`--features sqlite --no-default-features`). The sprint adds:

1. Android-specific Tauri capabilities + signing config
2. `cornell-diary/android-overrides/` — manifest + strings (already scaffolded; see [`cornell-diary/android-overrides/README.md`](cornell-diary/android-overrides/README.md))
3. UI tweaks for narrow viewports (some already shipped: header collapse, archive blank-row filter)
4. Tap-target sizing audit on cue items
5. Test build + sideload run on physical Android device

### 8.2 Beyond Android

- **Hybrid classifier latency fix** — three queued items in `parser/README.md` (per-sentence timeout, broader HIGH catalogue, batched calls). When latency drops below 20 s for a 7-day report, flip the default.
- **CRDT GC enablement** — once a backup + tail-latency monitoring story exists, set `ENABLE_CRDT_GC=true` to prevent unbounded `crdt_operations` growth.
- **Multi-replica Cloud** — replace in-memory idempotency cache with Redis; add session sticky for WS or move to Redis pub/sub fanout.
- **WS streaming for `/report`** — Server-Sent Events on the Reporter Bridge so reports feel snappy instead of blocking ~14 s.
- **Multi-account Diary** — drop the `sync_metadata` singleton constraint and add a profile-keyed table.

---

## 9. Non-obvious decisions

### 9.1 macOS main-thread rule

The auto-sync scheduler is **not** started inside the Tauri setup hook. macOS's app delegate can't unwind through a nested `tokio::block_on`, so we use `tauri::async_runtime::spawn(...)` and stash the handle in a `OnceCell` that the UI commands wait on. See `lib.rs:205-258`.

### 9.2 Profile switch requires restart

The active `CloudClient` is built once during the setup hook with the active profile's URL. Switching profiles persists the choice + clears auth + sets `pending_restart=true`. The URL only takes effect on next launch. Rationale: hot-swapping the client mid-run forces us to manage two parallel auth states (old token still valid against old URL, new token not yet issued) — easier to just restart.

### 9.3 Passwords never persisted

`connect_cloud(username, password, device_label)` accepts the password on the IPC wire but never logs or stores it. Only the resulting JWT pair lands in `sync_metadata`. The frontend's `CloudSyncPanel` sends the password directly to the command and forgets it on the next render.

### 9.4 CRDT and REST sync are separate concerns

Live char-level edits flow through WebSocket and are persisted to Cloud's `crdt_operations` table immediately. The REST sync surface (`SyncEngine::run_full_cycle`) operates at the entry level and pushes the materialised text. The two layers don't talk to each other directly: the Cloud snapshot loop materialises CRDT ops back into `entries.{cue_column,notes_column,…}` every 30 s, and the next REST pull picks up the updated entry. This is intentional — it lets offline-only devices (no WS) still sync correctly.

### 9.5 Cloud GC is opt-in

`enable_crdt_gc=false` by default. Once tombstone rows are deleted, a peer re-pushing old ops will re-create the same tombstones (RGA is convergent so this is safe), but you lose the audit trail and any in-flight long-disconnected peer with extremely old state could see ordering surprises. The flag is intended to be flipped only after a recent backup exists and latency tail is monitored.

### 9.6 `deployment_mode` ≠ `app_env`

`app_env` is a free-form telemetry tag (dev / staging / prod). `deployment_mode` is a strict Literal[dev, prod] that controls hardening (disables `/docs`, forces `debug=False`). You can run `app_env=staging` with `deployment_mode=prod` so staging is hardened but still privately tagged.

### 9.7 Wide vs. narrow schema

`diary_entries` stores `title_1..7` and `content_1..7` columns, but the TypeScript surface uses `cue_items: CueItem[]`. The repository implementations flatten on read and denormalise on write. New code should never see the wide shape.

### 9.8 Single-account by design

The Diary `sync_metadata` table has `CHECK (id = 1)` — this enforces a single Cloud account per Diary install. Multi-account would require a profile-keyed table and more complex auth UI; deliberately deferred.

### 9.9 LLM columns retained but unused

Migration 0006 (`llm_settings`, `ai_*` columns on `diary_entries`) was added during the Gemma-4 panel sprint and never removed when the panel was deleted on 2026-05-05. Reasoning: dropping the columns is an irreversible migration, and the on-device LLM may return in a different form. The columns are simply never written to.

### 9.10 Cloud port hard-coded

`5001` (HTTP) and `5434` (Postgres) are constants in `commands/cloud_service.rs`. A multi-Cloud deployment from one Diary would require parameterising them — currently profile-driven URL switching only changes the host, not the port.

---

## 10. Where to read more

- **Operational runbook (Diary)**: [cornell-diary/OPERATIONS.md](cornell-diary/OPERATIONS.md)
- **Sync behaviour walkthrough**: [cornell-diary/SYNC_BEHAVIOR.md](cornell-diary/SYNC_BEHAVIOR.md)
- **Threat model (Diary)**: [cornell-diary/THREAT_MODEL.md](cornell-diary/THREAT_MODEL.md)
- **Security checklist**: [cornell-diary/docs/SECURITY_CHECKLIST_DIARY.md](cornell-diary/docs/SECURITY_CHECKLIST_DIARY.md)
- **Historical handoff docs / phase trackers**: [docs/archive/](docs/archive/) (frozen — kept for context but no longer authoritative)
- **CHANGELOG**: [CHANGELOG.md](CHANGELOG.md)

---

*Document owner: Deniz Tanışma. If anything here disagrees with the code, the code wins — please patch this file.*
