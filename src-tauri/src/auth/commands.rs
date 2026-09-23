use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use super::client::OidcClient;
pub use super::client::UserInfo;
use super::hub_client::HubAuthError;
use super::server::CallbackServer;
use super::storage::TokenStorage;
use crate::error::{CommandError, CommandResult};

use super::hub_client::HubClient;

static PENDING_2FA_CODE: tokio::sync::Mutex<Option<String>> = tokio::sync::Mutex::const_new(None);

static CURRENT_AUTH_STATE: tokio::sync::RwLock<Option<AuthState>> =
    tokio::sync::RwLock::const_new(None);

impl From<HubAuthError> for CommandError {
    fn from(e: HubAuthError) -> Self {
        match e {
            HubAuthError::Requires2FA => Self::Requires2fa,
            HubAuthError::InvalidCredentials => Self::InvalidCredentials,
            HubAuthError::AccountLocked => Self::AccountLocked,
            HubAuthError::TokenExpired => Self::TokenExpired,
            HubAuthError::Network(msg) => Self::Network(msg),
            HubAuthError::Server(msg) => Self::Network(msg),
            HubAuthError::NotFound => Self::Network("Not found".into()),
            HubAuthError::Config(msg) => Self::NotConfigured { feature: msg },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type)]
pub struct AuthState {
    pub logged_in: bool,
    pub user: Option<UserInfo>,
    pub loading: bool,
    pub error: Option<String>,
}

impl AuthState {
    pub fn logged_out() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn loading() -> Self {
        Self {
            loading: true,
            ..Default::default()
        }
    }

    pub fn logged_in(user: UserInfo) -> Self {
        Self {
            logged_in: true,
            user: Some(user),
            loading: false,
            error: None,
        }
    }

    #[allow(dead_code)]
    pub fn error(message: String) -> Self {
        Self {
            error: Some(message),
            ..Default::default()
        }
    }
}

fn parse_hub_expiry(expire_time: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(expire_time).map_or_else(
        |_| chrono::Utc::now().timestamp() + 86400 * 30,
        |dt| dt.timestamp(),
    )
}

async fn fetch_user_info(token: &str) -> CommandResult<UserInfo> {
    let config = crate::config::get_config();
    if config.urls.hub_api.is_some() {
        Ok(HubClient::get_profile(token).await?)
    } else {
        OidcClient::get_userinfo(token).await
    }
}

async fn complete_login(
    app: &AppHandle,
    token: &str,
    refresh_token: Option<&str>,
    id_token: &str,
    expires_at: i64,
) -> CommandResult<AuthState> {
    TokenStorage::store_tokens(token, refresh_token, id_token, expires_at)?;
    tracing::debug!("Tokens stored, fetching user info");
    let user_info = match fetch_user_info(token).await {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Failed to fetch user info after login: {e:?}");
            return Err(e);
        }
    };
    let auth_state = AuthState::logged_in(user_info);
    *CURRENT_AUTH_STATE.write().await = Some(auth_state.clone());
    app.emit("auth-state-changed", &auth_state).ok();
    Ok(auth_state)
}

#[tauri::command]
#[specta::specta]
pub async fn start_login(app: AppHandle) -> CommandResult<AuthState> {
    tracing::info!("Starting login flow");
    let mut server = CallbackServer::start_without_state()?;
    let redirect_uri = server.redirect_uri();

    let auth_request = OidcClient::create_authorization_request(&redirect_uri)?;

    server.set_expected_state(auth_request.state.clone());

    crate::open_url::open(&auth_request.auth_url)?;

    let callback_result = tokio::task::spawn_blocking(move || server.wait_for_callback())
        .await
        .map_err(|e| CommandError::Internal(format!("Callback task failed: {e}")))??;

    tracing::info!("Callback received, exchanging code");

    let token_result = OidcClient::exchange_code(
        &callback_result.code,
        &redirect_uri,
        auth_request.pkce_verifier,
    )
    .await?;

    complete_login(
        &app,
        &token_result.access_token,
        token_result.refresh_token.as_deref(),
        token_result.id_token.as_deref().unwrap_or(""),
        token_result.expires_at,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn logout(app: AppHandle) -> CommandResult<AuthState> {
    tracing::info!("Logging out");
    TokenStorage::clear_tokens()?;

    let auth_state = AuthState::logged_out();
    *CURRENT_AUTH_STATE.write().await = Some(auth_state.clone());
    app.emit("auth-state-changed", &auth_state).ok();

    Ok(auth_state)
}

/// Returns the current auth state if the backend has resolved it, otherwise
/// `None`
#[tauri::command]
#[specta::specta]
pub async fn get_current_auth_state() -> CommandResult<Option<AuthState>> {
    Ok(CURRENT_AUTH_STATE.read().await.clone())
}

#[tauri::command]
#[specta::specta]
pub async fn get_auth_state() -> CommandResult<AuthState> {
    let result = get_auth_state_inner().await;

    if let Ok(ref state) = result {
        *CURRENT_AUTH_STATE.write().await = Some(state.clone());
    }

    result
}

async fn get_auth_state_inner() -> CommandResult<AuthState> {
    let Some(tokens) = TokenStorage::get_tokens()? else {
        tracing::info!("No stored tokens found, returning logged_out");
        return Ok(AuthState::logged_out());
    };

    if TokenStorage::is_expired() {
        tracing::info!("Stored tokens are expired, attempting refresh");
        let token_to_use = tokens
            .refresh_token
            .as_deref()
            .unwrap_or(&tokens.access_token);

        if let Ok(state) = refresh_tokens_internal(token_to_use).await {
            tracing::info!("Expired token refresh succeeded");
            return Ok(state);
        }
        tracing::warn!("Expired token refresh failed, clearing tokens");
        TokenStorage::clear_tokens()?;
        return Ok(AuthState::logged_out());
    }

    tracing::info!("Stored tokens are valid (not expired)");

    let config = crate::config::get_config();
    if config.urls.hub_api.is_some() {
        // Hub auth: refresh on launch to reset the 28-day expiry window.
        // If refresh fails (e.g. network issue), fall back to validating
        // the existing token so we don't log the user out unnecessarily.
        tracing::info!("Hub auth: attempting token refresh");
        match refresh_tokens_internal(&tokens.access_token).await {
            Ok(state) => {
                tracing::info!("Hub token refresh succeeded");
                Ok(state)
            }
            Err(e) => {
                tracing::warn!("Hub token refresh failed: {e:?}, falling back to get_profile");
                match HubClient::get_profile(&tokens.access_token).await {
                    Ok(user_info) => {
                        tracing::info!("Hub get_profile fallback succeeded");
                        Ok(AuthState::logged_in(user_info))
                    }
                    Err(profile_err) => {
                        tracing::warn!(
                            "Hub get_profile fallback also failed: {profile_err:?}, clearing tokens"
                        );
                        TokenStorage::clear_tokens()?;
                        Ok(AuthState::logged_out())
                    }
                }
            }
        }
    } else {
        match OidcClient::get_userinfo(&tokens.access_token).await {
            Ok(user_info) => Ok(AuthState::logged_in(user_info)),
            Err(_) => {
                if let Some(refresh_token) = &tokens.refresh_token {
                    refresh_tokens_internal(refresh_token).await
                } else {
                    TokenStorage::clear_tokens()?;
                    Ok(AuthState::logged_out())
                }
            }
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_auth(app: AppHandle) -> CommandResult<AuthState> {
    tracing::info!("Manually refreshing auth");
    let Some(tokens) = TokenStorage::get_tokens()? else {
        return Ok(AuthState::logged_out());
    };

    let refresh_token = tokens.refresh_token.ok_or(CommandError::NotAuthenticated)?;

    let auth_state = refresh_tokens_internal(&refresh_token).await?;
    app.emit("auth-state-changed", &auth_state).ok();

    Ok(auth_state)
}

#[tauri::command]
#[specta::specta]
pub async fn get_access_token() -> CommandResult<Option<String>> {
    match TokenStorage::get_tokens()? {
        Some(tokens) if !TokenStorage::is_expired() => Ok(Some(tokens.access_token)),
        _ => Ok(None),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn hub_login(
    app: AppHandle,
    username: String,
    password: String,
    totp_code: Option<String>,
) -> CommandResult<AuthState> {
    tracing::info!("Starting hub login for {}", username);

    let result = match HubClient::login(&username, &password, totp_code.as_deref()).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Hub login request failed: {e:?}");
            return Err(e.into());
        }
    };
    tracing::info!("Hub login succeeded for {}, fetching profile", username);
    let expires_at = parse_hub_expiry(&result.expire_time);

    complete_login(&app, &result.token, None, "", expires_at).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_hub_oauth_providers() -> CommandResult<Vec<String>> {
    let config = HubClient::get_hub_config().await?;

    let providers = config["oauth_providers"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(providers)
}

#[tauri::command]
#[specta::specta]
pub async fn hub_oauth_login(app: AppHandle, provider: String) -> CommandResult<AuthState> {
    tracing::info!("Starting hub OAuth login for provider: {}", provider);

    let config = crate::config::get_config();
    let hub_api = config
        .urls
        .hub_api
        .ok_or_else(|| CommandError::NotConfigured {
            feature: "hub_api".into(),
        })?;

    let server = CallbackServer::start_without_state()?;
    let redirect_uri = server.redirect_uri();

    let encoded_redirect: String =
        url::form_urlencoded::byte_serialize(redirect_uri.as_bytes()).collect();
    let authorize_url = format!(
        "{}/auth/oauth/{}/authorize?redirect_after={}",
        hub_api.trim_end_matches('/'),
        provider,
        encoded_redirect,
    );

    crate::open_url::open(&authorize_url)?;

    let callback_result = tokio::task::spawn_blocking(move || server.wait_for_callback())
        .await
        .map_err(|e| CommandError::Internal(format!("Callback task failed: {e}")))??;

    tracing::info!("OAuth callback received, exchanging code");

    match HubClient::oauth_exchange(&callback_result.code, None, None).await {
        Ok(result) => {
            let expires_at = parse_hub_expiry(&result.expire_time);
            complete_login(&app, &result.token, None, "", expires_at).await
        }
        Err(HubAuthError::Requires2FA) => {
            *PENDING_2FA_CODE.lock().await = Some(callback_result.code);
            Err(CommandError::Requires2fa)
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(feature = "steam")]
#[tauri::command]
#[specta::specta]
pub async fn hub_steam_login(
    app: AppHandle,
    steam_state: tauri::State<'_, std::sync::Arc<crate::steam::SteamState>>,
    accepted_tos: bool,
    preferred_username: Option<String>,
) -> CommandResult<AuthState> {
    tracing::info!("Starting hub Steam login");

    let config = crate::config::get_config();
    let steam_auth_url = config
        .urls
        .steam_auth
        .ok_or_else(|| CommandError::NotConfigured {
            feature: "steam_auth".into(),
        })?;

    let steam_id = steam_state.get_steam_id().to_string();
    let display_name = steam_state.get_display_name();

    let ticket_bytes = steam_state.get_auth_session_ticket().await?;
    let ticket = hex::encode(&ticket_bytes);

    let http = reqwest::Client::new();
    let response = http
        .post(steam_auth_url)
        .json(&serde_json::json!({
            "ticket": ticket,
            "steam_id": steam_id,
            "instance": crate::steam::get_steam_app_name(),
            "display_name": display_name,
            "create_account_if_missing": true,
            "accepted_tos": accepted_tos,
            "preferred_username": preferred_username,
        }))
        .send()
        .await
        .map_err(|e| CommandError::Network(format!("Failed to contact auth server: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        steam_state.cancel_auth_ticket();
        return Err(CommandError::Network(format!(
            "Auth server error ({status}): {body}"
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| CommandError::Network(format!("Failed to parse response: {e}")))?;

    if body["requires_tos"].as_bool() == Some(true) {
        steam_state.cancel_auth_ticket();
        return Err(CommandError::RequiresTos {
            suggested_username: body["suggested_username"].as_str().map(|s| s.to_string()),
        });
    }

    if body["success"].as_bool() != Some(true) {
        steam_state.cancel_auth_ticket();
        let error = body["error"]
            .as_str()
            .unwrap_or("Steam authentication failed");
        return Err(CommandError::Network(error.to_string()));
    }

    if body["requires_2fa"].as_bool() == Some(true) {
        if let Some(code) = body["pending_2fa_code"].as_str() {
            *PENDING_2FA_CODE.lock().await = Some(code.to_string());
        }
        return Err(CommandError::Requires2fa);
    }

    let token = body["access_token"]
        .as_str()
        .ok_or_else(|| CommandError::InvalidResponse("missing access_token in response".into()))?;

    let expires_at = chrono::Utc::now().timestamp() + 86400 * 30;

    complete_login(&app, token, None, "", expires_at).await
}

#[tauri::command]
#[specta::specta]
pub async fn hub_complete_2fa(app: AppHandle, totp_code: String) -> CommandResult<AuthState> {
    let code = PENDING_2FA_CODE
        .lock()
        .await
        .take()
        .ok_or_else(|| CommandError::InvalidInput("no pending 2FA exchange".into()))?;

    let result = HubClient::oauth_exchange(&code, Some(&totp_code), None)
        .await
        .map_err(|e| {
            let code_clone = code.clone();
            tokio::spawn(async move {
                *PENDING_2FA_CODE.lock().await = Some(code_clone);
            });
            CommandError::from(e)
        })?;

    let expires_at = parse_hub_expiry(&result.expire_time);
    complete_login(&app, &result.token, None, "", expires_at).await
}

async fn refresh_tokens_internal(token: &str) -> CommandResult<AuthState> {
    let config = crate::config::get_config();

    if config.urls.hub_api.is_some() {
        tracing::info!("refresh_tokens_internal: calling Hub refresh endpoint");
        let result = HubClient::refresh(token).await.map_err(|e| {
            tracing::warn!("refresh_tokens_internal: Hub refresh failed: {e:?}");
            CommandError::from(e)
        })?;
        let expires_at = parse_hub_expiry(&result.expire_time);
        tracing::info!(
            "refresh_tokens_internal: Hub refresh succeeded, new expire_time={}",
            result.expire_time
        );

        TokenStorage::store_tokens(&result.token, None, "", expires_at)?;
        tracing::info!("refresh_tokens_internal: fetching profile with new token");
        let user_info = HubClient::get_profile(&result.token).await.map_err(|e| {
            tracing::warn!("refresh_tokens_internal: get_profile after refresh failed: {e:?}");
            CommandError::from(e)
        })?;
        tracing::info!(
            "refresh_tokens_internal: complete, user={:?}",
            user_info.preferred_username
        );
        Ok(AuthState::logged_in(user_info))
    } else {
        tracing::info!("refresh_tokens_internal: calling OIDC refresh");
        let token_result = OidcClient::refresh_tokens(token).await?;

        TokenStorage::store_tokens(
            &token_result.access_token,
            token_result.refresh_token.as_deref(),
            token_result.id_token.as_deref().unwrap_or(""),
            token_result.expires_at,
        )?;
        let user_info = OidcClient::get_userinfo(&token_result.access_token).await?;
        Ok(AuthState::logged_in(user_info))
    }
}

pub async fn background_refresh_task(app: AppHandle) {
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    loop {
        if TokenStorage::should_refresh() {
            if let Ok(Some(tokens)) = TokenStorage::get_tokens() {
                tracing::info!("Background refreshing tokens");
                // For hub auth, access_token IS the session token to refresh
                // For OIDC auth, we need the refresh_token
                let token_to_use = tokens
                    .refresh_token
                    .as_deref()
                    .unwrap_or(&tokens.access_token);

                match refresh_tokens_internal(token_to_use).await {
                    Ok(auth_state) => {
                        app.emit("auth-state-changed", &auth_state).ok();
                    }
                    Err(e) => {
                        tracing::warn!("Background refresh failed: {}", e);
                        TokenStorage::clear_tokens().ok();
                        app.emit("auth-state-changed", &AuthState::logged_out())
                            .ok();
                    }
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_mins(1)).await;
    }
}
