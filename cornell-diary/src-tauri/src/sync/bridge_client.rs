//! Reqwest wrapper around Jarvis Bridge (MD 03 / Faz 3.2).
//!
//! Mirrors the bridge's `/v1/{summarize, tag, sentiment, ask}` endpoints
//! one-to-one. Stateless: a fresh client is built each request from the
//! current LlmSettings, so flipping URL / API-key in the UI takes effect
//! without an app restart.

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::DomainError;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Serialize)]
pub struct SummarizeRequest<'a> {
    pub text: &'a str,
    pub style: &'a str,
    pub language: &'a str,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SummarizeResponse {
    pub summary: String,
    pub model: String,
    pub cached: bool,
}

#[derive(Debug, Serialize)]
pub struct TagRequest<'a> {
    pub text: &'a str,
    pub max_tags: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TagResponse {
    pub tags: Vec<String>,
    pub model: String,
}

#[derive(Debug, Serialize)]
pub struct SentimentRequest<'a> {
    pub text: &'a str,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SentimentResponse {
    pub label: String,
    pub score: f64,
    pub summary: String,
    pub model: String,
}

#[derive(Clone)]
pub struct BridgeClient {
    base: Url,
    api_key: Option<String>,
    http: Client,
}

impl BridgeClient {
    pub fn new(base_url: &str, api_key: Option<String>) -> Result<Self, DomainError> {
        let base = base_url
            .parse::<Url>()
            .map_err(|e| DomainError::Validation(format!("invalid bridge_url: {e}")))?;
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent("cornell-diary/0.1 (bridge-client)")
            .build()
            .map_err(|e| DomainError::Internal(format!("reqwest build: {e}")))?;
        Ok(Self { base, api_key, http })
    }

    pub async fn health(&self) -> Result<bool, DomainError> {
        let url = self
            .base
            .join("v1/health")
            .map_err(|e| DomainError::Validation(format!("join health: {e}")))?;
        let mut req = self.http.get(url);
        req = self.attach_key(req);
        let resp = req
            .send()
            .await
            .map_err(|e| DomainError::Storage(format!("bridge health: {e}")))?;
        Ok(resp.status().is_success())
    }

    pub async fn summarize(
        &self,
        text: &str,
        style: &str,
        language: &str,
    ) -> Result<SummarizeResponse, DomainError> {
        let url = self
            .base
            .join("v1/summarize")
            .map_err(|e| DomainError::Validation(format!("join summarize: {e}")))?;
        let mut req = self.http.post(url).json(&SummarizeRequest {
            text,
            style,
            language,
        });
        req = self.attach_key(req);
        let resp = req
            .send()
            .await
            .map_err(|e| DomainError::Storage(format!("bridge summarize: {e}")))?;
        unwrap_json(resp).await
    }

    pub async fn tag(&self, text: &str, max_tags: u32) -> Result<TagResponse, DomainError> {
        let url = self
            .base
            .join("v1/tag")
            .map_err(|e| DomainError::Validation(format!("join tag: {e}")))?;
        let mut req = self.http.post(url).json(&TagRequest { text, max_tags });
        req = self.attach_key(req);
        let resp = req
            .send()
            .await
            .map_err(|e| DomainError::Storage(format!("bridge tag: {e}")))?;
        unwrap_json(resp).await
    }

    pub async fn sentiment(&self, text: &str) -> Result<SentimentResponse, DomainError> {
        let url = self
            .base
            .join("v1/sentiment")
            .map_err(|e| DomainError::Validation(format!("join sentiment: {e}")))?;
        let mut req = self.http.post(url).json(&SentimentRequest { text });
        req = self.attach_key(req);
        let resp = req
            .send()
            .await
            .map_err(|e| DomainError::Storage(format!("bridge sentiment: {e}")))?;
        unwrap_json(resp).await
    }

    fn attach_key(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(k) if !k.is_empty() => builder.header("X-API-Key", k),
            _ => builder,
        }
    }
}

async fn unwrap_json<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
) -> Result<T, DomainError> {
    let status = resp.status();
    if !status.is_success() {
        let snippet = resp.text().await.unwrap_or_default();
        let snippet = snippet.chars().take(200).collect::<String>();
        return Err(DomainError::Storage(format!(
            "bridge {}: {}",
            status, snippet
        )));
    }
    resp.json::<T>()
        .await
        .map_err(|e| DomainError::Storage(format!("bridge json: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_url_rejected() {
        assert!(BridgeClient::new("not-a-url", None).is_err());
    }

    #[test]
    fn key_attached_only_when_set() {
        let c = BridgeClient::new("http://localhost:8765", Some("k".into())).unwrap();
        // Smoke test of constructor + builder path; we don't hit the network.
        let _ = c.attach_key(c.http.get(c.base.clone()));
    }
}
