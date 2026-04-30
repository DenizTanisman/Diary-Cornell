//! Durable per-op queue for the WS pipeline.
//!
//! Every keystroke the UI emits while the live socket is **not** connected
//! lands here first. On reconnect the WS client drains rows where
//! `pushed = false` in chronological order, sends them, and flips the flag.
//! This is what makes char-level edits offline-durable in FAZ 3.2.
//!
//! Schema lives in `postgres_migrations/0003_pending_ops.sql`. We only
//! interact with the table through this thin repo so other modules don't
//! depend on sqlx directly.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::crdt::operations::CharOp;
use crate::error::DomainError;

/// One queued op + the metadata needed to round-trip it through the WS
/// pipeline. `id` is the bigserial PK; we hand it back to `mark_pushed`
/// after the socket acknowledges the broadcast.
#[derive(Debug, Clone)]
pub struct PendingOp {
    pub id: i64,
    pub entry_date: String,
    pub field_name: String,
    pub op: CharOp,
    #[allow(dead_code)] // surfaced via debug pages later
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct PendingOpRepo {
    pool: PgPool,
}

impl PendingOpRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Persist one op so it survives a Diary restart even if the WS
    /// socket never reconnects in this session.
    pub async fn queue(
        &self,
        entry_date: &str,
        field_name: &str,
        op: &CharOp,
    ) -> Result<i64, DomainError> {
        let payload = serde_json::to_value(op)
            .map_err(|e| DomainError::Storage(format!("serialize charop: {e}")))?;
        let row: (i64,) = sqlx::query_as(
            "INSERT INTO pending_ops (entry_date, field_name, op_payload, pushed) \
             VALUES ($1::date, $2, $3, FALSE) RETURNING id",
        )
        .bind(entry_date)
        .bind(field_name)
        .bind(payload)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DomainError::Storage(format!("queue pending op: {e}")))?;
        Ok(row.0)
    }

    /// Snapshot of unpushed ops in chronological order. The WS client
    /// iterates this on reconnect and pushes each in turn.
    pub async fn list_unpushed(&self) -> Result<Vec<PendingOp>, DomainError> {
        let rows = sqlx::query_as::<
            _,
            (
                i64,
                chrono::NaiveDate,
                String,
                serde_json::Value,
                DateTime<Utc>,
            ),
        >(
            "SELECT id, entry_date, field_name, op_payload, created_at \
             FROM pending_ops WHERE pushed = FALSE ORDER BY created_at, id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::Storage(format!("list pending ops: {e}")))?;

        let mut out = Vec::with_capacity(rows.len());
        for (id, entry_date, field_name, op_payload, created_at) in rows {
            let op: CharOp = serde_json::from_value(op_payload)
                .map_err(|e| DomainError::Storage(format!("decode charop {id}: {e}")))?;
            out.push(PendingOp {
                id,
                entry_date: entry_date.format("%Y-%m-%d").to_string(),
                field_name,
                op,
                created_at,
            });
        }
        Ok(out)
    }

    /// Marks one op as successfully broadcast. We keep the row for
    /// audit/debug; pruning is the sweeper's job.
    pub async fn mark_pushed(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("UPDATE pending_ops SET pushed = TRUE WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Storage(format!("mark pushed: {e}")))?;
        Ok(())
    }

    #[cfg(test)]
    pub async fn pushed_count(&self) -> Result<i64, DomainError> {
        let row: (i64,) = sqlx::query_as("SELECT count(*) FROM pending_ops WHERE pushed = TRUE")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DomainError::Storage(format!("count pushed: {e}")))?;
        Ok(row.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crdt::node::CharNode;
    use crate::db::pool::build_pool;
    use crate::db::postgres_impl::PostgresEntryRepository;
    use crate::db::repository::EntryRepository;
    use crate::db::run_migrations;
    use serial_test::serial;

    async fn fresh_pool() -> Option<PgPool> {
        let url = std::env::var("DATABASE_URL")
            .ok()
            .filter(|s| !s.is_empty())?;
        let pool = build_pool(&url).await.ok()?;
        run_migrations(&pool).await.ok()?;
        // Tests share one Postgres — wipe pending_ops AND any diary rows
        // tests inserted to keep the FK happy across runs.
        sqlx::query("TRUNCATE pending_ops RESTART IDENTITY CASCADE")
            .execute(&pool)
            .await
            .ok()?;
        sqlx::query("DELETE FROM diary_entries WHERE date = '2026-04-29'")
            .execute(&pool)
            .await
            .ok()?;
        Some(pool)
    }

    async fn seed_entry(pool: &PgPool) {
        // pending_ops.entry_date FKs to diary_entries — make sure the
        // referenced row exists before queueing.
        let repo = PostgresEntryRepository::new(pool.clone());
        repo.upsert(crate::db::DiaryEntry {
            date: "2026-04-29".into(),
            diary: "seed".into(),
            cue_items: vec![],
            summary: String::new(),
            quote: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            device_id: None,
            version: 0,
        })
        .await
        .unwrap();
    }

    fn sample_op() -> CharOp {
        let node = CharNode::new("alice", 1, 0, 'x', None);
        CharOp::from_insert(&node)
    }

    #[tokio::test]
    #[serial(postgres)]
    async fn queue_then_list_round_trips() {
        let Some(pool) = fresh_pool().await else {
            eprintln!("skipping queue_then_list_round_trips — DATABASE_URL not reachable");
            return;
        };
        seed_entry(&pool).await;
        let repo = PendingOpRepo::new(pool);

        let id = repo
            .queue("2026-04-29", "diary", &sample_op())
            .await
            .unwrap();
        assert!(id > 0);

        let pending = repo.list_unpushed().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].entry_date, "2026-04-29");
        assert_eq!(pending[0].field_name, "diary");
    }

    #[tokio::test]
    #[serial(postgres)]
    async fn mark_pushed_excludes_from_unpushed_listing() {
        let Some(pool) = fresh_pool().await else {
            eprintln!(
                "skipping mark_pushed_excludes_from_unpushed_listing — DATABASE_URL not reachable"
            );
            return;
        };
        seed_entry(&pool).await;
        let repo = PendingOpRepo::new(pool);

        let id = repo
            .queue("2026-04-29", "diary", &sample_op())
            .await
            .unwrap();
        assert_eq!(repo.list_unpushed().await.unwrap().len(), 1);

        repo.mark_pushed(id).await.unwrap();
        assert!(repo.list_unpushed().await.unwrap().is_empty());
        assert_eq!(repo.pushed_count().await.unwrap(), 1);
    }
}
