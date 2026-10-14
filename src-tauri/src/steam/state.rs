#![allow(clippy::unwrap_used)] // Mutex unwraps are idiomatic - poisoned mutex is unrecoverable

use std::sync::{Arc, Mutex};
use std::time::Duration;
use steamworks::{
    AuthTicket, Client, GameRichPresenceJoinRequested, NewUrlLaunchParameters,
    TicketForWebApiResponse,
};
use tauri::Emitter;
use tokio::sync::oneshot;

use super::parse_connect_target;

/// Emitted with a connect target when Steam asks a running launcher to join a server
pub const STEAM_JOIN_EVENT: &str = "steam-join-requested";

/// Manages Steam client state and authentication
pub struct SteamState {
    client: Client,
    active_ticket: Arc<Mutex<Option<AuthTicket>>>,
    /// Channel sender for pending web API ticket response
    pending_ticket_tx: Arc<Mutex<Option<oneshot::Sender<TicketForWebApiResponse>>>>,
    /// Callback handles kept alive for the lifetime of `SteamState`
    _callback_handle: steamworks::CallbackHandle,
    join_callback_handles: Mutex<Vec<steamworks::CallbackHandle>>,
}

impl SteamState {
    pub fn init() -> Result<Self, steamworks::SteamAPIInitError> {
        tracing::debug!("Initializing Steam client");
        let client = Client::init()?;

        // Start measuring relay pings now, so server list ping estimates and the first relayed
        // connection don't wait on it.
        ss13relay_proto::steam::install_debug_output(false);
        ss13relay_proto::steam::LinkConfig::default().apply_globally();
        client.networking_utils().init_relay_network_access();

        let pending_ticket_tx: Arc<Mutex<Option<oneshot::Sender<TicketForWebApiResponse>>>> =
            Arc::new(Mutex::new(None));

        let pending_tx_clone = Arc::clone(&pending_ticket_tx);
        let callback_handle = client.register_callback(move |response: TicketForWebApiResponse| {
            tracing::debug!("Received TicketForWebApiResponse callback");
            let mut pending = pending_tx_clone.lock().unwrap();
            if let Some(tx) = pending.take() {
                let _ = tx.send(response);
            }
        });

        Ok(Self {
            client,
            active_ticket: Arc::new(Mutex::new(None)),
            pending_ticket_tx,
            _callback_handle: callback_handle,
            join_callback_handles: Mutex::new(Vec::new()),
        })
    }

    pub fn get_steam_id(&self) -> u64 {
        self.client.user().steam_id().raw()
    }

    pub fn get_display_name(&self) -> String {
        self.client.friends().name()
    }

    pub async fn get_auth_session_ticket(&self) -> crate::error::CommandResult<Vec<u8>> {
        use crate::error::CommandError;

        {
            let mut active = self.active_ticket.lock().unwrap();
            *active = None;
        }

        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending_ticket_tx.lock().unwrap();
            *pending = Some(tx);
        }

        let ticket_handle = self
            .client
            .user()
            .authentication_session_ticket_for_webapi("");

        tracing::debug!("Requested web API auth ticket, waiting for callback...");

        let response = tokio::time::timeout(Duration::from_secs(10), rx)
            .await
            .map_err(|_| CommandError::Timeout {
                operation: "steam auth ticket".to_string(),
            })?
            .map_err(|_| {
                CommandError::Internal("Steam auth ticket channel closed unexpectedly".to_string())
            })?;

        if response.result.is_err() {
            return Err(CommandError::InvalidResponse(
                "Steam SDK refused to generate auth ticket".to_string(),
            ));
        }

        {
            let mut active = self.active_ticket.lock().unwrap();
            *active = Some(ticket_handle);
        }

        tracing::debug!(
            "Received web API auth ticket ({} bytes)",
            response.ticket.len()
        );

        Ok(response.ticket)
    }

    pub fn cancel_auth_ticket(&self) {
        let mut active = self.active_ticket.lock().unwrap();
        *active = None;
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn get_launch_command_line(&self) -> String {
        self.client.apps().launch_command_line()
    }

    /// for handling post-start join requests, passed through as GameRichPresenceJoinRequested or NewUrlLaunchParameters
    pub fn register_join_callbacks(&self, app: tauri::AppHandle) {
        let join_app = app.clone();
        let join_handle =
            self.client
                .register_callback(move |request: GameRichPresenceJoinRequested| {
                    tracing::info!("Steam join requested: {:?}", request.connect);
                    emit_join_target(&join_app, &request.connect);
                });

        let client = self.client.clone();
        let url_handle = self
            .client
            .register_callback(move |_: NewUrlLaunchParameters| {
                let command_line = client.apps().launch_command_line();
                tracing::info!("Steam sent new launch parameters: {:?}", command_line);
                emit_join_target(&app, &command_line);
            });

        let mut handles = self.join_callback_handles.lock().unwrap();
        handles.push(join_handle);
        handles.push(url_handle);
    }

    pub fn run_callbacks(&self) {
        self.client.run_callbacks();
    }
}

fn emit_join_target(app: &tauri::AppHandle, command_line: &str) {
    let Some(target) = parse_connect_target(command_line) else {
        tracing::warn!("Ignoring Steam join without a +connect target: {command_line:?}");
        return;
    };
    if let Err(e) = app.emit(STEAM_JOIN_EVENT, target) {
        tracing::warn!("Failed to emit Steam join request: {e}");
    }
}
