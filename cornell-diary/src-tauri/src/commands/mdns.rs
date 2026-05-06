//! Sprint D-3 — mDNS / Bonjour service for LAN-local Cloud discovery.
//!
//! Two complementary roles share this module:
//!
//! - **Mac side (advertising)**: when Cloud Service starts, we publish
//!   `_corneldiary._tcp.local.` on the LAN with the user's hostname so
//!   any other device on the same network can find this Mac without
//!   anyone typing an IP address.
//! - **Phone side (browsing)**: the Cloud Profile screen calls
//!   `discover_cloud_servers` to browse the same service type for a
//!   few seconds and offers the discovered servers as one-tap profile
//!   additions.
//!
//! The advertising lifecycle is wired into `cloud_service::start_impl`
//! / `cloud_service::stop_cloud_service` so users don't see "advertise
//! on LAN" as a separate setting — it's tied to whether Cloud is up.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::error::DomainError;

const SERVICE_TYPE: &str = "_corneldiary._tcp.local.";
const CLOUD_PORT: u16 = 5001;

#[derive(Default, Clone)]
pub struct MdnsState {
    inner: Arc<Mutex<MdnsInner>>,
}

#[derive(Default)]
struct MdnsInner {
    /// One persistent ServiceDaemon per process. mdns-sd holds an
    /// Arc internally so cloning is cheap; we keep the slot so the
    /// browse path on a phone-side build can reuse the same socket.
    daemon: Option<ServiceDaemon>,
    /// Set while we have an active advertisement; lets stop_advertising
    /// be idempotent.
    advertised_fullname: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredService {
    /// Instance label as advertised — typically "<hostname> Diary Cloud".
    pub name: String,
    /// First reachable IPv4 + the advertised port, ready to drop into
    /// the Cloud Profile URL field as `http://<addr>:<port>`.
    pub url: String,
    pub port: u16,
    pub addresses: Vec<String>,
}

/// Start advertising — called from `cloud_service::start_impl` after
/// uvicorn comes up. Idempotent: a second call while already advertising
/// is a no-op.
pub async fn start_advertising(state: &MdnsState) -> Result<(), DomainError> {
    let mut inner = state.inner.lock().await;
    if inner.advertised_fullname.is_some() {
        return Ok(());
    }

    let daemon = match &inner.daemon {
        Some(d) => d.clone(),
        None => {
            let d = ServiceDaemon::new()
                .map_err(|e| DomainError::Internal(format!("mdns daemon: {e}")))?;
            inner.daemon = Some(d.clone());
            d
        }
    };

    let lan_addrs = collect_lan_ipv4()?;
    if lan_addrs.is_empty() {
        // No LAN — nothing to advertise. Quietly skip; the user will
        // still see "Cloud aktif" in the panel.
        tracing::info!(target: "cornell_diary::mdns", "skipped: no LAN address");
        return Ok(());
    }

    // hostname() avoids pulling another crate; falls back to a
    // generic when the OS doesn't expose one.
    let host_label = std::env::var("HOST")
        .or_else(|_| std::env::var("HOSTNAME"))
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "diary".to_string());
    let instance_name = format!("{host_label} Diary Cloud");
    let host_name = format!("{host_label}.local.");

    let mut props: HashMap<String, String> = HashMap::new();
    props.insert("path".to_string(), "/".to_string());
    props.insert("schema".to_string(), "http".to_string());

    // mdns-sd's AsIpAddrs accepts &[String]/&[&str]; pass the IPv4
    // addresses through to_string so the trait bound resolves.
    let addr_strs: Vec<String> = lan_addrs.iter().map(|a| a.to_string()).collect();
    let info = ServiceInfo::new(
        SERVICE_TYPE,
        &instance_name,
        &host_name,
        &addr_strs[..],
        CLOUD_PORT,
        Some(props),
    )
    .map_err(|e| DomainError::Internal(format!("service info: {e}")))?;

    let fullname = info.get_fullname().to_string();
    daemon
        .register(info)
        .map_err(|e| DomainError::Internal(format!("mdns register: {e}")))?;
    inner.advertised_fullname = Some(fullname);

    tracing::info!(
        target: "cornell_diary::mdns",
        instance = %instance_name,
        port = CLOUD_PORT,
        "advertising Cloud on mDNS"
    );
    Ok(())
}

/// Stop advertising — paired with `cloud_service::stop_cloud_service`.
/// Leaves the daemon alive in case the user re-starts; only releases
/// the daemon at process exit.
pub async fn stop_advertising(state: &MdnsState) -> Result<(), DomainError> {
    let mut inner = state.inner.lock().await;
    // Lift the take() out of the if-let so the immutable borrow on
    // `inner.daemon` and the mutable take on `inner.advertised_fullname`
    // don't overlap.
    let advertised = inner.advertised_fullname.take();
    if let (Some(daemon), Some(name)) = (inner.daemon.as_ref(), advertised) {
        let _ = daemon.unregister(&name);
        tracing::info!(target: "cornell_diary::mdns", "stopped advertising");
    }
    Ok(())
}

/// Browse the LAN for `_corneldiary._tcp.local.` and return whatever
/// we resolve before the timeout. Caller picks one entry and drops it
/// into the Cloud Profile URL field.
///
/// Each call spins up a fresh daemon so a phone that hasn't advertised
/// anything (just a client) still gets the multicast socket bound
/// correctly. Daemon shuts down before we return so we don't leak
/// background threads on every browse.
#[tauri::command]
pub async fn discover_cloud_servers(
    timeout_ms: u64,
) -> Result<Vec<DiscoveredService>, DomainError> {
    let daemon = ServiceDaemon::new()
        .map_err(|e| DomainError::Internal(format!("mdns daemon: {e}")))?;
    let receiver = daemon
        .browse(SERVICE_TYPE)
        .map_err(|e| DomainError::Internal(format!("mdns browse: {e}")))?;

    // ServiceDaemon's receiver is sync (crossbeam-style). Wrap the
    // recv_timeout calls in spawn_blocking so we don't block the
    // tauri async runtime.
    let timeout = Duration::from_millis(timeout_ms.clamp(500, 10_000));
    let services = tokio::task::spawn_blocking(move || {
        let mut found: HashMap<String, DiscoveredService> = HashMap::new();
        let deadline = std::time::Instant::now() + timeout;
        while let Some(remaining) =
            deadline.checked_duration_since(std::time::Instant::now())
        {
            match receiver.recv_timeout(remaining) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    let addrs: Vec<String> = info
                        .get_addresses()
                        .iter()
                        .filter(|a| !a.is_loopback() && !a.is_unspecified())
                        .map(|a| a.to_string())
                        .collect();
                    if addrs.is_empty() {
                        continue;
                    }
                    let port = info.get_port();
                    let url = format!("http://{}:{}", addrs[0], port);
                    let name = info
                        .get_fullname()
                        .trim_end_matches(SERVICE_TYPE)
                        .trim_end_matches('.')
                        .to_string();
                    found.insert(
                        info.get_fullname().to_string(),
                        DiscoveredService {
                            name,
                            url,
                            port,
                            addresses: addrs,
                        },
                    );
                }
                Err(_) => break, // timeout or channel closed
                _ => {}          // ServiceFound / SearchStarted / etc.
            }
        }
        found
    })
    .await
    .map_err(|e| DomainError::Internal(format!("mdns task join: {e}")))?;

    let _ = daemon.shutdown();
    let mut out: Vec<DiscoveredService> = services.into_values().collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn collect_lan_ipv4() -> Result<Vec<std::net::Ipv4Addr>, DomainError> {
    let ifaces = local_ip_address::list_afinet_netifas()
        .map_err(|e| DomainError::Internal(format!("list interfaces: {e}")))?;
    let mut out: Vec<std::net::Ipv4Addr> = ifaces
        .into_iter()
        .filter_map(|(_, ip)| match ip {
            std::net::IpAddr::V4(v4)
                if !v4.is_loopback()
                    && !v4.is_link_local()
                    && !is_docker_bridge(v4) =>
            {
                Some(v4)
            }
            _ => None,
        })
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

fn is_docker_bridge(ip: std::net::Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 172 && b == 17
}
