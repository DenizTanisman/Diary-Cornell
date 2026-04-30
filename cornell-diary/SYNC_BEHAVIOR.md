# Sync behavior

How Diary keeps offline / online / live-edit states consistent — REST surface (FAZ 2) plus the
char-level CRDT (FAZ 3).

## State machine (per entry)

```
[offline edit]          [REST sync]               [live edit]
  user types     ──►   debounced upsert     ──►   crdt:text-updated
  is_dirty=true        version bumps              broadcast on WS
  pending_ops if WS        │                         │
  is up                    ▼                         ▼
                      cloud /push 200          remote peer applies
                      is_dirty=false           materialise → render
```

`is_dirty` is the bridge — set on every local upsert, cleared on a successful push. The dirty
indicator chip in the toolbar reads `dirtyCount > 0`.

## REST sync (FAZ 2)

| Trigger                       | Path                                              |
| ----------------------------- | ------------------------------------------------- |
| User clicks "Şimdi Senkronize Et" | `trigger_sync` → `SyncEngine::run_full_cycle`     |
| Network monitor: offline → online | `network::start` task triggers a single cycle    |
| Hourly cron                       | **Disabled** (macOS panic_cannot_unwind — see lib.rs). FAZ-final follow-up. |

`run_full_cycle` is a `pull → push → mark synced` loop, gated by `cycle_lock` so a manual click
mid-network-recovery doesn't double-fire.

### Pull

- `GET /journals/{id}/entries?since=…` returns rows the server saw after our last `last_pull_at`.
- `conflict::decide(local, dirty?, local_updated, cloud)` returns one of:
  - `InsertCloud` — no local row.
  - `OverwriteWithCloud` — local stale, clean.
  - `CloudWonOverDirtyLocal` — both dirty, cloud's `last_modified_at` is newer.
  - `LocalWon` — both dirty, local is newer; skip pull, push will carry.
  - `LocalAlreadyFresher` — local version ≥ cloud, leave it.
- `last_modified_at` falls back to `now()` if cloud sends `null`, so a freshly-pulled entry still
  beats stale local edits.

### Push

- One `PushRequest` per cycle, including every `is_dirty=true` row.
- Body fields: `journal_id`, `peer_id`, `device_label?`, `idempotency_key?`, `entries[]`,
  `crdt_ops[]`. `entries[i].version >= 1` is **enforced server-side** — fresh-row default in the
  frontend is `version=1`.
- Cloud responds with `merged_entries` (the canonical post-merge state) which the engine
  re-upserts locally so `is_dirty` clears in lock-step.

### Conflict matrix (FAZ 2 layer)

```
                            Cloud version
                       <  =  >
Local clean / dirty?  ┌──────────────────────────
   stale, clean       │ -    -    OverwriteWithCloud
   stale, dirty       │ -    -    CloudWonOverDirtyLocal | LocalWon
   equal, clean       │ -    LocalAlreadyFresher  -
   ahead              │ LocalAlreadyFresher  -    -
```

The CRDT layer (FAZ 3) is char-level and runs **per keystroke** during a live session — REST
conflict resolution still applies on every reconnect for legacy / offline paths.

## Live editing (FAZ 3)

### Subscribe

```
React mount(useCrdtChannel)
  → invoke('subscribe_crdt', {entryDate, fieldName, seedText})
  → WsClient.subscribe
      • lazy: ensure_connected (one shared WS for the app)
      • seed CrdtDocument with the saved text on first subscribe
      • send {"type":"subscribe","entry_date":...}
  → server emits presence_update
  → useCrdtChannel sets crdtMode = peers.length > 1
```

`presence_update` carrying `peers.length === 1` keeps the doc seeded but inactive — the UI stays
on the debounced REST path, no per-keystroke broadcast.

### Per-keystroke

```
React onChange(newText)
  → invoke('apply_local_text', {entryDate, fieldName, newText})
  → WsClient.apply_local_text
      • diff against doc.materialize() (prefix/suffix-trim)
      • emit local_delete (right-to-left) + local_insert chain (left-to-right)
      • per op: WS frame on live socket OR pending_ops queue (offline fallback)
  → returns the newly-materialised text
React applies the returned text + restores cursor (useLayoutEffect)
```

The diff is the simplest correct algorithm: trim a common prefix, trim a common suffix, treat the
remaining old chars as deletes and the remaining new chars as inserts. For paste-replace this
collapses to `len(old_middle)` deletes + `len(new_middle)` inserts. For single keystrokes it's
one op.

### Receiving

```
WS reader task
  → match WsIn:
       CrdtOpBroadcast → apply_remote → materialise → emit "crdt:text-updated"
       PresenceUpdate  → emit "crdt:presence"
       SnapshotUpdated → emit "crdt:snapshot-updated"
       Error           → log only
React useCrdtChannel listener (filtered by entry+field)
  → setState(text)
```

`apply_remote` is **idempotent** (same `char_id` is a no-op) and tolerates ops whose `prev_id`
hasn't arrived yet — they go to a per-doc pending queue and re-try on every successful insert.

### Convergence

The RGA invariant: any two peers that receive the same set of ops in any order materialise to the
same text. Verified by `convergence_holds_for_random_op_orderings` — synthesises 200 random ops
across 3 peers, applies each set in three orders (insertion, reverse, shuffled), and asserts all
three resulting texts match.

The integration choices that make convergence hold:
- `prev_id` is **immutable** — it's the RGA parent, not a linked-list pointer. Splicing only
  rewires `next_id`.
- Head-anchored ops normalise `None ↔ Some(HEAD_ID)` at storage and serialisation boundaries so
  the integration logic compares against a single shape.
- Among siblings sharing a parent, the higher `(lamport, peer_id)` sits closer to the parent.
  When walking past an outranking sibling we skip its **entire subtree** before re-evaluating
  (otherwise descendants get treated as direct competitors and convergence breaks).

## Offline durability — `pending_ops`

Schema (`postgres_migrations/0003_pending_ops.sql`):

```sql
CREATE TABLE pending_ops (
    id           BIGSERIAL PRIMARY KEY,
    entry_date   TEXT NOT NULL REFERENCES diary_entries(date) ON DELETE CASCADE,
    field_name   TEXT NOT NULL,
    op_payload   JSONB NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    pushed       BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX idx_pending_ops_unpushed ON pending_ops(pushed, created_at) WHERE pushed = FALSE;
```

`apply_local_text` falls back to `PendingOpRepo::queue` whenever the WS socket is down or the
wire send errors. On reconnect, `WsClient::flush_pending` drains rows where `pushed=false` in
chronological order, sends each as `crdt_op`, and flips the flag.

Dependent ops drain in causal order because the queue is sorted by `(created_at, id)` — a
delete + insert pair authored in the same tick goes out in the same order they were authored.

The `pushed=true` rows stay around for diagnostics. A separate sweeper (Final / FAZ-future
follow-up) prunes anything older than N days.

## Known operational quirks

- **macOS hourly cron**: disabled because `tokio-cron-scheduler::JobScheduler::new` spawns its
  own Tokio runtime, and calling `block_on` inside Cocoa's `did_finish_launching` panics
  `panic_cannot_unwind`. Manual + network-recovery triggers cover every functional path.
- **401 reactive refresh**: not implemented. A user-visible 401 today requires manual disconnect/
  reconnect (Backlog).
- **Test DB pollution**: `cargo test` shares the dev `DATABASE_URL` and writes test fixture rows
  (e.g. `peer_id='alice@laptop'`, version=0 entries). See OPERATIONS.md for cleanup queries.
  Backlog: separate test DB.
