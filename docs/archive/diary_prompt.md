# Diary Cornell — PostgreSQL Migration + Cloud Sync Integration (Claude Code Master Prompt)

> **Proje:** Mevcut Diary Cornell uygulamasını (FastAPI + SQLite + HTML/CSS/JS, iki kolonlu Cornell layout, debounced autosave) **hiçbir özelliği bozmadan** PostgreSQL'e geçirmek + Cloud server (`~/Project/Cloud/`) ile saatlik / online-trigger senkronizasyon eklemek + çoklu kullanıcı CRDT desteği bağlamak.
>
> **Hedef Kitle:** Bu prompt Claude Code (terminal-based agentic IDE) içinde çalıştırılır. Otonom Çalışma Modu aktiftir — sadece **🛑 TEST DURAĞI** noktalarında durulur.

---

## 0. BIG PICTURE (FEYNMAN)

Diary üç fazda dönüştürülecek:

```
FAZ 1: SQLite → PostgreSQL              (veri katmanı değişir, UI dokunulmaz)
FAZ 2: Sync Client                       (Cloud'a bağlan, saatlik kontrol et)
FAZ 3: WebSocket + CRDT                  (canlı çoklu kullanıcı)
```

**Kritik kısıt:** Hiçbir mevcut özellik bozulmayacak. Cornell layout, debounced autosave, date navigation, monthly tables (`entries_YYYY_MM` mantığı) — hepsi çalışmaya devam edecek. Sadece **arkasındaki motor** değişecek.

```
                     ┌─────────────────────────────────────┐
                     │         DIARY CORNELL APP           │
                     │   (FastAPI + Vanilla JS frontend)   │
                     └──────────────┬──────────────────────┘
                                    │
                ┌───────────────────┼───────────────────┐
                │                   │                   │
        ┌───────▼─────┐    ┌────────▼────────┐  ┌──────▼──────┐
        │ Local Cache │    │  Sync Client    │  │  WS Client  │
        │ (Postgres)  │    │  (REST/scheduler)│  │  (live ops) │
        └─────────────┘    └────────┬────────┘  └──────┬──────┘
                                    │                   │
                                    └─────────┬─────────┘
                                              ▼
                                    ┌──────────────────┐
                                    │   CLOUD SERVER   │
                                    │  (~/Project/Cloud)│
                                    └──────────────────┘
```

**İki-veritabanı stratejisi:**
- **Local Postgres** (port 5432) — Diary'nin asıl çalıştığı yer. Offline-first. Diary buraya yazar, buradan okur.
- **Cloud Postgres** (port 5433) — Cloud server'ın kullandığı kaynak. Sync sırasında local ↔ cloud merge edilir.

**Diary çevrimdışıyken** her şey local'de devam eder; **çevrimiçi olunca** Sync Client devreye girer.

---

## 1. ÖN ŞART KONTROLÜ (AŞAMA 0'DAN ÖNCE)

Bu prompt'a başlamadan önce şunlar **mutlaka** doğrulanmalı (kendin kontrol et, eksikse durup söyle):

- [ ] Mevcut Diary Cornell repo'su erişilebilir, `git status` temiz
- [ ] Cloud projesi (`~/Project/Cloud/`) ayakta, `/health` 200 dönüyor
- [ ] Cloud Postgres (port 5433) çalışıyor
- [ ] Diary için yeni Postgres (port 5432) için Docker hazır
- [ ] Mevcut SQLite DB dosyasının yedeği alındı (`cp diary.db diary.db.backup-{timestamp}`)

**Eksiksiz olarak doğrulayamıyorsan dur, eksiklikleri raporla, yönlendirme bekle.**

---

## 2. TEKNİK STACK (DEĞİŞMEYEN + EKLENEN)

**Değişmeyen (mevcut):**
- FastAPI backend
- HTML / Vanilla JS / CSS frontend
- Pydantic v2

**Değişen:**
- Storage: SQLite → **PostgreSQL 16** (asyncpg + SQLAlchemy 2 async)

**Eklenen:**
- `httpx` — Cloud REST API client
- `websockets` (veya FastAPI'nin `starlette.websockets` client'ı) — Cloud WS client
- `apscheduler` — saatlik sync trigger
- `bcrypt`, `PyJWT` — Cloud auth (token saklama, refresh)
- `keyring` (opsiyonel) — token'ları OS keychain'inde sakla; v1'de `.env` yeterli

---

## 3. DOSYA YAPISI

```
diary_cornell/
├── .env.example
├── .env                              # gitignored
├── .gitignore
├── README.md
├── docker-compose.yml                # local Postgres
├── pyproject.toml
├── requirements.txt
├── alembic.ini
├── pytest.ini
├── scripts/
│   ├── start_postgres.sh             # local diary postgres
│   ├── stop_postgres.sh
│   ├── migrate_sqlite_to_postgres.py # FAZ 1 veri taşıma scripti
│   ├── verify_migration.py           # SQLite ↔ Postgres satır karşılaştırma
│   └── manual_sync_test.py
├── alembic/
│   ├── env.py
│   └── versions/
├── src/
│   ├── __init__.py
│   ├── main.py                       # FastAPI app entry (mevcut, route'lar korunur)
│   ├── config.py                     # extended (cloud config eklendi)
│   ├── logger.py
│   ├── exceptions.py
│   ├── db/
│   │   ├── __init__.py
│   │   ├── base.py
│   │   ├── session.py
│   │   ├── models/
│   │   │   ├── entry.py              # Entry (mevcut alanlar + sync metadata)
│   │   │   ├── sync_metadata.py      # last_pulled_at, last_pushed_at, peer_id, dirty_flag
│   │   │   └── pending_op.py         # offline'da biriken CRDT op'lar (FAZ 3)
│   │   └── repository.py             # data access (mevcut SQLite kodu repository pattern'a çekildi)
│   ├── api/
│   │   ├── routes/                   # MEVCUT route'lar değişmeden çalışmalı
│   │   │   ├── entries.py
│   │   │   ├── pages.py              # HTML render
│   │   │   └── sync_admin.py         # YENİ: manuel sync trigger, sync status
│   │   └── dependencies.py
│   ├── services/
│   │   ├── entry_service.py          # mevcut autosave/CRUD logic (SQLite'tan repository'e taşındı)
│   │   └── ...
│   ├── sync/
│   │   ├── __init__.py
│   │   ├── client.py                 # HTTPCloudClient (REST)
│   │   ├── ws_client.py              # WSCloudClient (FAZ 3)
│   │   ├── scheduler.py              # apscheduler (saatlik trigger)
│   │   ├── network_monitor.py        # internet event yakalama
│   │   ├── sync_engine.py            # pull/push merge logic
│   │   ├── auth_manager.py           # JWT token store + refresh
│   │   └── conflict_handler.py       # local↔cloud çakışma çözümü
│   ├── crdt/                         # FAZ 3'te eklenecek; FAZ 1-2'de boş
│   │   ├── __init__.py
│   │   └── (Cloud'daki CRDT modülünü mirror'la — kod paylaşımı için ileride bir paket)
│   └── static/                       # MEVCUT HTML/JS/CSS dokunulmaz
│       ├── index.html
│       ├── app.js
│       └── style.css
├── tests/
│   ├── conftest.py
│   ├── unit/
│   │   ├── test_repository.py
│   │   ├── test_sync_engine.py
│   │   └── test_conflict_handler.py
│   ├── integration/
│   │   ├── test_legacy_endpoints.py  # ESKİ endpoint'ler hâlâ çalışıyor mu — REGRESSION GUARD
│   │   ├── test_sync_pull_push.py
│   │   └── test_offline_recovery.py
│   └── e2e/
│       └── test_full_sync_cycle.py
└── docs/
    ├── MIGRATION.md
    ├── SYNC_BEHAVIOR.md
    └── ROLLBACK.md
```

---

## 4. FAZ 1 — SQLITE → POSTGRESQL MIGRATION

### 4.1 Strateji: "Strangler Fig"

Mevcut kodu **bir tek seferde** kırmadan çevirmek için repository pattern uygulanır:
1. Mevcut tüm SQLite çağrıları `db/repository.py` arkasına soyutla
2. Repository önce SQLite implementation ile yazılır, mevcut testler geçer
3. PostgreSQL implementation eklenir, config flag ile hangisi aktif seçilir
4. Mock data ile Postgres testleri geçer
5. Production veri SQLite'tan Postgres'e taşınır
6. SQLite implementation silinir

### 4.2 Şema (Postgres tarafı)

Mevcut Cornell journal `entries_YYYY_MM` aylık tablo yapısı yerine **tek `entries` tablosu** kullanılır. Aylık sorgu performansı için index yeterli.

```sql
CREATE TABLE entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entry_date DATE NOT NULL UNIQUE,         -- bir günde tek entry (mevcut davranış)
    -- Cornell layout fields (mevcut)
    cue_column TEXT NOT NULL DEFAULT '',
    notes_column TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    planlar TEXT NOT NULL DEFAULT '',
    -- Sync metadata (YENİ)
    cloud_entry_id UUID,                     -- cloud'taki id, null = henüz sync olmadı
    cloud_journal_id UUID,                   -- hangi cloud journal'a bağlı
    version BIGINT NOT NULL DEFAULT 1,
    is_dirty BOOLEAN NOT NULL DEFAULT TRUE,  -- local'de değişti, cloud'a push bekliyor
    last_modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_synced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_entries_date ON entries(entry_date DESC);
CREATE INDEX idx_entries_dirty ON entries(is_dirty) WHERE is_dirty = TRUE;
CREATE INDEX idx_entries_cloud_id ON entries(cloud_entry_id);

CREATE TABLE sync_metadata (
    id INTEGER PRIMARY KEY DEFAULT 1,        -- singleton row
    peer_id VARCHAR(64) NOT NULL,            -- bu cihazın CRDT peer_id'si
    cloud_user_id UUID,
    cloud_journal_id UUID,                   -- aktif sync edilen journal
    access_token TEXT,                       -- JWT (kısa ömürlü)
    refresh_token TEXT,                      -- JWT (uzun ömürlü)
    token_expires_at TIMESTAMPTZ,
    last_pull_at TIMESTAMPTZ,
    last_push_at TIMESTAMPTZ,
    last_full_sync_at TIMESTAMPTZ,
    sync_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT singleton CHECK (id = 1)
);

-- FAZ 3'te kullanılacak; FAZ 1-2'de boş kalır
CREATE TABLE pending_ops (
    id BIGSERIAL PRIMARY KEY,
    entry_id UUID NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    field_name VARCHAR(32) NOT NULL,
    op_payload JSONB NOT NULL,               -- serialize edilmiş CRDT op
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    pushed BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX idx_pending_ops_unpushed ON pending_ops(pushed) WHERE pushed = FALSE;
```

### 4.3 Migration scripti

`scripts/migrate_sqlite_to_postgres.py`:

```python
"""
Strategy:
1. SQLite'taki tüm entries_YYYY_MM tablolarını UNION ile oku
2. Her satırı UUID ata, Postgres'e insert
3. cloud_entry_id NULL bırak (sonradan sync edilecek)
4. is_dirty = TRUE (Cloud'a push edilmemiş)
5. Migration sonu: SQLite row count == Postgres row count doğrula
"""
```

### 4.4 Repository pattern

```python
# src/db/repository.py
class EntryRepository(ABC):
    async def get_by_date(self, date: date) -> Entry | None: ...
    async def upsert(self, entry: EntryUpsert) -> Entry: ...
    async def list_by_month(self, year: int, month: int) -> list[Entry]: ...
    async def list_dirty(self) -> list[Entry]: ...
    async def mark_synced(self, entry_id: UUID, cloud_id: UUID) -> None: ...

class PostgresEntryRepository(EntryRepository): ...

# Factory in dependencies.py
def get_entry_repo() -> EntryRepository:
    return PostgresEntryRepository(...)
```

Mevcut `entry_service.py` repository'i kullanır — ne SQLite'a ne Postgres'e direkt bağlıdır.

### 4.5 Frontend dokunulmaz
- HTML/JS/CSS değişmez
- Mevcut endpoint'ler aynı response format'ını korur (Pydantic schema'lar değişmez)
- Frontend bir şeyin değiştiğini fark etmez

---

## 5. FAZ 2 — SYNC CLIENT (REST + SCHEDULER)

### 5.1 Auth flow (kullanıcı bir kere yapar)

```
1. Diary açılır, sync_metadata'da access_token yoksa:
   - Frontend "Cloud'a bağlan" butonu gösterir
   - Kullanıcı email/password girer
   - Diary backend POST /auth/login → Cloud
   - Token'lar sync_metadata'ya yazılır
2. Token expire olduğunda:
   - sync_engine ilk request 401 alır
   - auth_manager refresh_token ile POST /auth/refresh çağırır
   - Yeni access_token kaydedilir, request retry edilir
```

### 5.2 Saatlik sync (apscheduler)

```python
from apscheduler.schedulers.asyncio import AsyncIOScheduler

scheduler = AsyncIOScheduler()
scheduler.add_job(sync_engine.run_full_cycle, 'interval', hours=1, id='hourly_sync')
scheduler.start()
```

### 5.3 "Internet geldi" trigger

`network_monitor.py`:
```python
"""
Polling stratejisi (basit, OS-agnostic):
- Her 30 saniyede bir Cloud /health'e ping
- Önceki state offline, şimdi online → sync_engine.run_full_cycle() tetikle
- Ping başarılı: state=online, ping fail 3 ardışık: state=offline
"""
```

> **Neden polling?** OS-level network event API'leri platform-bağımlı (NetworkManager Linux, SCNetworkReachability macOS). Polling basit, taşınabilir, sistem yükü ihmal edilebilir.

### 5.4 Sync engine (FAZ 2 — field-level)

```python
async def run_full_cycle():
    if not network_online or not auth_valid:
        return
    
    last_sync = await meta_repo.get_last_pull_at()
    
    # 1. PULL
    pull_response = await cloud_client.get_sync_pull(since=last_sync)
    for cloud_entry in pull_response.entries:
        await merge_remote_entry(cloud_entry)
    
    # 2. PUSH
    dirty_entries = await entry_repo.list_dirty()
    if dirty_entries:
        push_response = await cloud_client.post_sync_push(entries=dirty_entries)
        for merged_entry in push_response.entries:
            await entry_repo.mark_synced(merged_entry.local_id, merged_entry.cloud_id)
    
    # 3. metadata güncelle
    await meta_repo.update_sync_timestamps()
```

### 5.5 Çakışma kuralları (FAZ 2 — basit, char-level CRDT yok)

Local entry tarihi 2026-04-29, cloud entry tarihi 2026-04-29 var, ikisinde de farklı içerik:

| Durum | Karar |
|---|---|
| Local `version` < Cloud `version` ve local **dirty değil** | Cloud kazanır, local güncellenir |
| Local `version` < Cloud `version` ve local **dirty** | **ÇAKIŞMA** — kullanıcıya sor (UI dialog) veya `last_modified_at` daha yeni olan kazanır + diğer versiyon `_conflict_backup` tablosuna yedeklenir |
| Local `version` >= Cloud `version` | Local kazanır, push edilir |

**FAZ 2'de** "last_modified_at daha yeni olan kazanır + yedek tut" stratejisi yeterli. UI dialog FAZ 3'te eklenebilir.

> **Önemli:** Aynı entry'i iki kullanıcı birlikte yazıyorsa FAZ 2 çakışma doğuracak; bu **beklenen** davranış. FAZ 3 (CRDT) bu durumu char-level çözecek.

### 5.6 Yeni endpoint'ler (frontend için)

```
GET  /api/sync/status          → { enabled, last_pull_at, last_push_at, online, dirty_count }
POST /api/sync/connect         → email/password ile cloud'a bağlan
POST /api/sync/disconnect      → token'ları sil, sync'i kapat
POST /api/sync/trigger         → manuel sync (kullanıcı butonuna basar)
```

Mevcut entries endpoint'leri aynen çalışır — sync **arka planda** ek olarak akar.

---

## 6. FAZ 3 — WEBSOCKET + CRDT (LIVE COLLABORATION)

### 6.1 Ne zaman aktif olur?

Kullanıcı bir entry'i açtığında:
1. WS bağlantısı `ws://cloud-host:5000/ws/journal/{cloud_journal_id}` kurulur
2. "subscribe" mesajı gönderilir
3. O entry üzerinde **başka kullanıcı varsa** "presence_update" alınır
4. Yalnızsa: WS bağlı kalır ama tek değişiklikler yine REST üzerinden push edilir (CRDT overhead'ı gereksiz)
5. **Başka kullanıcı bağlanırsa**: o andan itibaren keystroke'lar CRDT op olarak WS'ten yayınlanır

### 6.2 Local CRDT engine

Cloud'taki CRDT engine'i mirror'la — **iki proje aynı dataclass + algoritmaları paylaşmalı**. Kod tekrarını önlemek için iki seçenek:

**A. Git submodule / monorepo:** İki proje aynı `crdt_core/` paketini paylaşır
**B. Manuel mirror:** Cloud'daki dosyaları kopyala, tutarlı tut (basit ama bakım yükü)

> **Karar:** v1'de **B (manuel mirror)** + bir notebook test'i ile iki tarafın CRDT çıktılarının aynı olduğunu doğrulayan periyodik unit test. v2'de monorepo'ya geçilebilir.

### 6.3 Frontend keystroke yakalama

Mevcut `app.js`'e ek:

```javascript
// app.js
let crdtEnabled = false;
let activePeers = [];

const textarea = document.querySelector('#notes_column');

textarea.addEventListener('input', (e) => {
    if (crdtEnabled && activePeers.length > 0) {
        // CRDT mode: char-by-char op generate et, /api/crdt/apply'a yolla
        const op = generateOpFromInputEvent(e);
        fetch('/api/crdt/apply', { method: 'POST', body: JSON.stringify(op) });
    } else {
        // Mevcut debounced autosave: 500ms sonra full text save
        debouncedSave();
    }
});

// WS event listener
ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    if (msg.type === 'crdt_op_broadcast') {
        applyRemoteOp(msg.op);
        // textarea'yı update et (cursor position'ı koru!)
    } else if (msg.type === 'presence_update') {
        activePeers = msg.peers;
        crdtEnabled = activePeers.length > 1;  // başka kullanıcı varsa CRDT'ye geç
    }
};
```

> **Cursor position koruma:** Remote op uygulandığında cursor'un yeri kaymamalı. CodeMirror veya monaco-editor kullanmak işi kolaylaştırır ama **mevcut UI'yi değiştirmek istemiyorsun**, o yüzden vanilla textarea'da:
> ```javascript
> const cursorPos = textarea.selectionStart;
> applyOp(...);
> textarea.value = newText;
> textarea.setSelectionRange(adjustedCursor, adjustedCursor);
> ```
> Adjusted cursor: remote insert kullanıcının cursor'undan **önce** geldiyse cursor +1 kayar; **sonra** geldiyse aynı kalır.

### 6.4 Offline-first CRDT

Çevrimdışı yapılan keystroke'lar `pending_ops` tablosuna yazılır. Online olunca:
1. WS bağlanır
2. `pending_ops` sırasıyla server'a yollanır (idempotent — aynı `char_id` iki kez gelirse no-op)
3. Server diğer client'lara broadcast eder
4. Pending op'lar `pushed=TRUE` işaretlenir

---

## 7. GÜVENLİK PROTOKOLÜ (PROJEYE ÖZEL)

1. **Hardcoded secret yok** — tüm Cloud config (`CLOUD_URL`, `CLOUD_API_KEY` artık yok, JWT kullanılıyor) `.env`'den
2. **JWT token saklama:** `sync_metadata` tablosunda. Tablo file system'de Postgres data volume'unda — OS-level dosya izinleri (`chmod 700`) korur. v2: `keyring` ile OS keychain.
3. **Refresh token rotation:** Refresh kullanılınca eskisi invalidate edilir (Cloud tarafı sağlar; Diary sadece yeni token'ı yazar).
4. **HTTPS production'da zorunlu.** Development'ta http://localhost izinli.
5. **Sync request validation:** Cloud'tan gelen entry'ler Pydantic ile parse. Schema mismatch → log + skip, ama sync'i durdurma.
6. **WS message validation:** Aynı şekilde Pydantic + drop on mismatch.
7. **CRDT op signing yok (v1).** Cloud zaten JWT auth'la peer'i doğruluyor; client kendi peer_id'sini fake yaparsa Cloud reddeder.
8. **Local Postgres parolası** `.env`'de, gitignored. Default password `change_me_in_dev` ile gelmez — kullanıcı setup sırasında üretir.
9. **Migration scripti** SQLite dosyasını silmez, sadece kopyalar (rollback için).
10. **Sync scheduler tek instance.** Aynı Diary iki kez çalışırsa lock dosyası kontrol et (`/tmp/diary_sync.lock`).
11. **Error handling:** Sync fail olursa **hiçbir local data kaybolmamalı**. Dirty flag kalır, bir sonraki cycle'da tekrar denenir.
12. **Slopsquatting:** Yeni paketler (httpx, apscheduler, websockets, PyJWT) PyPI doğrula, exact pin.
13. **Threat model:** `docs/THREAT_MODEL.md` — Cloud unreachable → graceful degradation, token leak → kısa ömür, race condition (paralel sync trigger) → lock dosyası.

---

## 8. GELİŞTİRME DİSİPLİNİ + OTONOM ÇALIŞMA MODU

### Branch'ler
- `main` her zaman çalışır
- `feature/repository-pattern` (FAZ 1.0 — SQLite halen aktif)
- `feature/postgres-implementation` (FAZ 1.1)
- `feature/data-migration` (FAZ 1.2)
- `feature/sqlite-removal` (FAZ 1.3)
- `feature/sync-rest-client` (FAZ 2.1)
- `feature/sync-scheduler` (FAZ 2.2)
- `feature/sync-network-monitor` (FAZ 2.3)
- `feature/crdt-engine-mirror` (FAZ 3.1)
- `feature/websocket-client` (FAZ 3.2)
- `feature/frontend-crdt-integration` (FAZ 3.3)

### Otonom mod
**🛑 TEST DURAĞI**'larında dur. Aralarda tüm operasyonlar (branch, commit, merge, debug, test düzeltme, paket ekleme) sormadan yapılır. Aşama sonu 6-10 satır rapor, sıradakine sormadan geç.

### Regression guard (kritik)
Her aşamada `tests/integration/test_legacy_endpoints.py` çalışmaya devam etmeli. Bu test mevcut tüm Diary endpoint'lerinin response şeklini ve davranışını dondurur. Bu testler kırılırsa **mevcut özellik bozuldu** demektir, geri al + düzelt.

### Geri alınamaz işlemler
- SQLite silme (FAZ 1.3'ün son adımı) — verify_migration scripti %100 başarılı olmadan dokunma
- Production data migration — backup almadan başlama

---

## 9. ÇALIŞMA AKIŞI — FAZ FAZ

### Aşama 0: Hazırlık (otonom)
1. Yeni `docker-compose.yml` ile local Postgres'i ayağa kaldır (port 5432)
2. `requirements.txt`'e yeni paketler ekle (asyncpg, sqlalchemy[asyncio], httpx, apscheduler, websockets, PyJWT, alembic, bcrypt)
3. `.env.example` genişlet (DB_*, CLOUD_URL, CLOUD_PEER_DEVICE_LABEL, SYNC_INTERVAL_HOURS=1)
4. SQLite DB'nin yedeğini al
5. Mevcut testleri çalıştır, hepsi geçiyor mu doğrula → baseline
6. Bitiş raporu, FAZ 1.0'a geç

### FAZ 1.0 — Repository Pattern (otonom)
1. `feature/repository-pattern` branch
2. Mevcut SQLite çağrılarını `EntryRepository` interface'i arkasına soyutla
3. `SqliteEntryRepository` implementation yaz (mevcut kodu wrap)
4. Service layer'ı repository kullanacak şekilde refactor et
5. **Tüm mevcut testler hâlâ geçmeli (regression guard)**
6. Merge, bitiş raporu, FAZ 1.1'e geç

### FAZ 1.1 — Postgres Implementation (otonom)
1. `feature/postgres-implementation` branch
2. SQLAlchemy 2 async modelleri (Bölüm 4.2)
3. Alembic init + migration generate
4. `PostgresEntryRepository` implementation
5. Config flag: `STORAGE_BACKEND=sqlite|postgres`, default sqlite
6. Postgres'i `STORAGE_BACKEND=postgres` ile elle test et — temel CRUD çalışıyor mu
7. Tüm mevcut testleri **iki backend'de de** çalıştır, ikisinde de geç
8. Merge, bitiş raporu

### FAZ 1.2 — Veri Migrasyonu (otonom + 🛑 TEST DURAĞI)
1. `feature/data-migration` branch
2. `scripts/migrate_sqlite_to_postgres.py` yaz
3. `scripts/verify_migration.py` yaz (row count + her satır field-level eşitlik kontrolü)
4. Test DB'leri ile dry run
5. Merge

**🛑 TEST DURAĞI 1 — Gerçek veri migrasyonu**
   - Bana yedeği aldığını doğrula
   - `python scripts/migrate_sqlite_to_postgres.py --dry-run` çalıştırmamı söyle
   - Çıktıyı görüp onaylamamı bekle
   - Sonra gerçek migration komutunu ver
   - `verify_migration.py` çıktısını göster, %100 eşleşmiyorsa devam etme

### FAZ 1.3 — SQLite Çıkış (otonom)
1. `feature/sqlite-removal` branch
2. `STORAGE_BACKEND` config flag'ini kaldır
3. SQLite implementation kodlarını sil
4. SQLite paketini requirements.txt'ten çıkar
5. README MIGRATION.md'yi güncelle (taşınma tamamlandı)
6. **Regression test suite hâlâ %100 geçmeli**
7. Merge, bitiş raporu, FAZ 2'ye geç

### FAZ 2.1 — Sync REST Client (otonom)
1. `feature/sync-rest-client` branch
2. `sync/auth_manager.py` (JWT store + refresh)
3. `sync/client.py` (HTTPCloudClient — pull, push, login, refresh)
4. `sync/conflict_handler.py` (basit version compare)
5. `sync/sync_engine.py` (run_full_cycle)
6. `/api/sync/connect`, `/api/sync/status`, `/api/sync/trigger` endpoint'leri
7. Unit test (mock Cloud)
8. Merge, bitiş raporu

### FAZ 2.2 — Scheduler + Network Monitor (otonom)
1. `feature/sync-scheduler` branch
2. `sync/scheduler.py` (apscheduler hourly job)
3. `sync/network_monitor.py` (30s polling Cloud /health)
4. Lifespan event'inde scheduler ve monitor başlat/durdur
5. Lock file (tek instance)
6. Integration test (fake clock ile saatlik tetik simüle et)
7. Merge

### FAZ 2.3 — Frontend Sync UI (otonom + 🛑 TEST DURAĞI)
1. `feature/sync-frontend` branch
2. `static/index.html`'e küçük "Cloud" indicator ekle (sağ üst köşe: 🟢 senkron, 🟡 dirty, 🔴 offline, ⚪ disabled)
3. "Cloud'a bağlan" modal (email/password)
4. Status polling: `/api/sync/status` her 5 saniyede bir
5. Manuel sync butonu
6. Mevcut UI'yi minimum bozarak ekle (CSS uyumlu)
7. Merge

**🛑 TEST DURAĞI 2 — Sync uçtan uca**
   - Cloud server'ı ayağa kaldırmamı söyle
   - Diary'i ayağa kaldır
   - Cloud'a bağlan, bir entry yaz
   - Cloud'da entry görünüyor mu kontrol etmemi söyle (`psql` ile veya başka bir test client)
   - Bir başka entry'i Cloud'da elle değiştir, Diary'de manuel sync tetikle, Diary güncellendi mi
   - Çakışma senaryosu: aynı entry'i hem local hem cloud'da değiştir, sync sonrası beklenen davranışı doğrula
   - Onay bekle

### FAZ 3.1 — CRDT Engine Mirror (otonom)
1. `feature/crdt-engine-mirror` branch
2. Cloud'taki `crdt/` modülünü `src/crdt/` altına kopyala
3. Cross-test: Diary CRDT vs Cloud CRDT — aynı op sequence'i ikisinde de aynı text'i üretiyor mu
4. Pydantic op schemas aynı (Cloud'taki `protocol.py`'yi mirror'la)
5. Unit test (Cloud testlerinin aynısını burada da çalıştır)
6. Merge, bitiş raporu

### FAZ 3.2 — WebSocket Client (otonom)
1. `feature/websocket-client` branch
2. `sync/ws_client.py` (connect, send, receive loop, reconnect with exponential backoff)
3. `pending_ops` table integration (offline op queue)
4. Yeni endpoint: `POST /api/crdt/apply` (frontend buraya op gönderir, backend WS'e relay eder)
5. Unit + integration test (mock Cloud WS)
6. Merge

### FAZ 3.3 — Frontend CRDT (otonom + 🛑 TEST DURAĞI)
1. `feature/frontend-crdt-integration` branch
2. `app.js`'e CRDT mode toggle (Bölüm 6.3)
3. Cursor position preservation
4. Presence indicator ("Ali yazıyor...")
5. Crash test: ağ kopuşu sırasında yazma, dönüşte recovery
6. Merge

**🛑 TEST DURAĞI 3 — Çoklu kullanıcı CRDT**
   - İki tarayıcı aç (veya iki cihaz)
   - İki ayrı kullanıcıyla aynı entry'e gir
   - Aynı paragrafa paralel yaz
   - Her iki ekranda final text aynı mı, doğru harf sırasıyla mı
   - Birinin internetini kes, yaz, geri aç → senkronize oluyor mu
   - Onay bekle

### Final (otonom + 🛑 TEST DURAĞI)
1. End-to-end stress test
2. README, MIGRATION.md, SYNC_BEHAVIOR.md, ROLLBACK.md, THREAT_MODEL.md
3. OWASP API Top 10 checklist
4. Tüm test suite'i geçsin (legacy + sync + crdt)
5. Merge

**🛑 TEST DURAĞI 4 — Final onay (geri alınamaz)**
   - GitHub push komutlarını ver
   - Public/private tercih
   - Onay alınca push

---

## 10. .ENV.EXAMPLE

```env
# Local Postgres
DB_HOST=localhost
DB_PORT=5432
DB_NAME=diary_db
DB_USER=diary_user
DB_PASSWORD=change_me_in_dev

# Cloud Server
CLOUD_URL=http://localhost:5000
CLOUD_WS_URL=ws://localhost:5000

# Sync Config
SYNC_ENABLED=false                       # kullanıcı UI'dan açar
SYNC_INTERVAL_HOURS=1
NETWORK_PROBE_INTERVAL_SECONDS=30
DEVICE_LABEL=Deniz-Macbook
LOCK_FILE=/tmp/diary_sync.lock

# App
APP_ENV=development
APP_DEBUG=false
APP_HOST=0.0.0.0
APP_PORT=8001
LOG_LEVEL=INFO

# Migration
SQLITE_BACKUP_DIR=./backups
```

---

## 11. ROLLBACK PLANI (`docs/ROLLBACK.md`)

Bir şey ters giderse:

**FAZ 1'den geri dönüş:**
- `STORAGE_BACKEND=sqlite` config flag'ini geri getir (FAZ 1.3 silmeden önce)
- SQLite backup dosyasını geri yükle
- Postgres container'ı durdur

**FAZ 2'den geri dönüş:**
- `SYNC_ENABLED=false` set et
- Scheduler durur, Diary tek başına çalışmaya devam eder
- Local Postgres data korunur

**FAZ 3'ten geri dönüş:**
- WS bağlantısını kapat
- CRDT mode'u disable et
- REST sync FAZ 2 davranışına geri döner

---

## 12. KENDİNE KONTROL SORULARI

- [ ] Mevcut endpoint response format'ları korundu mu? (regression test geçti mi?)
- [ ] Frontend HTML/JS/CSS hâlâ değişmemiş mi? (sadece sync UI eklendi)
- [ ] Postgres connection leak var mı? (async session düzgün kapatılıyor mu)
- [ ] Sync fail'inde local data kaybı var mı?
- [ ] Token loglara sızdı mı?
- [ ] Lock file düzgün release ediliyor mu? (kill -9 sonrası tekrar başlatınca)
- [ ] Aynı entry'e paralel write race condition var mı?
- [ ] Conventional commit'ler?

---

## 13. CLOUD ↔ DIARY İLETİŞİM ÖZETİ (REFERANS)

### REST endpoint'ler (Cloud tarafında)
```
POST   /auth/register
POST   /auth/login                          → access + refresh token
POST   /auth/refresh
GET    /journals                            → kullanıcının journal'ları
POST   /journals                            → yeni journal
GET    /sync/pull?journal_id=X&since=ts     → değişenleri çek
POST   /sync/push  body:{entries:[...]}     → local değişiklikleri yolla
GET    /health
```

### WebSocket (FAZ 3)
```
ws://cloud:5000/ws/journal/{journal_id}?token=JWT

→ {"type": "subscribe"}
→ {"type": "crdt_op", "entry_id": "...", "op": {...}}
→ {"type": "presence"}

← {"type": "crdt_op_broadcast", "entry_id": "...", "op": {...}, "from_peer": "..."}
← {"type": "presence_update", "peers": ["alice", "bob"]}
← {"type": "snapshot_updated", "entry_id": "...", "version": 42}
← {"type": "error", "code": "...", "message": "..."}
```

### Auth flow
```
1. Diary first-time setup:
   POST /auth/register {username, email, password}
   → {user_id, peer_id, access_token, refresh_token}

2. Subsequent runs:
   Stored access_token kullanılır
   401 alınınca → refresh_token ile /auth/refresh

3. Diary her request'inde:
   Header: Authorization: Bearer {access_token}
```

### Veri akışı senaryoları

**Senaryo A — Tek kullanıcı, çevrimiçi:**
- Diary local Postgres'e yazar, dirty flag set
- Saatlik sync veya manuel trigger → POST /sync/push
- Cloud merge eder, version artırır
- Diary mark_synced

**Senaryo B — Tek kullanıcı, çevrimdışı sonra çevrimiçi:**
- Diary local'e yazar, dirty flag birikir
- Network monitor online detect eder
- sync_engine.run_full_cycle() tetikler
- Tüm dirty entry'ler push edilir

**Senaryo C — Çoklu kullanıcı, çevrimiçi, aynı entry:**
- Her iki Diary WS'e bağlanır
- Presence "2 peer" gösterir
- CRDT mode aktif, keystroke'lar op olarak yayınlanır
- Cloud broadcast eder, snapshot 30s'de bir materialize edilir

**Senaryo D — Çakışma (FAZ 2 davranışı, CRDT henüz yok):**
- Local entry version 5, cloud version 7, ikisi de aynı tarih
- conflict_handler last_modified_at karşılaştırır
- Daha yeni olan kazanır, diğeri `_conflict_backup` JSON'una append edilir

---

## 14. BAŞLA

Şu an Aşama 0'dasın. Otonom çalış, **🛑 TEST DURAĞI** noktalarında dur (FAZ 1.2, 2.3, 3.3, Final).

İlk işin (Aşama 0):
1. Bölüm 1'deki ön şart kontrollerini yap, eksiksiz mi
2. Mevcut SQLite DB'nin yedeğini al
3. Local Postgres docker-compose ekle, ayağa kaldır
4. Yeni paketleri requirements.txt'e ekle, install et
5. Mevcut testler hâlâ geçiyor mu doğrula (baseline)
6. Bitiş raporu (6-10 satır), durmadan FAZ 1.0'a geç

Hatırlatma: Otonom mod aktif. Sadece dört test durağında durulur (FAZ 1.2, 2.3, 3.3, Final). Geri kalan her şey otonom — sıradaki faz, branch, commit, merge, debug, test düzeltme.
