use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::auth::TokenStorage;
use crate::settings::{load_settings, AuthMode};

#[cfg(feature = "steam")]
use std::sync::Arc;
#[cfg(feature = "steam")]
use tauri::Manager;
#[cfg(feature = "steam")]
use crate::steam::{authenticate_with_steam, SteamState};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AccessMethod {
    HubTicket(String),
    SessionToken { variant: String, token: String },
    Steam(String),
    Byond,
    None,
}

impl AccessMethod {
    #[cfg_attr(not(any(target_os = "windows", target_os = "linux")), allow(dead_code))]
    pub fn is_byond(&self) -> bool {
        matches!(self, Self::Byond)
    }

    pub fn url_params(&self) -> Option<(&str, &str)> {
        match self {
            Self::HubTicket(ticket) => Some(("auth_ticket", ticket)),
            Self::SessionToken { variant, token } => Some((variant, token)),
            Self::Steam(token) => Some(("steam", token)),
            Self::Byond | Self::None => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
pub struct AuthError {
    pub code: String,
    pub message: String,
    pub linking_url: Option<String>,
}

fn resolve_auth_mode(preferred: AuthMode, server_auth_methods: &[String]) -> AuthMode {
    if server_auth_methods.is_empty() {
        return preferred;
    }

    let supports_hub = server_auth_methods.iter().any(|m| m == "hub");
    let supports_byond = server_auth_methods.iter().any(|m| m == "byond");

    match preferred {
        AuthMode::Oidc => AuthMode::Oidc,
        AuthMode::Steam => AuthMode::Steam,
        AuthMode::Hub if supports_hub => AuthMode::Hub,
        AuthMode::Hub | AuthMode::Byond if supports_byond => AuthMode::Byond,
        _ if supports_hub => AuthMode::Hub,
        _ => AuthMode::Byond,
    }
}

fn get_session_token() -> Result<AccessMethod, AuthError> {
    let tokens = TokenStorage::get_tokens().map_err(|e| AuthError {
        code: "token_error".to_string(),
        message: e.to_string(),
        linking_url: None,
    })?;

    match tokens {
        Some(t) if !TokenStorage::is_expired() => {
            let config = crate::config::get_config();
            Ok(AccessMethod::SessionToken {
                variant: config.variant.to_string(),
                token: t.access_token,
            })
        }
        _ => {
            let config = crate::config::get_config();
            Err(AuthError {
                code: "auth_required".to_string(),
                message: config.strings.login_prompt.to_string(),
                linking_url: None,
            })
        }
    }
}

async fn exchange_hub_ticket(token: &str, server_id: &str) -> Result<AccessMethod, AuthError> {
    crate::hwid::exchange_hub_ticket(token, server_id)
        .await
        .map(AccessMethod::HubTicket)
        .map_err(|e| AuthError {
            code: "ticket_error".to_string(),
            message: format!("Failed to get auth ticket: {e}"),
            linking_url: None,
        })
}

#[cfg(feature = "steam")]
async fn get_steam_token(app: &AppHandle) -> Result<AccessMethod, AuthError> {
    let steam_state = app
        .try_state::<Arc<SteamState>>()
        .ok_or_else(|| AuthError {
            code: "steam_unavailable".to_string(),
            message: "Steam is not available".to_string(),
            linking_url: None,
        })?;

    let result = authenticate_with_steam(&steam_state, false)
        .await
        .map_err(|e| AuthError {
            code: "steam_error".to_string(),
            message: e.to_string(),
            linking_url: None,
        })?;

    if result.success {
        Ok(result
            .access_token
            .map(AccessMethod::Steam)
            .unwrap_or(AccessMethod::None))
    } else if result.requires_linking {
        Err(AuthError {
            code: "steam_linking_required".to_string(),
            message: "Steam account linking required".to_string(),
            linking_url: result.linking_url,
        })
    } else {
        Err(AuthError {
            code: "steam_auth_failed".to_string(),
            message: result
                .error
                .unwrap_or_else(|| "Steam authentication failed".to_string()),
            linking_url: None,
        })
    }
}

/// Resolve the auth method for a connection.
///
/// When `auth_methods` is non-empty, the user's preferred auth mode is matched
/// against the server's capabilities. When empty, the user's preference is used as-is.
///
/// When the effective mode is Hub and a `server_id` is provided, the session token
/// is automatically exchanged for a one-time hub ticket.
pub async fn resolve_access_method(
    app: &AppHandle,
    auth_methods: &[String],
    server_id: Option<&str>,
) -> Result<AccessMethod, AuthError> {
    let settings = load_settings(app).map_err(|e| AuthError {
        code: "settings_error".to_string(),
        message: e.to_string(),
        linking_url: None,
    })?;

    let effective_mode = resolve_auth_mode(settings.auth_mode, auth_methods);

    match effective_mode {
        AuthMode::Hub => {
            let method = get_session_token()?;
            let has_hub_api = crate::config::get_config().urls.hub_api.is_some();
            if let (true, Some(sid), AccessMethod::SessionToken { ref token, .. }) =
                (has_hub_api, server_id, &method)
            {
                exchange_hub_ticket(token, sid).await
            } else {
                Ok(method)
            }
        }
        AuthMode::Oidc => get_session_token(),
        AuthMode::Steam => {
            #[cfg(feature = "steam")]
            {
                get_steam_token(app).await
            }
            #[cfg(not(feature = "steam"))]
            {
                Err(AuthError {
                    code: "steam_unavailable".to_string(),
                    message: "Steam support not compiled".to_string(),
                    linking_url: None,
                })
            }
        }
        AuthMode::Byond => Ok(AccessMethod::Byond),
    }
}
