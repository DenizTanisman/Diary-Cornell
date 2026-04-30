//! Reqwest wrapper around the Cloud REST surface.
//!
//! Two design rules:
//! 1. Methods take a token by reference rather than reaching for the
//!    AuthManager. Keeps the client testable in isolation and prevents an
//!    accidental refresh during a sync round-trip.
//! 2. Status-code handling lives here once. Anything 4xx/5xx becomes a
//!    typed `DomainError` so the engine can match instead of re-parsing.

use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use url::Url;
use uuid::Uuid;

use crate::error::DomainError;
use crate::sync::models::{
    CloudJournal, LoginRequest, PullResponse, PushRequest, PushResponse, RefreshRequest, TokenPair,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const SYNC_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct CloudClient {
    base: Url,
    http: Client,
}

impl CloudClient {
    pub fn new(base_url: &str) -> Result<Self, DomainError> {
        let base = base_url
            .parse::<Url>()
            .map_err(|e| DomainError::Path(format!("invalid CLOUD_URL: {e}")))?;
        let http = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .user_agent("cornell-diary/0.1 (sync-client)")
            .build()
            .map_err(|e| DomainError::Internal(format!("reqwest build: {e}")))?;
        Ok(Self { base, http })
    }

    /// Visible to tests so they can verify the client wires the right base.
    #[allow(dead_code)]
    pub(crate) fn base_url(&self) -> &Url {
        &self.base
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<TokenPair, DomainError> {
        let resp = self
            .http
            .post(self.base.join("auth/login").map_err(map_url)?)
            .json(&LoginRequest { email, password })
            .send()
            .await
            .map_err(|e| DomainError::Storage(format!("cloud login: {e}")))?;
        unwrap_json(resp).await
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenPair, DomainError> {
        let resp = self
            .http
            .post(self.base.join("auth/refresh").map_err(map_url)?)
            .json(&RefreshRequest { refresh_token })
            .send()
            .await
            .map_err(|e| DomainError::Storage(format!("cloud refresh: {e}")))?;
        unwrap_json(resp).await
    }

    pub async fn list_journals(&self, token: &str) -> Result<Vec<CloudJournal>, DomainError> {
        let resp = self
            .http
            .get(self.base.join("journals").map_err(map_url)?)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| DomainError::Storage(format!("cloud journals: {e}")))?;
        unwrap_json(resp).await
    }

    pub async fn create_journal(
        &self,
        token: &str,
        name: &str,
    ) -> Result<CloudJournal, DomainError> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            name: &'a str,
        }
        let resp = self
            .http
            .post(self.base.join("journals").map_err(map_url)?)
            .bearer_auth(token)
            .json(&Body { name })
            .send()
            .await
            .map_err(|e| DomainError::Storage(format!("cloud create_journal: {e}")))?;
        unwrap_json(resp).await
    }

    pub async fn pull(
        &self,
        token: &str,
        journal_id: Uuid,
        since: Option<DateTime<Utc>>,
    ) -> Result<PullResponse, DomainError> {
        let mut req = self
            .http
            .get(self.base.join("sync/pull").map_err(map_url)?)
            .bearer_auth(token)
            .timeout(SYNC_TIMEOUT)
            .query(&[("journal_id", journal_id.to_string())]);
        if let Some(since) = since {
            req = req.query(&[("since", since.to_rfc3339())]);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| DomainError::Storage(format!("cloud pull: {e}")))?;
        unwrap_json(resp).await
    }

    pub async fn push(&self, token: &str, body: &PushRequest) -> Result<PushResponse, DomainError> {
        let resp = self
            .http
            .post(self.base.join("sync/push").map_err(map_url)?)
            .bearer_auth(token)
            .timeout(SYNC_TIMEOUT)
            .json(body)
            .send()
            .await
            .map_err(|e| DomainError::Storage(format!("cloud push: {e}")))?;
        unwrap_json(resp).await
    }
}

async fn unwrap_json<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, DomainError> {
    let status = resp.status();
    if status.is_success() {
        return resp
            .json::<T>()
            .await
            .map_err(|e| DomainError::Storage(format!("decode response: {e}")));
    }
    let body = resp.text().await.unwrap_or_default();
    let msg = body.chars().take(200).collect::<String>();
    Err(match status {
        StatusCode::UNAUTHORIZED => DomainError::Validation(format!("cloud 401: {msg}")),
        StatusCode::TOO_MANY_REQUESTS => DomainError::Storage(format!("cloud 429: {msg}")),
        s if s.is_server_error() => DomainError::Storage(format!("cloud {s}: {msg}")),
        s => DomainError::Validation(format!("cloud {s}: {msg}")),
    })
}

fn map_url(e: url::ParseError) -> DomainError {
    DomainError::Path(format!("url join: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn login_posts_credentials_and_decodes_token_pair() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/auth/login")
            .match_body(mockito::Matcher::JsonString(
                r#"{"email":"a@b.c","password":"pw"}"#.into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"access_token":"acc","refresh_token":"ref","peer_id":"peer-1"}"#)
            .create_async()
            .await;

        let client = CloudClient::new(&format!("{}/", server.url())).unwrap();
        let tokens = client.login("a@b.c", "pw").await.unwrap();
        mock.assert_async().await;
        assert_eq!(tokens.access_token, "acc");
        assert_eq!(tokens.refresh_token, "ref");
        assert_eq!(tokens.peer_id.as_deref(), Some("peer-1"));
    }

    #[tokio::test]
    async fn login_401_becomes_validation_error() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/auth/login")
            .with_status(401)
            .with_body(r#"{"detail":"bad creds"}"#)
            .create_async()
            .await;

        let client = CloudClient::new(&format!("{}/", server.url())).unwrap();
        let err = client.login("a@b.c", "wrong").await.unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[tokio::test]
    async fn pull_passes_journal_id_and_since_as_query() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/sync/pull")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded(
                    "journal_id".into(),
                    "00000000-0000-0000-0000-000000000001".into(),
                ),
                mockito::Matcher::Regex("since=2026".into()),
            ]))
            .match_header("authorization", "Bearer test-token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"entries":[]}"#)
            .create_async()
            .await;

        let client = CloudClient::new(&format!("{}/", server.url())).unwrap();
        let since = chrono::DateTime::parse_from_rfc3339("2026-04-29T10:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let _ = client
            .pull(
                "test-token",
                Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
                Some(since),
            )
            .await
            .unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn push_serialises_body_and_returns_merged_entries() {
        use crate::sync::models::{MergedEntry, PushEntry};
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/sync/push")
            .match_header("authorization", "Bearer t")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"merged":[{"local_date":"2026-04-29","cloud_id":"00000000-0000-0000-0000-000000000002","version":2,"last_modified_at":"2026-04-29T10:00:00Z"}]}"#,
            )
            .create_async()
            .await;

        let client = CloudClient::new(&format!("{}/", server.url())).unwrap();
        let body = PushRequest {
            journal_id: Uuid::nil(),
            entries: vec![PushEntry {
                id: None,
                entry_date: "2026-04-29".into(),
                diary: "test".into(),
                title_1: None,
                content_1: None,
                title_2: None,
                content_2: None,
                title_3: None,
                content_3: None,
                title_4: None,
                content_4: None,
                title_5: None,
                content_5: None,
                title_6: None,
                content_6: None,
                title_7: None,
                content_7: None,
                summary: String::new(),
                quote: String::new(),
                version: 1,
                last_modified_at: chrono::Utc::now(),
            }],
        };
        let resp = client.push("t", &body).await.unwrap();
        mock.assert_async().await;
        assert_eq!(resp.merged.len(), 1);
        let m: &MergedEntry = &resp.merged[0];
        assert_eq!(m.local_date, "2026-04-29");
        assert_eq!(m.version, 2);
    }
}
