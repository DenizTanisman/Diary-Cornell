//! LLM settings repository (MD 03 / Faz 3.2).
//!
//! Singleton table — every read/write targets `id = 1`. The table is seeded
//! by migration 0006 so `get()` always returns a row on a healthy install.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::db::pool::DbPool;
use crate::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmSettings {
    pub enabled: bool,
    pub bridge_url: String,
    pub bridge_api_key: Option<String>,
    pub auto_summarize: bool,
    pub auto_tag: bool,
    pub preferred_language: String,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bridge_url: "http://localhost:8765".to_string(),
            bridge_api_key: None,
            auto_summarize: false,
            auto_tag: false,
            preferred_language: "auto".to_string(),
        }
    }
}

#[async_trait]
pub trait LlmSettingsRepository: Send + Sync {
    async fn get(&self) -> Result<LlmSettings, DomainError>;
    async fn upsert(&self, settings: LlmSettings) -> Result<(), DomainError>;
}

// ----------------------------------------------------------------------
// Postgres
// ----------------------------------------------------------------------

#[cfg(not(diary_sqlite))]
pub struct PostgresLlmSettingsRepo {
    pool: sqlx::PgPool,
}

#[cfg(not(diary_sqlite))]
impl PostgresLlmSettingsRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[cfg(not(diary_sqlite))]
#[async_trait]
impl LlmSettingsRepository for PostgresLlmSettingsRepo {
    async fn get(&self) -> Result<LlmSettings, DomainError> {
        let row = sqlx::query_as::<_, (bool, String, Option<String>, bool, bool, String)>(
            "SELECT enabled, bridge_url, bridge_api_key, auto_summarize, \
                    auto_tag, preferred_language FROM llm_settings WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row
            .map(
                |(enabled, bridge_url, bridge_api_key, auto_summarize, auto_tag, preferred_language)| {
                    LlmSettings {
                        enabled,
                        bridge_url,
                        bridge_api_key,
                        auto_summarize,
                        auto_tag,
                        preferred_language,
                    }
                },
            )
            .unwrap_or_default())
    }

    async fn upsert(&self, s: LlmSettings) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO llm_settings (id, enabled, bridge_url, bridge_api_key, \
                                       auto_summarize, auto_tag, preferred_language, updated_at) \
             VALUES (1, $1, $2, $3, $4, $5, $6, now()) \
             ON CONFLICT (id) DO UPDATE SET \
                 enabled = EXCLUDED.enabled, \
                 bridge_url = EXCLUDED.bridge_url, \
                 bridge_api_key = EXCLUDED.bridge_api_key, \
                 auto_summarize = EXCLUDED.auto_summarize, \
                 auto_tag = EXCLUDED.auto_tag, \
                 preferred_language = EXCLUDED.preferred_language, \
                 updated_at = now()",
        )
        .bind(s.enabled)
        .bind(&s.bridge_url)
        .bind(&s.bridge_api_key)
        .bind(s.auto_summarize)
        .bind(s.auto_tag)
        .bind(&s.preferred_language)
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
pub struct SqliteLlmSettingsRepo {
    pool: sqlx::SqlitePool,
}

#[cfg(diary_sqlite)]
impl SqliteLlmSettingsRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

#[cfg(diary_sqlite)]
#[async_trait]
impl LlmSettingsRepository for SqliteLlmSettingsRepo {
    async fn get(&self) -> Result<LlmSettings, DomainError> {
        let row = sqlx::query_as::<_, (i64, String, Option<String>, i64, i64, String)>(
            "SELECT enabled, bridge_url, bridge_api_key, auto_summarize, \
                    auto_tag, preferred_language FROM llm_settings WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row
            .map(
                |(enabled, bridge_url, bridge_api_key, auto_summarize, auto_tag, preferred_language)| {
                    LlmSettings {
                        enabled: enabled != 0,
                        bridge_url,
                        bridge_api_key,
                        auto_summarize: auto_summarize != 0,
                        auto_tag: auto_tag != 0,
                        preferred_language,
                    }
                },
            )
            .unwrap_or_default())
    }

    async fn upsert(&self, s: LlmSettings) -> Result<(), DomainError> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO llm_settings (id, enabled, bridge_url, bridge_api_key, \
                                       auto_summarize, auto_tag, preferred_language, updated_at) \
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(id) DO UPDATE SET \
                 enabled = excluded.enabled, \
                 bridge_url = excluded.bridge_url, \
                 bridge_api_key = excluded.bridge_api_key, \
                 auto_summarize = excluded.auto_summarize, \
                 auto_tag = excluded.auto_tag, \
                 preferred_language = excluded.preferred_language, \
                 updated_at = excluded.updated_at",
        )
        .bind(if s.enabled { 1 } else { 0 })
        .bind(&s.bridge_url)
        .bind(&s.bridge_api_key)
        .bind(if s.auto_summarize { 1 } else { 0 })
        .bind(if s.auto_tag { 1 } else { 0 })
        .bind(&s.preferred_language)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
}

fn map_sqlx(e: sqlx::Error) -> DomainError {
    DomainError::Storage(format!("llm_settings sqlx: {e}"))
}

pub fn create_repo(pool: DbPool) -> std::sync::Arc<dyn LlmSettingsRepository> {
    #[cfg(not(diary_sqlite))]
    {
        std::sync::Arc::new(PostgresLlmSettingsRepo::new(pool))
    }
    #[cfg(diary_sqlite)]
    {
        std::sync::Arc::new(SqliteLlmSettingsRepo::new(pool))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_are_disabled() {
        let s = LlmSettings::default();
        assert!(!s.enabled);
        assert_eq!(s.bridge_url, "http://localhost:8765");
        assert!(s.bridge_api_key.is_none());
        assert_eq!(s.preferred_language, "auto");
    }
}
