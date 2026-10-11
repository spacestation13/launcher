//! Steam builds: joins servers through their ss13relay over Steam Datagram Relay.
//!
//! Session is one P2P connection to the server's relay, with a local TCP listener that DreamSeeker
//! connects to. Connection persists after reboots/reconnects, so proxy port has to remain open

// DreamSeeker is only launched on Windows and Linux.
#![cfg_attr(not(any(target_os = "windows", target_os = "linux")), allow(dead_code))]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use ss13relay_proto::driver::{Connection, Driver, DriverEvent, Mode};
use ss13relay_proto::pump::{self, StreamInput};
use ss13relay_proto::steam::LinkConfig;
use ss13relay_proto::{CloseReason, Frame, StreamId, PROTOCOL_VERSION};
use steamworks::SteamId;
use tauri::{AppHandle, Manager};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

use crate::presence::PresenceManager;
use crate::server_ping::ServerPingState;
use crate::servers::{RelayInfo, ServerState};
use crate::settings::RelayRoute;
use crate::steam::SteamState;

/// How long a new connection may take before falling back to a direct connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_mins(1);
const IDLE_CHECK_INTERVAL: Duration = Duration::from_secs(5);
const STREAM_INPUT_CAPACITY: usize = 64;

#[derive(Default)]
pub struct RelayProxyState {
    session: tokio::sync::Mutex<Option<RelaySession>>,
}

struct RelaySession {
    server_id: String,
    port: u16,
    alive: Arc<AtomicBool>,
    /// Dropping this stops the session.
    _stop: oneshot::Sender<()>,
}

impl RelaySession {
    fn is_live_for(&self, server_id: &str) -> bool {
        self.server_id == server_id && self.alive.load(Ordering::SeqCst)
    }
}

/// The local proxy port to point `DreamSeeker` at for `server_id`, or `None` to connect directly.
///
/// Reuses the server's live session if there is one (even if the listing has briefly lost the
/// relay), otherwise connects to the listed relay when the route setting picks it. Any other
/// session is closed.
pub async fn route(app: &AppHandle, server_id: &str) -> Option<u16> {
    let state = app.try_state::<Arc<RelayProxyState>>()?;
    let mut session = state.session.lock().await;

    let route = crate::server_ping::relay_route(app);
    if route == RelayRoute::Never {
        *session = None;
        return None;
    }
    if let Some(existing) = session.as_ref().filter(|s| s.is_live_for(server_id)) {
        tracing::info!(
            server_id,
            port = existing.port,
            "reusing Steam relay session"
        );
        return Some(existing.port);
    }
    *session = None;

    let steam = app.try_state::<Arc<SteamState>>()?;
    let server = app
        .try_state::<Arc<ServerState>>()?
        .get_servers()
        .await
        .into_iter()
        .find(|s| s.id == server_id)?;
    let relay = server.relay.as_ref()?;
    let wanted = app
        .try_state::<Arc<ServerPingState>>()?
        .should_use_relay(&server.url, route)
        .await;
    if !wanted {
        return None;
    }

    match RelaySession::start(app, &steam, server_id, relay).await {
        Ok(new) => {
            let port = new.port;
            *session = Some(new);
            Some(port)
        }
        Err(e) => {
            tracing::warn!(server_id, %e, "couldn't connect through the Steam relay, connecting directly");
            None
        }
    }
}

/// The proxy port of `server_id`'s live session, without starting one.
pub async fn active_port(app: &AppHandle, server_id: &str) -> Option<u16> {
    let state = app.try_state::<Arc<RelayProxyState>>()?;
    let session = state.session.lock().await;
    session
        .as_ref()
        .filter(|s| s.is_live_for(server_id))
        .map(|s| s.port)
}

/// Close the current session, if any.
pub async fn close(app: &AppHandle) {
    if let Some(state) = app.try_state::<Arc<RelayProxyState>>() {
        *state.session.lock().await = None;
    }
}

impl RelaySession {
    async fn start(
        app: &AppHandle,
        steam: &SteamState,
        server_id: &str,
        relay: &RelayInfo,
    ) -> Result<RelaySession, String> {
        let remote = relay
            .steam_id
            .parse::<u64>()
            .map(SteamId::from_raw)
            .map_err(|e| format!("invalid relay SteamID {}: {e}", relay.steam_id))?;

        let started = Instant::now();
        let (events_tx, mut events) = mpsc::unbounded_channel();

        let driver = Driver::spawn(
            steam.client().networking_sockets(),
            || {},
            Mode::Connect {
                remote,
                virtual_port: i32::from(relay.virtual_port),
            },
            LinkConfig::default(),
            events_tx,
        )
        .map_err(|e| format!("couldn't start the Steam networking thread: {e}"))?;

        let Connection {
            mut inbound,
            outbound,
            ..
        } = match tokio::time::timeout(CONNECT_TIMEOUT, events.recv()).await {
            Ok(Some(DriverEvent::Connected(connection))) => connection,
            Ok(Some(DriverEvent::ConnectFailed { reason })) => return Err(reason),
            Ok(_) => return Err("Steam networking thread stopped".into()),
            Err(_) => return Err(format!("not connected after {CONNECT_TIMEOUT:?}")),
        };

        outbound
            .send(
                Frame::Hello {
                    version: PROTOCOL_VERSION,
                }
                .encode(),
            )
            .await
            .map_err(|_| "connection closed before HELLO")?;
        match tokio::time::timeout(HELLO_TIMEOUT, inbound.recv()).await {
            Ok(Some(bytes))
                if Frame::decode(&bytes)
                    == Ok(Frame::Hello {
                        version: PROTOCOL_VERSION,
                    }) => {}
            other => return Err(format!("relay didn't answer HELLO: {other:?}")),
        }

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("couldn't open the local proxy: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("couldn't open the local proxy: {e}"))?
            .port();

        tracing::info!(
            server_id,
            relay = %relay.steam_id,
            virtual_port = relay.virtual_port,
            port,
            elapsed = ?started.elapsed(),
            "connected to Steam relay"
        );

        let alive = Arc::new(AtomicBool::new(true));
        let (stop_tx, stop_rx) = oneshot::channel();
        tauri::async_runtime::spawn(serve(
            app.clone(),
            SteamLink {
                driver,
                events,
                inbound,
                outbound,
            },
            listener,
            stop_rx,
            Arc::clone(&alive),
        ));

        Ok(RelaySession {
            server_id: server_id.to_string(),
            port,
            alive,
            _stop: stop_tx,
        })
    }
}

struct StreamRoute {
    opened: Option<oneshot::Sender<Result<(), CloseReason>>>,
    input: mpsc::Sender<StreamInput>,
}

type Routes = Arc<Mutex<HashMap<StreamId, StreamRoute>>>;

struct SteamLink {
    driver: Driver,
    events: mpsc::UnboundedReceiver<DriverEvent>,
    inbound: mpsc::Receiver<Vec<u8>>,
    outbound: mpsc::Sender<Vec<u8>>,
}

async fn serve(
    app: AppHandle,
    link: SteamLink,
    listener: TcpListener,
    mut stop: oneshot::Receiver<()>,
    alive: Arc<AtomicBool>,
) {
    let SteamLink {
        driver,
        mut events,
        inbound,
        outbound,
    } = link;
    let routes: Routes = Arc::default();
    let open_streams = Arc::new(AtomicUsize::new(0));
    let mut mux = tokio::spawn(route_inbound(inbound, Arc::clone(&routes)));
    let mut next_stream: StreamId = 1;
    let mut last_active = Instant::now();
    let mut idle_check = tokio::time::interval(IDLE_CHECK_INTERVAL);

    let reason = loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((tcp, _)) => {
                    let stream = next_stream;
                    next_stream = next_stream.wrapping_add(1);
                    last_active = Instant::now();
                    tracing::info!(stream, "DreamSeeker connected, opening relay stream");
                    tokio::spawn(run_stream(
                        stream,
                        tcp,
                        Arc::clone(&routes),
                        outbound.clone(),
                        Arc::clone(&open_streams),
                    ));
                }
                Err(e) => break format!("local proxy failed: {e}"),
            },
            event = events.recv() => match event {
                Some(DriverEvent::Closed { reason, .. }) => break format!("Steam connection closed: {reason}"),
                None => break "Steam networking thread stopped".into(),
                Some(_) => {}
            },
            _ = &mut mux => break "Steam connection closed".into(),
            _ = &mut stop => break "closed by the launcher".into(),
            _ = idle_check.tick() => {
                if open_streams.load(Ordering::SeqCst) > 0 || game_running(&app) {
                    last_active = Instant::now();
                } else if last_active.elapsed() >= IDLE_TIMEOUT {
                    break format!("DreamSeeker gone and no connections for {IDLE_TIMEOUT:?}");
                }
            }
        }
    };

    alive.store(false, Ordering::SeqCst);
    tracing::info!(%reason, "Steam relay session ended");
    mux.abort();
    drop(outbound);
    // Joins the Steam networking thread, which notices within a millisecond or so.
    driver.shutdown();
}

fn game_running(app: &AppHandle) -> bool {
    app.try_state::<Arc<PresenceManager>>()
        .is_some_and(|manager| manager.check_game_running())
}

async fn route_inbound(mut inbound: mpsc::Receiver<Vec<u8>>, routes: Routes) {
    while let Some(bytes) = inbound.recv().await {
        let frame = match Frame::decode(&bytes) {
            Ok(frame) => frame,
            Err(e) => {
                tracing::warn!(%e, "malformed frame from relay");
                continue;
            }
        };
        match frame {
            Frame::Opened { stream } => {
                if let Some(opened) = routes.lock().get_mut(&stream).and_then(|r| r.opened.take()) {
                    let _ = opened.send(Ok(()));
                }
            }
            Frame::Data { stream, data } => {
                let input = routes.lock().get(&stream).map(|r| r.input.clone());
                if let Some(input) = input {
                    let _ = input.send(StreamInput::Data(data)).await;
                }
            }
            Frame::Close { stream, reason } => {
                let route = routes.lock().remove(&stream);
                if let Some(mut route) = route {
                    match route.opened.take() {
                        Some(opened) => {
                            let _ = opened.send(Err(reason));
                        }
                        None => {
                            let _ = route.input.send(StreamInput::Close(reason)).await;
                        }
                    }
                }
            }
            other => tracing::warn!(?other, "unexpected frame from relay"),
        }
    }
}

async fn run_stream(
    stream: StreamId,
    tcp: TcpStream,
    routes: Routes,
    outbound: mpsc::Sender<Vec<u8>>,
    open_streams: Arc<AtomicUsize>,
) {
    open_streams.fetch_add(1, Ordering::SeqCst);
    let (opened_tx, opened_rx) = oneshot::channel();
    let (input_tx, input_rx) = mpsc::channel(STREAM_INPUT_CAPACITY);
    routes.lock().insert(
        stream,
        StreamRoute {
            opened: Some(opened_tx),
            input: input_tx,
        },
    );

    if outbound.send(Frame::Open { stream }.encode()).await.is_ok() {
        match opened_rx.await {
            Ok(Ok(())) => {
                tracing::info!(stream, "relay stream opened");
                let stats = pump::pump(stream, tcp, input_rx, outbound).await;
                tracing::info!(
                    stream,
                    end = ?stats.end,
                    sent = stats.to_steam,
                    received = stats.from_steam,
                    "relay stream ended"
                );
            }
            Ok(Err(reason)) => tracing::warn!(stream, ?reason, "relay refused the stream"),
            Err(_) => {}
        }
    }

    routes.lock().remove(&stream);
    open_streams.fetch_sub(1, Ordering::SeqCst);
}
