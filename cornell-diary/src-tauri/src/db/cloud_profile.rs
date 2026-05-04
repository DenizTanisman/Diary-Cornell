//! Cloud profile repository (MD 03 / Faz 3.1).
//!
//! Two implementations behind `CloudProfileRepository`. Postgres stores
//! `is_active` as BOOLEAN; SQLite as INTEGER 0/1. Both back-ends keep
//! exactly one active row via a partial unique index, which the
//! repository preserves by toggling rows inside a single transaction.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::db::pool::DbPool;
use crate::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub is_active: bool,
}

#[async_trait]
pub trait CloudProfileRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<CloudProfile>, DomainError>;
    async fn get_active(&self) -> Result<CloudProfile, DomainError>;
    async fn set_active(&self, id: &str) -> Result<(), DomainError>;
    async fn upsert(&self, profile: CloudProfile) -> Result<(), DomainError>;
    async fn delete(&self, id: &str) -> Result<(), DomainError>;
}

// Local helper — both back-ends reject the two seeded rows so the user
// never deletes the only "Local" profile out from under themselves.
const PROTECTED_IDS: &[&str] = &["local", "production"];

pub fn is_protected(id: &str) -> bool {
    PROTECTED_IDS.iter().any(|p| *p == id)
}

// ----------------------------------------------------------------------
// Postgres
// ----------------------------------------------------------------------

#[cfg(not(diary_sqlite))]
pub struct PostgresCloudProfileRepo {
    pool: sqlx::PgPool,
}

#[cfg(not(diary_sqlite))]
impl PostgresCloudProfileRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[cfg(not(diary_sqlite))]
#[async_trait]
impl CloudProfileRepository for PostgresCloudProfileRepo {
    async fn list(&self) -> Result<Vec<CloudProfile>, DomainError> {
        let rows = sqlx::query_as::<_, (String, String, String, Option<String>, bool)>(
            "SELECT id, name, base_url, api_key, is_active \
             FROM cloud_profiles ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(rows
            .into_iter()
            .map(|(id, name, base_url, api_key, is_active)| CloudProfile {
                id,
                name,
                base_url,
                api_key,
                is_active,
            })
            .collect())
    }

    async fn get_active(&self) -> Result<CloudProfile, DomainError> {
        let row = sqlx::query_as::<_, (String, String, String, Option<String>, bool)>(
            "SELECT id, name, base_url, api_key, is_active \
             FROM cloud_profiles WHERE is_active = TRUE",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|(id, name, base_url, api_key, is_active)| CloudProfile {
            id,
            name,
            base_url,
            api_key,
            is_active,
        })
        .ok_or_else(|| DomainError::Storage("no active cloud profile".into()))
    }

    async fn set_active(&self, id: &str) -> Result<(), DomainError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        sqlx::query("UPDATE cloud_profiles SET is_active = FALSE WHERE is_active = TRUE")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        let result = sqlx::query(
            "UPDATE cloud_profiles \
             SET is_active = TRUE, last_used_at = now(), updated_at = now() \
             WHERE id = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            return Err(DomainError::Validation(format!("profile '{id}' not found")));
        }
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    async fn upsert(&self, profile: CloudProfile) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO cloud_profiles (id, name, base_url, api_key) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (id) DO UPDATE SET \
                 name = EXCLUDED.name, \
                 base_url = EXCLUDED.base_url, \
                 api_key = EXCLUDED.api_key, \
                 updated_at = now()",
        )
        .bind(&profile.id)
        .bind(&profile.name)
        .bind(&profile.base_url)
        .bind(&profile.api_key)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), DomainError> {
        if is_protected(id) {
            return Err(DomainError::Validation(format!("'{id}' cannot be deleted")));
        }
        sqlx::query("DELETE FROM cloud_profiles WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }
}

// ----------------------------------------------------------------------
// SQLite
// ----------------------------------------------------------------------

#[cfg(diary_sqlite)]
pub struct SqliteCloudProfileRepo {
    pool: sqlx::SqlitePool,
}

#[cfg(diary_sqlite)]
impl SqliteCloudProfileRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

#[cfg(diary_sqlite)]
#[async_trait]
impl CloudProfileRepository for SqliteCloudProfileRepo {
    async fn list(&self) -> Result<Vec<CloudProfile>, DomainError> {
        let rows = sqlx::query_as::<_, (String, String, String, Option<String>, i64)>(
            "SELECT id, name, base_url, api_key, is_active \
             FROM cloud_profiles ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(rows
            .into_iter()
            .map(|(id, name, base_url, api_key, is_active)| CloudProfile {
                id,
                name,
                base_url,
                api_key,
                is_active: is_active != 0,
            })
            .collect())
    }

    async fn get_active(&self) -> Result<CloudProfile, DomainError> {
        let row = sqlx::query_as::<_, (String, String, String, Option<String>, i64)>(
            "SELECT id, name, base_url, api_key, is_active \
             FROM cloud_profiles WHERE is_active = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|(id, name, base_url, api_key, is_active)| CloudProfile {
            id,
            name,
            base_url,
            api_key,
            is_active: is_active != 0,
        })
        .ok_or_else(|| DomainError::Storage("no active cloud profile".into()))
    }

    async fn set_active(&self, id: &str) -> Result<(), DomainError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        sqlx::query("UPDATE cloud_profiles SET is_active = 0 WHERE is_active = 1")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE cloud_profiles \
             SET is_active = 1, last_used_at = ?1, updated_at = ?1 \
             WHERE id = ?2",
        )
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            return Err(DomainError::Validation(format!("profile '{id}' not found")));
        }
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    async fn upsert(&self, profile: CloudProfile) -> Result<(), DomainError> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO cloud_profiles (id, name, base_url, api_key, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(id) DO UPDATE SET \
                 name = excluded.name, \
                 base_url = excluded.base_url, \
                 api_key = excluded.api_key, \
                 updated_at = excluded.updated_at",
        )
        .bind(&profile.id)
        .bind(&profile.name)
        .bind(&profile.base_url)
        .bind(&profile.api_key)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), DomainError> {
        if is_protected(id) {
            return Err(DomainError::Validation(format!("'{id}' cannot be deleted")));
        }
        sqlx::query("DELETE FROM cloud_profiles WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }
}

fn map_sqlx(e: sqlx::Error) -> DomainError {
    DomainError::Storage(format!("cloud_profile sqlx: {e}"))
}

/// Validate a base URL; rejects anything that isn't http/https.
pub fn validate_base_url(s: &str) -> Result<(), DomainError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(DomainError::Validation("base_url is empty".into()));
    }
    if !(s.starts_with("http://") || s.starts_with("https://")) {
        return Err(DomainError::Validation(
            "base_url must start with http:// or https://".into(),
        ));
    }
    url::Url::parse(s).map_err(|e| DomainError::Validation(format!("invalid url: {e}")))?;
    Ok(())
}

/// Construct the right backend implementation for the active build.
pub fn create_repo(pool: DbPool) -> std::sync::Arc<dyn CloudProfileRepository> {
    #[cfg(not(diary_sqlite))]
    {
        std::sync::Arc::new(PostgresCloudProfileRepo::new(pool))
    }
    #[cfg(diary_sqlite)]
    {
        std::sync::Arc::new(SqliteCloudProfileRepo::new(pool))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_ids_block_deletion() {
        assert!(is_protected("local"));
        assert!(is_protected("production"));
        assert!(!is_protected("custom"));
    }

    #[test]
    fn validate_base_url_accepts_http_and_https() {
        assert!(validate_base_url("http://localhost:5001").is_ok());
        assert!(validate_base_url("https://cloud.example.com").is_ok());
    }

    #[test]
    fn validate_base_url_rejects_garbage() {
        assert!(validate_base_url("").is_err());
        assert!(validate_base_url("ftp://x").is_err());
        assert!(validate_base_url("not a url").is_err());
    }
}
