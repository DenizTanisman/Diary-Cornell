//! Cloud profile Tauri commands (MD 03 / Faz 3.1).
//!
//! `set_active_profile` does NOT swap the live `CloudClient` in place —
//! it persists the choice, clears auth, and flips a `pending_restart`
//! flag the UI watches. Next launch, lib.rs reads the active profile
//! and starts up against the new base URL. See PROGRESS_TRACKER karar
//! günlüğü 2026-05-04 [03.1.4] for the rationale.

use std::sync::Arc;

use tauri::State;

use crate::commands::sync::SyncState;
use crate::db::cloud_profile::{validate_base_url, CloudProfile, CloudProfileRepository};
use crate::error::DomainError;

#[derive(Clone)]
pub struct ProfileState {
    pub repo: Arc<dyn CloudProfileRepository>,
}

#[tauri::command]
pub async fn list_cloud_profiles(
    state: State<'_, ProfileState>,
) -> Result<Vec<CloudProfile>, DomainError> {
    state.repo.list().await
}

#[tauri::command]
pub async fn get_active_cloud_profile(
    state: State<'_, ProfileState>,
) -> Result<CloudProfile, DomainError> {
    state.repo.get_active().await
}

#[tauri::command]
pub async fn set_active_cloud_profile(
    id: String,
    profile_state: State<'_, ProfileState>,
    sync_state: State<'_, SyncState>,
) -> Result<(), DomainError> {
    profile_state.repo.set_active(&id).await?;
    // Clear auth so the now-stale tokens don't accidentally hit the new
    // profile's URL on the next sync attempt. Errors here are non-fatal —
    // the user already disconnected if there was no active session.
    if let Err(e) = sync_state.engine.disconnect().await {
        tracing::warn!(target: "cornell_diary", error = %e, "disconnect on profile switch");
    }
    Ok(())
}

#[tauri::command]
pub async fn upsert_cloud_profile(
    profile: CloudProfile,
    state: State<'_, ProfileState>,
) -> Result<(), DomainError> {
    if profile.id.trim().is_empty() {
        return Err(DomainError::Validation("profile id is empty".into()));
    }
    if profile.name.trim().is_empty() {
        return Err(DomainError::Validation("profile name is empty".into()));
    }
    // Empty base_url allowed only for the seeded `production` row that the
    // user fills in later; everything else must be a valid http(s) URL.
    if !profile.base_url.is_empty() {
        validate_base_url(&profile.base_url)?;
        if profile.base_url.starts_with("http://") && !is_localish(&profile.base_url) {
            tracing::warn!(
                target: "cornell_diary",
                url = %profile.base_url,
                "non-https profile URL"
            );
        }
    }
    state.repo.upsert(profile).await
}

#[tauri::command]
pub async fn delete_cloud_profile(
    id: String,
    state: State<'_, ProfileState>,
) -> Result<(), DomainError> {
    state.repo.delete(&id).await
}

fn is_localish(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("://localhost")
        || lower.contains("://127.0.0.1")
        || lower.contains("://10.")
        || lower.contains("://192.168.")
        || lower.contains("://172.")
        || lower.contains("://10.0.2.2") // android emulator host
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localish_classifier() {
        assert!(is_localish("http://localhost:5001"));
        assert!(is_localish("http://127.0.0.1:5001"));
        assert!(is_localish("http://192.168.1.50:5001"));
        assert!(is_localish("http://10.0.2.2:5001"));
        assert!(!is_localish("http://example.com:5001"));
        assert!(!is_localish("https://cloud.example.com"));
    }
}
