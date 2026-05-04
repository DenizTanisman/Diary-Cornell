//! LLM Bridge Tauri commands (MD 03 / Faz 3.2).
//!
//! Stateless wrapper around `BridgeClient`. Every call rebuilds the client
//! from the current `LlmSettings` so the user's URL/key edits in Settings
//! take effect immediately, no app restart required.

use std::sync::Arc;

use tauri::State;

use crate::db::llm_settings::{LlmSettings, LlmSettingsRepository};
use crate::error::DomainError;
use crate::sync::bridge_client::{
    BridgeClient, SentimentResponse, SummarizeResponse, TagResponse,
};

const MAX_INPUT_CHARS: usize = 50_000;

#[derive(Clone)]
pub struct LlmState {
    pub repo: Arc<dyn LlmSettingsRepository>,
}

#[tauri::command]
pub async fn llm_get_settings(state: State<'_, LlmState>) -> Result<LlmSettings, DomainError> {
    state.repo.get().await
}

#[tauri::command]
pub async fn llm_save_settings(
    settings: LlmSettings,
    state: State<'_, LlmState>,
) -> Result<(), DomainError> {
    if settings.bridge_url.trim().is_empty() {
        return Err(DomainError::Validation("bridge_url is empty".into()));
    }
    state.repo.upsert(settings).await
}

#[tauri::command]
pub async fn llm_health(state: State<'_, LlmState>) -> Result<bool, DomainError> {
    let s = state.repo.get().await?;
    if !s.enabled {
        return Ok(false);
    }
    let client = BridgeClient::new(&s.bridge_url, s.bridge_api_key)?;
    client.health().await
}

#[tauri::command]
pub async fn llm_summarize(
    text: String,
    style: String,
    language: String,
    state: State<'_, LlmState>,
) -> Result<SummarizeResponse, DomainError> {
    let s = require_enabled(&state).await?;
    validate_text(&text)?;
    let style = require_style(&style)?;
    let client = BridgeClient::new(&s.bridge_url, s.bridge_api_key)?;
    client.summarize(&text, style, &language).await
}

#[tauri::command]
pub async fn llm_tag(
    text: String,
    max_tags: u32,
    state: State<'_, LlmState>,
) -> Result<TagResponse, DomainError> {
    let s = require_enabled(&state).await?;
    validate_text(&text)?;
    if !(1..=10).contains(&max_tags) {
        return Err(DomainError::Validation("max_tags must be 1..=10".into()));
    }
    let client = BridgeClient::new(&s.bridge_url, s.bridge_api_key)?;
    client.tag(&text, max_tags).await
}

#[tauri::command]
pub async fn llm_sentiment(
    text: String,
    state: State<'_, LlmState>,
) -> Result<SentimentResponse, DomainError> {
    let s = require_enabled(&state).await?;
    validate_text(&text)?;
    let client = BridgeClient::new(&s.bridge_url, s.bridge_api_key)?;
    client.sentiment(&text).await
}

async fn require_enabled(state: &State<'_, LlmState>) -> Result<LlmSettings, DomainError> {
    let s = state.repo.get().await?;
    if !s.enabled {
        return Err(DomainError::Validation("LLM features are disabled".into()));
    }
    Ok(s)
}

fn validate_text(text: &str) -> Result<(), DomainError> {
    if text.trim().is_empty() {
        return Err(DomainError::Validation("text is empty".into()));
    }
    if text.len() > MAX_INPUT_CHARS {
        return Err(DomainError::Validation(format!(
            "text exceeds {MAX_INPUT_CHARS} chars"
        )));
    }
    Ok(())
}

fn require_style(style: &str) -> Result<&'static str, DomainError> {
    match style {
        "brief" => Ok("brief"),
        "detailed" => Ok("detailed"),
        "bullet" => Ok("bullet"),
        _ => Err(DomainError::Validation(
            "style must be brief|detailed|bullet".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_text_rejects_empty_and_huge() {
        assert!(validate_text("").is_err());
        assert!(validate_text("   ").is_err());
        let big = "a".repeat(MAX_INPUT_CHARS + 1);
        assert!(validate_text(&big).is_err());
        assert!(validate_text("hello").is_ok());
    }

    #[test]
    fn require_style_accepts_known_values() {
        assert!(require_style("brief").is_ok());
        assert!(require_style("detailed").is_ok());
        assert!(require_style("bullet").is_ok());
        assert!(require_style("tweetstorm").is_err());
    }
}
