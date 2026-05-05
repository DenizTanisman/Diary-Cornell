# Cloud × Diary Cornell — Düzeltme Yol Haritası

**Kaynak:** Mimari Değerlendirme Raporu (1 Mayıs 2026)
**Hedef:** Sistemin şu anki çalışan REST + WS akışını bozmadan, kademeli olarak production-ready hale getirmek.
**Çalışma modeli:** Otonom mod aktif. Manuel test gerektirmeyen hiçbir adımda durma; raporla, devam et.

---

## 0. Çalışma Kuralları (Tüm Fazlar İçin Geçerli)

### 0.1 Otonom çalışma modu

Aşağıdaki tüm görevler otonom modda yürütülecektir:

- **Otomatik geç:** Aralardaki TÜM işlemler (sıradaki göreve geçme, branch açma, commit, merge, debug, refactor, test yazma + düzeltme, paket ekleme, dosya taşıma, linter, mock data) sormadan otonom yapılır.
- **Bitiş raporu:** Her aşama sonu 6–10 satır bitiş raporu düşülür, ardından sormadan sıradakine geçilir.
- **Sadece üç durumda dur:**
  - **(A) İşlevsel test** — Senin elle çalıştırıp gözlemlemen gerekiyorsa → 🛑 TEST DURAĞI
  - **(B) Geri alınamaz işlem** — GitHub public push, production secret değişikliği, başka repo'ya ilk dokunuş öncesi kısa özet ve onay
  - **(C) Tasarım belirsizliği** — Fonksiyonelliği değiştirecek seviyede ise sor; aksi halde en savunmacı seçenek alınır, log'a düşülür, devam edilir.

### 0.2 Branch ve commit disiplini

- Her madde ayrı feature branch (`feature/<madde-adı>`).
- Main her zaman çalışır kalır; merge sadece o feature'ın yeşil testleri + manuel test onayı sonrası.
- Feature flag yaklaşımı: yeni davranış env var ile opt-in; default = mevcut davranış.
- Local git akışı (branch → commit → merge); GitHub push sadece faz sonunda, geri alınamaz işlem olarak onayla.

### 0.3 Test stratejisi

- **Unit + integration:** Otonom — her commit öncesi otomatik koşar.
- **Manuel işlevsel test:** 🛑 ile işaretli; sen onaylayana kadar bekler.
- **Smoke test (her merge sonrası, otonom):** `pytest && cargo test && pnpm test && curl /health` zinciri.

### 0.4 Geri dönüş (rollback) garantisi

Her madde için:
- Default davranış değişmez (feature flag default OFF).
- Migration varsa → reversible olmalı (Alembic downgrade test edilmiş).
- Merge sonrası 24 saat gözlem süresi; sorun çıkarsa flag OFF + revert.

### 0.5 Toplam zaman tahmini

| Faz | Süre | Hedef |
|---|---|---|
| Faz 0 (önkoşul) | 1 gün | Bu yol haritası, env hazırlık |
| Faz 1 (kritik) | 7–10 gün | Veri kaybı kapatma + mobil + backup + observability |
| Faz 2 (önemli) | 10–14 gün | Auth olgunlaştırma + CRDT GC + deploy |
| Faz 3 (ölçek) | İhtiyaç doğunca | Redis + multi-worker + partitioning |

---

# FAZ 0 — Önkoşullar (1 gün)

> Faz 1'e başlamadan önce gereken hazırlık. Hiçbir kod değişikliği YOK; sadece organizasyon.

## 0.A · Repo durumu temizliği

**Branch:** `chore/phase1-prep`

1. Cloud repo: tüm açık feature branch'lerini main'e merge edilmiş veya silinmiş olduğunu doğrula.
2. Diary repo: WIP commit'i (build.rs + lib.rs Android SQLite ayarları) ayrı `feature/android-build` branch'ine taşı, ana branch'i temiz bırak.
3. `.env.example` dosyası her iki repo'da güncel olduğunu doğrula; yeni eklenecek env var'lar için placeholder ekle:
   ```
   SYNC_MERGE_STRATEGY=lmw
   ENABLE_CRDT_GC=false
   BROADCAST_BACKEND=memory
   SENTRY_DSN=
   PROMETHEUS_ENABLED=false
   ```
4. README'ye "Faz 1 in progress" başlığı ekle, mevcut feature flag listesi.
5. ✅ Otonom doğrulama: tüm testler yeşil, repo temiz.

🛑 **TEST DURAĞI 0.A:** Sen onayla → "Faz 1'e başlayabiliriz."

---

# FAZ 1 — Kritik Düzeltmeler (7–10 gün)

> **Hedef:** Sessiz veri kaybını kapat, mobil platformu doğrula, "sunucu yandı" senaryosunu kurtarılabilir hale getir, sistemin nasıl çalıştığını görebil.

## 1.1 · Sync Semantiği — CRDT-aware Merge (öncelik #1)

**Branch:** `feature/crdt-rest-merge`
**Süre:** 2–3 gün
**Risk:** Yüksek (veri yolu değişiyor) — feature flag ile mitigate.

### Adım 1.1.1 — Wire şeması genişletme (otonom)

- `cloud/app/schemas/sync.py`: `SyncPushRequest`'e `baseline_version: int` opsiyonel alan ekle (default `None` → eski client'lar bozulmasın).
- `diary/src-tauri/src/sync/models.rs`: Mirror change. Optional field.
- Test: mevcut 5 sync testi yeşil kalmalı (eski client wire formatı korundu).

### Adım 1.1.2 — Server-side strateji ayrımı (otonom)

- `cloud/app/services/sync_service.py`:
  - `merge_field_level()` fonksiyonunu iki path'e ayır:
    - Text alanları (`note_taking_column`, `cue_column`, `summary_column`, `notes_column`) → CRDT op-replay path
    - Numeric/timestamp alanları → mevcut LMW path
  - Yeni env var: `SYNC_MERGE_STRATEGY=lmw|crdt` (default `lmw` → eski davranış).
- Test: yeni 3 integration test ekle:
  1. `lmw` modunda eski test'ler aynen yeşil (regression yok).
  2. `crdt` modunda offline + concurrent edit → her iki yazı korunur.
  3. `crdt` modunda baseline_version mismatch → conflict response döner.

### Adım 1.1.3 — Client-side baseline tracking (otonom)

- `diary/src-tauri/src/sync/engine.rs`:
  - PULL response'tan gelen `version` alanı `sync_metadata.last_seen_version` olarak persiste edilsin.
  - PUSH request'e `baseline_version = last_seen_version` eklensin.
- `diary/src-tauri/src/sync/conflict.rs`:
  - `ConflictResolver::resolve()` artık 3 yol: server kazansın / local kazansın / merge gerekli (CRDT op'ları replay).
- Test: yeni 4 Rust unit test (her resolve yolu için).

### Adım 1.1.4 — Otomatik smoke test (otonom)

```bash
# Mac üstünde otomatik
SYNC_MERGE_STRATEGY=crdt pytest -k sync
cd diary && cargo test sync::
pnpm test sync
```

🛑 **TEST DURAĞI 1.1:**
> Manuel senaryo:
> 1. Mac Diary aç, journal seç. Bir entry yaz: "satır A".
> 2. Tarayıcı test panelinden aynı journal'a bağlan, AYNI entry'nin AYNI alanına: "satır B" yaz.
> 3. Mac'te "Şimdi sync" tıkla.
> 4. **Beklenen:** Hem A hem B alanda görünür, biri ezilmemiş.
> 5. Sonuç: ✅ → merge, ❌ → branch'te kal, sebep tartış.

---

## 1.2 · Postgres Backup + Restore Drill

**Branch:** `feature/postgres-backup`
**Süre:** 1 gün
**Risk:** Düşük (additive, mevcut akış etkilenmez)

### Adım 1.2.1 — Backup script (otonom)

- `cloud/scripts/backup.sh`:
  ```bash
  pg_dump -Fc -h localhost -p 5434 -U cloud cloud_db > /backups/cloud_$(date +%Y%m%d_%H%M).dump
  ```
- Cron template: günlük 03:00 + haftalık tam yedek.
- `.env`: `BACKUP_DIR=/var/backups/cloud` (default), `BACKUP_RETENTION_DAYS=30`.

### Adım 1.2.2 — Restore script (otonom)

- `cloud/scripts/restore.sh`: dump dosyasını parametre alır, staging DB'ye geri yükler.
- README'de prosedür adımları.

### Adım 1.2.3 — Test: drill (otonom)

- Script ile mock journal + 50 entry oluştur → backup al → DB drop → restore et → tüm entry'ler eşleşiyor mu?
- pytest entegrasyon testi: backup/restore round-trip.

🛑 **TEST DURAĞI 1.2:**
> Manuel doğrulama:
> 1. `./scripts/backup.sh` çalıştır → dosya oluştu mu?
> 2. `./scripts/restore.sh <dosya>` test DB'ye → veriler aynı mı?
> 3. Sonuç: ✅ → merge.

---

## 1.3 · Observability Temeli — /metrics + Sentry + /health/ready

**Branch:** `feature/observability`
**Süre:** 1–2 gün
**Risk:** Düşük (tamamen additive)

### Adım 1.3.1 — Prometheus instrumentation (otonom)

- `pip install prometheus-fastapi-instrumentator`
- `cloud/app/main.py`: instrumentator init, `/metrics` endpoint açık.
- `.env`: `PROMETHEUS_ENABLED=false` default; true ise endpoint expose edilir.

### Adım 1.3.2 — Custom metrics (otonom)

- `cloud/app/metrics.py`:
  - `sync_pull_duration_seconds` (Histogram)
  - `sync_push_duration_seconds` (Histogram)
  - `crdt_op_apply_duration_seconds` (Histogram)
  - `ws_active_connections` (Gauge)
  - `crdt_pending_queue_size` (Gauge)
  - `sync_conflicts_total{strategy}` (Counter) — yeni 1.1 entegrasyonu için
- Service'lere instrumentation ekle.

### Adım 1.3.3 — /health ayrımı (otonom)

- `/health/live` → process up mu (mevcut /health davranışı).
- `/health/ready` → DB ping + cache erişimi + (varsa) Redis ping. Hepsi OK ise 200, biri fail ise 503.
- Eski `/health` deprecated alias olarak `/health/live`'a redirect (geriye uyumlu).

### Adım 1.3.4 — Sentry SDK (otonom)

- `pip install sentry-sdk[fastapi]`
- `cloud/app/main.py`: `SENTRY_DSN` env var varsa init et; yoksa skip (no-op).
- structlog mevcut akış bozulmaz; Sentry exception'ları otomatik yakalar.

### Adım 1.3.5 — Diary tarafı (otonom)

- `diary/src-tauri/src/main.rs`: `sentry-rust` ekle, aynı env var pattern'i.
- React tarafı: opsiyonel `@sentry/react` (browser hataları için).

🛑 **TEST DURAĞI 1.3:**
> Manuel doğrulama:
> 1. `PROMETHEUS_ENABLED=true` ile cloud başlat → `curl :5001/metrics` → metric'ler görünür mü?
> 2. Bir sync yap → `sync_pull_duration_seconds_count` artmış mı?
> 3. `/health/ready` → 200; Postgres'i durdur → 503.
> 4. Sentry test event: `python -c "import sentry_sdk; sentry_sdk.capture_message('test')"` → dashboard'da göründü mü?
> 5. Sonuç: ✅ → merge.

---

## 1.4 · İlk Android APK Build + LAN Doğrulama

**Branch:** `feature/android-build` (Faz 0'da hazırlanan)
**Süre:** 2–3 gün (ilk denemede kırılma beklenir)
**Risk:** Yüksek (bilinmeyen platform sorunları)

### Adım 1.4.1 — Build chain hazırlama (otonom)

- WIP commit'leri (build.rs + lib.rs SQLite cfg) düzgün commit'lere böl.
- `diary/scripts/android_build.sh` template:
  ```bash
  export ANDROID_HOME=/path/to/sdk
  export NDK_HOME=$ANDROID_HOME/ndk/26.x
  export DIARY_CLOUD_URL=http://172.18.57.25:5001
  pnpm tauri android build --apk --target aarch64
  ```
- Doc: `docs/ANDROID_SETUP.md` — NDK kurulum, env var'lar, ilk build adımları.

### Adım 1.4.2 — Cargo features doğrulama (otonom)

- `Cargo.toml`: `diary_sqlite` feature'ı default olmadığını ve sadece android target'ta aktif olduğunu doğrula.
- `cargo check --target aarch64-linux-android --features diary_sqlite` (cross-compile, host'ta çalışır).
- Hata çıkarsa: feature gate'leri tek tek izole et, hangisi kırılıyor logla.

### Adım 1.4.3 — APK build (otonom, kırılma riskli)

- `pnpm tauri android init` (ilk kez ise).
- `pnpm tauri android build`.
- **Eğer build kırılırsa:** error mesajı `docs/ANDROID_TROUBLESHOOTING.md`'ye loglanır; en savunmacı çözüm uygulanır (örn. NDK versiyonu sabitleme); devam edilir.

🛑 **TEST DURAĞI 1.4.A — APK üretildi mi?**
> Manuel:
> 1. `app-release-unsigned.apk` dosyası `target/android/...` altında oluştu mu?
> 2. ADB ile telefona yükle: `adb install <apk>`
> 3. Telefon ekranında uygulama açıldı mı?
> 4. Sonuç: ✅ → 1.4.4'e geç. ❌ → log + tartış.

### Adım 1.4.4 — Çevrim-dışı SQLite testi (otonom kurulum)

- Telefon Wi-Fi'ı KAPALI iken:
  - Login YAPMAYACAK (çünkü Cloud'a erişim yok); local-only mod.
  - Test mock kullanıcı + journal seed'i bundle içinde olmalı (yeni `--feature mock_seed_data` flag'i).

🛑 **TEST DURAĞI 1.4.B — Local SQLite çalışıyor mu?**
> Manuel:
> 1. Telefon Wi-Fi kapalı.
> 2. Uygulamayı aç, mock journal göründü mü?
> 3. Bir entry yaz, kaydet.
> 4. Uygulamayı kapat-aç → entry hâlâ orada mı?
> 5. Sonuç: ✅ → 1.4.5. ❌ → SQLite migration log incele.

### Adım 1.4.5 — LAN içi Cloud bağlantısı (otonom kurulum)

- Telefon Wi-Fi açık, Cloud sunucusu Mac'te çalışıyor.
- `DIARY_CLOUD_URL` build-time veya UI'dan ayarlanabilir hale getir (settings ekranı).

🛑 **TEST DURAĞI 1.4.C — Telefon ↔ Cloud sync**
> Manuel:
> 1. Telefonda settings → Cloud URL: `http://<mac-lan-ip>:5001`.
> 2. Login (test/test2 kullanıcısı).
> 3. Journal listesi geldi mi?
> 4. Bir entry yaz → Şimdi sync → Mac Diary'de göründü mü?
> 5. Mac'te bir entry yaz → telefonda sync → göründü mü?
> 6. Sonuç: ✅ → merge.

### Adım 1.4.6 — Battery optimization gözlemi (otonom kurulum)

- Telefon ekranı 1 saat kapalı kalsın, Diary background'da.
- Sync log: kaç kez tetiklendi, ne kadar gecikmeli? Sonuç `docs/ANDROID_BATTERY.md`'ye.

🛑 **TEST DURAĞI 1.4.D — Background davranışı**
> Manuel:
> 1. Mac'ten 5 entry yaz, 10 dakika ara ile.
> 2. Telefon ekran kapalı, 1 saat sonra ekranı aç.
> 3. Senkronize oldu mu? Kaç saniye sürdü?
> 4. Sonuç: kabul edilebilir mi → merge. Çok yavaşsa → battery optimization muafiyet rehberi yaz, kullanıcıya yönlendir.

---

## 1.5 · Faz 1 Kapanışı

### Adım 1.5.1 — Smoke test full suite (otonom)

```bash
# Cloud
cd cloud && pytest --tb=short
# Diary
cd diary && cargo test && pnpm test
# E2E
./scripts/e2e_stress.sh
```

### Adım 1.5.2 — Doc güncelleme (otonom)

- README "Faz 1 tamamlandı" rozetli.
- `docs/ROADMAP.md`: Faz 2 hazırlığı.
- CHANGELOG.md: tüm 1.1–1.4 commit'leri özetlenmiş.

### Adım 1.5.3 — GitHub push (geri alınamaz işlem)

🛑 **GERİ ALINAMAZ İŞLEM ONAYI:**
> "Faz 1 tüm değişiklikler GitHub'a push edilecek. Cloud private kalır, Diary public. Onaylıyor musun?"

Sonra:
```bash
cd cloud && git push origin main
cd diary && git push origin main
```

### 🎯 Faz 1 Çıkış Kriteri

- ✅ 156 mevcut test + ~20 yeni test yeşil.
- ✅ Off-line + concurrent edit veri kaybı senaryosu kapatılmış.
- ✅ Postgres backup/restore drill başarılı.
- ✅ /metrics + /health/ready + Sentry çalışıyor.
- ✅ Android APK telefonda çalışıyor, LAN içi Cloud sync doğrulandı.

---

# FAZ 2 — Önemli Düzeltmeler (10–14 gün)

> **Hedef:** Auth/identity boşluklarını kapat, CRDT engine'i uzun ömür için olgunlaştır, production deployment akışını netleştir.

## 2.1 · Auth/Identity Olgunlaştırma

**Branch:** `feature/auth-hardening`
**Süre:** 3–4 gün

### Adım 2.1.1 — Email verification (otonom)

- `cloud/app/services/email_service.py`: SMTP wrapper (env var: `SMTP_HOST`, `SMTP_USER`, `SMTP_PASS`). Dev için Mailtrap, prod için Resend/Postmark.
- Yeni endpoint'ler:
  - `POST /auth/register` → user oluşturulur ama `email_verified=false`. Verification token email'le gönderilir.
  - `GET /auth/verify?token=...` → token tüketilir, `email_verified=true`.
- Migration: `users.email_verified BOOLEAN DEFAULT false`.
- Test: 4 yeni integration test.

### Adım 2.1.2 — Password reset (otonom)

- `POST /auth/forgot-password { email }` → 1 saatlik tek-kullanım token.
- `POST /auth/reset-password { token, new_password }` → token tüketilir.
- Migration: `password_reset_tokens` tablosu.
- Test: 3 yeni integration test (success, expired, reuse).

### Adım 2.1.3 — Login rate limit + brute force koruma (otonom)

- `slowapi` mevcut, ama login için ayrı limiter:
  - 5 başarısız login/min/IP+username kombinasyonu.
  - 10 başarısız sonrası 15 dakika kilitleme.
- Test: 2 yeni integration test.

### Adım 2.1.4 — Refresh token rotation + blacklist (otonom)

- Her `/auth/refresh` çağrısında eski refresh token blacklist'e (in-memory dict, prod'da Redis önerilir).
- Aynı token ikinci kez kullanılırsa → tüm token'lar invalidate (token reuse detection).
- Yeni endpoint: `POST /auth/logout` → o anki token'ı blacklist'e.
- Migration: `revoked_tokens` tablosu (kalıcı blacklist için).

### Adım 2.1.5 — Diary tarafı entegrasyon (otonom)

- `diary/src/components/auth/`: ForgotPassword + ResetPassword + EmailVerificationPending ekranları.
- `diary/src-tauri/src/sync/auth.rs`: yeni endpoint'ler için method'lar.

🛑 **TEST DURAĞI 2.1:**
> Manuel:
> 1. Yeni hesap oluştur → email geldi mi?
> 2. Verification linkine tıkla → onaylandı mı?
> 3. Şifremi unuttum → email geldi mi → reset oldu mu?
> 4. 6 kez yanlış şifre dene → kilitlendin mi?
> 5. Logout → tekrar giriş yap → yeni token aldın mı?
> 6. Sonuç: ✅ → merge.

---

## 2.2 · CRDT Tombstone GC

**Branch:** `feature/crdt-gc`
**Süre:** 2 gün
**Risk:** Orta (op log'a dokunuyor) — feature flag ile mitigate.

### Adım 2.2.1 — Causal stability bound hesabı (otonom)

- `cloud/app/services/crdt_service.py`:
  - `compute_stable_watermark(journal_id) -> int`: tüm aktif peer'ların `last_seen_op_id`'lerinin minimumu.
  - "Aktif peer" tanımı: son 30 gün içinde sync_state.last_pull_at güncellenmiş.

### Adım 2.2.2 — GC pass (otonom)

- `snapshot_service.py` 30s loop'una `gc_pass(journal_id)` adımı eklensin:
  ```python
  if not settings.ENABLE_CRDT_GC: return
  watermark = compute_stable_watermark(journal_id)
  await session.execute(
      delete(CrdtOperation).where(
          CrdtOperation.id < watermark,
          CrdtOperation.is_deleted == True
      )
  )
  ```
- Env var: `ENABLE_CRDT_GC=false` default.
- Metric: `crdt_gc_deleted_ops_total` counter.

### Adım 2.2.3 — Test (otonom)

- 3 yeni integration test:
  1. GC kapalı → op'lar silinmez.
  2. GC açık + tüm peer'lar güncel → eski tombstone'lar silinir.
  3. GC açık + bir peer geride → o peer'ın watermark'ı altındaki op'lar KORUNUR.

🛑 **TEST DURAĞI 2.2:**
> Manuel:
> 1. Mac + telefondan aynı journal'a yoğun edit yap (50+ char yaz, 20 sil).
> 2. `crdt_operations` tablo satır sayısını not et.
> 3. `ENABLE_CRDT_GC=true` ile cloud restart → 60s bekle.
> 4. Tablo satır sayısı azaldı mı? Görünür text bozulmadı mı?
> 5. Sonuç: ✅ → merge.

---

## 2.3 · Production Deployment Pipeline

**Branch:** `feature/prod-deployment`
**Süre:** 3–4 gün
**Risk:** Yüksek (yeni infra, ilk public exposure)

### Adım 2.3.1 — docker-compose.prod.yml (otonom)

- Servisler:
  - `nginx` (TLS termination, reverse proxy)
  - `cloud` (uvicorn, 1 worker — multi-worker Faz 3'te)
  - `postgres` (16, named volume, backup mount)
  - `prometheus` (opsiyonel profile: monitoring)
  - `grafana` (opsiyonel profile: monitoring)
- `.env.prod.example` template.

### Adım 2.3.2 — TLS + Caddy (otonom kurulum)

- Caddy ile Let's Encrypt otomatik (nginx alternatifi).
- `Caddyfile`:
  ```
  cloud.example.com {
      reverse_proxy cloud:8000
  }
  ```
- DNS yönlendirmesi senin tarafında (manuel adım).

### Adım 2.3.3 — Secret yönetimi (otonom)

- Prod'da `.env` yerine: Doppler veya 1Password CLI veya Hashicorp Vault.
- Önerilen: Doppler (free tier yeterli, kurulum kolay).
- `docker-compose.prod.yml` secret'ları doppler'dan çeker:
  ```yaml
  command: doppler run -- uvicorn app.main:app
  ```

### Adım 2.3.4 — CI/CD (GitHub Actions, otonom)

- `.github/workflows/test.yml`:
  - Her PR'de: pytest + cargo test + vitest yeşil olmalı.
- `.github/workflows/deploy.yml`:
  - `main` push → Docker image build + push to GHCR.
  - Production deploy MANUEL approve gerektirir.

### Adım 2.3.5 — Restore drill (otonom)

- Faz 1.2'deki backup script'i prod ortamda çalıştır.
- Staging DB'ye restore et, smoke test.

🛑 **GERİ ALINAMAZ İŞLEM ONAYI 2.3.A:**
> "İlk public deployment yapılacak. VPS hazır mı? DNS ayarlandı mı? Onayla."

🛑 **TEST DURAĞI 2.3:**
> Manuel:
> 1. `https://cloud.example.com/health/live` → 200?
> 2. Telefon (LAN dışında, mobil veriden) login olabildi mi?
> 3. Mac Diary'de Cloud URL'i prod'a çevir → sync çalıştı mı?
> 4. Sentry'de prod environment hata akışı görünüyor mu?
> 5. Grafana dashboard açık mı?
> 6. Sonuç: ✅ → merge + tag v1.0.0.

---

## 2.4 · Faz 2 Kapanışı

### Adım 2.4.1 — Doc + CHANGELOG (otonom)

- README v1.0 rozetli.
- `docs/PRODUCTION.md`: deployment runbook.
- `docs/INCIDENT_RESPONSE.md`: temel runbook (DB down, cache miss storm, vs).

### Adım 2.4.2 — GitHub release (geri alınamaz)

🛑 **ONAY:**
> "v1.0.0 tag + GitHub release oluşturulacak. Onayla."

### 🎯 Faz 2 Çıkış Kriteri

- ✅ Email verify + password reset + login rate limit + token rotation çalışıyor.
- ✅ CRDT tombstone GC opsiyonel olarak aktive edilebilir.
- ✅ Production deployment pipeline çalışıyor, TLS + secrets yönetimi var.
- ✅ Backup + restore drill prod ortamda doğrulandı.
- ✅ Sentry + Grafana prod metric'leri akıyor.

---

# FAZ 3 — Ölçek (Tetik: gerçek ihtiyaç doğunca)

> **Önemli:** Faz 3 maddelerine ŞU AN başlamayın. Tetikleyici metrikler aşağıda. O metrikler yakalanmadan başlamak prematür optimizasyondur.

## Tetikleyici metrikler

| Madde | Tetik |
|---|---|
| 3.1 Redis pub/sub + multi-worker | Eşzamanlı aktif kullanıcı > 10 VEYA P95 latency > 500ms |
| 3.2 Pending queue overflow protection | `crdt_pending_queue_size` peak > 5000 |
| 3.3 crdt_operations partitioning | Tablo satır sayısı > 1M VEYA pull batch > 2s |
| 3.4 MFA / TOTP | Kullanıcı talebi VEYA hassas veri sınıfı eklendiğinde |

## 3.1 · Redis Pub/Sub + Multi-Worker

**Branch:** `feature/redis-broadcast`
**Süre:** 4–5 gün

### Plan özeti

1. `BroadcastBus` interface tanımla (publish + subscribe).
2. İki implementation: `InMemoryBus` (mevcut) + `RedisBus` (yeni).
3. `ConnectionManager` bu interface'i kullansın.
4. `CRDTDocumentCache` aynı pattern: `InMemoryCache` + `RedisCache`.
5. `BROADCAST_BACKEND=memory|redis` env var.
6. `docker-compose.prod.yml`: 4 worker + Redis 7.

🛑 **TEST DURAĞI 3.1:**
> Manuel: 4 worker, 2 farklı browser sekmesinde aynı journal'a bağlan, broadcast tutarlı mı?

## 3.2 · Pending Queue Overflow Protection

**Branch:** `feature/crdt-overflow`
**Süre:** 1 gün

- `MAX_PENDING_OPS=10000` env var.
- Aşıldığında en eski op'lar drop, peer'a `resync_required` mesajı.
- Sentry alert.

## 3.3 · crdt_operations Partitioning

**Branch:** `feature/crdt-partitioning`
**Süre:** 2–3 gün
**Risk:** Çok yüksek (büyük tablo migration)

- Önkoşul: backup drill başarılı (Faz 1.2 + 2.3.5).
- pg_partman extension kur.
- Aylık partitioning, eski partition'lar archive policy.
- Migration sırasında downtime: maintenance window planla.

## 3.4 · MFA / TOTP

**Branch:** `feature/mfa-totp`
**Süre:** 2 gün

- pyotp + qrcode ile TOTP secret üretimi.
- Diary UI: QR kodu + 6-haneli kod doğrulama.

---

# Genel Zaman Çizelgesi

```
Hafta 1     ┃ Faz 0 + 1.1 (sync semantiği)
Hafta 2     ┃ 1.2 backup + 1.3 observability
Hafta 3     ┃ 1.4 Android (büyük blok)
Hafta 4     ┃ 1.5 kapanış + 2.1 auth
Hafta 5     ┃ 2.1 auth devam + 2.2 CRDT GC
Hafta 6     ┃ 2.3 production deployment
Hafta 7     ┃ 2.4 kapanış + v1.0 release
Sonra       ┃ Faz 3 — sadece tetikleyici metrik yakalandığında
```

---

# Hızlı Referans — Komutlar

## Otonom çalışma komutları

```bash
# Smoke test (her merge öncesi)
cd cloud && pytest --tb=short
cd diary && cargo test && pnpm test

# E2E
./scripts/e2e_stress.sh

# Backup
./cloud/scripts/backup.sh

# Local prod simülasyonu
docker-compose -f docker-compose.prod.yml up

# Android build
cd diary && pnpm tauri android build --apk
```

## Feature flag durumları (özet)

| Flag | Default | Faz |
|---|---|---|
| `SYNC_MERGE_STRATEGY` | `lmw` | 1.1 |
| `PROMETHEUS_ENABLED` | `false` | 1.3 |
| `SENTRY_DSN` | (empty) | 1.3 |
| `ENABLE_CRDT_GC` | `false` | 2.2 |
| `BROADCAST_BACKEND` | `memory` | 3.1 |
| `MAX_PENDING_OPS` | `10000` | 3.2 |

---

# Kritik Hatırlatmalar

1. **Sistem her merge sonrası ÇALIŞIR durumda kalmalı.** Default davranış değişmez; yeni davranış opt-in.
2. **Otonom modda her şey otomatik akar**, sadece 🛑 işaretli yerlerde dur.
3. **Faz 3'e prematür başlama** — gerçek metrik tetiklemesi olmadan ölçek altyapısı = gider.
4. **Yapılmaması gerekenler** (mimari raporundan): microservice ayrıştırması, K8s, Yjs'e geçiş, E2EE, yeni feature ekleme. Faz 1-2 bitmeden açma.
5. **Geri alınamaz işlemlerde** (GitHub push, prod deploy, v1.0 tag) HER ZAMAN onay iste.

---

**Yol haritası sonu.** Bir sonraki adım: Faz 0.A'yı başlatmak için onay.
