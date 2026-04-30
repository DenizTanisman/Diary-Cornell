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

    /// Force a `/auth/refresh` round-trip regardless of the cached
    /// `token_expires_at`. Used reactively by `with_retry` when Cloud
    /// answers a request with 401 mid-flight (server restart, manual
    /// revoke, …) — `get_or_refresh` would have returned the cached
    /// token unchanged because its `exp` looked fine.
    pub async fn force_refresh(&self, client: &CloudClient) -> Result<String, DomainError> {
        let _guard = self.refresh_lock.lock().await;
        let metadata = meta::read(&self.pool).await?;
        let refresh_token = metadata
            .refresh_token
            .as_deref()
            .ok_or(DomainError::Validation(
                "refresh token missing — re-connect".into(),
            ))?;
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

/// Returns true iff `e` is the validation error `unwrap_json` produces
/// for a Cloud 401 response.
pub fn is_unauthorized(e: &DomainError) -> bool {
    matches!(e, DomainError::Validation(s) if s.contains("cloud 401"))
}

/// Run `op` with the current access token. If the call comes back with
/// a Cloud 401, force-refresh tokens and retry exactly once. Used by
/// every authenticated REST call in `SyncEngine` so a token revoked /
/// rotated mid-session doesn't surface to the user.
///
/// The closure is called with `(CloudClient, token)` so callers can move
/// the cheap-Clone `CloudClient` into the closure body without a borrow
/// conflict on `&self.client`.
pub async fn with_retry<F, Fut, T>(
    auth: &AuthManager,
    client: CloudClient,
    mut op: F,
) -> Result<T, DomainError>
where
    F: FnMut(CloudClient, String) -> Fut,
    Fut: std::future::Future<Output = Result<T, DomainError>>,
{
    let token = auth.get_or_refresh(&client).await?;
    match op(client.clone(), token).await {
        Err(e) if is_unauthorized(&e) => {
            tracing::info!(
                target: "cornell_diary::sync",
                "cloud 401 mid-flight — force-refreshing token and retrying once"
            );
            let new_token = auth.force_refresh(&client).await?;
            op(client, new_token).await
        }
        other => other,
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
    use crate::db::pool::{build_pool, run_migrations};
    use chrono::Duration as ChronoDuration;
    use serial_test::serial;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

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

    #[test]
    fn is_unauthorized_only_matches_cloud_401_validation() {
        assert!(is_unauthorized(&DomainError::Validation(
            "cloud 401: token_invalid".into()
        )));
        // 422 / generic validation must NOT trigger a refresh — that would
        // turn every malformed-payload error into an extra refresh round-trip.
        assert!(!is_unauthorized(&DomainError::Validation(
            "cloud 422: missing field".into()
        )));
        assert!(!is_unauthorized(&DomainError::Storage("network".into())));
    }

    /// Tests for the reactive 401 path. Use a real Postgres pool +
    /// mockito Cloud server so the full force-refresh + retry round
    /// trip is exercised end-to-end.
    async fn fresh_pool_with_token(
        access: &str,
        refresh: &str,
        exp: chrono::DateTime<Utc>,
    ) -> Option<sqlx::PgPool> {
        let url = crate::db::test_helpers::test_database_url()?;
        let pool = build_pool(&url).await.ok()?;
        run_migrations(&pool).await.ok()?;
        sqlx::query("TRUNCATE sync_metadata RESTART IDENTITY CASCADE")
            .execute(&pool)
            .await
            .ok()?;
        sqlx::query(
            "INSERT INTO sync_metadata \
             (id, peer_id, access_token, refresh_token, token_expires_at, sync_enabled) \
             VALUES (1, $1, $2, $3, $4, TRUE)",
        )
        .bind("alice@laptop")
        .bind(access)
        .bind(refresh)
        .bind(exp)
        .execute(&pool)
        .await
        .ok()?;
        Some(pool)
    }

    #[tokio::test]
    #[serial(postgres)]
    async fn with_retry_force_refreshes_on_cloud_401_and_retries() {
        let exp = Utc::now() + ChronoDuration::hours(1);
        let Some(pool) = fresh_pool_with_token("stale-access", "good-refresh", exp).await else {
            eprintln!("skipping with_retry_force_refreshes — DATABASE_URL not reachable");
            return;
        };

        let mut server = mockito::Server::new_async().await;
        // Cloud's /auth/refresh hands back a fresh token pair.
        let refresh_mock = server
            .mock("POST", "/auth/refresh")
            .match_body(mockito::Matcher::PartialJson(serde_json::json!({
                "refresh_token": "good-refresh"
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "access_token": "fresh-access",
                    "refresh_token": "good-refresh-rotated",
                    "expires_at": null,
                    "user_id": null,
                    "peer_id": null
                })
                .to_string(),
            )
            .create_async()
            .await;

        let client = CloudClient::new(&server.url()).expect("client builds against mockito");
        let auth = AuthManager::new(pool);

        // The closure simulates a CloudClient call: first call sees the
        // stale token and returns a 401 validation error; second call
        // succeeds when given the fresh one.
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let observed_tokens = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));

        let result: Result<&'static str, DomainError> = with_retry(&auth, client, {
            let calls = calls.clone();
            let observed_tokens = observed_tokens.clone();
            move |_c, token| {
                let calls = calls.clone();
                let observed_tokens = observed_tokens.clone();
                async move {
                    observed_tokens.lock().unwrap().push(token.clone());
                    let n = calls.fetch_add(1, AtomicOrdering::SeqCst);
                    if n == 0 {
                        // First attempt — server says token is bad.
                        Err(DomainError::Validation("cloud 401: token_invalid".into()))
                    } else {
                        Ok("ok")
                    }
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "ok");
        assert_eq!(
            calls.load(AtomicOrdering::SeqCst),
            2,
            "must retry exactly once"
        );
        // Snapshot the recorder under the lock, drop it, then await — clippy
        // (correctly) refuses a MutexGuard held across an await point.
        let observed: Vec<String> = observed_tokens.lock().unwrap().clone();
        assert_eq!(observed[0], "stale-access");
        assert_eq!(observed[1], "fresh-access");
        refresh_mock.assert_async().await;
    }

    #[tokio::test]
    #[serial(postgres)]
    async fn with_retry_does_not_retry_non_401_errors() {
        let exp = Utc::now() + ChronoDuration::hours(1);
        let Some(pool) = fresh_pool_with_token("a", "r", exp).await else {
            eprintln!("skipping with_retry_no_retry — DATABASE_URL not reachable");
            return;
        };
        let server = mockito::Server::new_async().await;
        let client = CloudClient::new(&server.url()).unwrap();
        let auth = AuthManager::new(pool);

        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let result: Result<&'static str, DomainError> = with_retry(&auth, client, {
            let calls = calls.clone();
            move |_c, _token| {
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, AtomicOrdering::SeqCst);
                    Err(DomainError::Validation("cloud 422: missing field".into()))
                }
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(
            calls.load(AtomicOrdering::SeqCst),
            1,
            "422 must surface immediately without a refresh-and-retry round-trip"
        );
    }
}
