# Changelog

Roadmap-driven phases. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
"Unreleased" is what's on `main` but not yet tagged.

## [Unreleased]

Post-1.0.0 work on `main`. Will be tagged as **1.1.0** at the end of the
Android polish sprint.

### Added

- **Cloud auto-sync scheduler** (post-1.0 Tier 1) — `tokio-cron-scheduler`
  fires `engine.run_full_cycle()` every 2 minutes; `AutoSyncHandle`
  wraps an `AtomicBool` so the UI toggle flips it live without
  scheduler tear-down. Spawned off the setup hook to dodge the macOS
  app-delegate `panic_cannot_unwind` from a nested `block_on`.
  Setting key: `auto_sync_enabled` (default ON).
- **Cloud-from-Diary spawn** (Tier 2) — `start_cloud_service` /
  `stop_cloud_service` / `cloud_service_status` Tauri commands manage
  a uvicorn child via `tokio::process::Command` with `kill_on_drop`,
  plus `bash scripts/start_postgres.sh` orchestration when :5434 is
  idle. `CloudServicePanel` (Sync page) polls every 1.5 s and offers
  start/stop buttons + PID display. Means non-technical users get
  Cloud running without touching a terminal.
- **Cloud auto-start on Diary launch** (Tier 3) — opt-in toggle
  (`auto_start_cloud_on_launch`, default OFF). When on, the setup hook
  spawns the same `start_cloud_service_internal` path right after
  `app.manage(...)` so phone-from-Mac sync flows just work.
- **Clickable date header** — the Cornell header date label is now a
  button that triggers a hidden `<input type="date">` via
  `showPicker()` (or a synthetic click on older WebViews). Lets users
  jump to any date instead of clicking ← / → repeatedly.

### Changed

- **Cornell header layout** tightened on narrow viewports — date
  centred between nav + meta, ellipsis on overflow, single row even
  at 720 px (was wrapping to two rows on small Android screens).
- **Cloud readiness probe** swapped from reqwest `/health/live` HTTP
  call to a 500 ms `tokio::net::TcpStream::connect` on `:5001`. uvicorn
  binds the port only after FastAPI startup events succeed, so a
  successful TCP connect is equivalent to "Cloud is ready" — and is
  faster + more reliable than reqwest's connect+TLS init on cold
  launch (panel was occasionally showing "Cloud kapalı" while Cloud
  was actually serving).

### Fixed

- **Archive hides blank entries** — `diary_list_dates` now filters out
  rows where every user-visible field (`diary`, `cue_*`, `summary`,
  `quote`) is empty. Autosave was leaving deleted-but-not-empty rows
  in the archive after a user cleared everything on a day.

### Removed

- **On-device LLM (Gemma-4) panel + backend wiring** — every AI path
  was already going through user-controlled APIs, so the local model
  was pure cost (~7 GB disk, ~9 GB RAM, ~25 s warmup, no marginal
  value over the network paths). Dropped:
  - Frontend: `LlmInsightsPanel`, `LlmSettings`, `llmSettings.ts`
    types + tests
  - Backend: `commands/llm.rs`, `db/llm_settings.rs`,
    `sync/bridge_client.rs`
  - `journal_ai_reporter` sibling repo is left in tree (still
    consumed by ImaginingJarvis) but Diary itself no longer talks to
    it.
  - DB migration 0006 (`llm_settings` singleton + `ai_*` columns on
    `diary_entries`) is **not reverted** — dropping the columns is an
    irreversible migration and a future LLM integration could revive
    them. They're simply never written to.

### Docs

- **Single source of truth** — root-level docs collapsed into one
  `ARCHITECTURE.md` (~1100 lines, code-driven). Covers Diary +
  Cloud + journal_ai_reporter end-to-end: component breakdown,
  IPC commands, DB schema, sync engine, CRDT layer, deployment, and
  a "non-obvious decisions" section. Replaces the 14-file root sprawl
  (handoffs, prompts, phase trackers).
- Legacy MDs moved to `docs/archive/` (frozen — kept for context but
  no longer authoritative).
- README updated to point at `ARCHITECTURE.md`, refresh test counts
  (62 vitest + 57 cargo), call out Cloud sync as implemented (was
  marked roadmap), flag Android sprint as active.

### Known follow-ups

- **Android polish sprint** — UI tap-target audit, narrow viewport
  tweaks, dark mode contrast, SafeArea + status bar styling. SQLite
  backend is already wired (Faz 1.4); these are presentation-layer
  refinements.

---

## [1.0.0] — 2026-05-01

First production release. Faz 0 → Faz 2.3 of the roadmap, all behind
default-on or default-off feature flags so existing deployments don't
shift behaviour at upgrade time.

### Added (Faz 2)

- **Auth hardening** (Faz 2.1) — pairs with Cloud `feature/auth-hardening`
  - `CloudClient.{logout, forgot_password, reset_password}` and three
    matching Tauri commands (`disconnect_cloud` now best-effort revokes
    the refresh token's jti server-side before clearing local meta)
  - CloudSyncPanel grows a mode toggle (`login → forgot → reset`) with
    separate forms and a non-destructive notice band
- **CI: Rust integration tests** (Faz 2.3)
  - New `rust-test` GitHub Actions job runs cargo test against a
    postgres service container (was: integration tests skipped on CI)
  - Tauri build job uploads `.dmg / .app / .AppImage / .msi` bundles
    as run artifacts

### Changed
- `tauri-build` now depends on `rust-test`; binaries don't ship if the
  underlying tests are red

---

## [0.9.0] — Faz 1 wrap-up (2026-05-01)

### Added
- **CRDT-aware merge** (Faz 1.1, [Cloud `6ca8829`](https://github.com/DenizTanisman/Cloud) +
  [Diary `ca13e10`](https://github.com/DenizTanisman/Diary-Cornell))
  - `SYNC_MERGE_STRATEGY=lmw|crdt` env flag on Cloud (default `lmw` →
    pre-1.1 behaviour byte-for-byte unchanged).
  - `PushEntry.baseline_version` optional field — when the client
    supplies it under `crdt` strategy and the server has moved past
    that baseline, text fields are kept on the server and the per-char
    CRDT op log handles merge. Numeric/timestamp fields still LMW.
  - Diary 0004 migration adds `baseline_version` column to
    `diary_entries` (Postgres + SQLite mirrors); `mark_synced` pins it
    to the server version on every successful push. `now()` SQL calls
    on engine.rs swapped for bound `Utc::now()` so the same statement
    compiles under SQLite.
  - 3 new Cloud sync tests + 2 new Diary engine tests; 33 sync-touching
    tests still green under the new strategy flag.
- **Postgres backup + restore** (Faz 1.2)
  - `cloud/scripts/backup.sh` + `restore.sh` with docker-exec defaults
    (host `pg_dump` 15 vs server 16 mismatch made the host path
    unreliable). 30-day retention, weekly tag opt-in.
  - End-to-end `tests/integration/test_backup_restore.py` drill that
    actually runs both scripts and asserts row counts match between
    source and restored DB.
- **Observability surface** (Faz 1.3)
  - Cloud: `prometheus_enabled` + `sentry_dsn` env flags, both
    default-off so untrusted setups don't accidentally expose request
    timings or burn Sentry quota. `/metrics` (only when enabled),
    `/health/live`, `/health/ready` (DB ping → 503 on degraded). Old
    `/health` kept as alias.
  - `src/metrics.py` adds histograms (sync pull/push duration, CRDT
    op apply duration), gauges (`ws_active_connections`,
    `crdt_pending_queue_size{journal_id}`), and counter
    (`sync_conflicts_total{strategy}`).
  - Diary: `sentry 0.34` crate, `lib.rs::run` SENTRY_DSN-gated init.
    Empty DSN → SDK never loaded, zero overhead, zero network.
- **Android APK + LAN sync** (Faz 1.4)
  - `build.rs` forwards `DIARY_CLOUD_URL` env to the binary via
    `cargo:rustc-env`; `lib.rs` consumes it through `option_env!` so
    the Android APK can carry the laptop's LAN IP without runtime env.
    Default loopback bumped to `:5001` because macOS Control Center
    holds `*:5000`.
  - `scripts/android-env.sh` source-able helper that finds the highest
    NDK under `$ANDROID_HOME` and sets all four ABI cross-compilers
    (cc-rs looks for `aarch64-linux-android-clang`, NDK only ships
    `…-android24-clang` — bridges the gap).
  - Verified end-to-end: phone APK boots, SQLite migrations run on
    fresh DB, Cloud login + journal join + push/pull round-trip
    against the Mac LAN IP.

### Changed
- `count_dirty` SQL switched from `SELECT COUNT(*)::BIGINT` (Postgres
  cast) to plain `COUNT(*)` so SQLite doesn't choke on `::`. The cast
  was a SQLite-incompat foot-gun on the Android device.
- `archive_local`: `INSERT INTO sync_log … VALUES … now() …`
  → bound `Utc::now()`. Same dialect-portability fix.

### Fixed
- Stale `peer_id='alice@laptop'` and `version=0` rows leaking from
  test fixtures into the dev DB now fixable via the documented
  cleanup queries in `cornell-diary/OPERATIONS.md` (Faz 0.A docs)
  + dedicated `TEST_DATABASE_URL` going forward.

### Tests
- Diary Postgres: **51 → 53** (+2 baseline_version unit tests).
- Diary SQLite: **49 → 51** (mirror tests).
- Diary frontend Vitest: **58/58** unchanged.
- Cloud: **57 → 67** (+3 sync strategy + 6 observability + 1 backup
  drill).
- Cross-mode smoke (`SYNC_MERGE_STRATEGY=crdt`): 33 sync tests +
  19 Diary sync tests green.

### Migration notes
- Cloud env: add `SYNC_MERGE_STRATEGY=lmw` (default), `PROMETHEUS_ENABLED=false`,
  `SENTRY_DSN=`, `BACKUP_DIR=/var/backups/cloud`,
  `BACKUP_RETENTION_DAYS=30` to your `.env`. None of them flip behaviour
  unless explicitly enabled.
- Diary `.env` example: `CLOUD_URL` bumped from `http://127.0.0.1:5000`
  to `http://127.0.0.1:5001` (port collision with macOS Control Center).
- Run `alembic upgrade head` on Cloud (no schema additions in 1.1, just
  config); Diary applies migration `0004_baseline_version.sql`
  automatically on first launch (idempotent, backfills existing rows
  with `baseline_version = version`).

### Known follow-ups (deferred to Faz 2/3)
- **Faz 1.4.D — Android background battery soak** (1-hour ekran-kapalı
  test). The other three Android stops landed; this one is opt-in
  monitoring, not a blocker for the Faz 1 close.
- **Multi-client live CRDT manuel test** the full Faz 3.3 stop, blocked
  on `journal_ai_reporter` sidecar restart and a second cloud account
  setup. Auto coverage via the convergence + diff-soak property tests
  in `cornell-diary/src-tauri/src/crdt/document.rs::tests` is in place.

---

## [v0.1.0] — FAZ 1.0–3.3 + Final (earlier sessions, summary)

Pre-Yol-Haritası baseline that this changelog tracks against. Brief:

- **FAZ 1.0–1.3**: Postgres backend (no SQLite anywhere), repo trait
  abstraction, Tauri 2 setup hook + DB pool + migrations runner.
- **FAZ 2.1–2.3**: Cloud REST sync (auth, journals, pull, push,
  conflict resolver, network monitor, hourly cron groundwork).
- **FAZ 3.1–3.3**: char-level RGA CRDT engine in Rust + WS pipeline +
  pending_ops persistence + React `useCrdtChannel` hook + cursor
  preservation in `MainNotesArea`.
- **Final**: README rewrite, OPERATIONS / SYNC_BEHAVIOR / THREAT_MODEL
  docs, 500-mutation diff soak, `cargo audit` (2 accepted risks),
  reactive 401 refresh helper, dedicated `TEST_DATABASE_URL`
  convention.
- **Bölüm 11.5**: `journal_ai_reporter` sidecar moved off SQLite onto
  asyncpg/Postgres so it stays in sync with the FAZ 1.3 storage swap.

48 Rust + 58 frontend (Diary) + 113 Reporter = 219 tests green at the
Faz-0 baseline.
