# Diary Cornell — Tauri/Rust + Postgres Migration + Cloud Sync + CRDT (Claude Code Master Prompt)

> **Audience:** Claude Code, çalışacağın gerçek hedef bir **Tauri 2 + Rust + React 18 + TypeScript + Vite + tauri-plugin-sql** uygulamasıdır. Python yok, FastAPI yok, `app.js` yok. Önceki `diary_prompt.md`'yi yok say — bu prompt onun yerine geçer.
>
> **Operating Mode:** Otonom Çalışma Modu aktiftir. Sadece **🛑 TEST DURAĞI** noktalarında dur ve elle test iste; aralardaki tüm operasyonlar (branch, commit, merge, debug, refactor, paket ekleme, test düzeltme) sormadan otonom yürütülür. Her aşama sonu 6-10 satır bitiş raporu, ardından sormadan sıradakine geçilir.
>
> **Pre-flight oku:** Bu prompt'a başlamadan önce `~/Projects/DiaryCornell/PROJECT_STATE_FOR_HANDOFF.md` dosyasını sonuna kadar oku. Aşağıdaki tüm kararlar oradaki gerçeklikle uyumludur — uyumsuz bir şey görürsen önce **handoff** kazanır, prompt değil.

---

## 0. BIG PICTURE (FEYNMAN)

Diary Cornell üç fazda dönüştürülecek. Her faz **mevcut özellikleri bozmadan** ekleme yapar:

```
FAZ 1 — Postgres'e geçiş               (Tauri Rust içinde sqlx, SQLite çıkar)
FAZ 2 — Cloud sync client              (REST + saatlik scheduler + network monitor)
FAZ 3 — Live çoklu kullanıcı (CRDT)    (Tauri Rust içinde WS + char-level engine + React UI)
```

**Mevcut sistem (handoff §3):**

```
[Tauri App: Rust + React/TS]
        │
        │ (tauri-plugin-sql IPC)
        ▼
   SQLite dosyası
   ~/Library/Application Support/com.deniz.cornelldiary/cornell_diary.db
        │
        │ (read-only, mode=ro)
        ▼
[Cornell Sidecar (FastAPI :8001)] ──► [Reporter Bridge (FastAPI :8002)] ──► [Jarvis (:8000)]
```

**Hedef sistem:**

```
[Tauri App: Rust + React/TS]                 [Cloud Server (~/Projects/Cloud/, :5000)]
        │                                              │
        │ (sqlx)                                       │ (asyncpg)
        ▼                                              ▼
   Local Postgres (:5432)  ◄──── REST sync ──────►   Cloud Postgres (:5433)
        │                  (saatlik / online trigger)
        │                  
        │   ◄──── WS CRDT ops ────►  (sadece çoklu kullanıcı aktifken)
        │
   (Sidecar projection devam eder — Reporter ekosistemi bozulmasın diye
    Postgres'ten okuyacak hale getirilir, ayrı issue)
```

**Üç hedef:**

1. SQLite → Postgres (Tauri'nin kendi içinde, sqlx ile)
2. Cloud sync (Tauri Rust backend'inde reqwest + tokio-cron-scheduler)
3. CRDT (Tauri Rust backend'inde tokio-tungstenite + char-level engine, React frontend WebView'a Tauri command + event ile bağlı)

**Kritik kısıt:** Mevcut frontend davranışı bozulmaz. Cornell layout (title_1..7 + content_1..7), debounced autosave, date navigation, QR/JSON sync — hepsi çalışmaya devam eder.

**Reporter ekosistemi (handoff §4):** Bu prompt'un **doğrudan** sorumluluğunda değil. Diary Postgres'e geçince Reporter'ın sidecar'ı (`cornell_journal_api/`) eski SQLite'a bakmaya devam eder ve **kırılır**. Bu prompt'un son aşamasında sidecar'ı **Postgres'e bağlayan ayrı bir issue** açılır (Bölüm 11.5).

---

## 1. ÖN ŞART KONTROLÜ — AŞAMA 0'DAN ÖNCE

Aşağıdakileri sırayla doğrula. Eksik varsa **dur**, raporla, kullanıcı yönlendirsin:

- [ ] `~/Projects/DiaryCornell/cornell-diary/` mevcut, `git status` temiz
- [ ] `cornell-diary` `main` branch güncel, push edilmemiş local commit yok
- [ ] Cloud server `:5000` çalışıyor: `curl http://127.0.0.1:5000/health` → 200
- [ ] Local Postgres `:5432` çalışıyor: `docker ps | grep postgres` veya `psql -p 5432 -l`
- [ ] Cloud Postgres `:5433` çalışıyor (referans)
- [ ] SQLite DB var: `~/Library/Application Support/com.deniz.cornelldiary/cornell_diary.db` (~45 KB, ~7 satır)
- [ ] SQLite yedeği alındı: `cp <db_path> ~/Projects/DiaryCornell/backups/sqlite_pre_migration_$(date +%Y%m%d_%H%M%S).db`
- [ ] Reporter ekosistemi çalışıyor (sidecar :8001 + Bridge :8002 + Jarvis :8000)
- [ ] `rustup` kurulu, `rustc --version` 1.75+ çıktısı veriyor
- [ ] `cargo --version` çalışıyor
- [ ] `node` 20+, `pnpm` veya `npm` çalışıyor
- [ ] `cloud_prompt.md`'nin Aşama 7'si tamamlanmış mı (Cloud REST + WS endpoint'leri canlı mı)

**Cloud endpoint reality check** (handoff §7 "not verified" diyor):

```bash
# Auth flow exists?
curl -X POST http://127.0.0.1:5000/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"_probe","email":"_probe@local","password":"_probe1234"}'

# Sync surface exists?
curl http://127.0.0.1:5000/sync/pull -i  # 401 beklenir, 404 değil
```

Sonuçlar 404 dönerse **Cloud yeterince hazır değil** — dur, kullanıcıya bildir. 401/422 dönerse endpoint var, devam.

---

## 2. TEKNİK STACK

### Mevcut (handoff §3.2, hiç değişmiyor)
- Tauri 2, edition 2021
- tauri-plugin-sql 2.4.0 (sqlite feature) — **kaldırılacak**
- tauri-plugin-fs / dialog / os / clipboard-manager / opener
- React 18.x + TypeScript + Vite
- react-router-dom v6
- Zustand + immer (varsayım)
- react-hook-form
- vitest

### Eklenen (Rust)

```toml
[dependencies]
# Database
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "macros", "chrono", "uuid"] }
tokio = { version = "1.40", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.10", features = ["v4", "serde"] }

# HTTP client (Cloud REST)
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# WebSocket client (Cloud WS, FAZ 3)
tokio-tungstenite = { version = "0.23", features = ["rustls-tls-webpki-roots"] }
futures-util = "0.3"

# Scheduler
tokio-cron-scheduler = "0.13"

# JWT decoding (refresh detection only; no signing)
jsonwebtoken = "9.3"

# CRDT support
once_cell = "1.20"
parking_lot = "0.12"  # daha hızlı RwLock
dashmap = "6.1"       # concurrent HashMap

# Utility
thiserror = "1.0"     # error types
tracing = "0.1"       # structured logging
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
anyhow = "1.0"        # internal errors
url = "2.5"

# Test
[dev-dependencies]
tokio = { version = "1.40", features = ["test-util", "macros"] }
mockito = "1.5"
tempfile = "3.13"
```

### Eklenen (Frontend, sadece FAZ 2-3 UI ekleri)

```json
{
  "dependencies": {
    // mevcut hepsi korunur
  }
}
```

Frontend'e yeni paket **gerekmez**. Tauri'nin native `invoke` ve `event` API'leri ile sync UI ve CRDT akışı sağlanır.

---

## 3. DOSYA YAPISI (DELTA — MEVCUT YAPIYA EK)

```
cornell-diary/
├── src-tauri/
│   ├── Cargo.toml                       # genişletilir
│   ├── migrations/                      # SQLite migrations (legacy, FAZ 1.3'te silinir)
│   │   └── 001_initial.sql
│   ├── postgres_migrations/             # YENİ — sqlx migrate
│   │   ├── 0001_initial.sql
│   │   ├── 0002_sync_metadata.sql
│   │   └── 0003_pending_ops.sql         # FAZ 3
│   ├── build.rs                         # mevcut, dokunulmaz
│   ├── tauri.conf.json                  # plugin listesi güncellenir (sql kaldırılır)
│   └── src/
│       ├── main.rs                      # mevcut (`mobile_entry_point` vs.)
│       ├── lib.rs                       # **genişletilir** — yeni komutlar register edilir
│       ├── db/                          # YENİ
│       │   ├── mod.rs
│       │   ├── pool.rs                  # PgPool factory + lazy init
│       │   ├── repository.rs            # Repository trait + impl'ler
│       │   ├── models.rs                # DiaryEntry, SyncMetadata, PendingOp structs
│       │   └── migrations.rs            # sqlx::migrate!() runner
│       ├── commands/                    # YENİ — Tauri komutları (frontend buradan invoke eder)
│       │   ├── mod.rs
│       │   ├── entries.rs               # get_entry / upsert_entry / list_entries / list_dirty
│       │   ├── sync.rs                  # connect_cloud / trigger_sync / get_sync_status / disconnect
│       │   └── crdt.rs                  # FAZ 3: apply_local_op / get_active_peers
│       ├── sync/                        # YENİ
│       │   ├── mod.rs
│       │   ├── auth.rs                  # JWT store + refresh
│       │   ├── client.rs                # CloudClient (reqwest)
│       │   ├── engine.rs                # SyncEngine — pull/push merge
│       │   ├── scheduler.rs             # tokio-cron-scheduler hourly job
│       │   ├── network.rs               # health-probe loop
│       │   └── conflict.rs              # version compare + last-write-wins
│       ├── crdt/                        # YENİ — FAZ 3
│       │   ├── mod.rs
│       │   ├── node.rs                  # CharNode struct
│       │   ├── document.rs              # CrdtDocument
│       │   ├── operations.rs            # InsertOp / DeleteOp serde
│       │   ├── conflict_resolver.rs     # peer_id tie-break
│       │   ├── snapshot.rs              # op log → text materialization
│       │   └── ws_client.rs             # tokio-tungstenite manager
│       ├── migration/                   # YENİ — bir-kerelik SQLite → Postgres
│       │   ├── mod.rs
│       │   ├── sqlite_reader.rs         # rusqlite ile read-only oku
│       │   └── migrate_command.rs       # Tauri command: migrate_sqlite_to_postgres
│       └── error.rs                     # YENİ — DomainError thiserror
├── src/                                 # frontend, çoğu MEVCUT korunur
│   ├── App.tsx
│   ├── pages/
│   │   ├── EntryEditor.tsx              # MEVCUT, FAZ 3'te WS event listener eklenir
│   │   └── SyncSettings.tsx             # YENİ — Cloud bağlantı + status UI
│   ├── components/
│   │   ├── SyncIndicator.tsx            # YENİ — sağ üst köşe 🟢🟡🔴⚪ chip
│   │   └── PresenceBadge.tsx            # YENİ FAZ 3 — "Ali yazıyor..."
│   ├── hooks/
│   │   ├── useSyncStatus.ts             # YENİ — `invoke('get_sync_status')` polling
│   │   ├── useDebouncedSave.ts          # MEVCUT, FAZ 3'te CRDT mode toggle eklenir
│   │   └── useCrdtChannel.ts            # YENİ FAZ 3 — Tauri event listener + invoke
│   ├── stores/
│   │   ├── (mevcut Zustand stores korunur)
│   │   └── syncStore.ts                 # YENİ
│   ├── db/                              # MEVCUT — tauri-plugin-sql wrapper'ları
│   │                                    # → FAZ 1.2'de invoke('xxx') çağrılarına çevrilir
│   └── types/
│       └── sync.ts                      # YENİ — TypeScript types for sync IPC
└── tests-rust/                          # YENİ — Rust integration tests
    ├── repository_test.rs
    ├── sync_engine_test.rs
    ├── crdt_test.rs
    └── migration_test.rs
```

> **`tauri-plugin-sql` neden kalkıyor?** Çünkü Postgres bağlantısını Rust kodu içinden `sqlx` ile yönetiyoruz; frontend artık doğrudan SQL çalıştırmaz, `invoke('get_entry')` gibi Tauri komutları üzerinden veri ister. Bu **mimari olarak daha güvenli** (frontend SQL yazamaz) ve sync mantığını backend'de toplar.

---

## 4. POSTGRES ŞEMASI

Bu şema `cornell-diary/src-tauri/postgres_migrations/0001_initial.sql`'a yazılır. **Mevcut SQLite şemasını birebir koruyan** bir tasarımdır — frontend kodu bunu hiç fark etmemeli.

```sql
-- 0001_initial.sql
CREATE EXTENSION IF NOT EXISTS pgcrypto;  -- gen_random_uuid() için

CREATE TABLE diary_entries (
    -- BİREBİR korunan kolonlar (handoff §3.3)
    date            DATE PRIMARY KEY,                 -- ISO YYYY-MM-DD; SQLite'ta TEXT idi
    diary           TEXT NOT NULL DEFAULT '',
    title_1         TEXT, content_1 TEXT,
    title_2         TEXT, content_2 TEXT,
    title_3         TEXT, content_3 TEXT,
    title_4         TEXT, content_4 TEXT,
    title_5         TEXT, content_5 TEXT,
    title_6         TEXT, content_6 TEXT,
    title_7         TEXT, content_7 TEXT,
    summary         TEXT NOT NULL DEFAULT '',
    quote           TEXT NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    device_id       TEXT,                             -- KORUNUR — handoff §3.3 madde 5
    version         INTEGER NOT NULL DEFAULT 1,       -- KORUNUR
    -- YENİ kolonlar (sync için)
    cloud_entry_id  UUID,                             -- Cloud'taki id; null = senkronize değil
    cloud_journal_id UUID,                            -- aktif Cloud journal'a referans
    is_dirty        BOOLEAN NOT NULL DEFAULT TRUE,    -- local'de değişti, push bekliyor
    last_synced_at  TIMESTAMPTZ
);
CREATE INDEX idx_diary_entries_updated ON diary_entries(updated_at DESC);
CREATE INDEX idx_diary_entries_dirty ON diary_entries(is_dirty) WHERE is_dirty = TRUE;
CREATE INDEX idx_diary_entries_cloud_id ON diary_entries(cloud_entry_id);

-- Mevcut sync_log tablosu BİREBİR korunur (handoff §3.3 madde 4)
CREATE TABLE sync_log (
    id              BIGSERIAL PRIMARY KEY,
    sync_type       TEXT NOT NULL CHECK (sync_type IN ('export', 'import')),
    method          TEXT NOT NULL CHECK (method IN ('qr', 'json_file', 'cloud')),  -- 'cloud' eklendi
    device_id       TEXT NOT NULL,
    peer_device_id  TEXT,
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT now(),
    entry_count     INTEGER NOT NULL DEFAULT 0,
    checksum        TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('success', 'partial', 'failed')),
    error_message   TEXT
);
CREATE INDEX idx_sync_log_timestamp ON sync_log(timestamp DESC);

-- Mevcut app_settings BİREBİR korunur
CREATE TABLE app_settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

```sql
-- 0002_sync_metadata.sql
CREATE TABLE sync_metadata (
    id                 INTEGER PRIMARY KEY DEFAULT 1,
    peer_id            TEXT NOT NULL,
    cloud_user_id      UUID,
    cloud_journal_id   UUID,
    access_token       TEXT,
    refresh_token      TEXT,
    token_expires_at   TIMESTAMPTZ,
    last_pull_at       TIMESTAMPTZ,
    last_push_at       TIMESTAMPTZ,
    last_full_sync_at  TIMESTAMPTZ,
    sync_enabled       BOOLEAN NOT NULL DEFAULT FALSE,
    device_label       TEXT,
    CONSTRAINT singleton CHECK (id = 1)
);
INSERT INTO sync_metadata (id, peer_id) VALUES (1, '');  -- migration sonrası set edilir
```

```sql
-- 0003_pending_ops.sql (FAZ 3)
CREATE TABLE pending_ops (
    id           BIGSERIAL PRIMARY KEY,
    entry_date   DATE NOT NULL REFERENCES diary_entries(date) ON DELETE CASCADE,
    field_name   TEXT NOT NULL,
    op_payload   JSONB NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    pushed       BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX idx_pending_ops_unpushed ON pending_ops(pushed) WHERE pushed = FALSE;
```

**Şema kararları (gerekçeli):**

1. **`date DATE PRIMARY KEY`** — SQLite'ta `TEXT` ile saklanıyordu, Postgres'te native `DATE`. sqlx `chrono::NaiveDate` ile bunu doğrudan map'ler. Frontend ISO string aldığında parse hatası olmasın diye TS tarafında `format(date, 'yyyy-MM-dd')` kuralı uygulanır.
2. **`title_1..7` + `content_1..7` ARRAY'a çevrilmedi** — frontend bu kolonlara isimle erişiyor, schema'yı değiştirirsek **frontend değişir**, kısıtla çelişir. Aynı düz kolon yapısı korundu.
3. **`device_id` ve `version` mevcuttu, KORUNDU** (handoff §3.3 madde 5). Migration sırasında SQLite'taki değerler aynen taşınır.
4. **`is_dirty` default TRUE** — migration sonrası tüm satırlar Cloud'a "henüz push edilmedi" sayılır, ilk full sync'te yüklenir.
5. **`sync_log.method` CHECK constraint genişletildi** — `'cloud'` değeri eklendi (mevcut QR/JSON sync de korunur).

---

## 5. FAZ 1 — SQLITE → POSTGRES (TAURI İÇİNDE)

### 5.1 Strateji: Repository Pattern + Feature Flag

Frontend'in tek bir saatlik sync düşmesin diye, geçişi **iki backend paralel** yaparak yapacağız.

```
FAZ 1.0 — Repository trait yazılır, mevcut SQL kodu trait arkasına soyutlanır
FAZ 1.1 — PostgresRepository implementation; SqliteRepository de aktif kalır
FAZ 1.2 — Migration command + dry-run + verify
FAZ 1.3 — Feature flag kapatılır, SqliteRepository silinir, tauri-plugin-sql kalkar
```

### 5.2 Repository trait (Rust)

```rust
// src-tauri/src/db/repository.rs
use async_trait::async_trait;
use chrono::NaiveDate;

#[async_trait]
pub trait EntryRepository: Send + Sync {
    async fn get_by_date(&self, date: NaiveDate) -> Result<Option<DiaryEntry>, DomainError>;
    async fn upsert(&self, entry: DiaryEntryUpsert) -> Result<DiaryEntry, DomainError>;
    async fn list_by_month(&self, year: i32, month: u32) -> Result<Vec<DiaryEntry>, DomainError>;
    async fn list_dirty(&self) -> Result<Vec<DiaryEntry>, DomainError>;
    async fn mark_synced(&self, date: NaiveDate, cloud_id: Uuid) -> Result<(), DomainError>;
    async fn list_all(&self) -> Result<Vec<DiaryEntry>, DomainError>;
}

pub struct PostgresEntryRepository {
    pool: PgPool,
}

pub struct SqliteEntryRepository {
    /* mevcut tauri-plugin-sql IPC üzerinden bridge — geçici, FAZ 1.3'te silinir */
}
```

### 5.3 Tauri komutları (frontend interface'i değişmeden kalır)

Frontend bugün tauri-plugin-sql ile direkt SQL çalıştırıyor (handoff §3.5). Yeni mimaride:

```rust
// src-tauri/src/commands/entries.rs
#[tauri::command]
pub async fn get_entry(
    state: tauri::State<'_, AppState>,
    date: String,
) -> Result<Option<DiaryEntry>, DomainError> {
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")?;
    state.repo.get_by_date(date).await
}

#[tauri::command]
pub async fn upsert_entry(
    state: tauri::State<'_, AppState>,
    entry: DiaryEntryUpsert,
) -> Result<DiaryEntry, DomainError> {
    let saved = state.repo.upsert(entry).await?;
    state.sync.mark_dirty(saved.date).await; // FAZ 2'de scheduler için
    Ok(saved)
}

#[tauri::command]
pub async fn list_entries_by_month(
    state: tauri::State<'_, AppState>,
    year: i32,
    month: u32,
) -> Result<Vec<DiaryEntry>, DomainError> {
    state.repo.list_by_month(year, month).await
}
```

### 5.4 Frontend `db/` katmanı (handoff §3.5)

Mevcut `cornell-diary/src/db/`'deki tauri-plugin-sql wrapper'ları, **invoke** çağrılarına çevrilir:

```typescript
// src/db/entries.ts (BEFORE)
import Database from '@tauri-apps/plugin-sql';
const db = await Database.load('sqlite:cornell_diary.db');
const result = await db.select<DiaryEntry[]>("SELECT * FROM diary_entries WHERE date = ?", [date]);

// src/db/entries.ts (AFTER)
import { invoke } from '@tauri-apps/api/core';
const result = await invoke<DiaryEntry | null>('get_entry', { date });
```

Bu sayede **React component'leri ve hook'lar değişmez**, sadece `db/` modülünün içi değişir. Bu, regression'ı minimize etmek için kasıtlı bir tercihtir.

### 5.5 Migration komutu

```rust
// src-tauri/src/migration/migrate_command.rs
#[tauri::command]
pub async fn migrate_sqlite_to_postgres(
    state: tauri::State<'_, AppState>,
    sqlite_path: String,
    dry_run: bool,
) -> Result<MigrationReport, DomainError> {
    let mut report = MigrationReport::new();
    
    // 1. SQLite'ı read-only aç (rusqlite + URI mode=ro)
    let sqlite = SqliteReader::open(&sqlite_path)?;
    
    // 2. Tüm tabloları oku
    let entries = sqlite.read_all_diary_entries()?;
    let sync_logs = sqlite.read_all_sync_logs()?;
    let settings = sqlite.read_all_settings()?;
    
    report.source_counts = SourceCounts {
        diary_entries: entries.len(),
        sync_logs: sync_logs.len(),
        app_settings: settings.len(),
    };
    
    if dry_run {
        return Ok(report);
    }
    
    // 3. Postgres tarafında transaction içinde yaz
    let mut tx = state.repo.pool().begin().await?;
    
    for e in entries {
        sqlx::query!(
            r#"INSERT INTO diary_entries
               (date, diary, title_1, content_1, title_2, content_2, title_3, content_3,
                title_4, content_4, title_5, content_5, title_6, content_6, title_7, content_7,
                summary, quote, created_at, updated_at, device_id, version, is_dirty)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                       $17, $18, $19, $20, $21, $22, TRUE)"#,
            e.date, e.diary,
            e.title_1, e.content_1, e.title_2, e.content_2, e.title_3, e.content_3,
            e.title_4, e.content_4, e.title_5, e.content_5, e.title_6, e.content_6, e.title_7, e.content_7,
            e.summary, e.quote, e.created_at, e.updated_at, e.device_id, e.version
        ).execute(&mut *tx).await?;
    }
    
    // sync_logs ve app_settings de benzer şekilde
    
    tx.commit().await?;
    
    // 4. Verify — row count + checksum
    report.verify(&state.repo).await?;
    
    Ok(report)
}
```

Frontend bu komutu **bir kerelik onboarding** akışında tetikler:

```typescript
// src/pages/MigrationOnboarding.tsx (geçici, FAZ 1.3'te silinir)
const dryRunReport = await invoke('migrate_sqlite_to_postgres', {
    sqlitePath: '/Users/.../com.deniz.cornelldiary/cornell_diary.db',
    dryRun: true
});
// Kullanıcıya raporu göster, "Devam et" butonuna basarsa
const realReport = await invoke('migrate_sqlite_to_postgres', { sqlitePath, dryRun: false });
```

### 5.6 Regression testleri

`tests-rust/repository_test.rs` ve **frontend integration testler** (vitest) hem SqliteRepository hem PostgresRepository ile aynı davranışı doğrular:

```rust
#[tokio::test]
async fn entries_round_trip_sqlite_and_postgres() {
    let sqlite_repo = SqliteEntryRepository::test_instance().await;
    let pg_repo = PostgresEntryRepository::test_instance().await;
    
    for repo in [&sqlite_repo as &dyn EntryRepository, &pg_repo as &dyn EntryRepository] {
        let entry = DiaryEntryUpsert::new_sample();
        repo.upsert(entry.clone()).await.unwrap();
        let fetched = repo.get_by_date(entry.date).await.unwrap().unwrap();
        assert_eq!(fetched.diary, entry.diary);
        assert_eq!(fetched.title_1, entry.title_1);
        assert_eq!(fetched.summary, entry.summary);
    }
}
```

---

## 6. FAZ 2 — CLOUD SYNC CLIENT (RUST İÇİNDE)

### 6.1 Cloud client (reqwest)

```rust
// src-tauri/src/sync/client.rs
pub struct CloudClient {
    base: Url,
    client: reqwest::Client,
    auth: Arc<AuthManager>,
}

impl CloudClient {
    pub async fn login(&self, email: &str, password: &str) -> Result<TokenPair, DomainError> {
        let resp = self.client.post(self.base.join("/auth/login")?)
            .json(&LoginRequest { email, password })
            .timeout(Duration::from_secs(30))
            .send().await?;
        // 401/4xx mapping
        match resp.status() {
            StatusCode::OK => Ok(resp.json().await?),
            StatusCode::UNAUTHORIZED => Err(DomainError::CloudAuthFailed),
            s => Err(DomainError::CloudHttp(s.as_u16())),
        }
    }
    
    pub async fn pull(&self, journal_id: Uuid, since: Option<DateTime<Utc>>) -> Result<PullResponse, DomainError> {
        let token = self.auth.get_or_refresh().await?;
        let mut req = self.client.get(self.base.join("/sync/pull")?)
            .bearer_auth(&token)
            .query(&[("journal_id", journal_id.to_string())]);
        if let Some(since) = since {
            req = req.query(&[("since", since.to_rfc3339())]);
        }
        let resp = req.timeout(Duration::from_secs(60)).send().await?;
        Ok(resp.json().await?)
    }
    
    pub async fn push(&self, batch: PushRequest) -> Result<PushResponse, DomainError> { /* ... */ }
}
```

### 6.2 Auth manager + JWT refresh

```rust
// src-tauri/src/sync/auth.rs
pub struct AuthManager {
    pool: PgPool,
    refresh_lock: tokio::sync::Mutex<()>,
}

impl AuthManager {
    pub async fn get_or_refresh(&self) -> Result<String, DomainError> {
        let meta = self.read_metadata().await?;
        let token = meta.access_token.ok_or(DomainError::NotConnected)?;
        
        // JWT decode (no verification — server zaten doğrular, sadece exp okuyoruz)
        let exp = jsonwebtoken::decode_header(&token)?.exp; // basitleştirilmiş
        if exp < Utc::now() + Duration::from_secs(60) {
            return self.refresh().await;
        }
        Ok(token)
    }
    
    async fn refresh(&self) -> Result<String, DomainError> {
        let _guard = self.refresh_lock.lock().await;  // çift refresh önlenir
        // /auth/refresh çağır, sonucu sync_metadata'ya yaz
        // ...
    }
}
```

> **JWT'yi nereye saklarım?** İlk versiyonda `sync_metadata` tablosuna düz metin. Postgres data dir OS-level dosya izinleriyle korunuyor. v2 için `tauri-plugin-stronghold` veya OS keychain (`security-framework` macOS, `secret-service` Linux). Bu prompt v1'de basit yolu seçer ama threat model'de açıkça not düşülür.

### 6.3 Sync engine

```rust
// src-tauri/src/sync/engine.rs
pub struct SyncEngine {
    repo: Arc<dyn EntryRepository>,
    client: Arc<CloudClient>,
    auth: Arc<AuthManager>,
    state: Arc<RwLock<SyncState>>,
}

impl SyncEngine {
    pub async fn run_full_cycle(&self) -> Result<SyncReport, DomainError> {
        if !self.network_online().await || !self.auth.is_connected().await {
            return Err(DomainError::OfflineOrDisconnected);
        }
        
        let _lock = self.acquire_lock().await?;  // lock file kontrol — handoff §1
        let meta = self.repo.read_sync_metadata().await?;
        
        // 1. PULL
        let pull = self.client.pull(meta.cloud_journal_id.unwrap(), meta.last_pull_at).await?;
        let mut report = SyncReport::default();
        for cloud_entry in pull.entries {
            self.merge_remote(cloud_entry, &mut report).await?;
        }
        
        // 2. PUSH
        let dirty = self.repo.list_dirty().await?;
        if !dirty.is_empty() {
            let push = self.client.push(PushRequest::from(dirty)).await?;
            for merged in push.entries {
                self.repo.mark_synced(merged.local_date, merged.cloud_id).await?;
                report.pushed += 1;
            }
        }
        
        // 3. metadata update
        self.repo.update_sync_timestamps(Utc::now()).await?;
        
        Ok(report)
    }
    
    async fn merge_remote(&self, cloud: CloudEntry, report: &mut SyncReport) -> Result<(), DomainError> {
        let local = self.repo.get_by_date(cloud.entry_date).await?;
        match local {
            None => {
                self.repo.insert_from_cloud(cloud).await?;
                report.pulled += 1;
            }
            Some(local) if local.version < cloud.version && !local.is_dirty => {
                self.repo.overwrite_from_cloud(cloud).await?;
                report.pulled += 1;
            }
            Some(local) if local.version < cloud.version && local.is_dirty => {
                // ÇAKIŞMA — last_modified_at karşılaştır
                if cloud.last_modified_at > local.updated_at {
                    self.repo.archive_local_then_overwrite(local, cloud).await?;
                    report.conflicts_cloud_won += 1;
                } else {
                    // local kazanır; push aşamasında zaten gönderilecek
                    report.conflicts_local_won += 1;
                }
            }
            Some(_) => {
                // local >= cloud, dokunma
            }
        }
        Ok(())
    }
}
```

### 6.4 Scheduler + network monitor

```rust
// src-tauri/src/sync/scheduler.rs
pub async fn start_hourly_sync(engine: Arc<SyncEngine>) -> Result<JobScheduler, DomainError> {
    let sched = JobScheduler::new().await?;
    
    let engine_clone = engine.clone();
    sched.add(Job::new_async("0 0 * * * *", move |_uuid, _l| {  // saat başı
        let engine = engine_clone.clone();
        Box::pin(async move {
            if let Err(e) = engine.run_full_cycle().await {
                tracing::warn!("hourly sync failed: {e:?}");
            }
        })
    })?).await?;
    
    sched.start().await?;
    Ok(sched)
}

// src-tauri/src/sync/network.rs
pub async fn start_network_monitor(engine: Arc<SyncEngine>) {
    let mut state = NetworkState::Unknown;
    let probe_interval = Duration::from_secs(30);
    
    loop {
        let online = probe_cloud_health().await;
        match (state, online) {
            (NetworkState::Offline, true) | (NetworkState::Unknown, true) => {
                tracing::info!("network came online — triggering sync");
                let _ = engine.run_full_cycle().await;
                state = NetworkState::Online;
            }
            (_, false) => state = NetworkState::Offline,
            _ => {}
        }
        tokio::time::sleep(probe_interval).await;
    }
}

async fn probe_cloud_health() -> bool {
    reqwest::Client::new()
        .get("http://127.0.0.1:5000/health")
        .timeout(Duration::from_secs(5))
        .send().await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
```

### 6.5 Tauri commands (frontend → backend)

```rust
#[tauri::command]
pub async fn connect_cloud(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
    device_label: String,
) -> Result<ConnectReport, DomainError> {
    let tokens = state.cloud.login(&email, &password).await?;
    state.cloud.auth.persist(tokens, device_label).await?;
    Ok(ConnectReport { /* ... */ })
}

#[tauri::command]
pub async fn trigger_sync(state: tauri::State<'_, AppState>) -> Result<SyncReport, DomainError> {
    state.sync.run_full_cycle().await
}

#[tauri::command]
pub async fn get_sync_status(state: tauri::State<'_, AppState>) -> Result<SyncStatus, DomainError> {
    Ok(SyncStatus {
        enabled: state.sync.is_enabled().await,
        online: state.network.is_online(),
        last_pull_at: state.sync.last_pull().await,
        last_push_at: state.sync.last_push().await,
        dirty_count: state.repo.count_dirty().await?,
    })
}

#[tauri::command]
pub async fn disconnect_cloud(state: tauri::State<'_, AppState>) -> Result<(), DomainError> {
    state.cloud.auth.clear().await
}
```

### 6.6 Frontend sync UI (yeni, minimal)

```typescript
// src/components/SyncIndicator.tsx
const status = useSyncStatus(); // 5 sn'de bir poll
const color = !status.enabled ? 'gray'
            : !status.online   ? 'red'
            : status.dirtyCount > 0 ? 'yellow'
            : 'green';
return <Chip color={color} label={statusLabel(status)} />;

// src/pages/SyncSettings.tsx
// Email + password form, "Bağlan" butonu invoke('connect_cloud') çağırır
// Status, last sync at, dirty count, manual trigger butonu
```

Mevcut UI'ya **dokunulmaz**, yalnızca yeni component'ler eklenir (sağ üst chip + ayrı sayfa).

---

## 7. FAZ 3 — CRDT + WEBSOCKET LIVE (RUST + REACT)

### 7.1 Strateji: Lazy aktivasyon

CRDT motoru sadece **gerçekten birden çok kullanıcı bağlıyken** devreye girer. Tek kullanıcı → REST yolu. Bunun için presence sinyali kullanılır.

```
Kullanıcı entry açar → Tauri WS bağlantısı kurar → "subscribe" mesajı yollar
Server "presence_update": [bu peer]  → tek kullanıcı, CRDT uyur
Başka kullanıcı bağlanır → "presence_update": [peer1, peer2]  → CRDT uyanır
İkisi yazar → keystroke'lar CharOp olarak WS'e gider
Snapshot 30 sn'de bir Cloud'da materialize → "snapshot_updated" event
```

### 7.2 CharNode + Document (Rust)

Verdiğin orijinal Python doküman temelini koruyarak, Rust'ın ownership semantiği ile uyumlu hale getiriyoruz:

```rust
// src-tauri/src/crdt/node.rs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CharNode {
    pub char_id: String,        // "{peer_id}-{lamport}-{seq}"
    pub character: char,
    pub peer_id: String,
    pub lamport: u64,
    pub seq: u32,
    pub prev_id: Option<String>,
    pub next_id: Option<String>,
    pub is_deleted: bool,
}

// src-tauri/src/crdt/document.rs
pub struct CrdtDocument {
    pub entry_date: NaiveDate,
    pub field: String,
    pub local_peer: String,
    lamport: AtomicU64,
    nodes: DashMap<String, CharNode>,   // O(1) lookup, concurrent
    head_id: String,
    tail_id: String,
    pending: Mutex<Vec<CharOp>>,        // prev_id bilinmeyen op'lar
}

impl CrdtDocument {
    pub fn local_insert(&self, character: char, prev_id: Option<&str>, next_id: Option<&str>) -> CharOp {
        let lamport = self.lamport.fetch_add(1, Ordering::SeqCst) + 1;
        let seq = self.next_seq_for(lamport);
        let char_id = format!("{}-{}-{}", self.local_peer, lamport, seq);
        let node = CharNode {
            char_id: char_id.clone(),
            character,
            peer_id: self.local_peer.clone(),
            lamport,
            seq,
            prev_id: prev_id.map(String::from),
            next_id: next_id.map(String::from),
            is_deleted: false,
        };
        self.link_neighbors(&node);
        self.nodes.insert(char_id.clone(), node.clone());
        CharOp::Insert(node)
    }
    
    pub fn apply_remote(&self, op: CharOp) -> Result<(), CrdtError> {
        match op {
            CharOp::Insert(node) => {
                self.lamport.fetch_max(node.lamport, Ordering::SeqCst);
                
                // İdempotent
                if self.nodes.contains_key(&node.char_id) {
                    return Ok(());
                }
                
                // prev_id bilinmiyorsa pending
                if let Some(ref prev) = node.prev_id {
                    if !self.nodes.contains_key(prev) {
                        self.pending.lock().push(CharOp::Insert(node));
                        return Ok(());
                    }
                }
                
                // Çakışma kontrolü
                let resolved_node = self.resolve_conflict(node);
                self.link_neighbors(&resolved_node);
                self.nodes.insert(resolved_node.char_id.clone(), resolved_node);
                
                self.flush_pending()?;
            }
            CharOp::Delete { char_id, peer_id, lamport } => {
                self.lamport.fetch_max(lamport, Ordering::SeqCst);
                if let Some(mut node) = self.nodes.get_mut(&char_id) {
                    node.is_deleted = true;
                }
                // Tombstone — düğüm bellekte kalır (handoff §0)
            }
        }
        Ok(())
    }
    
    fn resolve_conflict(&self, mut incoming: CharNode) -> CharNode {
        // Aynı (prev_id) içine başka bir node zaten varsa peer_id alfabetik karşılaştır
        if let Some(ref prev) = incoming.prev_id {
            if let Some(prev_node) = self.nodes.get(prev) {
                if let Some(ref existing_next) = prev_node.next_id {
                    if let Some(existing) = self.nodes.get(existing_next) {
                        if existing.char_id != incoming.char_id {
                            // Çakışma — alfabetik daha büyük peer_id sağa
                            if incoming.peer_id > existing.peer_id {
                                incoming.prev_id = Some(existing.char_id.clone());
                                incoming.next_id = existing.next_id.clone();
                            }
                            // değilse mevcut node sağda kalır, incoming ortaya girer
                        }
                    }
                }
            }
        }
        incoming
    }
    
    pub fn materialize(&self) -> String {
        let mut text = String::new();
        let mut current = Some(self.head_id.clone());
        while let Some(id) = current {
            if let Some(node) = self.nodes.get(&id) {
                if !node.is_deleted && id != self.head_id && id != self.tail_id {
                    text.push(node.character);
                }
                current = node.next_id.clone();
            } else {
                break;
            }
        }
        text
    }
}
```

### 7.3 WS client

```rust
// src-tauri/src/crdt/ws_client.rs
pub struct WsClient {
    cloud_ws_url: Url,
    auth: Arc<AuthManager>,
    documents: Arc<DashMap<(NaiveDate, String), Arc<CrdtDocument>>>,
    app_handle: tauri::AppHandle,  // event emit için
}

impl WsClient {
    pub async fn subscribe(&self, journal_id: Uuid, entry_date: NaiveDate) -> Result<(), DomainError> {
        let token = self.auth.get_or_refresh().await?;
        let url = format!("{}/ws/journal/{}?token={}", self.cloud_ws_url, journal_id, token);
        let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();
        
        write.send(Message::text(serde_json::to_string(&WsOut::Subscribe { entry_date })?)).await?;
        
        let app = self.app_handle.clone();
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(incoming) = serde_json::from_str::<WsIn>(&text) {
                            handle_incoming(incoming, &app).await;
                        }
                    }
                    Err(e) => { tracing::warn!("ws error: {e:?}"); break; }
                    _ => {}
                }
            }
        });
        
        Ok(())
    }
    
    pub async fn send_op(&self, entry_date: NaiveDate, field: &str, op: CharOp) -> Result<(), DomainError> {
        // Aktif WS bağlantısı yoksa pending_ops'a yaz
        if !self.is_connected(entry_date).await {
            self.repo.queue_pending_op(entry_date, field, &op).await?;
            return Ok(());
        }
        // Bağlıysa direkt yolla
        // ...
    }
}

async fn handle_incoming(msg: WsIn, app: &tauri::AppHandle) {
    match msg {
        WsIn::CrdtOpBroadcast { entry_date, field, op, .. } => {
            // Document'a uygula
            let doc = /* lookup */;
            doc.apply_remote(op).unwrap();
            let new_text = doc.materialize();
            // Frontend'e event yolla (React state'i güncelleyecek)
            app.emit("crdt:text-updated", json!({
                "entry_date": entry_date,
                "field": field,
                "text": new_text,
            })).unwrap();
        }
        WsIn::PresenceUpdate { peers } => {
            app.emit("crdt:presence", json!({ "peers": peers })).unwrap();
        }
        WsIn::SnapshotUpdated { entry_date, version } => {
            // Local cache invalidation
        }
    }
}
```

### 7.4 React frontend integration

```typescript
// src/hooks/useCrdtChannel.ts
export function useCrdtChannel(entryDate: string, fieldName: string) {
    const [crdtMode, setCrdtMode] = useState(false);
    const [activePeers, setActivePeers] = useState<string[]>([]);
    
    useEffect(() => {
        const unlisten1 = listen<CrdtTextUpdate>('crdt:text-updated', (event) => {
            if (event.payload.entry_date === entryDate && event.payload.field === fieldName) {
                applyRemoteText(event.payload.text);
            }
        });
        const unlisten2 = listen<PresenceUpdate>('crdt:presence', (event) => {
            setActivePeers(event.payload.peers);
            setCrdtMode(event.payload.peers.length > 1);
        });
        invoke('subscribe_crdt', { entryDate, fieldName });
        return () => {
            unlisten1.then(fn => fn());
            unlisten2.then(fn => fn());
        };
    }, [entryDate, fieldName]);
    
    const sendLocalOp = useCallback(async (op: CharOp) => {
        await invoke('apply_local_op', { entryDate, fieldName, op });
    }, [entryDate, fieldName]);
    
    return { crdtMode, activePeers, sendLocalOp };
}
```

```typescript
// src/hooks/useDebouncedSave.ts (MEVCUT, GENİŞLETİLİR)
export function useDebouncedSave(/*...*/) {
    const { crdtMode, sendLocalOp } = useCrdtChannel(/*...*/);
    
    const onChange = useCallback((event: ChangeEvent<HTMLTextAreaElement>) => {
        if (crdtMode) {
            // Char-by-char op generate et
            const op = diffToOp(event);
            sendLocalOp(op);
        } else {
            debouncedFullSave(event.target.value);  // mevcut davranış
        }
    }, [crdtMode]);
    
    return { onChange };
}
```

> **Cursor preservation:** React controlled input'larda cursor pozisyonu re-render sırasında kayar. `useLayoutEffect` ile remote text uygulandıktan sonra cursor pozisyonu manuel restore edilir. Detay implementasyon kısmında.

### 7.5 Offline-first CRDT

WS bağlı değilken yapılan keystroke'lar `pending_ops` tablosuna yazılır. WS reconnect olunca:

```rust
async fn flush_pending_ops(&self) -> Result<(), DomainError> {
    let pending = self.repo.list_pending_ops().await?;
    for p in pending {
        self.send_op(p.entry_date, &p.field_name, p.op).await?;
        self.repo.mark_op_pushed(p.id).await?;
    }
    Ok(())
}
```

İdempotency: aynı `char_id` Cloud'a iki kez gelirse no-op (Cloud server bunu zaten garanti eder, Cloud prompt §5.3).

---

## 8. CRDT MIRROR DOĞRULAMA (CLOUD ↔ DIARY)

Cloud Python'da, Diary Rust'ta CRDT yazıyoruz. **İki implementation aynı op sequence'ını aynı text'e materialize etmeli.** Bunu doğrulamak için:

`tests-rust/crdt_test.rs::test_cross_implementation_parity`:

1. JSON fixture: 200 random op (insert/delete, 4 farklı peer)
2. Rust implementation'da uygula → text A
3. Cloud HTTP endpoint'e fixture yolla (`POST /test/apply_ops` — Cloud tarafında dev-only test endpoint), text B döndür
4. `assert_eq!(text_a, text_b)`

Bu test her PR'da çalışır. Implementations'lardan biri çakışma çözümünü farklı yaparsa hemen yakalanır.

> Cloud tarafına bu test endpoint'i eklenmesi gerek (dev-only, prod'da disable). Bu prompt'un Aşama 3.4'ünde Cloud repo'suna küçük bir PR açılır.

---

## 9. GÜVENLİK PROTOKOLÜ (13 KURAL — PROJEYE ÖZEL)

1. **Hardcoded secret yok.** Tüm Cloud config (`CLOUD_URL`, `CLOUD_WS_URL`, `JWT`'ler) `app_settings` veya `sync_metadata` tablosunda. `.env` Rust tarafında **gerekmez** — kullanıcı UI'dan girer.
2. **Postgres connection string `.env`'de** veya OS keychain'de. Tauri build'inde environment'tan okur, runtime'da değiştirilebilir.
3. **Input validation.** Tüm Tauri komutları struct serde ile validate edilir. Frontend güvenilmez — backend her invoke parametresini check eder.
4. **SQL injection.** `sqlx::query!` macro ile compile-time parameterized query. Raw string SQL **yok**.
5. **Auth kontrolü.** Tüm sync komutları (`trigger_sync`, `apply_local_op`, vs.) ilk satırda `auth.is_connected()` kontrol eder.
6. **JWT saklama.** `sync_metadata` tablosu Postgres'te. Postgres data dir OS user-only readable (`chmod 700`). v2: `tauri-plugin-stronghold` veya OS keychain — threat model'de açıkça not.
7. **Refresh token rotation.** Her refresh sonrası eski token Cloud tarafında invalidate edilir; Diary sadece yeniyi kullanır.
8. **HTTPS production'da zorunlu.** Development localhost http izinli. URL scheme check `url::Url::scheme()`.
9. **WS validation.** Gelen mesajlar serde ile parse edilir, schema mismatch → drop + warn log.
10. **Logging — PII yok.** `tracing` ile structured log; entry içeriği, password, token ASLA loglanmaz. Sadece `entry_date`, op_type, peer_id, lamport, status.
11. **CRDT op poisoning.** Cloud server JWT'den peer_id türetir; Diary fake peer_id koyamaz (Cloud reddeder). Lokal tarafta da self-validation: incoming op'un `peer_id` Cloud'tan gelen presence listesinde olmalı.
12. **Rate limiting.** Frontend keystroke flooding'ini önlemek için Rust tarafında token bucket: max 200 op/sn per session. Üstü ise 100 ms delay.
13. **Threat model:** `docs/THREAT_MODEL.md`:
    - Stolen device: Postgres data + sync_metadata erişilebilir → v2'de OS keychain
    - Cloud unreachable: graceful degradation, dirty flag korunur, kullanıcıya UI'da gösterilir
    - Race condition (paralel sync trigger): app-level lock (Tokio Mutex) + DB-level unique advisory lock
    - Slopsquatting: tüm Cargo crates exact version, `cargo audit` CI'da

**Not:** Bu prompt JWT signature **doğrulamaz** — sadece exp okur (refresh kararı için). Cloud server signature'ı zaten her endpoint'te doğrular. Bu kasıtlı bir tasarımdır (single source of truth).

---

## 10. GELİŞTİRME DİSİPLİNİ + OTONOM ÇALIŞMA MODU

### Branch'ler
- `main` her zaman çalışır
- `feature/postgres-repository` (FAZ 1.0–1.1)
- `feature/sqlite-migration-command` (FAZ 1.2)
- `feature/sqlite-removal` (FAZ 1.3)
- `feature/cloud-rest-client` (FAZ 2.1)
- `feature/sync-engine-scheduler` (FAZ 2.2)
- `feature/sync-frontend-ui` (FAZ 2.3)
- `feature/crdt-engine` (FAZ 3.1)
- `feature/ws-client` (FAZ 3.2)
- `feature/crdt-frontend-integration` (FAZ 3.3)
- `feature/sidecar-postgres-update` (Bölüm 11.5 — ayrı PR, journal-ai-reporter repo'su)

### Otonom mod
**🛑 TEST DURAĞI**'larında dur. Aralarda branch açma, commit, merge, debug, refactor, paket ekleme, test düzeltme, linter (`cargo clippy --all-targets`, `cargo fmt`, frontend `npm run typecheck`), mock data — sormadan otonom. Her aşama 6-10 satır bitiş raporu, sıradakine sormadan geç.

### Geri alınamaz işlemler — kısa özet sonrası devam
- SQLite silme (FAZ 1.3 son adım) — verify_migration %100 olmadan dokunma
- Sidecar repo'da PR açma (Bölüm 11.5)
- GitHub push (final)

### Belirsizlikte
En savunmacı seçenek seç, log'a düş, devam et.

### Mevcut kod regression guard
Frontend integration testleri (`vitest`) **her aşamada** geçmeli. Mevcut entry editor, date navigation, QR sync — hepsi davranışsal olarak korunur. Bu testler kırılırsa **özellik bozuldu** demektir, geri al + düzelt.

---

## 11. ÇALIŞMA AKIŞI — FAZ FAZ

### Aşama 0: Hazırlık (otonom)
1. Bölüm 1'deki ön şart kontrollerini yap, eksiksiz mi
2. SQLite yedeği al (verify edilmiş)
3. `cornell-diary/src-tauri/Cargo.toml`'a yeni Rust paketleri ekle, `cargo build` geçsin
4. `postgres_migrations/` klasörü oluştur
5. Local Postgres'e bağlan: `psql -p 5432 -d postgres -c "CREATE DATABASE diary_db;"`
6. `cargo sqlx prepare` veya runtime migration setup
7. Mevcut Tauri app `cargo tauri dev` ile açılıyor mu, hâlâ SQLite ile çalışıyor mu — baseline doğrula
8. Mevcut frontend testlerini çalıştır, hepsi geçiyor mu — baseline
9. Bitiş raporu, FAZ 1.0'a geç

### FAZ 1.0 — Repository trait soyutlaması (otonom)
1. `feature/postgres-repository` branch
2. `EntryRepository` trait yaz
3. `SqliteEntryRepository` impl: mevcut tauri-plugin-sql çağrılarını sarar (geçici bridge)
4. Tauri command'lar yazılır (`get_entry`, `upsert_entry`, vs.) — şu anlık SqliteRepository kullanır
5. Frontend `src/db/` modüllerini `invoke()` çağrılarına çevir
6. Mevcut tüm frontend testleri hâlâ geçmeli (regression guard)
7. Manuel doğrulama: `cargo tauri dev` aç, bir entry yaz, kaydet, yeniden aç — veri görünüyor mu
8. Merge, bitiş raporu

### FAZ 1.1 — Postgres impl (otonom)
1. Aynı branch devam (veya yeni `feature/postgres-impl`)
2. SQL migration dosyaları (Bölüm 4) yaz, `sqlx::migrate!()` runtime'da çalıştır
3. `PostgresEntryRepository` impl yaz (her trait method'u sqlx query macro ile)
4. `AppState` yapısı: Cargo feature flag `cfg(feature = "postgres")` ile hangi repo aktif olacak seçilir
5. Default feature'a `postgres` ekle
6. Mevcut frontend testleri hâlâ geçmeli (artık Postgres'e karşı)
7. Cargo testler: `repository_test.rs` her iki backend'i ile geçmeli
8. Manuel: `cargo tauri dev` ile Postgres backend açılıyor mu, entry CRUD çalışıyor mu
9. Merge

### FAZ 1.2 — Migration command (otonom + 🛑 TEST DURAĞI)
1. `feature/sqlite-migration-command` branch
2. `migration/sqlite_reader.rs` (`rusqlite` dev-dependency olarak ekle)
3. `migration/migrate_command.rs` (Bölüm 5.5)
4. `MigrationOnboarding.tsx` geçici sayfa
5. Test fixture SQLite ile dry-run + real-run testi
6. Merge

**🛑 TEST DURAĞI 1 — Gerçek migration**
   - Yedek aldığını doğrulamamı söyle
   - `cargo tauri dev` ile uygulamayı aç, MigrationOnboarding sayfasına git
   - Önce `Dry Run` butonu — raporu kontrol etmemi iste (kaç entry, kaç sync_log, kaç setting)
   - Onaylarsam `Migrate` butonu — gerçek migration
   - Verify raporu (row count + checksum) %100 eşleşmiyorsa **devam etme**, debug et
   - Manual doğrulama: bir entry'e gir, içerik orijinal SQLite'taki gibi mi
   - Onayımı al

### FAZ 1.3 — SQLite kaldırma (otonom)
1. `feature/sqlite-removal` branch
2. `tauri-plugin-sql` paketini Cargo.toml'dan çıkar, `tauri.conf.json` plugin listesinden de
3. `SqliteEntryRepository` ve onunla ilgili kod silinir
4. Cargo feature flag kaldırılır, sadece Postgres
5. `migrations/` (eski SQLite) klasörü silinir
6. `MigrationOnboarding.tsx` arşivlenir (`docs/legacy/`'e taşınır, kod tabanından çıkar)
7. Frontend `db/` modüllerinde SQLite'a referans kalmamış olmalı
8. Mevcut frontend test suite'i hâlâ %100
9. `cargo clippy --all-targets -- -D warnings` temiz
10. Merge, bitiş raporu, FAZ 2'ye geç

### FAZ 2.1 — Cloud REST Client (otonom)
1. `feature/cloud-rest-client` branch
2. `sync/auth.rs` (AuthManager + JWT exp parse + refresh)
3. `sync/client.rs` (CloudClient — login, refresh, pull, push, register)
4. `sync/conflict.rs` (version compare logic)
5. `sync/engine.rs` (run_full_cycle iskeleti, pull+push)
6. Tauri commands: `connect_cloud`, `disconnect_cloud`, `trigger_sync`, `get_sync_status`
7. `mockito` ile unit test (mock Cloud)
8. Merge

### FAZ 2.2 — Scheduler + Network Monitor (otonom)
1. `feature/sync-engine-scheduler` branch
2. `sync/scheduler.rs` (tokio-cron-scheduler)
3. `sync/network.rs` (probe loop)
4. `lib.rs` `setup` hook'unda her ikisini başlat
5. Tek-instance lock (Tokio Mutex + advisory lock Postgres `pg_try_advisory_lock`)
6. Integration test (fake clock)
7. Merge

### FAZ 2.3 — Frontend Sync UI (otonom + 🛑 TEST DURAĞI)
1. `feature/sync-frontend-ui` branch
2. `SyncIndicator.tsx`, `SyncSettings.tsx`, `useSyncStatus.ts`
3. App router'a sync settings page ekle
4. SyncIndicator app shell sağ üst köşesine
5. Mevcut UI değişmedi mi kontrol et (regression)
6. vitest testleri yaz (3-5 yeni test)
7. Merge

**🛑 TEST DURAĞI 2 — Sync uçtan uca**
   - Cloud server ayakta mı kontrol etmemi söyle
   - Diary'i `cargo tauri dev` ile aç
   - Sync Settings → Cloud'a Bağlan (email/password, dev için Cloud'da test user oluşturmamı söyle)
   - Bir entry yaz, manuel sync trigger
   - `psql -p 5433 -d cloud_db -c "SELECT * FROM entries;"` ile Cloud'da görünüyor mu kontrol etmemi söyle
   - Cloud'da entry'i elle değiştir, Diary'de manuel sync, local güncellendi mi
   - Çakışma testi: hem local hem cloud aynı entry'i değiştir, sync sonrası last-write-wins doğru çalışıyor mu, archive backup oluştu mu
   - Onayımı al

### FAZ 3.1 — CRDT Engine (otonom)
1. `feature/crdt-engine` branch
2. `crdt/node.rs`, `crdt/document.rs`, `crdt/operations.rs`, `crdt/conflict_resolver.rs`, `crdt/snapshot.rs`
3. Birim testler:
   - Tek peer insert/delete sıralı uygulanıyor mu
   - İki peer aynı pozisyonda insert → tie-breaker
   - 1000 random op → final state aynı mı (commutativity)
   - Aynı op iki kez → idempotent mi
   - Pending queue: prev_id bilinmeyen op gelir, daha sonra parent gelir, doğru sıraya oturur mu
4. Cross-implementation parity test (Bölüm 8) — Cloud test endpoint'i lazım
5. Merge

### FAZ 3.2 — WS Client (otonom)
1. `feature/ws-client` branch
2. `crdt/ws_client.rs` (tokio-tungstenite, reconnect with exponential backoff)
3. Tauri command: `subscribe_crdt`, `apply_local_op`, `unsubscribe_crdt`
4. Tauri event emit: `crdt:text-updated`, `crdt:presence`, `crdt:snapshot-updated`
5. `pending_ops` tablo entegrasyonu
6. Integration test (mock WS server)
7. Merge

### FAZ 3.3 — Frontend CRDT (otonom + 🛑 TEST DURAĞI)
1. `feature/crdt-frontend-integration` branch
2. `useCrdtChannel.ts`, `PresenceBadge.tsx`
3. `useDebouncedSave.ts` extend: CRDT mode aktifse char-by-char op gönder
4. Cursor preservation: `useLayoutEffect` ile remote update sonrası restore
5. Frontend test: mock invoke + event emit
6. Merge

**🛑 TEST DURAĞI 3 — Çoklu kullanıcı CRDT**
   - İki Diary instance gerekir (iki tarayıcı yetmez — Tauri native app). Geçici çözüm: Tauri'nin dev mode iki paralel açılışı için `--config dev_user_2` profile, veya iki ayrı bilgisayar
   - Setup talimatlarını ver (iki instance açma, iki ayrı user ile login)
   - Aynı entry'e gir, presence göstergesi "2 peer" mi
   - İki kullanıcı aynı paragrafa paralel yaz → her iki ekranda final text aynı mı
   - Birinin ağını kes (Cloud /health'i mock fail), yaz, geri aç → pending_ops flush edip senkron oluyor mu
   - Onayımı al

### Final (otonom + 🛑 TEST DURAĞI)
1. End-to-end stress test (10 dakika, random ops, 2 client)
2. README, MIGRATION.md, SYNC_BEHAVIOR.md, ROLLBACK.md, THREAT_MODEL.md güncellemeleri
3. OWASP API Top 10 checklist (Diary boyutu için adapte)
4. `cargo audit` temiz
5. `cargo clippy --all-targets -- -D warnings` temiz
6. Frontend: `npm run typecheck` + `npm run test` + `npm run lint` temiz
7. Merge

**🛑 TEST DURAĞI 4 — Final onay (geri alınamaz)**
   - Tüm test suite'i (Rust + frontend) çalıştırmamı söyle
   - GitHub push edileceğini bildir, branch stratejisi onay (`main` push)
   - Onay alınca push

### Bölüm 11.5 — Sidecar Postgres update (ayrı PR, journal-ai-reporter repo'su)

Diary Postgres'e geçtikten sonra `cornell_journal_api/` (Reporter sidecar) eski SQLite'a bakıyor → kırılır. Çözüm:

1. journal-ai-reporter repo'sunda `feature/sidecar-postgres-source` branch
2. `cornell_journal_api/src/db.py`'yi sqlite3 yerine `asyncpg` kullanacak şekilde rewrite et
3. Aynı `RawEntry` projection'ı koru (handoff §4.2.4 schema mapping):
   - `cue_column` ← concat of `title_i: content_i` non-empty pairs
   - `notes_column` ← `diary`
   - `summary` ← `summary`
   - `planlar` ← `quote`
4. Reporter ekosistemi testleri %100 geçmeli
5. Bu PR Diary FAZ 1.3 merge'inden **sonra** açılır (önce SQLite kalksın, sonra sidecar geçsin)

> **Bu adım ayrı bir PR'dır**, Diary prompt'unun ana akışına dahil değildir, ama Aşama 1.3 sonunda sormadan başla — Reporter çalışmıyorsa Jarvis de çalışmıyor demektir.

---

## 12. .ENV.EXAMPLE (Cargo build / runtime)

```env
# Local Postgres
DATABASE_URL=postgres://diary_user:change_me@127.0.0.1:5432/diary_db

# Cloud (default; runtime'da app_settings'e overload edilebilir)
CLOUD_URL=http://127.0.0.1:5000
CLOUD_WS_URL=ws://127.0.0.1:5000

# Sync
SYNC_INTERVAL_HOURS=1
NETWORK_PROBE_INTERVAL_SECONDS=30
DEVICE_LABEL_DEFAULT=Deniz-Macbook

# Logging
RUST_LOG=cornell_diary=info,sqlx=warn

# Migration (geçici, FAZ 1.3'te kalkar)
SQLITE_LEGACY_PATH=/Users/<user>/Library/Application Support/com.deniz.cornelldiary/cornell_diary.db
```

---

## 13. ROLLBACK PLANI (`docs/ROLLBACK.md`)

**FAZ 1'den geri dönüş:**
- `cargo` feature flag `sqlite` aktif edilir, `postgres` deaktive (FAZ 1.3'ten önce)
- SQLite yedek dosyası geri yüklenir
- FAZ 1.3 sonrası geri dönüş: SqliteEntryRepository kodu `git revert` ile çağrılır

**FAZ 2'den geri dönüş:**
- `sync_metadata.sync_enabled = FALSE`
- Scheduler ve network monitor durur
- Diary local Postgres ile tek başına çalışmaya devam eder

**FAZ 3'ten geri dönüş:**
- WS bağlantısı kapatılır, `crdtMode` her zaman `false`
- Mevcut debounced autosave davranışı tek aktif yol
- REST sync FAZ 2 modunda devam eder

**Reporter sidecar geri dönüş:**
- Yedek SQLite dosyası varsa, sidecar'ın `db.py` revert edilir, dosya path eski yere konur

---

## 14. KENDİNE KONTROL SORULARI (HER AŞAMA SONUNDA)

- [ ] Mevcut frontend test suite'i %100 geçti mi (regression)?
- [ ] Tauri app `cargo tauri dev` ile başlıyor mu, ana akışlar (entry yaz, oku, gez) çalışıyor mu?
- [ ] `cargo clippy --all-targets -- -D warnings` temiz mi?
- [ ] `cargo fmt --check` temiz mi?
- [ ] `npm run typecheck` temiz mi?
- [ ] Yeni Cargo crate eklediysem PyPI/crates.io'da gerçek mi (slopsquatting)?
- [ ] Hardcoded secret yok mu? (`grep -rE "(secret|password|token)" src-tauri/src/`)
- [ ] Yeni Tauri command input validation yapıyor mu?
- [ ] Stack trace response'a sızıyor mu? (DomainError mapping check)
- [ ] Log'da PII / token / entry içeriği yok mu?
- [ ] Conventional commit mesajları?
- [ ] `main` branch hâlâ çalışır durumda mı?

---

## 15. CLOUD ↔ DIARY İLETİŞİM ÖZETİ (REFERANS)

### REST endpoint'ler (Cloud sağlar; Diary tüketir)

```
POST   /auth/register           {username, email, password} → {user, peer_id, tokens}
POST   /auth/login              {email, password} → {access, refresh}
POST   /auth/refresh            {refresh} → {access, refresh}
GET    /journals                → list
POST   /journals                → create
GET    /sync/pull?journal_id=X&since=ts  → {entries: [...], cursor}
POST   /sync/push   {entries: [...]}     → {merged: [...], rejected: [...]}
GET    /health                  → {status, version}
```

### WebSocket (FAZ 3)

```
ws://cloud:5000/ws/journal/{journal_id}?token=JWT

→ {"type": "subscribe", "entry_date": "2026-04-29"}
→ {"type": "crdt_op", "entry_date": "...", "field": "diary", "op": {...}}
→ {"type": "presence"}

← {"type": "crdt_op_broadcast", "entry_date": "...", "field": "...", "op": {...}, "from_peer": "..."}
← {"type": "presence_update", "peers": ["alice@laptop", "bob@phone"]}
← {"type": "snapshot_updated", "entry_date": "...", "version": 42}
← {"type": "error", "code": "...", "message": "..."}
```

### Auth flow

```
1. İlk kullanım:
   Frontend SyncSettings.tsx → invoke('connect_cloud', {email, password, deviceLabel})
   → Rust: cloud.login() → tokens → sync_metadata'ya yaz
   
2. Sonraki çalışmalar:
   AuthManager.get_or_refresh() her sync veya WS bağlantısında çağrılır
   exp < now+60s ise /auth/refresh otomatik

3. Diary her HTTP request'inde:
   Authorization: Bearer {access_token}
```

### Senaryolar

**A. Tek kullanıcı, çevrimiçi:**
- Diary local Postgres'e yazar, dirty=true
- Saatlik veya manuel: pull → push → mark synced

**B. Tek kullanıcı, çevrimdışıydı, online oldu:**
- network_monitor probe başarılı → run_full_cycle
- Tüm dirty entry'ler push edilir

**C. Çoklu kullanıcı, çevrimiçi, aynı entry açık:**
- WS bağlanır, "subscribe" yollanır
- presence_update peer count > 1 → React state crdtMode = true
- Keystroke'lar CharOp → invoke('apply_local_op') → Rust → WS broadcast
- Server snapshot 30 sn'de bir materialize, "snapshot_updated" event

**D. Çakışma (FAZ 2 davranışı):**
- Local v5, cloud v7, ikisi farklı içerik
- conflict_handler last_modified_at karşılaştırır
- Yeni olan kazanır, eski versiyon JSON olarak `sync_log.error_message`'a (veya ayrı tabloya) arşivlenir

---

## 16. BAŞLA

Şu an Aşama 0'dasın. Otonom çalış, sadece **🛑 TEST DURAĞI** noktalarında dur (FAZ 1.2, 2.3, 3.3, Final).

İlk işin (Aşama 0):
1. Bölüm 1 ön şart kontrolleri — eksik varsa dur, yoksa devam
2. SQLite yedeği al
3. `Cargo.toml` yeni Rust paketleri ekle, `cargo build` geçsin
4. `postgres_migrations/` klasörü
5. Local Postgres `diary_db` create
6. Mevcut Tauri app `cargo tauri dev` ile baseline doğrula (hâlâ SQLite ile)
7. Mevcut frontend testleri çalıştır, baseline
8. Bitiş raporu (6-10 satır), durmadan FAZ 1.0'a geç

**Hatırlatma:** Otonom mod aktif. Sadece dört test durağında durulur (FAZ 1.2, 2.3, 3.3, Final). Geri kalan her şey otonom — sıradaki faz, branch, commit, merge, debug, test düzeltme, paket ekleme, refactor.
