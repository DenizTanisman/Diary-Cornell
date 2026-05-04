# Diary Security Checklist — MD 03 Sprint

## Cloud Profile Switch
- [x] URL validation — `validate_base_url` rejects anything not http/https
      and anything `Url::parse` rejects.
- [x] HTTPS warning logged via `tracing::warn!` for non-localhost http
      profiles (`commands/profile.rs::is_localish`).
- [x] API key field is `<input type="password" autoComplete="off">` —
      browser autofill suppressed.
- [x] Active profile change triggers `engine.disconnect()` so stale auth
      tokens never reach the new base URL.
- [x] Seeded `local` + `production` rows are protected from delete
      (`is_protected`).
- [x] Restart-required UX prevents partial in-flight sync hitting the
      new URL mid-cycle (decision: 2026-05-04 [03.1.4]).

## LLM Panel
- [x] Bridge URL re-validated each call (`BridgeClient::new`).
- [x] Input length limit (50K char) enforced in
      `commands/llm.rs::validate_text` AND on the bridge side.
- [x] Style allowlist (`brief`/`detailed`/`bullet`) enforced both in
      Rust (`require_style`) and bridge (`pattern="^(brief|detailed|bullet)$"`).
- [x] `enabled=false` returns `Validation("LLM features are disabled")`
      from every llm_* command before any network call.
- [x] AI outputs (`ai_summary`, `ai_tags`, `ai_sentiment`) are NOT
      pushed to the Cloud sync surface — privacy-by-default. See
      `db/migrations/0006` and SyncEngine push payload (no ai_* fields).
- [x] `BridgeClient` never logs the API key.
- [x] Bridge unreachable → typed `DomainError::Storage` → React shows
      it via `setError(String(e))`; the app does not crash.

## Android-specific
- [x] `network_security_config.xml` allows cleartext only for:
      localhost, 127.0.0.1, 10.0.2.2 (emulator), 10/8, 172.16/12,
      192.168/16. Production URLs (cloud.example.com) MUST be HTTPS.
- [x] `AndroidManifest.xml` references the new config alongside the
      existing `usesCleartextTraffic` flag (config wins per-domain).
- [ ] **Follow-up:** API key + tokens in Android Keystore-backed
      storage (currently SQLite). Tracked as a Faz 4 item.

## Tests
- [x] Cargo: 62 unit tests pass (was 51 → +9 new: cloud_profile 3,
      llm_settings 1, bridge_client 2, profile commands 1, llm commands 2).
      No regression.
- [x] Vitest: 66 tests pass (was 58 → +8 new: cloudProfile 4 +
      llmSettings 4).
- [x] Both backend builds compile (`--features postgres` default,
      `--features sqlite --no-default-features`).
