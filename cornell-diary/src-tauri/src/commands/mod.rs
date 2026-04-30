//! Tauri commands the React frontend invokes via `@tauri-apps/api/core::invoke`.
//! Mirrors the `IDiaryRepository` TypeScript contract one-to-one so the
//! frontend's existing repository abstraction can swap implementations
//! without touching React components.

pub mod entries;
pub mod sync;
