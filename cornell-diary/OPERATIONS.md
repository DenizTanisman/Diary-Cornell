# Operations

Day-2 operational reference for Cornell Diary (Tauri 2 + Postgres 16 + optional Cloud).

## Environment

The Rust setup hook reads `DATABASE_URL` and `CLOUD_URL` from the **process environment** — it
does **not** auto-load `.env`. Pick one of these:

```bash
# Option A: export in shell before launching
export DATABASE_URL='postgres://diary_user:change_me_in_dev@127.0.0.1:5435/diary_db'
export CLOUD_URL='http://127.0.0.1:5000'
export CLOUD_WS_URL='ws://127.0.0.1:5000'
export RUST_LOG='cornell_diary=info,sqlx=warn'
pnpm tauri dev

# Option B: use direnv / mise / your shell init
```

`SQLITE_LEGACY_PATH` is the one-shot migration source path used by FAZ 1.2 only — you can leave it
unset on a fresh install.

## Schema migrations

`sqlx::migrate!` reads the directory at compile time and embeds every `*.sql` file in the binary.
Re-running is idempotent — sqlx tracks state in the `_sqlx_migrations` table.

| Version | File                                         | Adds                                |
| ------- | -------------------------------------------- | ----------------------------------- |
| 1       | `postgres_migrations/0001_initial.sql`       | `diary_entries`, `sync_log`, `app_settings` |
| 2       | `postgres_migrations/0002_sync_metadata.sql` | `sync_metadata` singleton + dirty bit |
| 3       | `postgres_migrations/0003_pending_ops.sql`   | `pending_ops` queue (FAZ 3.2)       |

### Confirming migration state

```bash
psql "$DATABASE_URL" -c 'SELECT version, success FROM _sqlx_migrations ORDER BY version'
```

If the latest migration shows `success=f` or is missing, the app failed at boot — see the panic
reason in the launch log. FAZ 3.2's `0003_pending_ops` requires `entry_date TEXT` (matching
`diary_entries.date TEXT`). Postgres rejects `DATE → TEXT` foreign keys with
`cannot be implemented`; the migration ships with the correct type, but be wary if you fork it.

### Re-running a failed migration

`sqlx::migrate!` won't re-run a migration that already has a row in `_sqlx_migrations` — even a
failed one. To retry:

```sql
DELETE FROM _sqlx_migrations WHERE version = 3;
-- relaunch the app; it will re-apply the SQL file
```

Drop any half-created tables manually first.

## SQLite → Postgres migration (FAZ 1.2, one-shot)

This was the bootstrap path for upgrading existing pre-FAZ-1 installs. It is idempotent and safe
to re-run.

1. Set `SQLITE_LEGACY_PATH` to the old `cornell_diary.db` (default macOS location:
   `~/Library/Application Support/com.deniz.cornelldiary/cornell_diary.db`).
2. Set `DATABASE_URL` to the empty Postgres.
3. Launch the app. On first run the migration command lifts every diary row, version + cue cells
   intact, into Postgres.
4. After confirming entries are visible, you can unset `SQLITE_LEGACY_PATH` — the legacy file is
   never modified.

The migration command is registered as a Tauri command (`migrate_from_sqlite`) and surfaced from
the Sync settings panel. There is no automatic trigger on boot — the user must opt in.

## Rollback

The branches here are merged with `--no-ff` so each FAZ has its own merge commit on `main` and is
revertable:

| FAZ                | Revert merge commit                          | Side effects                                                  |
| ------------------ | -------------------------------------------- | ------------------------------------------------------------- |
| FAZ 1.3 (Postgres) | `git revert -m 1 <merge sha for FAZ 1.3>`    | App refuses to boot if `STORAGE_BACKEND=postgres` not unset.   |
| FAZ 2 (Cloud REST) | `git revert -m 1 <merge sha for FAZ 2.x>`    | `connect_cloud` / `trigger_sync` commands disappear; metadata stays in `sync_metadata`. |
| FAZ 3.1 (CRDT eng) | revert FAZ 3 stack                           | `crdt::*` modules disappear; no schema impact.                |
| FAZ 3.2 (WS pipe)  | revert + drop `pending_ops`                  | `pending_ops` table left behind — drop manually if rolling back fully. |
| FAZ 3.3 (frontend) | revert merge                                 | `useCrdtChannel`, `PresenceBadge` removed; UI returns to debounced REST save only. |

`git log --oneline --merges main` shows every FAZ merge.

## Manual data fixups

Test fixtures historically leaked into shared dev DBs. Two known shapes:

### Test peer_id pollution (`alice@laptop`)

If `disconnect_cloud → reconnect` keeps showing a peer like `alice@laptop`, a test seeded
`sync_metadata` directly. Clean it:

```sql
UPDATE sync_metadata
SET peer_id = '', cloud_journal_id = NULL,
    access_token = NULL, refresh_token = NULL, token_expires_at = NULL,
    cloud_user_id = NULL, sync_enabled = FALSE
WHERE id = 1;
```

Then re-connect from the UI; a fresh UUID peer_id is generated.

### `version=0` row blocking pushes

Cloud's `PushEntry` enforces `version >= 1`. If a test seeded a row with `version=0`:

```sql
UPDATE diary_entries SET version = 1, is_dirty = TRUE
WHERE date = '<the date>' AND version = 0;
```

This is also documented in the THREAT_MODEL as an accepted risk of sharing a single dev DB across
`cargo test` and `tauri dev`.

## Auth / token lifecycle

`AuthManager::get_or_refresh` is purely **proactive** — it calls `/auth/refresh` only when the
locally-stored `token_expires_at` is within 60 s of expiring.

If the user gets a `401 token_invalid` mid-sync (Cloud restart, server-side revoke) the only
remedy today is **Bağlantıyı Kes → Cloud'a Bağlan**. Reactive 401-detect → refresh-and-retry is a
known backlog item.

## Logs

`RUST_LOG=cornell_diary=info,sqlx=warn` is the default. Useful greps:

```bash
# Postgres backend confirmation
grep "postgres backend"

# Network monitor transitions
grep "network state transition"

# Sync engine errors (pull/push)
grep -E "trigger_sync|push|pull"

# CRDT WS pipeline
grep "cornell_diary::ws"
```

The user's Cloud **password** never appears in logs — it lives only in the `connect_cloud`
command argument and is dropped after `client.login`. Diary content also never lands in logs
(only counts, durations, peer ids).
