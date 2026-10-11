use crate::error::CommandResult;
use crate::servers::Server;
use crate::settings::RelayRoute;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
#[cfg(feature = "steam")]
use tauri::Manager;
use tauri::{AppHandle, Emitter};
use tokio::net::TcpStream;
use tokio::sync::RwLock;

const PING_TIMEOUT: Duration = Duration::from_secs(3);

/// `RelayRoute::Auto` only picks the relay when it's at least this much faster than direct.
const AUTO_RELAY_MARGIN_MS: u32 = 5;

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ServerPingUpdate {
    pub pings: HashMap<String, Option<u32>>,
    /// URLs of servers that would be joined through their Steam relay.
    pub relayed: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ServerPingState {
    pings: RwLock<HashMap<String, Option<u32>>>,
    /// Estimated ping through the Steam relay, for servers that list one. Steam builds only.
    relay_pings: RwLock<HashMap<String, Option<u32>>>,
}

/// Whether to join a server that lists a relay over it, given its direct and relay pings.
pub fn use_relay(route: RelayRoute, direct: Option<u32>, relay: Option<u32>) -> bool {
    match route {
        RelayRoute::Never => false,
        RelayRoute::Always => true,
        RelayRoute::Auto => match (direct, relay) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(direct), Some(relay)) => relay.saturating_add(AUTO_RELAY_MARGIN_MS) <= direct,
        },
    }
}

impl ServerPingState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_pings(&self, route: RelayRoute) -> ServerPingUpdate {
        let direct = self.pings.read().await;
        let relay = self.relay_pings.read().await;
        let mut update = ServerPingUpdate {
            pings: direct.clone(),
            relayed: Vec::new(),
        };
        for (url, relay_ping) in relay.iter() {
            let direct_ping = direct.get(url).copied().flatten();
            if use_relay(route, direct_ping, *relay_ping) {
                update.pings.insert(url.clone(), *relay_ping);
                update.relayed.push(url.clone());
            }
        }
        update
    }

    /// Whether a server at `url` should be joined over its relay under `route`.
    #[cfg_attr(not(feature = "steam"), allow(dead_code))]
    pub async fn should_use_relay(&self, url: &str, route: RelayRoute) -> bool {
        let Some(relay) = self.relay_pings.read().await.get(url).copied() else {
            return false;
        };
        let direct = self.pings.read().await.get(url).copied().flatten();
        use_relay(route, direct, relay)
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
    app: AppHandle,
    state: tauri::State<'_, Arc<ServerPingState>>,
) -> CommandResult<ServerPingUpdate> {
    Ok(state.get_pings(relay_route(&app)).await)
}

pub fn relay_route(app: &AppHandle) -> RelayRoute {
    crate::settings::load_settings(app)
        .map(|s| s.relay_route)
        .unwrap_or_default()
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

    // Relay pings are estimates from Steam's ping data, not network round trips
    #[cfg(feature = "steam")]
    if handle
        .try_state::<Arc<crate::steam::SteamState>>()
        .is_some()
    {
        let relay_pings = servers
            .iter()
            .filter(|s| s.status == "available")
            .filter_map(|s| {
                let relay = s.relay.as_ref()?;
                let ping = ss13relay_proto::steam::estimate_ping_ms(&relay.ping_location);
                Some((s.url.clone(), ping))
            })
            .collect();
        *ping_state.relay_pings.write().await = relay_pings;
    }

    let update = ping_state.get_pings(relay_route(handle)).await;
    let _ = handle.emit("server-pings-updated", update);
}
