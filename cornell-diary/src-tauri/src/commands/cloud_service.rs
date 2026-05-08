//! Spawn / supervise the local Cloud sync server from Diary.
//!
//! Diary needs Cloud running on `:5001` to sync. Today users start it
//! by hand from a terminal — this module exposes Tauri commands that
//! Diary's UI can call instead:
//!
//! - `start_cloud_service` — `docker compose up -d postgres` then
//!   `uvicorn src.main:app --port 5001` as a managed child process.
//! - `stop_cloud_service` — kills the child + (optionally) the postgres
//!   container if Diary started it.
//! - `cloud_service_status` — reports `idle` / `starting` / `running`
//!   / `error`. The UI polls this every second while the user watches
//!   the start animation.
//!
//! The Cloud project path defaults to `~/Projects/Cloud`. When that
//! path doesn't exist, the start command returns a friendly error
//! instead of crashing the child.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, MutexGuard};

use crate::commands::mdns::{self, MdnsState};
use crate::error::DomainError;

const DEFAULT_CLOUD_DIR: &str = "Projects/Cloud";
const CLOUD_PORT: u16 = 5001;

#[derive(Default, Clone)]
pub struct CloudServiceState {
    inner: Arc<Mutex<CloudInner>>,
}

#[derive(Default)]
struct CloudInner {
    child: Option<Child>,
    /// Did we run `start_postgres.sh` ourselves? If yes, stop will tear
    /// it down too. If postgres was already up before we touched it,
    /// we leave it running.
    started_postgres: bool,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudServiceStatus {
    /// "idle" | "starting" | "running" | "error"
    pub state: String,
    pub pid: Option<u32>,
    pub last_error: Option<String>,
    pub healthy: bool,
}

fn cloud_dir() -> PathBuf {
    // `~` expansion without pulling in the `dirs` crate. `$HOME` is the
    // canonical way on Unix; on macOS the Tauri parent always has it
    // set. Falling back to a relative path means the start command
    // bails out cleanly with a "klasör bulunamadı" error if we somehow
    // run without HOME.
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(DEFAULT_CLOUD_DIR)
    } else {
        PathBuf::from(DEFAULT_CLOUD_DIR)
    }
}

/// Cheap port probe. uvicorn only binds `:5001` after FastAPI's startup
/// events succeed, so a successful TCP connect == Cloud is ready to
/// serve. This used to be an HTTP GET on `/health/live`, but reqwest's
/// connect/build cost made the 1.5s panel-poll occasionally race past
/// the 2s timeout on slow launch. TCP is single-syscall and never
/// reaches that cliff.
async fn cloud_listening() -> bool {
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tokio::net::TcpStream::connect(("127.0.0.1", CLOUD_PORT)),
        )
        .await,
        Ok(Ok(_))
    )
}

/// Auto-start helper used by `lib.rs` setup when the
/// `auto_start_cloud_on_launch` toggle is on. Takes `&CloudServiceState`
/// directly (no Tauri State wrapper) so it can be called before the
/// state is `app.manage(...)`-d.
pub async fn start_cloud_service_internal(
    state: &CloudServiceState,
    mdns_state: &MdnsState,
) -> Result<CloudServiceStatus, DomainError> {
    start_impl(state, mdns_state).await
}

#[tauri::command]
pub async fn start_cloud_service(
    state: State<'_, CloudServiceState>,
    mdns_state: State<'_, MdnsState>,
) -> Result<CloudServiceStatus, DomainError> {
    start_impl(&state, &mdns_state).await
}

async fn start_impl(
    state: &CloudServiceState,
    mdns_state: &MdnsState,
) -> Result<CloudServiceStatus, DomainError> {
    let mut inner = state.inner.lock().await;

    // Already running and healthy → no-op.
    if inner.child.is_some() && cloud_listening().await {
        return Ok(snapshot(&inner, true).await);
    }

    let dir = cloud_dir();
    if !dir.exists() {
        let msg = format!(
            "Cloud klasörü bulunamadı: {} — proje yolunu doğrula.",
            dir.display()
        );
        inner.last_error = Some(msg.clone());
        return Err(DomainError::Validation(msg));
    }
    let venv_uvicorn = dir.join(".venv/bin/uvicorn");
    if !venv_uvicorn.exists() {
        let msg = format!(
            "Cloud venv kurulu değil: {} mevcut değil.",
            venv_uvicorn.display()
        );
        inner.last_error = Some(msg.clone());
        return Err(DomainError::Validation(msg));
    }

    // Bring up Postgres if it's not already exposing :5434. We use the
    // bundled start_postgres.sh because it knows the right `.env`
    // password and waits for the container to become ready.
    let postgres_was_up = postgres_listening(5434).await;
    if !postgres_was_up {
        let postgres_script = dir.join("scripts/start_postgres.sh");
        if !postgres_script.exists() {
            let msg = format!(
                "scripts/start_postgres.sh bulunamadı: {}",
                postgres_script.display()
            );
            inner.last_error = Some(msg.clone());
            return Err(DomainError::Validation(msg));
        }
        tracing::info!(target: "cornell_diary::cloud_service", "starting postgres via start_postgres.sh");
        let postgres_status = Command::new("bash")
            .arg(postgres_script)
            .current_dir(&dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|e| DomainError::Internal(format!("postgres script: {e}")))?;
        if !postgres_status.success() {
            let msg = format!("postgres script exit code {postgres_status:?}");
            inner.last_error = Some(msg.clone());
            return Err(DomainError::Storage(msg));
        }
        inner.started_postgres = true;
    }

    // Spawn uvicorn as a managed child. We deliberately don't capture
    // stdout/stderr — they go to /dev/null so the parent's pipe buffers
    // can't fill up and stall the child. Logs land in Cloud's own log
    // file (or stdout if the user wants more, they can set RUST_LOG /
    // tail the process).
    let mut cmd = Command::new(venv_uvicorn);
    cmd.arg("src.main:app")
        .arg("--host")
        .arg("0.0.0.0")
        .arg("--port")
        .arg("5001")
        .current_dir(&dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let child = cmd
        .spawn()
        .map_err(|e| DomainError::Internal(format!("uvicorn spawn: {e}")))?;
    let pid = child.id();
    inner.child = Some(child);
    inner.last_error = None;
    tracing::info!(
        target: "cornell_diary::cloud_service",
        pid = ?pid,
        "Cloud uvicorn spawned"
    );

    // Don't wait for /health/live here — the UI polls. We just confirm
    // the process is actually still alive a moment later (catches an
    // immediate-exit failure like "port already in use").
    drop(inner);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let inner = state.inner.lock().await;
    let healthy = cloud_listening().await;

    // Once the listener is up, advertise on mDNS so phones on the LAN
    // can find us. Failure to advertise should NEVER block the Cloud
    // start path — log and move on.
    if healthy {
        if let Err(e) = mdns::start_advertising(mdns_state).await {
            tracing::warn!(
                target: "cornell_diary::cloud_service",
                error = %e,
                "mdns advertise failed (non-fatal)"
            );
        }
    }

    Ok(snapshot(&inner, healthy).await)
}

#[tauri::command]
pub async fn stop_cloud_service(
    state: State<'_, CloudServiceState>,
    mdns_state: State<'_, MdnsState>,
) -> Result<CloudServiceStatus, DomainError> {
    // Pull the mdns advertise down first; clients shouldn't see us in
    // the discovery list while the uvicorn process is in the middle of
    // tearing down.
    let _ = mdns::stop_advertising(&mdns_state).await;

    let mut inner = state.inner.lock().await;
    if let Some(mut child) = inner.child.take() {
        // `kill_on_drop` would handle this on its own, but explicit
        // kill + wait gives us a deterministic exit before snapshot.
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    // Fallback: if Cloud was started outside Diary (the user ran
    // `uvicorn src.main:app …` from a terminal), the child handle is
    // None and the kill above was a no-op. The user still pressed
    // Stop expecting :5001 to die — find any external listener on
    // that port and SIGTERM it (SIGKILL after 800ms if it ignores).
    // Mac/Linux only; on Windows we log and move on.
    if cloud_listening().await {
        kill_external_listener(5001).await;
    }
    if inner.started_postgres {
        // Best-effort: if postgres is unique to Diary's needs, stop the
        // container so we don't leave it running when Diary exits.
        let dir = cloud_dir();
        let stop_script = dir.join("scripts/stop_postgres.sh");
        if stop_script.exists() {
            let _ = Command::new("bash")
                .arg(stop_script)
                .current_dir(&dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
        inner.started_postgres = false;
    }
    Ok(snapshot(&inner, false).await)
}

#[tauri::command]
pub async fn cloud_service_status(
    state: State<'_, CloudServiceState>,
) -> Result<CloudServiceStatus, DomainError> {
    let inner = state.inner.lock().await;
    let healthy = cloud_listening().await;
    Ok(snapshot(&inner, healthy).await)
}

/// Sprint D-1 — list every IPv4 address the host owns that another
/// device on the LAN could reach Cloud through. Skips loopback,
/// link-local (169.254.x.x — DHCP fallback that nobody can route to),
/// and interfaces whose *name* identifies them as virtual bridges or
/// VPN tunnels (Docker, VirtualBox, VMware, …).
///
/// The phone-from-Mac flow needs this: `localhost:5001` works on the
/// Mac itself, but a phone has to dial the Mac's actual LAN address
/// (e.g. `192.168.1.5:5001`). We surface the candidates so the user
/// can copy one into their phone's Cloud Profile without digging
/// through System Settings → Network.
#[tauri::command]
pub fn get_lan_addresses() -> Result<Vec<String>, DomainError> {
    let ifaces = local_ip_address::list_afinet_netifas()
        .map_err(|e| DomainError::Internal(format!("list interfaces: {e}")))?;

    let mut addrs: Vec<String> = ifaces
        .into_iter()
        .filter_map(|(name, ip)| match ip {
            std::net::IpAddr::V4(v4) if !is_excluded_interface(&name) => Some(v4),
            // IPv6 link-local addresses are common on Mac (fe80::…) but
            // require a zone identifier to be reachable; not useful for
            // a manual "type this into your phone" flow.
            _ => None,
        })
        .filter(|v4| !v4.is_loopback() && !v4.is_link_local())
        .map(|v4| v4.to_string())
        .collect();
    addrs.sort();
    addrs.dedup();
    Ok(addrs)
}

/// Filter virtual / container / tunnel interfaces by name, not by IP
/// range. Earlier versions blanket-suppressed the entire 172.16/12
/// RFC1918 block to dodge Docker bridges; that backfired the moment
/// the user joined a mobile hotspot whose DHCP scope happens to live
/// in 172.x (e.g. iOS personal hotspot 172.20.10/28, certain Android
/// hotspots in 172.18). Real LAN, real Wi-Fi, can't reach Cloud
/// because we hid the only routable address.
///
/// Interface NAME is the right signal: `docker0`, `br-*`, `bridge*`,
/// `vmnet*`, `vboxnet*`, `utun*`, `tun*`, `tap*`, `awdl*`, `llw*`,
/// `lo*` are the virtual / tunnel families across macOS + Linux.
/// Real LAN interfaces use `en*` (macOS), `eth*`/`enp*` (Linux),
/// `wl*`/`wlan*`/`wlp*` (Wi-Fi), and pass through unchanged.
pub(crate) fn is_excluded_interface(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with("docker")
        || n.starts_with("br-")
        || n.starts_with("bridge")
        || n.starts_with("vmnet")
        || n.starts_with("vboxnet")
        || n.starts_with("utun")
        || n.starts_with("tun")
        || n.starts_with("tap")
        || n.starts_with("awdl")
        || n.starts_with("llw")
        || n.starts_with("lo")
}

/// Best-effort: kill any process listening on `port` that wasn't
/// spawned by Diary. Used as a stop_cloud_service fallback when a
/// user starts uvicorn directly from a terminal — Diary's Stop
/// button still does what it says on the tin. Unix-only (lsof +
/// kill); on other platforms it's a no-op + warning.
async fn kill_external_listener(port: u16) {
    #[cfg(unix)]
    {
        use std::time::Duration;
        let probe = || async move {
            tokio::process::Command::new("lsof")
                .args([
                    "-nP",
                    &format!("-iTCP:{port}"),
                    "-sTCP:LISTEN",
                    "-t",
                ])
                .output()
                .await
        };
        let send = |sig: &str, pid: &str| {
            let sig = sig.to_string();
            let pid = pid.to_string();
            async move {
                let _ = tokio::process::Command::new("kill")
                    .args([sig.as_str(), pid.as_str()])
                    .status()
                    .await;
            }
        };
        let Ok(out) = probe().await else { return };
        let stdout = String::from_utf8_lossy(&out.stdout);
        for pid in stdout.lines().filter(|l| !l.trim().is_empty()) {
            tracing::info!(target: "cornell_diary", pid = %pid, "stop_cloud: SIGTERM external listener");
            send("-TERM", pid).await;
        }
        // Give the process a beat to wind down; SIGKILL holdouts.
        tokio::time::sleep(Duration::from_millis(800)).await;
        let Ok(still) = probe().await else { return };
        let still_out = String::from_utf8_lossy(&still.stdout);
        for pid in still_out.lines().filter(|l| !l.trim().is_empty()) {
            tracing::warn!(target: "cornell_diary", pid = %pid, "stop_cloud: SIGKILL external listener (TERM ignored)");
            send("-KILL", pid).await;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = port;
        tracing::warn!(target: "cornell_diary", "stop_cloud: external-listener kill not implemented on this platform");
    }
}

async fn snapshot(inner: &MutexGuard<'_, CloudInner>, healthy: bool) -> CloudServiceStatus {
    let pid = inner.child.as_ref().and_then(|c| c.id());
    let state = match (pid, healthy, inner.last_error.is_some()) {
        (_, _, true) if inner.child.is_none() => "error",
        (Some(_), true, _) => "running",
        (Some(_), false, _) => "starting",
        (None, true, _) => "running", // someone else started Cloud — surface it as healthy
        (None, false, _) => "idle",
    };
    CloudServiceStatus {
        state: state.to_string(),
        pid,
        last_error: inner.last_error.clone(),
        healthy,
    }
}

async fn postgres_listening(port: u16) -> bool {
    tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_ok()
}
