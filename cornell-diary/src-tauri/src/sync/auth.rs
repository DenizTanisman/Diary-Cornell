//! JWT-token cache that lazily refreshes when the access token is about to
//! expire.
//!
//! We deliberately do **not** verify JWT signatures here — Cloud verifies on
//! every request, so doing it twice is just attack-surface (we'd need its
//! signing key locally). We only decode the payload to read `exp` and decide
//! whether to call `/auth/refresh`.

use std::sync::Arc;

use chrono::{DateTime, Duration, TimeZone, Utc};
use jsonwebtoken::DecodingKey;
use serde::Deserialize;
use sqlx::PgPool;
use tokio::sync::Mutex;

use crate::error::DomainError;
use crate::sync::client::CloudClient;
use crate::sync::meta;

/// Minimum slack we want before token expiry. We refresh anything that
/// would expire within this window so a single sync round-trip never
/// straddles the cutoff.
pub const REFRESH_WINDOW_SECS: i64 = 60;

#[derive(Debug, Deserialize)]
struct JwtClaims {
    /// Standard JWT exp claim — seconds since epoch.
    exp: Option<i64>,
}

pub struct AuthManager {
    pool: PgPool,
    /// Single in-flight refresh at a time. Two concurrent sync_engine
    /// triggers shouldn't both call /auth/refresh.
    refresh_lock: Mutex<()>,
}

impl AuthManager {
    pub fn new(pool: PgPool) -> Arc<Self> {
        Arc::new(Self {
            pool,
            refresh_lock: Mutex::new(()),
        })
    }

    /// Visible to the scheduler / network monitor (FAZ 2.2) so they can
    /// piggyback on the same pool the auth path uses without rebuilding one.
    #[allow(dead_code)]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Returns a usable access token, refreshing it if necessary. Holds an
    /// internal lock so concurrent calls don't both hit /auth/refresh.
    pub async fn get_or_refresh(&self, client: &CloudClient) -> Result<String, DomainError> {
        let metadata = meta::read(&self.pool).await?;
        let access = metadata
            .access_token
            .as_deref()
            .ok_or(DomainError::Validation("not connected to cloud".into()))?;
        let exp = jwt_expiry(access);

        // Use the cached expiry if it's already past the window; otherwise
        // peek at the JWT itself in case the DB column was missing.
        let now = Utc::now();
        let should_refresh = match metadata.token_expires_at.or(exp) {
            Some(when) => when <= now + Duration::seconds(REFRESH_WINDOW_SECS),
            None => false,
        };

        if !should_refresh {
            return Ok(access.to_string());
        }

        let _guard = self.refresh_lock.lock().await;
        // Re-read in case another task refreshed while we were queued.
        let metadata = meta::read(&self.pool).await?;
        let still_stale = match metadata.token_expires_at {
            Some(when) => when <= now + Duration::seconds(REFRESH_WINDOW_SECS),
            None => true,
        };
        if !still_stale {
            return metadata
                .access_token
                .ok_or(DomainError::Validation("not connected".into()));
        }

        let refresh_token = metadata
            .refresh_token
            .as_deref()
            .ok_or(DomainError::Validation("refresh token missing".into()))?;
        let pair = client.refresh(refresh_token).await?;

        let new_exp = pair.expires_at.or_else(|| jwt_expiry(&pair.access_token));
        meta::save_tokens(
            &self.pool,
            &pair.access_token,
            &pair.refresh_token,
            new_exp,
            pair.user_id,
            pair.peer_id.as_deref(),
            None,
        )
        .await?;
        Ok(pair.access_token)
    }

    /// True if a non-empty access token exists.
    pub async fn is_connected(&self) -> Result<bool, DomainError> {
        Ok(meta::read(&self.pool).await?.access_token.is_some())
    }
}

/// Decode a JWT and return its `exp` as a UTC `DateTime`, without verifying
/// the signature.
fn jwt_expiry(token: &str) -> Option<DateTime<Utc>> {
    let mut validation = jsonwebtoken::Validation::default();
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.required_spec_claims.clear();
    let key = DecodingKey::from_secret(b"unused-because-validation-disabled");
    let data = jsonwebtoken::decode::<JwtClaims>(token, &key, &validation).ok()?;
    let exp = data.claims.exp?;
    Utc.timestamp_opt(exp, 0).single()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pre-encoded JWT — payload `{"exp":1900000000}`. Avoids pulling base64
    /// into dev-deps just for this test.
    /// header `{"alg":"HS256","typ":"JWT"}` → eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9
    /// payload `{"exp":1900000000}`            → eyJleHAiOjE5MDAwMDAwMDB9
    const JWT_EXP_1_900_000_000: &str =
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJleHAiOjE5MDAwMDAwMDB9.placeholder";

    #[test]
    fn jwt_expiry_reads_exp_field() {
        let got = jwt_expiry(JWT_EXP_1_900_000_000).unwrap();
        assert_eq!(got.timestamp(), 1_900_000_000);
    }

    #[test]
    fn jwt_expiry_returns_none_for_garbage() {
        assert!(jwt_expiry("not-a-jwt").is_none());
        assert!(jwt_expiry("a.b.c").is_none());
    }
}
