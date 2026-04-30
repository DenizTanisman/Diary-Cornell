# Threat model

Adapted OWASP API Top 10 review for Cornell Diary's three-tier surface (React frontend, Tauri/
Rust backend, optional Cloud REST + WS server). The Cloud server itself has its own threat model
in the `journal_ai_reporter` repo — this document covers the Diary client and the trust
boundaries between it and the Cloud.

## Trust boundaries

```
┌──────────────────────────────────────────┐
│ React UI (browser process, sandboxed)   │
│   ▲ invoke()                             │
│   ▼                                      │
│ Tauri/Rust backend (full OS access)     │ ◄── trust boundary 1
│   ▲ libpq via sqlx                       │
│   ▼                                      │
│ Postgres (local, dev: Docker)           │
└──────────────────────────────────────────┘
                  │ HTTPS REST + WSS
                  ▼
            Cloud server                  ◄── trust boundary 2
```

Trust boundary 1: untrusted user input from the WebView reaches Rust. Mitigated by
Tauri's `#[command]` binding (typed args, no arbitrary RPC), capability scope, and parameterized
SQL.

Trust boundary 2: Cloud is a different security domain. Diary trusts Cloud's signed responses
**only** for sync state — not for code, not for UI, not for filesystem.

## Adapted OWASP API Top 10 (2023)

### 1. Broken Object Level Authorization

**Diary side**: every Postgres query scopes to the local single-tenant DB. There is no per-row
authorisation needed. Cloud-bound IPC includes `journal_id` from `sync_metadata` — never from
arbitrary user input on the wire.

**Mitigation**: `meta::read` is the single source of truth for `cloud_journal_id`; commands like
`trigger_sync` never accept a journal id from the React side.

### 2. Broken Authentication

**Diary side**: the user's Cloud password is only on the wire during `connect_cloud`. `AuthManager`
caches access + refresh tokens in `sync_metadata` (Postgres row, full-disk-encryption assumed).
JWTs are decoded with signature verification **disabled** locally — Cloud verifies on every
request, so doing it twice is just attack surface (we'd need its key locally).

**Known gap**: a 401 mid-session today requires manual reconnect. Reactive 401 → refresh path is
backlog.

### 3. Broken Object Property Level Authorization (formerly excessive data exposure)

**Diary side**: `DiaryEntry` JSON shipped to React is the same shape produced by `EntryRepository`
— no extra fields. Sync engine uses a separate `PushEntry` projection (`cue_column`,
`notes_column`, `summary`, `planlar`) so internal columns like `device_id` / `is_dirty` never
leak to Cloud.

### 4. Unrestricted Resource Consumption

**Diary side**:
- sqlx pool capped at 5 connections.
- WS client holds **one shared socket**, not one per entry.
- `pending_ops` queue is unbounded **by design** — offline editors can pile up keystrokes — but
  drains aggressively on reconnect.
- `materialize` walks the linked list with a hop limit (`nodes.len() + 4`) so a corrupt doc
  bails instead of looping.
- `is_in_subtree` and `end_of_subtree` carry the same defensive cap.

### 5. Broken Function Level Authorization

**Diary side**: every Tauri command is reachable from the WebView, but they all operate on the
single local user's data. There is no admin path. Migration commands (`migrate_from_sqlite`) are
gated by env vars (`SQLITE_LEGACY_PATH`) so they no-op in steady state.

### 6. Unrestricted Access to Sensitive Business Flows

**Diary side**: not applicable in a single-user desktop app.

### 7. Server Side Request Forgery

**Diary side**: `CLOUD_URL` is read once at boot from the env. No user-supplied URL is ever
fetched. `reqwest` uses rustls (no system OpenSSL) and respects timeouts.

### 8. Security Misconfiguration

- Tauri capabilities (`src-tauri/capabilities/default.json`) restrict filesystem access to
  `$APPDATA`, `$DOCUMENT`, `$DOWNLOAD`, `$HOME`. No arbitrary `fs.readFile`.
- React build does **not** ship sourcemaps in production.
- `tracing` env filter defaults to `cornell_diary=info,sqlx=warn` — no diary content reaches
  logs (only counts, durations, peer ids).
- `.env.example` ships with placeholder credentials so a fresh checkout doesn't accidentally
  point at a real DB.

### 9. Improper Inventory Management

Versioned migrations (`_sqlx_migrations`), versioned cargo deps (`Cargo.lock` committed), pnpm
lockfile committed. `cargo audit` runs in the Final phase checklist.

**Accepted risks** (`cargo audit` 2026-04-30):

- **RUSTSEC-2023-0071** (`rsa 0.9.10`, Marvin timing sidechannel) — pulled transitively by
  `sqlx-mysql`, which we never use but which sqlx 0.8 always builds. No upstream fix as of this
  writing. Diary doesn't speak the MySQL protocol, so the vulnerable code path isn't reachable
  from any of our calls. **Re-evaluate quarterly**; drop when sqlx 0.9 splits the protocol crates.
- **RUSTSEC-2024-0413** (`atk 0.18.2`, gtk3 unmaintained) — pulled by `wry → gtk` on Linux only.
  We don't ship Linux binaries today, so the path is dead in shipping artifacts. Tracking
  upstream tao/wry for the gtk4 migration.

### 10. Unsafe Consumption of APIs

- `serde_json::from_str` over Cloud responses uses typed structs with `#[serde(deny_unknown_fields)]`
  where it matters (`PushResponse`, `WsIn`).
- `CharOp` deserialisation enforces the `char_value` is exactly one Unicode scalar value
  (`CharString::deserialize`).
- WS frame handling silently drops malformed frames with a log line; never panics on garbage.

## Capability scope

[`src-tauri/capabilities/default.json`](src-tauri/capabilities/default.json) is the source of
truth. The notable allow-listed permissions:

- `core:*` — base Tauri runtime.
- `fs:scope-app`, `fs:scope-document`, `fs:scope-download`, `fs:scope-home` — limited file IO
  (export / import dialogs).
- `dialog:default` — open / save dialogs only.
- `clipboard-manager:default` — copy / paste of QR JSON.
- `os:default` — hostname for the device label.

Notably **denied**: `shell:execute`, `http:default` (Diary uses its own reqwest path, not Tauri's
HTTP plugin), arbitrary path access.

## Secrets handling

| Secret               | Lives where                             | Lifetime                          |
| -------------------- | --------------------------------------- | --------------------------------- |
| Cloud password       | `connect_cloud` argument only           | Dropped at end of `engine.connect` |
| Access token (JWT)   | `sync_metadata.access_token` (Postgres) | Until expiry (~1 h)               |
| Refresh token (JWT)  | `sync_metadata.refresh_token`           | Until rotation                    |
| Local `peer_id`      | `sync_metadata.peer_id`                 | Forever (durable identity)        |

Postgres rows are not in-app-encrypted. Full-disk encryption (FileVault on macOS, equivalent on
other OSes) is assumed for at-rest protection.

## Out of scope

- **Cloud server hardening** — see the `journal_ai_reporter` repo's own threat model.
- **Multi-tenant Diary** — there is no per-user isolation; the Postgres user's full DB is the
  single tenant.
- **Hostile-OS scenarios** — a malicious app running with the same user has full access. Mitigation
  is OS-level (sandboxing / different users), not in scope here.
