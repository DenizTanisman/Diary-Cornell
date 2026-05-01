# Changelog

Roadmap-driven phases. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
"Unreleased" is what's on `main` but not yet tagged.

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
