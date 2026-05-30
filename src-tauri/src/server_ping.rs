use crate::error::CommandResult;
use crate::servers::Server;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpStream;
use tokio::sync::RwLock;

const PING_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ServerPingUpdate {
    pub pings: HashMap<String, Option<u32>>,
}

#[derive(Debug, Default)]
pub struct ServerPingState {
    pings: RwLock<HashMap<String, Option<u32>>>,
}

impl ServerPingState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_pings(&self) -> HashMap<String, Option<u32>> {
        self.pings.read().await.clone()
    }

    async fn set_ping(&self, url: &str, ping: Option<u32>) {
        self.pings.write().await.insert(url.to_string(), ping);
    }

    async fn has_ping(&self, url: &str) -> bool {
        self.pings.read().await.contains_key(url)
    }

    async fn remove_stale(&self, active_urls: &HashSet<&str>) {
        self.pings
            .write()
            .await
            .retain(|url, _| active_urls.contains(url.as_str()));
    }
}

fn parse_host_port(url: &str) -> Option<(String, u16)> {
    let address = url.strip_prefix("byond://")?;
    let (host, port_str) = address.rsplit_once(':')?;
    let port = port_str.parse::<u16>().ok()?;
    Some((host.to_string(), port))
}

#[allow(clippy::cast_possible_truncation)]
async fn tcp_ping(host: &str, port: u16) -> Option<u32> {
    let addr = format!("{host}:{port}");
    let start = Instant::now();
    let result = tokio::time::timeout(PING_TIMEOUT, TcpStream::connect(&addr)).await;

    match result {
        Ok(Ok(_)) => Some(start.elapsed().as_millis() as u32),
        _ => None,
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_server_pings(
    state: tauri::State<'_, Arc<ServerPingState>>,
) -> CommandResult<HashMap<String, Option<u32>>> {
    Ok(state.get_pings().await)
}

pub async fn ping_servers(
    handle: &AppHandle,
    ping_state: &Arc<ServerPingState>,
    servers: &[Server],
) {
    if crate::config::get_config().features.relay_selector {
        return;
    }

    let online_urls: HashSet<&str> = servers
        .iter()
        .filter(|s| s.status == "available")
        .map(|s| s.url.as_str())
        .collect();

    ping_state.remove_stale(&online_urls).await;

    let futures: Vec<_> = servers
        .iter()
        .filter(|s| s.status == "available")
        .filter_map(|s| {
            let (host, port) = parse_host_port(&s.url)?;
            let url = s.url.clone();
            let ping_state = Arc::clone(ping_state);
            Some(async move {
                if ping_state.has_ping(&url).await {
                    return;
                }
                let ping = tcp_ping(&host, port).await;
                ping_state.set_ping(&url, ping).await;
            })
        })
        .collect();

    futures_util::future::join_all(futures).await;

    let pings = ping_state.get_pings().await;
    let _ = handle.emit("server-pings-updated", ServerPingUpdate { pings });
}
