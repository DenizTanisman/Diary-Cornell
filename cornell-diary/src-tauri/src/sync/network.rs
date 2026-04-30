//! Cloud reachability monitor.
//!
//! A 30-second probe loop hits Cloud `/health`. When the state flips
//! offline → online we proactively trigger a full sync — covers the
//! "laptop opened in a cafe" case without waiting for the next hourly tick.
//!
//! No OS network APIs are used; the loop is pure HTTP polling so the same
//! code works on macOS, Linux, Windows, iOS, Android.

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use tokio::sync::watch;

use crate::sync::SyncEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkState {
    Unknown,
    Online,
    Offline,
}

#[derive(Clone)]
pub struct NetworkMonitor {
    state_rx: watch::Receiver<NetworkState>,
}

impl NetworkMonitor {
    pub fn snapshot(&self) -> NetworkState {
        *self.state_rx.borrow()
    }

    pub fn is_online(&self) -> bool {
        matches!(self.snapshot(), NetworkState::Online)
    }
}

const PROBE_INTERVAL: Duration = Duration::from_secs(30);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Spawn the probe loop. Returns a NetworkMonitor handle that callers can
/// query for the current state. The task runs until the runtime exits;
/// FAZ 2.2 doesn't expose a stop button (Tauri kills the runtime on app
/// quit, which is enough).
pub fn start(cloud_url: String, engine: Arc<SyncEngine>) -> NetworkMonitor {
    let (tx, rx) = watch::channel(NetworkState::Unknown);
    tokio::spawn(probe_loop(cloud_url, engine, tx));
    NetworkMonitor { state_rx: rx }
}

async fn probe_loop(cloud_url: String, engine: Arc<SyncEngine>, tx: watch::Sender<NetworkState>) {
    let client = match Client::builder().timeout(PROBE_TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: "cornell_diary::sync", error = %e, "network monitor disabled — reqwest build failed");
            return;
        }
    };
    let health_url = format!("{}/health", cloud_url.trim_end_matches('/'));
    let mut state = NetworkState::Unknown;
    let mut consecutive_failures = 0u32;

    loop {
        let online = client
            .get(&health_url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        let next = if online {
            consecutive_failures = 0;
            NetworkState::Online
        } else {
            consecutive_failures = consecutive_failures.saturating_add(1);
            // Three consecutive failures before we declare offline — keeps
            // the chip from flickering on a single dropped packet.
            if consecutive_failures >= 3 {
                NetworkState::Offline
            } else {
                state
            }
        };

        if next != state {
            tracing::info!(
                target: "cornell_diary::sync",
                ?state,
                ?next,
                "network state transition"
            );
            let _ = tx.send(next);
            // Coming online → kick off a full sync. We deliberately fire
            // and forget; the engine handles its own auth/online checks
            // and any error logs come from there.
            if matches!(next, NetworkState::Online) && !matches!(state, NetworkState::Online) {
                let engine = engine.clone();
                tokio::spawn(async move {
                    if let Err(e) = engine.run_full_cycle().await {
                        tracing::info!(
                            target: "cornell_diary::sync",
                            error = %e,
                            "network-triggered sync skipped"
                        );
                    }
                });
            }
            state = next;
        }

        tokio::time::sleep(PROBE_INTERVAL).await;
    }
}
