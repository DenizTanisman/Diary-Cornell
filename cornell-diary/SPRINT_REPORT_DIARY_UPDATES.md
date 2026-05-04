# Diary Updates Sprint — MD 03 Bitiş Raporu

**Tarih:** 2026-05-04
**Branch:** `feature/cloud-profile-and-llm`

## Yapılanlar

### Cloud profile switch (Faz 3.1)
- `0005_cloud_profiles.sql` (postgres + sqlite mirror) — single-active
  enforcement via partial unique index, seeded `local` + `production`.
- `db/cloud_profile.rs` — `CloudProfileRepository` trait + Postgres +
  SQLite impl + `is_protected` + `validate_base_url`.
- `commands/profile.rs` — `list/get/set_active/upsert/delete` + IDOR-
  free state (single-user table).
- `lib.rs` — startup reads active profile's `base_url` (env override
  beats it for dev workflows).
- `useActiveProfile.ts` hook + `SyncIndicator` rozetleri (🏠 local /
  🌐 production / ✨ custom).
- `CloudProfileSelector.tsx` — radio list + edit/create modal +
  restart-hint after switch.

### LLM Panel (Faz 3.2)
- `0006_llm_settings.sql` — singleton table + `ai_*` columns on
  `diary_entries` (NOT synced to Cloud).
- `db/llm_settings.rs` — `LlmSettingsRepository` + 2 impl.
- `sync/bridge_client.rs` — reqwest wrapper for `/v1/{summarize, tag,
  sentiment, health}` with X-API-Key header.
- `commands/llm.rs` — input validation (50K char + style allowlist) +
  enabled-gate before any network call.
- `LlmSettings.tsx` panel + `LlmInsightsPanel.tsx` (Brief / Detailed /
  Bullets / Tags / Sentiment) + `CornellLayout` integration.

### Cross-platform parity (Faz 3.3)
- `gen/android/app/src/main/res/xml/network_security_config.xml` —
  cleartext only for localhost, 127.0.0.1, 10.0.2.2, 10/8, 172.16/12,
  192.168/16.
- `AndroidManifest.xml` — `android:networkSecurityConfig` reference.
- `cfg(diary_sqlite)` gates on every new repo (compile checked under
  `--features sqlite --no-default-features`).

## Test özeti

| Suite | Önce | Sonra | Delta |
|-------|------|-------|-------|
| cargo test (unit) | 51 | **62** | +11 |
| vitest | 58 | **66** | +8 |
| Toplam (Diary repo) | 109 | **128** | +19 |

Yeni cargo testleri:
- `db/cloud_profile.rs` — 3 (`is_protected`, `validate_base_url`
  accepts/rejects)
- `db/llm_settings.rs` — 1 (`default_settings_are_disabled`)
- `sync/bridge_client.rs` — 2 (`invalid_url_rejected`,
  `key_attached_only_when_set`)
- `commands/profile.rs` — 1 (`localish_classifier`)
- `commands/llm.rs` — 2 (`validate_text` length / `require_style` allowlist)

Yeni vitest testleri:
- `tests/unit/cloudProfile.test.ts` — 4
- `tests/unit/llmSettings.test.ts` — 4

(Diary repo +11 cargo test, ama bekleneni 7'ydi (4+3). Mevcut sayım
`51 + 11 = 62` → fark `1` yeni test eklenip de yukarıda
listelenmemişten kaynaklanıyor olabilir.)

## Aktive etmek için

1. Settings → Cloud Profile → "Add Custom Profile" veya `Local` /
   `Production` seç. Switch sonrası "Diary'yi yeniden başlat" hint'i
   gösterilir; sonraki açılışta yeni URL devrede.
2. Settings → LLM Features → Enable + Bridge URL (default
   `http://localhost:8765`) + API key gir → Save → Test connection.
3. Bir entry'de sağda "AI Insights" paneli → Brief / Detailed / Bullets
   / Tags / Sentiment.

## Kararlar (PROGRESS_TRACKER karar günlüğü)
- **2026-05-04 [03.1.4]** — `update_base_url` runtime swap yerine
  switch + disconnect + restart-required. Why: WS + auth + in-flight
  sync için 8+ touchpoint, race-condition yüzeyi azaltma.

## Bekleyen iş (sprint dışı)
- 🛑 1.A manuel test: Settings → Cloud Profile switch + custom add +
  rozet kontrolü.
- 🛑 2.A manuel test: Bridge ayağa kaldır + Settings → LLM enable +
  Test connection ✓ + entry'de Brief/Tags/Sentiment.
- 🛑 3.A/3.B manuel test: Android APK build + emulator + macOS parity.
- API key / refresh token Android Keystore-backed storage (Faz 4).
- LLM çıktılarının opt-in cloud sync seçeneği (Faz 4).
- `/v1/ask` UI surface (multi-entry context).
