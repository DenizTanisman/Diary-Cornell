//! Read/write the singleton `sync_metadata` row.
//!
//! Singleton check is enforced by the table itself (CONSTRAINT id = 1).
//! Every write is an UPDATE so we never have to chase row-existence races.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::db::DbPool;
use uuid::Uuid;

use crate::error::DomainError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncMetadata {
    pub peer_id: String,
    pub cloud_user_id: Option<Uuid>,
    pub cloud_journal_id: Option<Uuid>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub last_pull_at: Option<DateTime<Utc>>,
    pub last_push_at: Option<DateTime<Utc>>,
    pub last_full_sync_at: Option<DateTime<Utc>>,
    pub sync_enabled: bool,
    pub device_label: Option<String>,
}

type MetadataRow = (
    String,
    Option<Uuid>,
    Option<Uuid>,
    Option<String>,
    Option<String>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    bool,
    Option<String>,
);

pub async fn read(pool: &DbPool) -> Result<SyncMetadata, DomainError> {
    let row: MetadataRow = sqlx::query_as(
        "SELECT peer_id, cloud_user_id, cloud_journal_id, access_token, refresh_token, \
                token_expires_at, last_pull_at, last_push_at, last_full_sync_at, \
                sync_enabled, device_label \
         FROM sync_metadata WHERE id = 1",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| DomainError::Storage(format!("read sync_metadata: {e}")))?;

    Ok(SyncMetadata {
        peer_id: row.0,
        cloud_user_id: row.1,
        cloud_journal_id: row.2,
        access_token: row.3,
        refresh_token: row.4,
        token_expires_at: row.5,
        last_pull_at: row.6,
        last_push_at: row.7,
        last_full_sync_at: row.8,
        sync_enabled: row.9,
        device_label: row.10,
    })
}

pub async fn save_tokens(
    pool: &DbPool,
    access_token: &str,
    refresh_token: &str,
    token_expires_at: Option<DateTime<Utc>>,
    cloud_user_id: Option<Uuid>,
    peer_id: Option<&str>,
    device_label: Option<&str>,
) -> Result<(), DomainError> {
    sqlx::query(
        "UPDATE sync_metadata SET \
            access_token = $1, \
            refresh_token = $2, \
            token_expires_at = $3, \
            cloud_user_id = COALESCE($4, cloud_user_id), \
            peer_id = COALESCE($5, peer_id), \
            device_label = COALESCE($6, device_label), \
            sync_enabled = TRUE \
         WHERE id = 1",
    )
    .bind(access_token)
    .bind(refresh_token)
    .bind(token_expires_at)
    .bind(cloud_user_id)
    .bind(peer_id)
    .bind(device_label)
    .execute(pool)
    .await
    .map_err(|e| DomainError::Storage(format!("save tokens: {e}")))?;
    Ok(())
}

pub async fn save_journal(pool: &DbPool, journal_id: Uuid) -> Result<(), DomainError> {
    sqlx::query("UPDATE sync_metadata SET cloud_journal_id = $1 WHERE id = 1")
        .bind(journal_id)
        .execute(pool)
        .await
        .map_err(|e| DomainError::Storage(format!("save journal id: {e}")))?;
    Ok(())
}

pub async fn save_pull_at(pool: &DbPool, at: DateTime<Utc>) -> Result<(), DomainError> {
    sqlx::query("UPDATE sync_metadata SET last_pull_at = $1 WHERE id = 1")
        .bind(at)
        .execute(pool)
        .await
        .map_err(|e| DomainError::Storage(format!("save last_pull_at: {e}")))?;
    Ok(())
}

pub async fn save_push_at(pool: &DbPool, at: DateTime<Utc>) -> Result<(), DomainError> {
    sqlx::query("UPDATE sync_metadata SET last_push_at = $1, last_full_sync_at = $1 WHERE id = 1")
        .bind(at)
        .execute(pool)
        .await
        .map_err(|e| DomainError::Storage(format!("save last_push_at: {e}")))?;
    Ok(())
}

pub async fn clear(pool: &DbPool) -> Result<(), DomainError> {
    sqlx::query(
        "UPDATE sync_metadata SET \
            access_token = NULL, refresh_token = NULL, token_expires_at = NULL, \
            cloud_user_id = NULL, cloud_journal_id = NULL, sync_enabled = FALSE, \
            last_pull_at = NULL, last_push_at = NULL, last_full_sync_at = NULL \
         WHERE id = 1",
    )
    .execute(pool)
    .await
    .map_err(|e| DomainError::Storage(format!("clear sync_metadata: {e}")))?;
    Ok(())
}
